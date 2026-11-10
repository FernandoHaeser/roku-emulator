//! BusySpinner's internal Poster and monotonic-clock rotation.
use super::runtime::{create_node, set_field};
use super::{node_ref, with_node, with_node_mut, Object, ObjectRef, Value};
use crate::{error::RuntimeError, Vm};
use std::rc::{Rc, Weak};
type Result<T> = std::result::Result<T, RuntimeError>;
#[derive(Default)]
pub struct Spinners {
    entries: Vec<Spinner>,
}
struct Spinner {
    node: Weak<std::cell::RefCell<Object>>,
    poster: Option<Weak<std::cell::RefCell<Object>>>,
    last_ms: f64,
}
fn is_spinner(node: &ObjectRef) -> bool {
    with_node(node, |n| n.ancestry.iter().any(|a| &**a == "BusySpinner"))
}
fn poster(node: &ObjectRef) -> Option<ObjectRef> {
    with_node(node, |n| n.field("poster").and_then(|f| node_ref(&f.value)))
}
pub fn owns_poster(node: &ObjectRef) -> bool {
    let parent = with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade));
    parent.filter(is_spinner).is_some_and(|p| {
        with_node(&p, |n| {
            n.children.iter().any(|image| Rc::ptr_eq(image, node))
        })
    })
}
fn running(node: &ObjectRef) -> bool {
    with_node(node, |n| {
        n.text("control") != "stop" && n.number("spinInterval") > 0.0
    })
}
impl Spinners {
    pub fn active(&self) -> bool {
        self.entries
            .iter()
            .filter_map(|s| s.node.upgrade())
            .any(|n| running(&n))
    }
}
pub fn initialize(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    if !is_spinner(node) {
        return Ok(());
    }
    let image = create_node(vm, "Poster")?.expect("Poster");
    with_node_mut(&image, |n| n.parent = Some(Rc::downgrade(node)));
    with_node_mut(node, |n| n.children.push(image.clone()));
    set_field(vm, node, "poster", Value::Object(image.clone()))?;
    let now = vm.now_ms();
    vm.sg.spinners.entries.push(Spinner {
        node: Rc::downgrade(node),
        poster: Some(Rc::downgrade(&image)),
        last_ms: now,
    });
    Ok(())
}
pub fn changed(vm: &mut Vm, node: &ObjectRef, field: &str) -> Result<()> {
    if is_spinner(node) {
        if field.eq_ignore_ascii_case("uri") {
            if let Some(image) = poster(node) {
                let uri = with_node(node, |n| n.text("uri"));
                set_field(vm, &image, "uri", Value::string(uri))?;
            }
        }
        if ["control", "spinInterval", "clockwise"]
            .iter()
            .any(|f| field.eq_ignore_ascii_case(f))
        {
            let now = vm.now_ms();
            for spinner in &mut vm.sg.spinners.entries {
                if spinner.node.upgrade().is_some_and(|n| Rc::ptr_eq(&n, node)) {
                    spinner.last_ms = now;
                }
            }
        }
    }
    Ok(())
}
pub fn service(vm: &mut Vm) -> Result<()> {
    let now = vm.now_ms();
    let entries = std::mem::take(&mut vm.sg.spinners.entries);
    for mut spinner in entries {
        let Some(node) = spinner.node.upgrade() else {
            continue;
        };
        let last = spinner.last_ms;
        let image = spinner.poster.as_ref().and_then(Weak::upgrade);
        spinner.last_ms = now;
        // Restore tracking before notifying observers, which may change controls.
        vm.sg.spinners.entries.push(spinner);
        if running(&node) && now > last {
            if let Some(image) = image {
                let (interval, clockwise) =
                    with_node(&node, |n| (n.number("spinInterval"), n.flag("clockwise")));
                let angle = with_node(&image, |n| n.number("rotation"));
                let direction = if clockwise { -1.0 } else { 1.0 };
                let angle = (angle
                    + direction * (now - last) / (interval * 1000.0) * std::f64::consts::TAU)
                    % std::f64::consts::TAU;
                set_field(vm, &image, "rotation", Value::Float(angle as f32))?;
            }
        }
    }
    Ok(())
}
