//! Native LayoutGroup positioning, measured by scenegraph/layout-group.
use super::{
    render::{self, Rect},
    runtime, with_node, ObjectRef, Value,
};
use crate::Object;
use crate::{error::RuntimeError, Vm};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};
#[derive(Default)]
pub struct Layouts {
    nodes: Vec<Weak<RefCell<Object>>>,
    bounds: HashMap<usize, (Weak<RefCell<Object>>, Rect)>,
    revision: Option<u64>,
    servicing: bool,
}
impl Layouts {
    pub fn bounds(&self, node: &ObjectRef) -> Option<Rect> {
        self.bounds
            .get(&(Rc::as_ptr(node) as usize))
            .and_then(|(owner, rect)| {
                owner
                    .upgrade()
                    .filter(|owner| Rc::ptr_eq(owner, node))
                    .map(|_| *rect)
            })
    }
}
pub fn initialize(vm: &mut Vm, node: &ObjectRef) {
    let managed = with_node(node, |n| {
        n.ancestry.iter().any(|a| &**a == "LayoutGroup")
            && !n.ancestry.iter().any(|a| &**a == "ButtonGroup")
    });
    if managed {
        vm.sg.layouts.nodes.push(Rc::downgrade(node));
        vm.sg.layouts.revision = None;
    }
}
fn depth(node: &ObjectRef) -> usize {
    let mut depth = 0;
    let mut parent = with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade));
    while let Some(node) = parent {
        depth += 1;
        parent = with_node(&node, |n| n.parent.as_ref().and_then(Weak::upgrade));
    }
    depth
}
pub fn service(vm: &mut Vm) -> Result<(), RuntimeError> {
    if vm.sg.layouts.servicing || vm.sg.layouts.revision == Some(vm.sg.revision) {
        return Ok(());
    }
    vm.sg.layouts.servicing = true;
    let result = service_inner(vm);
    vm.sg.layouts.servicing = false;
    if result.is_ok() {
        vm.sg.layouts.revision = Some(vm.sg.revision);
    }
    result
}
fn service_inner(vm: &mut Vm) -> Result<(), RuntimeError> {
    vm.sg.layouts.nodes.retain(|node| node.strong_count() > 0);
    vm.sg
        .layouts
        .bounds
        .retain(|_, (node, _)| node.strong_count() > 0);
    let mut nodes: Vec<_> = vm
        .sg
        .layouts
        .nodes
        .iter()
        .filter_map(Weak::upgrade)
        .collect();
    nodes.sort_by_key(|node| std::cmp::Reverse(depth(node)));
    for node in nodes {
        let (children, horizontal, h_align, v_align, after, spacings) = with_node(&node, |n| {
            let spacings = n
                .field("itemSpacings")
                .and_then(|field| {
                    if let Value::Object(array) = &field.value {
                        if let Object::Array(values) = &*array.borrow() {
                            return Some(
                                values
                                    .iter()
                                    .map(|v| v.as_f64().unwrap_or(0.0))
                                    .collect::<Vec<_>>(),
                            );
                        }
                    }
                    None
                })
                .unwrap_or_default();
            (
                n.children.clone(),
                n.text("layoutDirection").eq_ignore_ascii_case("horiz"),
                n.text("horizAlignment"),
                n.text("vertAlignment"),
                n.flag("addItemSpacingAfterChild"),
                spacings,
            )
        });
        let gap = |index: usize| {
            spacings
                .get(index)
                .or_else(|| spacings.last())
                .copied()
                .unwrap_or(0.0)
        };
        let footprints: Vec<_> = children
            .iter()
            .map(|child| {
                let r = render::local_bounds(vm, child);
                let scale = with_node(child, |n| n.vector("scale"));
                let (x1, x2) = (r.x * scale.0, (r.x + r.width) * scale.0);
                let (y1, y2) = (r.y * scale.1, (r.y + r.height) * scale.1);
                Rect {
                    x: x1.min(x2),
                    y: y1.min(y2),
                    width: (x2 - x1).abs(),
                    height: (y2 - y1).abs(),
                }
            })
            .collect();
        let total: f64 = footprints
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let spacing = if !after || i + 1 < children.len() {
                    gap(i)
                } else {
                    0.0
                };
                if horizontal {
                    r.width + spacing
                } else {
                    r.height + spacing
                }
            })
            .sum();
        let along = if horizontal { &h_align } else { &v_align };
        let anchor = match along.as_str() {
            "center" => -total / 2.0,
            "right" | "bottom" => -total,
            _ => 0.0,
        };
        let cross = if horizontal { &v_align } else { &h_align };
        let mut cursor = anchor;
        let mut bounds: Option<Rect> = None;
        for (index, (child, r)) in children.iter().zip(&footprints).enumerate() {
            if !after {
                cursor += gap(index);
            }
            let old = with_node(child, |n| n.vector("translation"));
            let (start, size) = if horizontal {
                (r.y, r.height)
            } else {
                (r.x, r.width)
            };
            let across = match cross.as_str() {
                "custom" => {
                    if horizontal {
                        old.1
                    } else {
                        old.0
                    }
                }
                "center" => -size / 2.0 - start,
                "right" | "bottom" => -size - start,
                _ => -start,
            };
            let next = if horizontal {
                (cursor - r.x, across)
            } else {
                (across, cursor - r.y)
            };
            if old != next {
                runtime::set_field(
                    vm,
                    child,
                    "translation",
                    super::float_array(&[next.0, next.1]),
                )?;
            }
            let rect = Rect {
                x: r.x + next.0,
                y: r.y + next.1,
                width: r.width,
                height: r.height,
            };
            bounds = Some(if let Some(b) = bounds {
                let x = b.x.min(rect.x);
                let y = b.y.min(rect.y);
                Rect {
                    x,
                    y,
                    width: (b.x + b.width).max(rect.x + rect.width) - x,
                    height: (b.y + b.height).max(rect.y + rect.height) - y,
                }
            } else {
                rect
            });
            cursor += if horizontal { r.width } else { r.height };
            if after {
                cursor += gap(index);
            }
        }
        vm.sg.layouts.bounds.insert(
            Rc::as_ptr(&node) as usize,
            (Rc::downgrade(&node), bounds.unwrap_or_default()),
        );
    }
    Ok(())
}
