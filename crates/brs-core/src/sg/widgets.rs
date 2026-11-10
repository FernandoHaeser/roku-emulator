//! Built-in widget behavior. Device probes, not OS UI controls, define keys/events.
use super::runtime::{create_node, set_field};
use super::{node_ref, with_node, with_node_mut, Object, ObjectRef, Value};
use crate::{error::RuntimeError, Vm};
mod keyboard;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

struct Binding {
    content: ObjectRef,
    node: ObjectRef,
    component: String,
}

type Result<T> = std::result::Result<T, RuntimeError>;
#[derive(Default)]
pub struct Widgets {
    entries: HashMap<usize, State>,
    modal: Option<ObjectRef>,
    servicing: bool,
}
struct State {
    node: Weak<std::cell::RefCell<Object>>,
    dirty: bool,
    initial_layout: bool,
    rendered: Vec<ObjectRef>,
    row_positions: HashMap<usize, usize>,
    consumed_keys: HashSet<String>,
    bindings: Vec<Binding>,
    keyboard: keyboard::KeyboardState,
}
fn id(node: &ObjectRef) -> usize {
    Rc::as_ptr(node) as usize
}
fn is_list(node: &ObjectRef) -> bool {
    with_node(node, |n| {
        n.ancestry
            .iter()
            .any(|t| t.eq_ignore_ascii_case("ArrayGrid"))
    })
}
fn is_rows(node: &ObjectRef) -> bool {
    with_node(node, |n| {
        n.ancestry.iter().any(|t| t.eq_ignore_ascii_case("RowList"))
            || n.ancestry
                .iter()
                .any(|t| t.eq_ignore_ascii_case("ZoomRowList"))
    })
}
fn content(node: &ObjectRef) -> Vec<ObjectRef> {
    let root = with_node(node, |n| {
        n.field("content").and_then(|f| node_ref(&f.value))
    });
    root.map(|r| with_node(&r, |n| n.children.clone()))
        .unwrap_or_default()
}
fn number(node: &ObjectRef, field: &str) -> i32 {
    with_node(node, |n| n.number(field) as i32)
}
fn track<'a>(vm: &'a mut Vm, node: &ObjectRef) -> &'a mut State {
    vm.sg
        .widgets
        .entries
        .entry(id(node))
        .or_insert_with(|| State {
            node: Rc::downgrade(node),
            dirty: true,
            initial_layout: false,
            rendered: vec![],
            row_positions: HashMap::new(),
            consumed_keys: HashSet::new(),
            bindings: vec![],
            keyboard: keyboard::KeyboardState::default(),
        })
}
pub fn changed(vm: &mut Vm, node: &ObjectRef, field: &str) -> Result<()> {
    controls_changed(vm, node, field)?;
    keyboard::changed(vm, node, field)?;
    if with_node(node, |n| {
        n.ancestry
            .iter()
            .any(|t| t.eq_ignore_ascii_case("ContentNode"))
    }) {
        for state in vm.sg.widgets.entries.values_mut() {
            state.dirty = true;
        }
    }
    if !is_list(node) && !has(node, "Button") && !has(node, "Dialog") {
        return Ok(());
    }
    if matches!(
        field.to_ascii_lowercase().as_str(),
        "content"
            | "itemsize"
            | "itemspacing"
            | "numrows"
            | "numcolumns"
            | "itemfocused"
            | "text"
            | "color"
            | "focusedcolor"
            | "font"
            | "focusedfont"
            | "rowitemfocused"
            | "itemcomponentname"
            | "title"
            | "message"
            | "buttons"
            | "buttonfocused"
            | "checkeditem"
            | "checkedstate"
            | "checkediconuri"
            | "focusedcheckediconuri"
            | "wrapdividerheight"
            | "wrapdividerwidth"
            | "wrapdividerbitmapuri"
    ) {
        track(vm, node).dirty = true;
    }
    match field.to_ascii_lowercase().as_str() {
        "jumptoitem" | "animatetoitem" => move_to(vm, node, number(node, field), 0)?,
        "jumptorowitem" => {
            let pair = with_node(node, |n| n.field(field).map(|f| f.value.clone()))
                .unwrap_or(Value::Invalid);
            if let Value::Object(pair) = pair {
                let pair = pair.borrow();
                if let Object::Array(values) = &*pair {
                    if let [row, col] = values.as_slice() {
                        move_to(
                            vm,
                            node,
                            row.as_i64().unwrap_or(-1) as i32,
                            col.as_i64().unwrap_or(-1) as i32,
                        )?;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn emit(vm: &mut Vm, node: &ObjectRef, field: &str, value: Value) -> Result<()> {
    // Native item events may repeat the same index (e.g. OK pressed twice).
    with_node_mut(node, |n| {
        if let Some(f) = n.field_mut(field) {
            f.always_notify = true;
        }
    });
    set_field(vm, node, field, value)
}
fn move_to(vm: &mut Vm, node: &ObjectRef, row: i32, col: i32) -> Result<()> {
    let items = content(node);
    if row < 0 || row as usize >= items.len() {
        return Ok(());
    }
    let previous = number(node, "itemFocused");
    if previous != row && previous >= 0 {
        emit(vm, node, "itemUnfocused", Value::Integer(previous))?;
    }
    if previous != row {
        emit(vm, node, "itemFocused", Value::Integer(row))?;
    }
    if is_rows(node) {
        let count = with_node(&items[row as usize], |n| n.children.len());
        if col >= 0 && (col as usize) < count {
            track(vm, node)
                .row_positions
                .insert(row as usize, col as usize);
            set_field(
                vm,
                node,
                "rowItemFocused",
                Value::object(Object::Array(vec![
                    Value::Integer(row),
                    Value::Integer(col),
                ])),
            )?;
        }
    }
    Ok(())
}
pub fn focus(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    if has(node, "Keyboard") || has(node, "MiniKeyboard") {
        track(vm, node).dirty = true;
    }
    if has(node, "Button") {
        track(vm, node).dirty = true;
        if let Some(parent) = with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade)) {
            if has(&parent, "ButtonGroup") {
                if let Some(index) = with_node(&parent, |n| {
                    n.children.iter().position(|c| Rc::ptr_eq(c, node))
                }) {
                    set_field(vm, &parent, "buttonFocused", Value::Integer(index as i32))?;
                }
            }
        }
    }
    if !is_list(node) {
        return Ok(());
    }
    track(vm, node).initial_layout = true;
    if !content(node).is_empty() && number(node, "itemFocused") < 0 {
        if is_rows(node) {
            emit(vm, node, "itemFocused", Value::Integer(0))?;
        } else {
            move_to(vm, node, 0, 0)?;
        }
    }
    Ok(())
}
pub fn key(vm: &mut Vm, node: &ObjectRef, key: &str, pressed: bool) -> Result<bool> {
    if let Some(handled) = keyboard::key(vm, node, key, pressed)? {
        return Ok(handled);
    }
    if let Some(handled) = controls_key(vm, node, key, pressed)? {
        return Ok(handled);
    }
    if !is_list(node) {
        return Ok(false);
    }
    let items = content(node);
    if items.is_empty() {
        return Ok(false);
    }
    let current = number(node, "itemFocused").max(0);
    if key == "OK" {
        let radio_check =
            has(node, "RadioButtonList") && with_node(node, |n| n.flag("checkOnSelect"));
        if pressed {
            if is_rows(node) {
                let pair = with_node(node, |n| n.field("rowItemFocused").unwrap().value.clone());
                emit(vm, node, "rowItemSelected", pair)?;
            }
            if radio_check {
                set_field(vm, node, "checkedItem", Value::Integer(current))?;
            }
            emit(vm, node, "itemSelected", Value::Integer(current))?;
        }
        return Ok((is_rows(node) || radio_check) && pressed);
    }
    if is_rows(node) {
        if !pressed {
            return Ok(track(vm, node).consumed_keys.remove(key));
        }
        let col = track(vm, node)
            .row_positions
            .get(&(current as usize))
            .copied()
            .unwrap_or(0);
        let row_count = items.len() as i32;
        let column_count = with_node(&items[current as usize], |n| n.children.len());
        let (row, next_col) = match key {
            "left" if col > 0 => (current, col - 1),
            "right" if col + 1 < column_count => (current, col + 1),
            "up" if current > 0 => {
                let row = current - 1;
                (
                    row,
                    track(vm, node)
                        .row_positions
                        .get(&(row as usize))
                        .copied()
                        .unwrap_or(0),
                )
            }
            "down" if current + 1 < row_count => {
                let row = current + 1;
                (
                    row,
                    track(vm, node)
                        .row_positions
                        .get(&(row as usize))
                        .copied()
                        .unwrap_or(0),
                )
            }
            _ => return Ok(false),
        };
        track(vm, node).consumed_keys.insert(key.to_string());
        move_to(vm, node, row, next_col as i32)?;
        return Ok(true);
    }
    let columns = number(node, "numColumns").max(1);
    let delta = match key {
        "up" => -columns,
        "down" => columns,
        "left" if columns > 1 => -1,
        "right" if columns > 1 => 1,
        _ => return Ok(false),
    };
    if !pressed {
        return Ok(track(vm, node).consumed_keys.remove(key));
    }
    let wrap = with_node(node, |n| {
        n.text(if matches!(key, "left" | "right") {
            "horizFocusAnimationStyle"
        } else {
            "vertFocusAnimationStyle"
        })
        .eq_ignore_ascii_case("fixedfocuswrap")
    });
    let mut target = current + delta;
    let count = items.len() as i32;
    if matches!(key, "left" | "right") {
        let first = current / columns * columns;
        let end = (first + columns).min(count);
        if target < first || target >= end {
            if !wrap {
                return Ok(false);
            }
            target = if key == "right" { first } else { end - 1 };
        }
    } else if target < 0 || target >= count {
        if !wrap {
            return Ok(false);
        }
        let column = current % columns;
        target = if key == "down" {
            column
        } else {
            column + (count - 1 - column) / columns * columns
        };
    }
    track(vm, node).consumed_keys.insert(key.to_string());
    move_to(vm, node, target, 0)?;
    Ok(true)
}

pub fn children(vm: &Vm, node: &ObjectRef) -> Vec<ObjectRef> {
    vm.sg
        .widgets
        .entries
        .get(&id(node))
        .map(|s| s.rendered.clone())
        .unwrap_or_default()
}
pub fn service(vm: &mut Vm) -> Result<()> {
    if vm.sg.widgets.servicing {
        return Ok(());
    }
    vm.sg.widgets.servicing = true;
    let result = service_inner(vm);
    vm.sg.widgets.servicing = false;
    result
}
fn service_inner(vm: &mut Vm) -> Result<()> {
    // Bindings can enqueue native child widgets. Drain a bounded number of
    // layout passes so a channel blocked on wait() still has its first frame.
    for _ in 0..4 {
        service_once(vm)?;
        if !vm
            .sg
            .widgets
            .entries
            .values()
            .any(|s| s.dirty || s.initial_layout)
        {
            break;
        }
    }
    Ok(())
}
fn service_once(vm: &mut Vm) -> Result<()> {
    let entries = std::mem::take(&mut vm.sg.widgets.entries);
    for (key, mut state) in entries {
        let Some(node) = state.node.upgrade() else {
            continue;
        };
        if state.initial_layout {
            state.initial_layout = false;
            let focused = number(&node, "itemFocused");
            if focused >= 0 {
                if is_rows(&node) {
                    move_to(vm, &node, focused, 0)?;
                }
                emit(vm, &node, "itemFocused", Value::Integer(focused))?;
            }
        }
        if state.dirty {
            state.rendered.clear();
            let is_label = with_node(&node, |n| {
                n.ancestry
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case("LabelList"))
            });
            if is_label {
                let items = content(&node);
                let focus = number(&node, "itemFocused").max(0) as usize;
                let (size, spacing, has_focus) = with_node(&node, |n| {
                    (n.vector("itemSize"), n.vector("itemSpacing"), n.has_focus)
                });
                if has_focus && !items.is_empty() {
                    let background = create_node(vm, "Rectangle")?.expect("Rectangle");
                    set_field(
                        vm,
                        &background,
                        "width",
                        Value::Float((size.0 + 60.0 * design_scale(vm)) as f32),
                    )?;
                    set_field(
                        vm,
                        &background,
                        "height",
                        Value::Float((size.1 + 4.0 * design_scale(vm)) as f32),
                    )?;
                    set_field(
                        vm,
                        &background,
                        "translation",
                        super::float_array(&[-30.0 * design_scale(vm), -4.0 * design_scale(vm)]),
                    )?;
                    set_field(
                        vm,
                        &background,
                        "color",
                        Value::Integer(0xD0D0_D0FFu32 as i32),
                    )?;
                    state.rendered.push(background);
                }
                for offset in 0..items
                    .len()
                    .min(number(&node, "numRows").max(1) as usize)
                    .min(256)
                {
                    let index = (focus + offset) % items.len();
                    let item = &items[index];
                    let label = create_node(vm, "Label")?.expect("Label");
                    let title = with_node(item, |n| n.text("title"));
                    let focused = has_focus && offset == 0;
                    let scale = design_scale(vm);
                    let radio = has(&node, "RadioButtonList");
                    let text_x = if radio { 54.0 * scale } else { 0.0 };
                    let mut y = offset as f64 * (size.1 + spacing.1);
                    // The native fixed-focus radio list separates its last and
                    // first entries when the visible rows wrap (selection-controls).
                    if radio && focus + offset >= items.len() {
                        let divider = with_node(&node, |n| n.number("wrapDividerHeight"));
                        let divider = if divider > 0.0 { divider } else { 36.0 * scale };
                        if index == 0 {
                            add_rectangle(
                                vm,
                                &mut state,
                                0.0,
                                y + divider / 2.0,
                                size.0,
                                1.5 * scale,
                                0x9999_99FFu32,
                            )?;
                        }
                        y += divider;
                    }
                    if radio && number(&node, "checkedItem") == index as i32 {
                        let uri = with_node(&node, |n| {
                            n.text(if focused {
                                "focusedCheckedIconUri"
                            } else {
                                "checkedIconUri"
                            })
                        });
                        let icon =
                            create_node(vm, if uri.is_empty() { "Label" } else { "Poster" })?
                                .expect("selection icon");
                        set_field(vm, &icon, "width", Value::Float((36.0 * scale) as f32))?;
                        set_field(vm, &icon, "height", Value::Float((36.0 * scale) as f32))?;
                        set_field(
                            vm,
                            &icon,
                            "translation",
                            super::float_array(&[0.0, y + (size.1 - 36.0 * scale) / 2.0]),
                        )?;
                        if uri.is_empty() {
                            set_field(vm, &icon, "text", Value::string("✓"))?;
                            let color = with_node(&node, |n| {
                                n.raw(if focused { "focusedColor" } else { "color" })
                            });
                            set_field(vm, &icon, "color", color)?;
                        } else {
                            set_field(vm, &icon, "uri", Value::string(uri))?;
                        }
                        state.rendered.push(icon);
                    }
                    set_field(vm, &label, "text", Value::string(title))?;
                    set_field(
                        vm,
                        &label,
                        "width",
                        Value::Float((size.0 - text_x).max(0.0) as f32),
                    )?;
                    set_field(vm, &label, "height", Value::Float(size.1 as f32))?;
                    set_field(vm, &label, "vertAlign", Value::string("center"))?;
                    set_field(vm, &label, "translation", super::float_array(&[text_x, y]))?;
                    for (target, source) in [
                        ("font", if focused { "focusedFont" } else { "font" }),
                        ("color", if focused { "focusedColor" } else { "color" }),
                        ("horizAlign", "textHorizAlign"),
                    ] {
                        let value = with_node(&node, |n| n.field(source).unwrap().value.clone());
                        set_field(vm, &label, target, value)?;
                    }
                    state.rendered.push(label);
                }
                for child in &state.rendered {
                    with_node_mut(child, |n| n.parent = Some(Rc::downgrade(&node)));
                }
            }
            if !is_label && is_list(&node) {
                build_cells(vm, &node, &mut state)?;
            }
            if has(&node, "Button") {
                build_button(vm, &node, &mut state)?;
            }
            if has(&node, "Dialog") {
                build_dialog(vm, &node, &mut state)?;
            }
            if has(&node, "Keyboard") || has(&node, "MiniKeyboard") {
                keyboard::render(vm, &node, &mut state)?;
            }
            state.dirty = false;
        }
        // Mutations from an observer during layout must survive this pass.
        if let Some(next) = vm.sg.widgets.entries.remove(&key) {
            state.dirty |= next.dirty;
            state.initial_layout |= next.initial_layout;
        }
        vm.sg.widgets.entries.insert(key, state);
    }
    Ok(())
}

fn dimension(node: &ObjectRef, field: &str, index: usize, fallback: (f64, f64)) -> (f64, f64) {
    let value = with_node(node, |n| n.field(field).map(|f| f.value.clone()));
    if let Some(Value::Object(array)) = value {
        let item = match &*array.borrow() {
            Object::Array(items) => items.get(index).cloned(),
            _ => None,
        };
        if let Some(Value::Object(pair)) = item {
            if let Object::Array(values) = &*pair.borrow() {
                if let [x, y] = values.as_slice() {
                    return (
                        x.as_f64().unwrap_or(fallback.0),
                        y.as_f64().unwrap_or(fallback.1),
                    );
                }
            }
        }
    }
    fallback
}
fn build_cells(vm: &mut Vm, widget: &ObjectRef, state: &mut State) -> Result<()> {
    let source = content(widget);
    let (component, size, spacing, focused, has_focus, rows, columns) = with_node(widget, |n| {
        (
            n.text("itemComponentName"),
            n.vector("itemSize"),
            n.vector("itemSpacing"),
            n.number("itemFocused").max(0.0) as usize,
            n.has_focus,
            n.number("numRows").max(1.0) as usize,
            n.number("numColumns").max(1.0) as usize,
        )
    });
    let row_list = is_rows(widget);
    let mut old = std::mem::take(&mut state.bindings);
    let mut positions = Vec::new();
    if row_list {
        let first = focused.saturating_sub(rows - 1);
        for (row, source_row) in source.iter().enumerate().skip(first).take(rows + 1) {
            let items = with_node(source_row, |n| n.children.clone());
            let item_size = dimension(widget, "rowItemSize", row, size);
            let item_spacing = dimension(widget, "rowItemSpacing", row, (0.0, 0.0));
            let col_focus = state.row_positions.get(&row).copied().unwrap_or(0);
            for (col, item) in items.into_iter().enumerate().take(256) {
                positions.push((
                    row,
                    col,
                    item,
                    (
                        col as f64 * (item_size.0 + item_spacing.0),
                        (row - first) as f64 * (size.1 + spacing.1),
                    ),
                    item_size,
                    row == focused && col == col_focus,
                ));
            }
        }
    } else {
        let first_row = (focused / columns).saturating_sub(rows - 1);
        for (index, item) in source
            .into_iter()
            .enumerate()
            .skip(first_row * columns)
            .take(rows.saturating_mul(columns).min(256))
        {
            positions.push((
                index / columns,
                index,
                item,
                (
                    (index % columns) as f64 * (size.0 + spacing.0),
                    (index / columns - first_row) as f64 * (size.1 + spacing.1),
                ),
                size,
                index == focused,
            ));
        }
    }
    for (row, index, item, position, item_size, is_focused) in positions {
        let kind = if component.is_empty() {
            "Poster"
        } else {
            &component
        };
        let cell = if let Some(index) = old
            .iter()
            .position(|b| Rc::ptr_eq(&b.content, &item) && b.component == kind)
        {
            old.remove(index).node
        } else {
            let Some(node) = create_node(vm, kind)? else {
                continue;
            };
            node
        };
        with_node_mut(&cell, |n| n.parent = Some(Rc::downgrade(widget)));
        set_field(
            vm,
            &cell,
            "translation",
            super::float_array(&[position.0, position.1]),
        )?;
        // Roku binds dimensions and indices before content callbacks.
        set_field(vm, &cell, "width", Value::Float(item_size.0 as f32))?;
        set_field(vm, &cell, "height", Value::Float(item_size.1 as f32))?;
        set_field(vm, &cell, "index", Value::Integer(index as i32))?;
        if row_list {
            set_field(vm, &cell, "rowIndex", Value::Integer(row as i32))?;
            set_field(
                vm,
                &cell,
                "rowHasFocus",
                Value::Boolean(has_focus && row == focused),
            )?;
            set_field(vm, &cell, "rowListHasFocus", Value::Boolean(has_focus))?;
        }
        if component.is_empty() {
            let uri = with_node(&item, |n| n.text("HDPosterUrl"));
            set_field(vm, &cell, "uri", Value::string(uri))?;
        } else {
            set_field(vm, &cell, "itemContent", Value::Object(item.clone()))?;
        }
        set_field(
            vm,
            &cell,
            "itemHasFocus",
            Value::Boolean(has_focus && is_focused),
        )?;
        set_field(
            vm,
            &cell,
            "focusPercent",
            Value::Float(if has_focus && is_focused { 1.0 } else { 0.0 }),
        )?;
        set_field(
            vm,
            &cell,
            "rowFocusPercent",
            Value::Float(if has_focus && row == focused {
                1.0
            } else {
                0.0
            }),
        )?;
        state.rendered.push(cell.clone());
        state.bindings.push(Binding {
            content: item,
            node: cell,
            component: kind.to_string(),
        });
    }
    Ok(())
}

fn has(node: &ObjectRef, class: &str) -> bool {
    with_node(node, |n| {
        n.ancestry.iter().any(|t| t.eq_ignore_ascii_case(class))
    })
}
pub fn focus_target(node: &ObjectRef) -> ObjectRef {
    if has(node, "ButtonGroup") {
        let index = number(node, "buttonFocused").max(0) as usize;
        return with_node(node, |n| n.children.get(index).cloned()).unwrap_or_else(|| node.clone());
    }
    node.clone()
}
pub fn blur(vm: &mut Vm, node: &ObjectRef) {
    if is_list(node) || has(node, "Button") || has(node, "Keyboard") || has(node, "MiniKeyboard") {
        track(vm, node).dirty = true;
    }
}
fn strings(node: &ObjectRef, field: &str) -> Vec<String> {
    let value = with_node(node, |n| n.field(field).map(|f| f.value.clone()));
    if let Some(Value::Object(array)) = value {
        if let Object::Array(values) = &*array.borrow() {
            return values
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
        }
    }
    vec![]
}
fn controls_changed(vm: &mut Vm, node: &ObjectRef, field: &str) -> Result<()> {
    if has(node, "Scene") && field.eq_ignore_ascii_case("dialog") {
        let dialog = with_node(node, |n| n.field("dialog").and_then(|f| node_ref(&f.value)));
        if let Some(old) = vm.sg.widgets.modal.take() {
            emit(vm, &old, "wasClosed", Value::Invalid)?;
        }
        vm.sg.widgets.modal = dialog;
    }
    if has(node, "Dialog")
        && field.eq_ignore_ascii_case("close")
        && vm
            .sg
            .widgets
            .modal
            .as_ref()
            .is_some_and(|d| Rc::ptr_eq(d, node))
    {
        vm.sg.widgets.modal = None;
        emit(vm, node, "wasClosed", Value::Invalid)?;
    }
    if has(node, "Dialog") {
        let group = with_node(node, |n| {
            n.field("buttonGroup").and_then(|f| node_ref(&f.value))
        });
        if let Some(group) = group {
            if field.eq_ignore_ascii_case("buttons") || field.eq_ignore_ascii_case("buttonFocused")
            {
                let value = with_node(node, |n| n.field(field).unwrap().value.clone());
                set_field(vm, &group, field, value)?;
            }
        }
    }
    if has(node, "ButtonGroup") {
        if field.eq_ignore_ascii_case("buttons") {
            let labels = strings(node, "buttons");
            with_node_mut(node, |n| n.children.clear());
            for (index, text) in labels.into_iter().enumerate() {
                let button = create_node(vm, "Button")?.expect("Button");
                set_field(vm, &button, "text", Value::string(text))?;
                set_field(
                    vm,
                    &button,
                    "translation",
                    super::float_array(&[0.0, index as f64 * 84.0 * design_scale(vm)]),
                )?;
                with_node_mut(&button, |n| n.parent = Some(Rc::downgrade(node)));
                with_node_mut(node, |n| n.children.push(button));
            }
        }
        if field.eq_ignore_ascii_case("buttonFocused") {
            if let Some(parent) = with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade)) {
                if has(&parent, "Dialog") {
                    set_field(
                        vm,
                        &parent,
                        "buttonFocused",
                        Value::Integer(number(node, "buttonFocused")),
                    )?;
                }
            }
        }
        if matches!(
            field.to_ascii_lowercase().as_str(),
            "minwidth"
                | "maxwidth"
                | "textcolor"
                | "focusedtextcolor"
                | "textfont"
                | "focusedtextfont"
                | "buttonheight"
        ) {
            let value = with_node(node, |n| n.field(field).unwrap().value.clone());
            let children = with_node(node, |n| n.children.clone());
            for child in children {
                if has(&child, "Button") {
                    set_field(
                        vm,
                        &child,
                        if field.eq_ignore_ascii_case("buttonHeight") {
                            "height"
                        } else {
                            field
                        },
                        value.clone(),
                    )?;
                }
            }
        }
        if field.eq_ignore_ascii_case("focusButton") {
            let index = number(node, "focusButton");
            if let Some(child) = with_node(node, |n| n.children.get(index.max(0) as usize).cloned())
            {
                super::runtime::set_focus(vm, Some(child))?;
            }
        }
    }
    Ok(())
}
fn controls_key(vm: &mut Vm, node: &ObjectRef, key: &str, pressed: bool) -> Result<Option<bool>> {
    if has(node, "Button") && key == "OK" {
        if pressed {
            emit(vm, node, "buttonSelected", Value::Invalid)?;
            if let Some(parent) = with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade)) {
                if has(&parent, "ButtonGroup") {
                    let index = number(&parent, "buttonFocused");
                    emit(vm, &parent, "buttonSelected", Value::Integer(index))?;
                }
            }
        }
        return Ok(Some(pressed));
    }
    if has(node, "ButtonGroup") && pressed && matches!(key, "up" | "down") {
        let index = number(node, "buttonFocused") + if key == "down" { 1 } else { -1 };
        let child = with_node(node, |n| {
            if index >= 0 {
                n.children.get(index as usize).cloned()
            } else {
                None
            }
        });
        if let Some(child) = child {
            super::runtime::set_focus(vm, Some(child))?;
            return Ok(Some(true));
        }
    }
    Ok(None)
}
pub fn modal_key(vm: &mut Vm, key: &str, pressed: bool) -> Result<bool> {
    let Some(dialog) = vm.sg.widgets.modal.clone() else {
        return Ok(false);
    };
    if pressed {
        if key == "back" || (key == "options" && with_node(&dialog, |n| n.flag("optionsDialog"))) {
            set_field(vm, &dialog, "close", Value::Boolean(true))?;
        } else if key == "OK" {
            let index = number(&dialog, "buttonFocused");
            emit(vm, &dialog, "buttonSelected", Value::Integer(index))?;
        } else if matches!(key, "up" | "down") {
            let next = number(&dialog, "buttonFocused") + if key == "down" { 1 } else { -1 };
            if next >= 0 && (next as usize) < strings(&dialog, "buttons").len() {
                set_field(vm, &dialog, "buttonFocused", Value::Integer(next))?;
            }
        }
    }
    Ok(true)
}
fn build_button(vm: &mut Vm, node: &ObjectRef, state: &mut State) -> Result<()> {
    let (text, focused, height, min, max) = with_node(node, |n| {
        (
            n.text("text"),
            n.has_focus,
            n.number("height"),
            n.number("minWidth"),
            n.number("maxWidth"),
        )
    });
    let font_field = if focused {
        "focusedTextFont"
    } else {
        "textFont"
    };
    let font = with_node(node, |n| n.field(font_field).unwrap().value.clone());
    let spec = super::render::label_font(vm, &font);
    let width = (super::text::text_width(&spec, &text, vm.sg.design_height >= 1080.0)
        + 84.0 * design_scale(vm))
    .max(min)
    .min(max);
    if focused {
        let bg = create_node(vm, "Rectangle")?.expect("Rectangle");
        set_field(vm, &bg, "width", Value::Float(width as f32))?;
        set_field(vm, &bg, "height", Value::Float(height as f32))?;
        set_field(vm, &bg, "color", Value::Integer(0xD0D0_D0FFu32 as i32))?;
        state.rendered.push(bg);
    }
    let label = create_node(vm, "Label")?.expect("Label");
    set_field(vm, &label, "text", Value::string(text))?;
    set_field(vm, &label, "font", font)?;
    set_field(vm, &label, "width", Value::Float(width as f32))?;
    set_field(vm, &label, "height", Value::Float(height as f32))?;
    set_field(vm, &label, "vertAlign", Value::string("center"))?;
    set_field(vm, &label, "horizAlign", Value::string("center"))?;
    let color = with_node(node, |n| {
        n.field(if focused {
            "focusedTextColor"
        } else {
            "textColor"
        })
        .unwrap()
        .value
        .clone()
    });
    set_field(vm, &label, "color", color)?;
    state.rendered.push(label);
    Ok(())
}

pub fn modal(vm: &Vm) -> Option<ObjectRef> {
    vm.sg.widgets.modal.clone()
}
fn add_rectangle(
    vm: &mut Vm,
    state: &mut State,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: u32,
) -> Result<()> {
    let rect = create_node(vm, "Rectangle")?.expect("Rectangle");
    set_field(vm, &rect, "translation", super::float_array(&[x, y]))?;
    set_field(vm, &rect, "width", Value::Float(w as f32))?;
    set_field(vm, &rect, "height", Value::Float(h as f32))?;
    set_field(vm, &rect, "color", Value::Integer(color as i32))?;
    state.rendered.push(rect);
    Ok(())
}
fn add_label(
    vm: &mut Vm,
    state: &mut State,
    text: String,
    bounds: (f64, f64, f64, f64),
    bold: bool,
) -> Result<()> {
    let (x, y, w, h) = bounds;
    let label = create_node(vm, "Label")?.expect("Label");
    set_field(vm, &label, "translation", super::float_array(&[x, y]))?;
    set_field(vm, &label, "width", Value::Float(w as f32))?;
    set_field(vm, &label, "height", Value::Float(h as f32))?;
    set_field(vm, &label, "text", Value::string(text))?;
    set_field(
        vm,
        &label,
        "font",
        Value::string(if bold {
            "font:MediumBoldSystemFont"
        } else {
            "font:MediumSystemFont"
        }),
    )?;
    set_field(vm, &label, "wrap", Value::Boolean(true))?;
    set_field(vm, &label, "color", Value::Integer(0xFFFF_FFFFu32 as i32))?;
    state.rendered.push(label);
    Ok(())
}
fn build_dialog(vm: &mut Vm, node: &ObjectRef, state: &mut State) -> Result<()> {
    let (title, message, desired, selected) = with_node(node, |n| {
        (
            n.text("title"),
            n.text("message"),
            n.number("width"),
            n.number("buttonFocused") as usize,
        )
    });
    let buttons = strings(node, "buttons");
    let width = if desired > 0.0 {
        desired
    } else {
        vm.sg.design_width.min(1100.0) * 0.85
    };
    let height = 300.0 + buttons.len() as f64 * 84.0;
    let x = (vm.sg.design_width - width) / 2.0;
    let y = (vm.sg.design_height - height) / 2.0;
    add_rectangle(
        vm,
        state,
        0.0,
        0.0,
        vm.sg.design_width,
        vm.sg.design_height,
        0x0000_00B0,
    )?;
    add_rectangle(vm, state, x, y, width, height, 0x2828_28FF)?;
    add_label(
        vm,
        state,
        title,
        (x + 40.0, y + 36.0, width - 80.0, 60.0),
        true,
    )?;
    add_label(
        vm,
        state,
        message,
        (x + 40.0, y + 116.0, width - 80.0, 120.0),
        false,
    )?;
    if let Some(group) = with_node(node, |n| {
        n.field("buttonGroup").and_then(|f| node_ref(&f.value))
    }) {
        set_field(
            vm,
            &group,
            "translation",
            super::float_array(&[x + 40.0, y + 250.0]),
        )?;
        set_field(vm, &group, "minWidth", Value::Float((width - 80.0) as f32))?;
        let children = with_node(&group, |n| n.children.clone());
        for (index, button) in children.into_iter().enumerate() {
            with_node_mut(&button, |n| n.has_focus = index == selected);
            track(vm, &button).dirty = true;
        }
        state.rendered.push(group);
    }
    Ok(())
}

pub fn initialize(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    let scale = design_scale(vm);
    if (has(node, "LabelList") || has(node, "MarkupList")) && scale != 1.0 {
        set_field(
            vm,
            node,
            "itemSize",
            super::float_array(&[510.0 * scale, 72.0 * scale]),
        )?;
    }
    if has(node, "Button") && scale != 1.0 {
        set_field(vm, node, "height", Value::Float((72.0 * scale) as f32))?;
    }

    if has(node, "Dialog") {
        let group = create_node(vm, "ButtonGroup")?.expect("ButtonGroup");
        with_node_mut(&group, |n| n.parent = Some(Rc::downgrade(node)));
        set_field(vm, node, "buttonGroup", Value::Object(group))?;
    }
    keyboard::initialize(vm, node)
}

fn design_scale(vm: &Vm) -> f64 {
    if vm.sg.design_height > 0.0 {
        vm.sg.design_height / 1080.0
    } else {
        1.0
    }
}
