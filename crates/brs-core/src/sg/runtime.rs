//! SceneGraph behavior that needs the VM: node creation (components, XML
//! children, init order), field sets with observers, focus, remote keys,
//! Task threads, timers and roSGScreen.
//!
//! Ordering rules come from fidelity/scenegraph goldens:
//! - CreateScene returns at once; children and init run on the render thread
//!   (at the latest when Show() is called).
//! - A component's XML children are created (their init runs) before its own
//!   init; XML attributes of a child are applied after the child's init, so
//!   they trigger onChange/observers.
//! - In an `extends` chain every level's own init runs, base first.
//! - Interface onChange fires before observeField callbacks, which fire in
//!   registration order. Setting an equal value does not notify unless the
//!   field has alwaysNotify.
//! - A Task setting a field blocks until the render thread ran the observers.

use std::cell::RefCell;
use std::rc::Rc;

use super::{
    component::ComponentDef, node_ref, schema, with_node, with_node_mut, ComponentInstance, Field,
    FieldKind, Node, NodeEvent, Observer, ScreenHandle, ScreenState, Timer,
};
use crate::error::RuntimeError;
use crate::value::{AssocArray, Object, ObjectRef, Value};
use crate::vm::{Vm, MAIN_THREAD};
use crate::xml::XmlElement;

fn node_value(object: &ObjectRef) -> Value {
    Value::Object(object.clone())
}

// ---- creation -----------------------------------------------------------------

/// Component definitions of `type_name`'s chain, most derived first, and the
/// built-in type the chain ends in.
fn type_chain(vm: &Vm, type_name: &str) -> Option<(Vec<Rc<ComponentDef>>, &'static str)> {
    let mut components = Vec::new();
    let mut current = type_name.to_string();
    for _ in 0..64 {
        if let Some(def) = vm.sg.components.get(&current.to_lowercase()) {
            current = def.extends.clone();
            components.push(def.clone());
            continue;
        }
        return schema::canonical(&current).map(|builtin| (components, builtin));
    }
    None
}

fn builtin_levels(builtin: &'static str) -> Vec<&'static str> {
    let mut levels = vec![builtin];
    let mut current = builtin;
    while let Some(parent) = schema::parent(current) {
        let Some(canonical) = schema::canonical(parent) else {
            break;
        };
        levels.push(canonical);
        current = canonical;
    }
    levels
}

pub fn global_node(vm: &mut Vm) -> ObjectRef {
    if let Some(global) = &vm.sg.global {
        return global.clone();
    }
    let global = allocate_node(vm, "Node").expect("built-in Node");
    vm.sg.global = Some(global.clone());
    global
}

/// Builds the node object with its fields (no children, no init yet).
fn allocate_node(vm: &mut Vm, type_name: &str) -> Option<ObjectRef> {
    let (components, builtin) = type_chain(vm, type_name)?;
    let builtins = builtin_levels(builtin);

    let mut ancestry: Vec<Rc<str>> = components
        .iter()
        .map(|c| Rc::from(c.name.as_str()))
        .collect();
    ancestry.extend(builtins.iter().map(|b| Rc::from(*b)));

    let mut fields = Vec::new();
    for def in &components {
        for interface in &def.fields {
            let value = interface
                .value
                .as_ref()
                .and_then(|text| interface.kind.convert(&Value::string(text.as_str())))
                .unwrap_or_else(|| interface.kind.default_value());
            fields.push(Field {
                name: interface.id.clone(),
                kind: interface.kind,
                value,
                always_notify: interface.always_notify,
                on_change: interface.on_change.clone(),
                observers: Vec::new(),
            });
        }
    }
    for level in &builtins {
        for spec in schema::fields(level) {
            if fields
                .iter()
                .any(|f: &Field| f.name.eq_ignore_ascii_case(spec.name))
            {
                continue;
            }
            fields.push(Field {
                name: spec.name.to_string(),
                kind: spec.kind,
                value: (spec.default)(),
                always_notify: spec.always_notify,
                on_change: None,
                observers: Vec::new(),
            });
        }
    }

    let subtype: Rc<str> = ancestry[0].clone();
    let mut node = Node::new(subtype, Rc::new(ancestry), fields);
    node.functions = Rc::new(
        components
            .iter()
            .flat_map(|c| c.functions.iter().map(|f| f.to_lowercase()))
            .collect(),
    );
    let object = Rc::new(RefCell::new(Object::Node(node)));

    if let Some(leaf) = components.first() {
        let global = if vm.sg.global.is_some() || !type_name.eq_ignore_ascii_case("Node") {
            Value::Object(global_node(vm))
        } else {
            Value::Invalid
        };
        let mut m = AssocArray::new();
        m.set("top", node_value(&object));
        m.set("global", global);
        with_node_mut(&object, |n| {
            n.component = Some(ComponentInstance {
                scope: leaf.scope,
                m: Value::object(Object::AssocArray(m)),
            })
        });
    }

    if builtins.contains(&"Scene") {
        if let Some(background) = allocate_node(vm, "Poster") {
            with_node_mut(&background, |n| {
                // The device draws its theme background here, but BrightScript
                // reads `uri` as "" (fidelity/scenegraph/render-composite).
                n.internal = true;
                if let Some(f) = n.field_mut("inheritParentTransform") {
                    f.value = Value::Boolean(false);
                }
            });
            attach(&object, &background, None);
        }
    }
    Some(object)
}

/// Creates the XML children of each component level and runs the inits.
fn populate_node(vm: &mut Vm, object: &ObjectRef) -> Result<(), RuntimeError> {
    let type_name = with_node(object, |n| n.subtype.to_string());
    let Some((components, _)) = type_chain(vm, &type_name) else {
        return Ok(());
    };
    for def in components.iter().rev() {
        if let Some(children) = &def.children {
            create_children(vm, object, children, &def.file)?;
        }
    }
    let m = with_node(object, |n| n.component.as_ref().map(|c| c.m.clone()));
    if let Some(m) = m {
        for def in components.iter().rev() {
            if let Some(init) = def.init {
                let m = m.clone();
                vm.run_in_render(|vm| vm.call_sync(init, Vec::new(), m))?;
            }
        }
    }
    vm.sg.revision += 1;
    Ok(())
}

fn create_children(
    vm: &mut Vm,
    parent: &ObjectRef,
    xml: &XmlElement,
    file: &str,
) -> Result<(), RuntimeError> {
    for element in xml.elements() {
        let Some(child) = create_node(vm, &element.name)? else {
            vm.write(&format!(
                "BRIGHTSCRIPT: ERROR: roSGNode: Failed to create roSGNode with type {}: {file}\n",
                element.name
            ));
            continue;
        };
        // `role="field"` stores the child in that parent field instead of the
        // child list (e.g. `<Font role="font" .../>` inside a Label).
        let mut role = None;
        for (name, text) in &element.attributes {
            if name.eq_ignore_ascii_case("role") {
                role = Some(text.clone());
                continue;
            }
            set_field_from_text(vm, &child, name, text)?;
        }
        create_children(vm, &child, element, file)?;
        match role {
            Some(field) => set_field(vm, parent, &field, Value::Object(child))?,
            None => attach(parent, &child, None),
        }
    }
    Ok(())
}

fn set_field_from_text(
    vm: &mut Vm,
    node: &ObjectRef,
    name: &str,
    text: &str,
) -> Result<(), RuntimeError> {
    let kind = with_node(node, |n| n.field(name).map(|f| f.kind));
    let value = match kind {
        Some(FieldKind::Font) => Value::string(text),
        Some(_) | None => Value::string(text),
    };
    set_field(vm, node, name, value)
}

/// `CreateObject("roSGNode", type)`: full creation, init included.
pub fn create_node(vm: &mut Vm, type_name: &str) -> Result<Option<ObjectRef>, RuntimeError> {
    let Some(object) = allocate_node(vm, type_name) else {
        return Ok(None);
    };
    populate_node(vm, &object)?;
    super::widgets::initialize(vm, &object)?;
    super::video::initialize(vm, &object)?;
    Ok(Some(object))
}

// ---- tree -----------------------------------------------------------------------

fn parent_of(object: &ObjectRef) -> Option<ObjectRef> {
    with_node(object, |n| n.parent.as_ref().and_then(|p| p.upgrade()))
}

fn detach(child: &ObjectRef) {
    if let Some(parent) = parent_of(child) {
        with_node_mut(&parent, |p| p.children.retain(|c| !Rc::ptr_eq(c, child)));
    }
    with_node_mut(child, |c| c.parent = None);
}

fn attach(parent: &ObjectRef, child: &ObjectRef, index: Option<usize>) {
    if Rc::ptr_eq(parent, child) {
        return;
    }
    detach(child);
    with_node_mut(child, |c| c.parent = Some(Rc::downgrade(parent)));
    with_node_mut(parent, |p| {
        let position = index.unwrap_or(p.children.len()).min(p.children.len());
        p.children.insert(position, child.clone());
    });
}

pub(super) fn find_node(object: &ObjectRef, id: &str) -> Option<ObjectRef> {
    let (matches, children) = with_node(object, |n| (n.id() == id, n.children.clone()));
    if matches {
        return Some(object.clone());
    }
    children.iter().find_map(|child| find_node(child, id))
}

pub(super) fn ancestors(object: &ObjectRef) -> Vec<ObjectRef> {
    let mut chain = vec![object.clone()];
    while let Some(parent) = parent_of(chain.last().expect("chain")) {
        chain.push(parent);
    }
    chain
}

// ---- fields ---------------------------------------------------------------------

pub fn get_field(object: &ObjectRef, name: &str) -> Value {
    with_node(object, |n| {
        n.field(name)
            .map_or(Value::Invalid, |field| field.kind.read(&field.value))
    })
}

fn same_value(old: &Value, new: &Value) -> bool {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b) && is_node(a),
        (Value::Invalid, Value::Invalid) => true,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (a, b) if a.is_numeric() && b.is_numeric() => {
            a.as_f64() == b.as_f64() && a.type_name() == b.type_name()
        }
        _ => false,
    }
}

fn is_node(object: &ObjectRef) -> bool {
    matches!(&*object.borrow(), Object::Node(_))
}

struct Change {
    field: String,
    data: Value,
    on_change: Option<String>,
    observers: Vec<Observer>,
}

/// Stores a value and notifies observers. Unknown fields are added on
/// ContentNode (content metadata) and ignored elsewhere.
pub fn set_field(
    vm: &mut Vm,
    object: &ObjectRef,
    name: &str,
    value: Value,
) -> Result<(), RuntimeError> {
    // A Timer restarts on every `control = "start"`, even when the field
    // already holds it (it still reads "start" after firing; video/drm
    // restarts its stop timer per phase on the device).
    let restart = name.eq_ignore_ascii_case("control")
        && with_node(object, |n| n.ancestry.iter().any(|t| &**t == "Timer"));
    let change = with_node_mut(object, |n| {
        if n.field_index(name).is_none() {
            if !n.ancestry.iter().any(|t| &**t == "ContentNode") {
                return None;
            }
            let kind = infer_kind(&value);
            n.add_field(Field {
                name: name.to_string(),
                kind,
                value: kind.default_value(),
                always_notify: false,
                on_change: None,
                observers: Vec::new(),
            });
        }
        let field = n.field_mut(name)?;
        let converted = field.kind.convert(&value)?;
        let changed = field.always_notify
            || field.kind == FieldKind::Trigger
            || !same_value(&field.value, &converted);
        field.value = converted;
        changed.then(|| Change {
            field: field.name.clone(),
            data: field.kind.read(&field.value),
            on_change: field.on_change.clone(),
            observers: field.observers.clone(),
        })
    });
    vm.sg.revision += 1;
    let Some(change) = change else {
        if restart {
            side_effects(vm, object, "control")?;
        }
        return Ok(());
    };
    side_effects(vm, object, &change.field)?;
    super::widgets::changed(vm, object, &change.field)?;
    notify(vm, object, change)
}

fn infer_kind(value: &Value) -> FieldKind {
    match value.unboxed() {
        Value::Integer(_) => FieldKind::Integer,
        Value::LongInteger(_) => FieldKind::LongInteger,
        Value::Float(_) => FieldKind::Float,
        Value::Double(_) => FieldKind::Double,
        Value::Boolean(_) => FieldKind::Boolean,
        Value::Object(object) => match &*object.borrow() {
            Object::Node(_) => FieldKind::Node,
            Object::AssocArray(_) => FieldKind::AssocArray,
            _ => FieldKind::Array,
        },
        _ => FieldKind::String,
    }
}

fn notify(vm: &mut Vm, object: &ObjectRef, change: Change) -> Result<(), RuntimeError> {
    let (component, node_id) = with_node(object, |n| (n.component.clone(), n.id()));
    let event = || {
        Value::object(Object::NodeEvent(NodeEvent {
            field: change.field.clone(),
            data: change.data.clone(),
            node: node_value(object),
            node_id: node_id.clone(),
        }))
    };
    if let (Some(function), Some(component)) = (&change.on_change, &component) {
        if let Some(index) = vm.program().lookup(component.scope, function) {
            let m = component.m.clone();
            let args = vec![event()];
            vm.run_in_render(|vm| vm.call_sync(index, args, m))?;
        }
    }
    for observer in &change.observers {
        match observer {
            Observer::Callback { scope, function, m } => {
                if let Some(index) = vm.program().lookup(*scope, function) {
                    let m = m.clone();
                    let args = vec![event()];
                    vm.run_in_render(|vm| vm.call_sync(index, args, m))?;
                }
            }
            Observer::Port { port } => vm.post_message(*port, event()),
        }
    }
    Ok(())
}

/// Built-in behavior attached to fields (Task.control, Timer.control).
fn side_effects(vm: &mut Vm, object: &ObjectRef, field: &str) -> Result<(), RuntimeError> {
    if field.eq_ignore_ascii_case("uri") {
        let poster = with_node(object, |n| {
            n.ancestry
                .iter()
                .any(|t| &**t == "Poster")
                .then(|| n.text("uri"))
        });
        if let Some(uri) = poster {
            return super::bitmap::load(vm, object, &uri);
        }
        let font = with_node(object, |n| {
            n.ancestry
                .iter()
                .any(|t| &**t == "Font")
                .then(|| n.text("uri"))
        });
        if let Some(uri) = font.filter(|uri| uri.starts_with("pkg:/")) {
            if !vm.sg.fonts.contains_key(&uri) {
                let data = vm.env.read_file(&uri).map(std::rc::Rc::new);
                vm.sg.fonts.insert(uri, data);
            }
            vm.sg.revision += 1;
            return Ok(());
        }
    }
    let (is_animation, is_interpolator) = with_node(object, |n| {
        (
            n.ancestry.iter().any(|t| &**t == "AnimationBase"),
            n.ancestry.iter().any(|t| t.ends_with("FieldInterpolator")),
        )
    });
    if with_node(object, |n| n.ancestry.iter().any(|t| &**t == "Video")) {
        if field.eq_ignore_ascii_case("control") {
            let control = with_node(object, |n| n.text("control").to_lowercase());
            return super::video::control(vm, object, &control);
        }
        if field.eq_ignore_ascii_case("seek") {
            return super::video::seek(vm, object);
        }
    }
    if is_interpolator && field.eq_ignore_ascii_case("fraction") {
        return super::animation::interpolate(vm, object);
    }
    if is_animation && field.eq_ignore_ascii_case("control") {
        let control = with_node(object, |n| n.text("control").to_lowercase());
        return super::animation::control(vm, object, &control);
    }
    let (is_task, is_timer) = with_node(object, |n| {
        (
            n.ancestry.iter().any(|t| &**t == "Task"),
            n.ancestry.iter().any(|t| &**t == "Timer"),
        )
    });
    if !field.eq_ignore_ascii_case("control") {
        return Ok(());
    }
    let control = with_node(object, |n| n.text("control").to_lowercase());
    if is_task {
        match control.as_str() {
            "run" => {
                let (function_name, component) =
                    with_node(object, |n| (n.text("functionName"), n.component.clone()));
                set_field(vm, object, "state", Value::string("run"))?;
                let function = component
                    .as_ref()
                    .and_then(|c| vm.program().lookup(c.scope, &function_name));
                match (function, component) {
                    (Some(function), Some(component)) => {
                        vm.spawn_task(function, component.m.clone(), object.clone())
                    }
                    _ => set_field(vm, object, "state", Value::string("stop"))?,
                }
            }
            "stop" | "done" => {
                vm.stop_task(object);
                set_field(vm, object, "state", Value::string("stop"))?;
            }
            _ => {}
        }
    } else if is_timer {
        vm.sg
            .timers
            .retain(|t| !t.node.upgrade().is_some_and(|n| Rc::ptr_eq(&n, object)));
        if control == "start" {
            let duration = with_node(object, |n| n.number("duration"));
            vm.sg.timers.push(Timer {
                node: Rc::downgrade(object),
                due_ms: vm.now_ms() + duration * 1000.0,
            });
        }
    }
    Ok(())
}

// ---- focus and keys ---------------------------------------------------------------

fn set_raw_and_notify(
    vm: &mut Vm,
    object: &ObjectRef,
    name: &str,
    value: Value,
) -> Result<(), RuntimeError> {
    let change = with_node_mut(object, |n| {
        let field = n.field_mut(name)?;
        if same_value(&field.value, &value) {
            return None;
        }
        field.value = value;
        Some(Change {
            field: field.name.clone(),
            data: field.kind.read(&field.value),
            on_change: field.on_change.clone(),
            observers: field.observers.clone(),
        })
    });
    match change {
        Some(change) => notify(vm, object, change),
        None => Ok(()),
    }
}

pub fn focused_node(vm: &Vm) -> Option<ObjectRef> {
    vm.sg.focus.as_ref().and_then(|w| w.upgrade())
}

pub(super) fn set_focus(vm: &mut Vm, target: Option<ObjectRef>) -> Result<(), RuntimeError> {
    let target = target.map(|node| super::widgets::focus_target(&node));
    if let Some(old) = focused_node(vm) {
        with_node_mut(&old, |n| n.has_focus = false);
        super::widgets::blur(vm, &old);
        let keep: Vec<ObjectRef> = target.as_ref().map(ancestors).unwrap_or_default();
        for node in ancestors(&old) {
            if !keep.iter().any(|k| Rc::ptr_eq(k, &node)) {
                set_raw_and_notify(vm, &node, "focusedChild", Value::Invalid)?;
            }
        }
    }
    vm.sg.focus = target.as_ref().map(Rc::downgrade);
    if let Some(target) = target {
        with_node_mut(&target, |n| n.has_focus = true);
        super::widgets::focus(vm, &target)?;
        let chain = ancestors(&target);
        set_raw_and_notify(vm, &target, "focusedChild", node_value(&target))?;
        for pair in chain.windows(2) {
            set_raw_and_notify(vm, &pair[1], "focusedChild", node_value(&pair[0]))?;
        }
    }
    vm.sg.revision += 1;
    Ok(())
}

/// onKeyEvent names (fidelity/scenegraph/focus-keys): lowercase, except "OK".
pub fn key_name(remote_key: &str) -> Option<&'static str> {
    Some(match remote_key {
        "Up" => "up",
        "Down" => "down",
        "Left" => "left",
        "Right" => "right",
        "OK" | "Select" => "OK",
        "Back" => "back",
        "Replay" | "InstantReplay" => "replay",
        "Rev" => "rewind",
        "Fwd" => "fastforward",
        "Play" => "play",
        "Options" | "Info" => "options",
        _ => return None,
    })
}

fn active_scene(vm: &Vm) -> Option<(usize, ObjectRef)> {
    vm.sg
        .screens
        .iter()
        .enumerate()
        .rev()
        .find(|(_, s)| s.shown && !s.closed)
        .and_then(|(i, s)| s.scene.clone().map(|scene| (i, scene)))
}

fn dispatch_key(vm: &mut Vm, key: &str, pressed: bool) -> Result<(), RuntimeError> {
    let Some((screen, scene)) = active_scene(vm) else {
        return Ok(());
    };
    if super::widgets::modal_key(vm, key, pressed)? {
        return Ok(());
    }
    let start = focused_node(vm).unwrap_or_else(|| scene.clone());
    let mut handled = false;
    for node in ancestors(&start) {
        let component = with_node(&node, |n| n.component.clone());
        if let Some(component) = component {
            if let Some(function) = vm.program().lookup(component.scope, "onKeyEvent") {
                let args = vec![Value::string(key), Value::Boolean(pressed)];
                let result =
                    vm.run_in_render(|vm| vm.call_sync(function, args, component.m.clone()))?;
                if crate::ops::condition(&result).unwrap_or(false) {
                    handled = true;
                    break;
                }
            }
        }
        if super::widgets::key(vm, &node, key, pressed)? {
            handled = true;
            break;
        }
        if Rc::ptr_eq(&node, &scene) {
            break;
        }
    }
    if !handled && pressed && key == "back" && with_node(&scene, |n| n.flag("backExitsScene")) {
        close_screen(vm, screen);
    }
    Ok(())
}

fn close_screen(vm: &mut Vm, screen: usize) {
    let Some(state) = vm.sg.screens.get_mut(screen) else {
        return;
    };
    if state.closed {
        return;
    }
    state.closed = true;
    if let Some(port) = state.port {
        vm.post_message(port, Value::object(Object::ScreenEvent { closed: true }));
    }
    vm.sg.revision += 1;
}

// ---- scheduler hook -----------------------------------------------------------------

/// Render-thread work between slices.
pub fn service(vm: &mut Vm) {
    if let Err(error) = service_inner(vm) {
        if !error.is_aborted() {
            vm.write(&format!("{error}\n"));
        }
    }
}

fn service_inner(vm: &mut Vm) -> Result<(), RuntimeError> {
    // The render thread runs concurrently with Main: pending scenes are built
    // once Main blocks (or explicitly by Show()).
    if !vm.sg.pending_scenes.is_empty() && !vm.thread_runnable(MAIN_THREAD) {
        flush_pending_scenes(vm)?;
    }
    for node in vm.take_finished_tasks() {
        set_field(vm, &node, "state", Value::string("stop"))?;
    }
    super::widgets::service(vm)?;
    let now = vm.now_ms();
    let due: Vec<Timer> = vm
        .sg
        .timers
        .iter()
        .filter(|t| t.due_ms <= now)
        .cloned()
        .collect();
    vm.sg.timers.retain(|t| t.due_ms > now);
    for timer in due {
        let Some(node) = timer.node.upgrade() else {
            continue;
        };
        let (repeat, duration) = with_node(&node, |n| (n.flag("repeat"), n.number("duration")));
        if repeat {
            vm.sg.timers.push(Timer {
                node: timer.node.clone(),
                due_ms: now + (duration * 1000.0).max(1.0),
            });
        }
        set_field(vm, &node, "fire", Value::Invalid)?;
    }
    if !vm.sg.animations.is_empty() {
        super::animation::tick(vm)?;
    }
    if vm.sg.images.has_results() {
        super::bitmap::apply_results(vm)?;
    }
    super::video::service(vm)?;
    if vm.sg.pending_scenes.is_empty() {
        while let Some((key, pressed)) = vm.sg.keys.pop_front() {
            dispatch_key(vm, &key, pressed)?;
        }
    }
    Ok(())
}

fn flush_pending_scenes(vm: &mut Vm) -> Result<(), RuntimeError> {
    while !vm.sg.pending_scenes.is_empty() {
        let scene = vm.sg.pending_scenes.remove(0);
        populate_node(vm, &scene)?;
    }
    Ok(())
}

/// Remote key from the host (`key` uses ECP/remote names, e.g. "OK").
pub fn queue_key(vm: &mut Vm, remote_key: &str, pressed: bool) {
    if let Some(name) = key_name(remote_key) {
        vm.sg.keys.push_back((name.to_string(), pressed));
    }
}

// ---- methods ----------------------------------------------------------------------------

fn arg(args: &[Value], index: usize) -> &Value {
    args.get(index).unwrap_or(&Value::Invalid)
}

fn arg_str(args: &[Value], index: usize) -> String {
    arg(args, index).as_str().unwrap_or_default().to_string()
}

fn arg_int(args: &[Value], index: usize, default: i64) -> i64 {
    args.get(index).and_then(Value::as_i64).unwrap_or(default)
}

fn bool_value(b: bool) -> Value {
    Value::Boolean(b)
}

fn node_list(nodes: Vec<ObjectRef>) -> Value {
    Value::object(Object::Array(nodes.iter().map(node_value).collect()))
}

fn current_scope(vm: &Vm) -> u32 {
    vm.current_function_scope()
}

fn current_m(vm: &Vm) -> Value {
    vm.current_m()
}

pub fn node_method(
    vm: &mut Vm,
    object: &ObjectRef,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let children = || with_node(object, |n| n.children.clone());
    Ok(match name {
        "subtype" => Value::string(with_node(object, |n| n.subtype.to_string())),
        "issubtype" => bool_value(with_node(object, |n| n.is_subtype(&arg_str(&args, 0)))),
        "parentsubtype" => {
            let target = arg_str(&args, 0);
            with_node(object, |n| {
                let position = n
                    .ancestry
                    .iter()
                    .position(|t| t.eq_ignore_ascii_case(&target));
                position
                    .and_then(|p| n.ancestry.get(p + 1))
                    .map_or(Value::string(""), |t| Value::string(t.to_string()))
            })
        }
        "issamenode" => bool_value(node_ref(arg(&args, 0)).is_some_and(|o| Rc::ptr_eq(&o, object))),
        "getchildcount" => Value::Integer(children().len() as i32),
        "getchild" => {
            let index = arg_int(&args, 0, -1);
            usize::try_from(index)
                .ok()
                .and_then(|i| children().get(i).map(node_value))
                .unwrap_or(Value::Invalid)
        }
        "getchildren" => {
            let all = children();
            let count = arg_int(&args, 0, -1);
            let start = arg_int(&args, 1, 0).max(0) as usize;
            let end = if count < 0 {
                all.len()
            } else {
                (start + count as usize).min(all.len())
            };
            node_list(
                all.get(start..end)
                    .map(<[ObjectRef]>::to_vec)
                    .unwrap_or_default(),
            )
        }
        "getparent" => parent_of(object).map_or(Value::Invalid, |p| node_value(&p)),
        "getscene" => ancestors(object)
            .into_iter()
            .find(|n| with_node(n, |n| n.ancestry.iter().any(|t| &**t == "Scene")))
            .map_or(Value::Invalid, |s| node_value(&s)),
        "appendchild" => match node_ref(arg(&args, 0)) {
            Some(child) => {
                attach(object, &child, None);
                vm.sg.revision += 1;
                bool_value(true)
            }
            None => bool_value(false),
        },
        "appendchildren" | "insertchildren" => {
            let mut index =
                (name == "insertchildren").then(|| arg_int(&args, 1, 0).max(0) as usize);
            for child in array_nodes(arg(&args, 0)) {
                attach(object, &child, index);
                index = index.map(|i| i + 1);
            }
            vm.sg.revision += 1;
            bool_value(true)
        }
        "insertchild" => match node_ref(arg(&args, 0)) {
            Some(child) => {
                attach(object, &child, Some(arg_int(&args, 1, 0).max(0) as usize));
                vm.sg.revision += 1;
                bool_value(true)
            }
            None => bool_value(false),
        },
        "removechild" => match node_ref(arg(&args, 0)) {
            Some(child) if parent_of(&child).is_some_and(|p| Rc::ptr_eq(&p, object)) => {
                detach(&child);
                vm.sg.revision += 1;
                bool_value(true)
            }
            _ => bool_value(false),
        },
        "removechildren" => {
            for child in array_nodes(arg(&args, 0)) {
                if parent_of(&child).is_some_and(|p| Rc::ptr_eq(&p, object)) {
                    detach(&child);
                }
            }
            vm.sg.revision += 1;
            bool_value(true)
        }
        "removechildindex" | "removechildrenindex" => {
            let (count, start) = if name == "removechildindex" {
                (1, arg_int(&args, 0, -1))
            } else {
                (arg_int(&args, 0, 0), arg_int(&args, 1, 0))
            };
            let all = children();
            let mut removed = false;
            if start >= 0 {
                for child in all.iter().skip(start as usize).take(count.max(0) as usize) {
                    detach(child);
                    removed = true;
                }
            }
            vm.sg.revision += 1;
            bool_value(removed)
        }
        "replacechild" => {
            let index = arg_int(&args, 1, -1);
            let all = children();
            match (
                node_ref(arg(&args, 0)),
                usize::try_from(index).ok().and_then(|i| all.get(i)),
            ) {
                (Some(child), Some(old)) => {
                    let position = index as usize;
                    detach(old);
                    attach(object, &child, Some(position));
                    vm.sg.revision += 1;
                    bool_value(true)
                }
                _ => bool_value(false),
            }
        }
        "createchild" => {
            let type_name = arg_str(&args, 0);
            match create_node(vm, &type_name)? {
                Some(child) => {
                    attach(object, &child, None);
                    vm.sg.revision += 1;
                    node_value(&child)
                }
                None => Value::Invalid,
            }
        }
        "findnode" => {
            find_node(object, &arg_str(&args, 0)).map_or(Value::Invalid, |n| node_value(&n))
        }
        "addfield" => {
            let field_name = arg_str(&args, 0);
            let kind = FieldKind::parse(&arg_str(&args, 1));
            let always_notify = crate::ops::condition(arg(&args, 2)).unwrap_or(false);
            bool_value(with_node_mut(object, |n| {
                n.add_field(Field {
                    name: field_name,
                    kind,
                    value: kind.default_value(),
                    always_notify,
                    on_change: None,
                    observers: Vec::new(),
                })
            }))
        }
        "addfields" => {
            for (key, value) in aa_entries(arg(&args, 0)) {
                let kind = infer_kind(&value);
                let added = with_node_mut(object, |n| {
                    n.add_field(Field {
                        name: key.clone(),
                        kind,
                        value: kind.default_value(),
                        always_notify: false,
                        on_change: None,
                        observers: Vec::new(),
                    })
                });
                if added {
                    set_field(vm, object, &key, value)?;
                }
            }
            bool_value(true)
        }
        "removefield" => bool_value(with_node_mut(object, |n| {
            n.remove_field(&arg_str(&args, 0))
        })),
        "removefields" => {
            for key in array_strings(arg(&args, 0)) {
                with_node_mut(object, |n| n.remove_field(&key));
            }
            bool_value(true)
        }
        "hasfield" => bool_value(with_node(object, |n| {
            n.field_index(&arg_str(&args, 0)).is_some()
        })),
        "getfield" => get_field(object, &arg_str(&args, 0)),
        "getfields" => {
            let mut aa = AssocArray::new();
            with_node(object, |n| {
                for field in &n.fields {
                    aa.set(&field.name, field.kind.read(&field.value));
                }
            });
            Value::object(Object::AssocArray(aa))
        }
        "setfield" => {
            let field_name = arg_str(&args, 0);
            let exists = with_node(object, |n| n.field_index(&field_name).is_some());
            set_field(vm, object, &field_name, arg(&args, 1).clone())?;
            bool_value(exists)
        }
        "setfields" | "update" => {
            for (key, value) in aa_entries(arg(&args, 0)) {
                let exists = with_node(object, |n| n.field_index(&key).is_some());
                if exists {
                    set_field(vm, object, &key, value)?;
                } else if name == "update" && crate::ops::condition(arg(&args, 1)).unwrap_or(false)
                {
                    let kind = infer_kind(&value);
                    with_node_mut(object, |n| {
                        n.add_field(Field {
                            name: key.clone(),
                            kind,
                            value: kind.default_value(),
                            always_notify: false,
                            on_change: None,
                            observers: Vec::new(),
                        })
                    });
                    set_field(vm, object, &key, value)?;
                }
            }
            bool_value(true)
        }
        "observefield" | "observefieldscoped" => {
            let field_name = arg_str(&args, 0);
            let observer = match arg(&args, 1) {
                Value::String(function) => Observer::Callback {
                    scope: current_scope(vm),
                    function: function.to_string(),
                    m: current_m(vm),
                },
                Value::Object(port) => match &*port.borrow() {
                    Object::MessagePort(id) => Observer::Port { port: *id },
                    _ => return Ok(bool_value(false)),
                },
                _ => return Ok(bool_value(false)),
            };
            bool_value(with_node_mut(object, |n| match n.field_mut(&field_name) {
                Some(field) => {
                    field.observers.push(observer);
                    true
                }
                None => false,
            }))
        }
        "unobservefield" | "unobservefieldscoped" => {
            let field_name = arg_str(&args, 0);
            bool_value(with_node_mut(object, |n| match n.field_mut(&field_name) {
                Some(field) => {
                    field.observers.clear();
                    true
                }
                None => false,
            }))
        }
        "callfunc" => {
            let function_name = arg_str(&args, 0);
            let (declared, component) = with_node(object, |n| {
                (
                    n.functions.contains(&function_name.to_lowercase()),
                    n.component.clone(),
                )
            });
            let function = component
                .as_ref()
                .filter(|_| declared)
                .and_then(|c| vm.program().lookup(c.scope, &function_name));
            match (function, component) {
                (Some(function), Some(component)) => {
                    let call_args = args.into_iter().skip(1).collect();
                    let m = component.m.clone();
                    vm.run_in_render(|vm| vm.call_sync(function, call_args, m))?
                }
                _ => Value::Invalid,
            }
        }
        "setfocus" => {
            let focus = crate::ops::condition(arg(&args, 0)).unwrap_or(false);
            if focus {
                set_focus(vm, Some(object.clone()))?;
            } else if focused_node(vm).is_some_and(|f| Rc::ptr_eq(&f, object)) {
                set_focus(vm, None)?;
            }
            Value::Invalid
        }
        "hasfocus" => bool_value(with_node(object, |n| n.has_focus)),
        "isinfocuschain" => bool_value(
            focused_node(vm).is_some_and(|f| ancestors(&f).iter().any(|a| Rc::ptr_eq(a, object))),
        ),
        "boundingrect" | "localboundingrect" | "scenboundingrect" | "sceneboundingrect" => {
            let rect = match name {
                "localboundingrect" => super::render::local_bounds(vm, object),
                "sceneboundingrect" | "scenboundingrect" => super::render::scene_bounds(vm, object),
                _ => super::render::parent_bounds(vm, object),
            };
            super::rect_value(rect.x, rect.y, rect.width, rect.height)
        }
        "clone" => {
            let deep = crate::ops::condition(arg(&args, 0)).unwrap_or(false);
            node_value(&clone_node(object, deep))
        }
        "count" => Value::Integer(children().len() as i32),
        _ => return Err(RuntimeError::member_not_found()),
    })
}

fn clone_node(object: &ObjectRef, deep: bool) -> ObjectRef {
    let mut copy = with_node(object, |n| n.clone());
    copy.parent = None;
    copy.has_focus = false;
    let children = std::mem::take(&mut copy.children);
    for field in &mut copy.fields {
        field.observers.clear();
    }
    let clone = Rc::new(RefCell::new(Object::Node(copy)));
    if deep {
        for child in children {
            let child_copy = clone_node(&child, true);
            attach(&clone, &child_copy, None);
        }
    }
    clone
}

fn array_nodes(value: &Value) -> Vec<ObjectRef> {
    match value {
        Value::Object(object) => match &*object.borrow() {
            Object::Array(items) | Object::List(items) => {
                items.iter().filter_map(node_ref).collect()
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn array_strings(value: &Value) -> Vec<String> {
    match value {
        Value::Object(object) => match &*object.borrow() {
            Object::Array(items) | Object::List(items) => items
                .iter()
                .filter_map(|v| v.unboxed().as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// AA entries in the device's setFields order (provisional: sorted keys).
fn aa_entries(value: &Value) -> Vec<(String, Value)> {
    match value {
        Value::Object(object) => match &*object.borrow() {
            Object::AssocArray(aa) => aa.sorted_entries(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

pub fn node_event_method(event: &NodeEvent, name: &str) -> Result<Value, RuntimeError> {
    Ok(match name {
        "getdata" => event.data.clone(),
        "getfield" => Value::string(event.field.as_str()),
        "getnode" => Value::string(event.node_id.as_str()),
        "getrosgnode" => event.node.clone(),
        "getinfo" => Value::object(Object::AssocArray(AssocArray::new())),
        _ => return Err(RuntimeError::member_not_found()),
    })
}

pub fn new_screen(vm: &mut Vm) -> Value {
    vm.sg.screens.push(ScreenState::default());
    Value::object(Object::Screen(ScreenHandle {
        id: vm.sg.screens.len() - 1,
    }))
}

pub fn screen_method(
    vm: &mut Vm,
    handle: ScreenHandle,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    Ok(match name {
        "setmessageport" => {
            if let Value::Object(port) = arg(&args, 0) {
                if let Object::MessagePort(id) = &*port.borrow() {
                    vm.sg.screens[handle.id].port = Some(*id);
                }
            }
            Value::Invalid
        }
        "getmessageport" => vm.sg.screens[handle.id]
            .port
            .map_or(Value::Invalid, |id| Value::object(Object::MessagePort(id))),
        "createscene" => {
            let scene_type = arg_str(&args, 0);
            let is_scene = type_chain(vm, &scene_type)
                .is_some_and(|(_, builtin)| builtin_levels(builtin).contains(&"Scene"));
            if !is_scene {
                let location = vm.current_location();
                vm.write(&format!(
                    "BRIGHTSCRIPT: ERROR: roSGScreen.CreateScene: No such node {scene_type}: {location}\n"
                ));
                return Ok(Value::Invalid);
            }
            let scene = allocate_node(vm, &scene_type).expect("scene type");
            vm.sg.screens[handle.id].scene = Some(scene.clone());
            vm.sg.pending_scenes.push(scene.clone());
            node_value(&scene)
        }
        "getscene" => vm.sg.screens[handle.id]
            .scene
            .as_ref()
            .map_or(Value::Invalid, node_value),
        "show" => {
            flush_pending_scenes(vm)?;
            vm.sg.screens[handle.id].shown = true;
            vm.sg.revision += 1;
            Value::Boolean(true)
        }
        "close" => {
            close_screen(vm, handle.id);
            Value::Invalid
        }
        "getglobalnode" => node_value(&global_node(vm)),
        _ => return Err(RuntimeError::member_not_found()),
    })
}
