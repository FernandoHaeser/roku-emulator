//! Geometry (boundingRect) and the display list handed to host renderers.
//!
//! Coordinates are in the channel's design resolution (`ui_resolutions`);
//! hosts scale to their surface. Label metrics come from device calibration
//! (see `text.rs`).

use std::rc::Rc;

use super::{with_node, Node};
use crate::value::{Object, ObjectRef, Value};
use crate::vm::Vm;

use super::text::{self, FontSpec, PackageFont};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    fn union(self, other: Rect) -> Rect {
        if self.width <= 0.0 && self.height <= 0.0 {
            return other;
        }
        if other.width <= 0.0 && other.height <= 0.0 {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Rect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }
}

/// Font of a Label's `font` field: a `font:` system URI or a Font node.
pub fn label_font(vm: &Vm, font: &Value) -> FontSpec {
    let fhd = is_fhd(vm);
    let output_scale = super::bitmap::design_per_output(vm);
    let (name, explicit) = match font {
        Value::String(name) => (name.to_string(), None),
        Value::Object(object) => match &*object.borrow() {
            Object::Node(node) => (node.text("uri"), Some(node.number("size"))),
            _ => (String::new(), None),
        },
        _ => (String::new(), None),
    };
    let package = vm
        .sg
        .fonts
        .get(&name)
        .cloned()
        .flatten()
        .map(|data| PackageFont {
            uri: name.as_str().into(),
            data,
        });
    let bold = package.is_none() && name.to_lowercase().contains("bold");
    // A Font node with its own size is not a calibrated system font.
    if let Some(size) = explicit.filter(|s| *s > 0.0) {
        return FontSpec {
            id: None,
            size,
            bold,
            package,
            output_scale,
        };
    }
    // Unknown names fall back to the Label default (MediumSystemFont).
    let id = text::system_font(&name).or_else(|| text::system_font("MediumSystemFont"));
    let fhd_size = id.map_or(36.0, text::system_size);
    FontSpec {
        id: if package.is_some() { None } else { id },
        size: if fhd {
            fhd_size
        } else {
            (fhd_size * 2.0 / 3.0).round()
        },
        bold,
        package,
        output_scale,
    }
}

/// Lines and metrics of a Label (design px, before any transform).
pub fn label_layout(vm: &Vm, node: &Node) -> (FontSpec, text::Layout) {
    let font = label_font(vm, &node.raw("font"));
    let layout = text::layout(
        &font,
        &node.text("text"),
        is_fhd(vm),
        node.number("width"),
        node.flag("wrap"),
        node.number("maxLines").max(0.0) as usize,
        node.number("lineSpacing"),
    );
    (font, layout)
}

fn is_fhd(vm: &Vm) -> bool {
    vm.sg.design_height >= 1080.0 || vm.sg.design_height == 0.0
}

/// Poster size: explicit width/height, else the loaded bitmap size (design px).
fn poster_size(node: &Node) -> (f64, f64) {
    let pick = |explicit: &str, bitmap: &str| {
        let value = node.number(explicit);
        if value > 0.0 {
            value
        } else {
            node.number(bitmap)
        }
    };
    (pick("width", "bitmapWidth"), pick("height", "bitmapHeight"))
}

fn has(node: &Node, type_name: &str) -> bool {
    node.ancestry.iter().any(|t| &**t == type_name)
}

/// Bounds in the node's own coordinate system (before its translation).
pub fn local_bounds(vm: &Vm, object: &ObjectRef) -> Rect {
    let (own, mut children) = with_node(object, |node| {
        let own = if has(node, "Label") {
            let (_, layout) = label_layout(vm, node);
            let pick = |name: &str, measured: f64| {
                let value = node.number(name);
                if value > 0.0 {
                    value
                } else {
                    measured
                }
            };
            Some(Rect {
                x: 0.0,
                y: 0.0,
                width: pick("width", layout.width()),
                height: pick("height", layout.height()),
            })
        } else if has(node, "Keyboard") {
            // Default native footprint measured by widgets/selection-controls
            // in both design resolutions, including the widget's padding.
            let (width, height) = if is_fhd(vm) {
                (1416.0, 452.0)
            } else {
                (942.0, 302.0)
            };
            Some(Rect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            })
        } else if has(node, "Poster") {
            let (width, height) = poster_size(node);
            Some(Rect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            })
        } else if has(node, "Rectangle") {
            Some(Rect {
                x: 0.0,
                y: 0.0,
                width: node.number("width"),
                height: node.number("height"),
            })
        } else {
            None
        };
        (own, node.children.clone())
    });
    children.extend(super::widgets::children(vm, object));
    let mut bounds = own.unwrap_or_default();
    for child in &children {
        if with_node(child, |n| n.internal || !visible(n)) {
            continue;
        }
        bounds = bounds.union(parent_bounds(vm, child));
    }
    bounds
}

fn visible(node: &Node) -> bool {
    node.field("visible")
        .is_none_or(|f| matches!(f.value, Value::Boolean(true)))
}

/// Bounds in the parent's coordinate system (`boundingRect()`).
pub fn parent_bounds(vm: &Vm, object: &ObjectRef) -> Rect {
    let local = local_bounds(vm, object);
    let ((tx, ty), (sx, sy)) = with_node(object, |n| {
        let scale = if n.field("scale").is_some() {
            n.vector("scale")
        } else {
            (1.0, 1.0)
        };
        (n.vector("translation"), scale)
    });
    Rect {
        x: tx + local.x * sx,
        y: ty + local.y * sy,
        width: local.width * sx,
        height: local.height * sy,
    }
}

pub fn scene_bounds(vm: &Vm, object: &ObjectRef) -> Rect {
    let mut rect = parent_bounds(vm, object);
    let mut current = with_node(object, |n| n.parent.as_ref().and_then(|p| p.upgrade()));
    while let Some(parent) = current {
        let ((tx, ty), (sx, sy)) =
            with_node(&parent, |n| (n.vector("translation"), n.vector("scale")));
        let (sx, sy) = if sx == 0.0 && sy == 0.0 {
            (1.0, 1.0)
        } else {
            (sx, sy)
        };
        rect = Rect {
            x: tx + rect.x * sx,
            y: ty + rect.y * sy,
            width: rect.width * sx,
            height: rect.height * sy,
        };
        current = with_node(&parent, |n| n.parent.as_ref().and_then(|p| p.upgrade()));
    }
    rect
}

// ---- display list ----------------------------------------------------------------

#[derive(Clone, Copy)]
struct Transform {
    x: f64,
    y: f64,
    sx: f64,
    sy: f64,
    opacity: f64,
}

/// JSON display list of the active screen, or `None` when no scene is shown.
///
/// `{"w":1920,"h":1080,"bg":<rgba>,"items":[...]}` where items are
/// `["r",x,y,w,h,rgba,opacity]`, `["i",x,y,w,h,uri,opacity,blendRgba]`,
/// `["t",x,y,w,h,[[line,width]...],size,bold,rgba,opacity,halign,valign,lineHeight,lineSpacing,fontUri,ascent]`,
/// `["v",x,y,w,h,playerId]` (video plane), `["c",x,y,w,h]` (push clip) and
/// `["C"]` (pop clip).
pub fn display_list(vm: &Vm) -> Option<String> {
    let screen = vm.sg.screens.iter().rev().find(|s| s.shown && !s.closed)?;
    let scene = screen.scene.clone()?;
    let background = with_node(&scene, |n| n.number("backgroundColor") as i64 as i32);
    let mut out = format!(
        "{{\"w\":{},\"h\":{},\"bg\":{},\"items\":[",
        vm.sg.design_width, vm.sg.design_height, background
    );
    let mut first = true;
    let root = Transform {
        x: 0.0,
        y: 0.0,
        sx: 1.0,
        sy: 1.0,
        opacity: 1.0,
    };
    emit_node(vm, &scene, root, &mut out, &mut first);
    if let Some(dialog) = super::widgets::modal(vm) {
        emit_node(vm, &dialog, root, &mut out, &mut first);
    }
    out.push_str("]}");
    Some(out)
}

fn push_item(out: &mut String, first: &mut bool, item: &str) {
    if !*first {
        out.push(',');
    }
    *first = false;
    out.push_str(item);
}

fn json_string(text: &str) -> String {
    crate::json::format(&Value::string(text)).unwrap_or_else(|_| "\"\"".into())
}

fn emit_node(vm: &Vm, object: &ObjectRef, parent: Transform, out: &mut String, first: &mut bool) {
    let node_ptr = Rc::clone(object);
    let (internal, children) = with_node(&node_ptr, |n| (n.internal, n.children.clone()));
    if internal {
        return;
    }
    let state = with_node(&node_ptr, |node| {
        if !visible(node) {
            return None;
        }
        let (tx, ty) = node.vector("translation");
        let (sx, sy) = if node.field("scale").is_some() {
            node.vector("scale")
        } else {
            (1.0, 1.0)
        };
        let opacity = if node.field("opacity").is_some() {
            node.number("opacity")
        } else {
            1.0
        };
        let inherit =
            node.field("inheritParentTransform").is_none() || node.flag("inheritParentTransform");
        let base = if inherit {
            parent
        } else {
            Transform {
                x: 0.0,
                y: 0.0,
                sx: 1.0,
                sy: 1.0,
                opacity: parent.opacity,
            }
        };
        let transform = Transform {
            x: base.x + tx * base.sx,
            y: base.y + ty * base.sy,
            sx: base.sx * sx,
            sy: base.sy * sy,
            opacity: base.opacity * opacity,
        };
        let w = node.number("width") * transform.sx;
        let h = node.number("height") * transform.sy;
        let mut items = Vec::new();
        if has(node, "Rectangle") {
            items.push(format!(
                "[\"r\",{},{},{},{},{},{}]",
                transform.x,
                transform.y,
                w,
                h,
                node.number("color") as i64 as i32,
                transform.opacity
            ));
        } else if has(node, "Video") {
            // Video plane: hosts with a player show it under the graphics
            // plane; width/height 0 means full screen.
            if let Some(id) = super::video::player_id(vm, &node_ptr) {
                let (vw, vh) = if w > 0.0 && h > 0.0 {
                    (w, h)
                } else {
                    (vm.sg.design_width, vm.sg.design_height)
                };
                items.push(format!(
                    "[\"v\",{},{},{},{},{}]",
                    transform.x, transform.y, vw, vh, id
                ));
            }
        } else if has(node, "Poster") {
            let uri = node.text("uri");
            if !uri.is_empty() {
                let (pw, ph) = poster_size(node);
                items.push(format!(
                    "[\"i\",{},{},{},{},{},{},{}]",
                    transform.x,
                    transform.y,
                    pw * transform.sx,
                    ph * transform.sy,
                    json_string(&uri),
                    transform.opacity,
                    node.number("blendColor") as i64 as i32
                ));
            }
        } else if has(node, "Label") {
            let (font, layout) = label_layout(vm, node);
            let width = if w > 0.0 {
                w
            } else {
                layout.width() * transform.sx
            };
            let height = if h > 0.0 {
                h
            } else {
                layout.height() * transform.sy
            };
            let lines: Vec<String> = layout
                .lines
                .iter()
                .map(|(line, line_width)| {
                    format!("[{},{}]", json_string(line), line_width * transform.sx)
                })
                .collect();
            items.push(format!(
                "[\"t\",{},{},{},{},[{}],{},{},{},{},{},{},{},{},{},{}]",
                transform.x,
                transform.y,
                width,
                height,
                lines.join(","),
                font.size * transform.sy,
                font.bold,
                node.number("color") as i64 as i32,
                transform.opacity,
                json_string(&node.text("horizAlign")),
                json_string(&node.text("vertAlign")),
                layout.line_height * transform.sy,
                layout.line_spacing * transform.sy,
                json_string(font.package.as_ref().map_or("", |p| &p.uri)),
                text::ascent(&font) * transform.sy
            ));
        }
        let clip = node.field("clippingRect").and_then(|f| match &f.value {
            Value::Object(object) => match &*object.borrow() {
                Object::AssocArray(aa) => {
                    let get = |k: &str| aa.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                    let (cw, ch) = (get("width"), get("height"));
                    (cw > 0.0 && ch > 0.0).then(|| {
                        format!(
                            "[\"c\",{},{},{},{}]",
                            transform.x + get("x") * transform.sx,
                            transform.y + get("y") * transform.sy,
                            cw * transform.sx,
                            ch * transform.sy
                        )
                    })
                }
                _ => None,
            },
            _ => None,
        });
        // Rotation (radians, counterclockwise) about scaleRotateCenter, applied
        // by the host to this node's items and its whole subtree.
        let rotation = node.number("rotation");
        let rotate = (rotation != 0.0).then(|| {
            let (cx, cy) = node.vector("scaleRotateCenter");
            format!(
                "[\"R\",{},{},{}]",
                transform.x + cx * transform.sx,
                transform.y + cy * transform.sy,
                rotation
            )
        });
        Some((transform, items, clip, rotate))
    });
    let Some((transform, items, clip, rotate)) = state else {
        return;
    };
    if let Some(rotate) = &rotate {
        push_item(out, first, rotate);
    }
    if let Some(clip) = &clip {
        push_item(out, first, clip);
    }
    for item in &items {
        push_item(out, first, item);
    }
    for child in &children {
        emit_node(vm, child, transform, out, first);
    }
    for child in super::widgets::children(vm, &node_ptr) {
        emit_node(vm, &child, transform, out, first);
    }
    if clip.is_some() {
        push_item(out, first, "[\"C\"]");
    }
    if rotate.is_some() {
        push_item(out, first, "[\"Q\"]");
    }
}
