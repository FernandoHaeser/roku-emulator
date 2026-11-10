//! Standard keyboard dialog text binding and remote navigation.
use super::*;

pub(super) fn initialize(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    if !has(node, "StandardKeyboardDialog") {
        return Ok(());
    }
    let keyboard = create_node(vm, "DynamicKeyboard")?.expect("DynamicKeyboard");
    with_node_mut(&keyboard, |n| n.parent = Some(Rc::downgrade(node)));
    let edit = with_node(&keyboard, |n| {
        n.field("textEditBox").and_then(|f| node_ref(&f.value))
    })
    .expect("textbox");
    set_field(vm, node, "textEditBox", Value::Object(edit))?;
    track(vm, node).dialog_keyboard = Some(keyboard);
    Ok(())
}
pub(super) fn changed(vm: &mut Vm, node: &ObjectRef, field: &str) -> Result<()> {
    if has(node, "StandardKeyboardDialog") {
        let keyboard = track(vm, node).dialog_keyboard.clone();
        if let Some(keyboard) = keyboard {
            if field.eq_ignore_ascii_case("text") || field.eq_ignore_ascii_case("keyboardDomain") {
                let value = with_node(node, |n| n.field(field).unwrap().value.clone());
                set_field(vm, &keyboard, field, value)?;
            }
        }
        track(vm, node).dirty = true;
    } else if has(node, "DynamicKeyboard") && field.eq_ignore_ascii_case("text") {
        if let Some(parent) = with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade)) {
            if has(&parent, "StandardKeyboardDialog") {
                let text = with_node(node, |n| n.text("text"));
                set_field(vm, &parent, "text", Value::string(text))?;
            }
        }
    }
    Ok(())
}
pub(super) fn key(vm: &mut Vm, node: &ObjectRef, key: &str, pressed: bool) -> Result<bool> {
    if !has(node, "StandardKeyboardDialog") || key == "back" {
        return Ok(false);
    }
    let Some(keyboard) = track(vm, node).dialog_keyboard.clone() else {
        return Ok(false);
    };
    let buttons = track(vm, node).dialog_buttons;
    if !buttons {
        if keyboard::key(vm, &keyboard, key, pressed)?.unwrap_or(false) {
            return Ok(true);
        }
        if pressed && key == "down" && !strings(node, "buttons").is_empty() {
            track(vm, node).dialog_buttons = true;
            with_node_mut(&keyboard, |n| n.has_focus = false);
            track(vm, &keyboard).dirty = true;
            track(vm, node).dirty = true;
        }
        return Ok(true);
    }
    if pressed && key == "up" && number(node, "buttonFocused") == 0 {
        track(vm, node).dialog_buttons = false;
        with_node_mut(&keyboard, |n| n.has_focus = true);
        track(vm, &keyboard).dirty = true;
        track(vm, node).dirty = true;
        return Ok(true);
    }
    Ok(false)
}
pub(super) fn render(vm: &mut Vm, node: &ObjectRef, state: &mut State) -> Result<()> {
    let scale = design_scale(vm);
    let height = 783.0 + strings(node, "buttons").len().saturating_sub(1) as f64 * 84.0;
    let x = (vm.sg.design_width - 1548.0 * scale) / 2.0;
    let y = (vm.sg.design_height - height * scale) / 2.0;
    add_rectangle(
        vm,
        state,
        0.0,
        0.0,
        vm.sg.design_width,
        vm.sg.design_height,
        0x0D001DB0,
    )?;
    add_rectangle(vm, state, x, y, 1548.0 * scale, height * scale, 0x665D71FF)?;
    add_label(
        vm,
        state,
        with_node(node, |n| n.text("title")),
        (
            x + 90.0 * scale,
            y + 54.0 * scale,
            1368.0 * scale,
            69.0 * scale,
        ),
        false,
    )?;
    add_rectangle(
        vm,
        state,
        x + 90.0 * scale,
        y + 124.5 * scale,
        1368.0 * scale,
        3.0 * scale,
        0xFFFFFF26,
    )?;
    if let Some(keyboard) = &state.dialog_keyboard {
        set_field(
            vm,
            keyboard,
            "translation",
            super::super::float_array(&[x + 90.0 * scale, y + 156.0 * scale]),
        )?;
        with_node_mut(keyboard, |n| n.has_focus = !state.dialog_buttons);
        track(vm, keyboard).dirty = true;
        state.rendered.push(keyboard.clone());
    }
    let selected = number(node, "buttonFocused");
    for (index, text) in strings(node, "buttons").into_iter().enumerate() {
        let by = y + (654.0 + index as f64 * 84.0) * scale;
        let focused = state.dialog_buttons && selected == index as i32;
        add_rectangle(
            vm,
            state,
            x + 90.0 * scale,
            by,
            1368.0 * scale,
            60.0 * scale,
            if focused { 0xEEEEEEFF } else { 0xFFFFFF0D },
        )?;
        add_label(
            vm,
            state,
            text,
            (
                x + 112.0 * scale,
                by + 6.0 * scale,
                1324.0 * scale,
                48.0 * scale,
            ),
            false,
        )?;
        if focused {
            if let Some(label) = state.rendered.last() {
                set_field(vm, label, "color", Value::Integer(0x000000FF))?;
            }
        }
    }
    Ok(())
}
