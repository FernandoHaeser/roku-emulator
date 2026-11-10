//! Virtual keyboard geometry and input measured by widgets/keyboard-input.
use super::*;
#[derive(Default)]
pub(super) struct KeyboardState {
    selected: usize,
    shift: bool,
    caps: bool,
    mode: u8,
}
struct Key {
    text: String,
    x: f64,
    y: f64,
    width: f64,
}
fn keys(state: &KeyboardState) -> Vec<Key> {
    let letters = match state.mode {
        1 => "!?#$%&*()+/=:;\"'[]{}<>\\|~^`",
        2 => "áéíóúàèìòùâêîôûãõäëïöüçñåø",
        _ => "abcdefghijklmnopqrstuvwxyz",
    };
    let letters: Vec<char> = letters.chars().collect();
    let mut result = Vec::new();
    for row in 0..4 {
        let mut append = |text: String, x: f64, width: f64| {
            result.push(Key {
                text,
                x,
                y: row as f64 * 84.0,
                width,
            })
        };
        if row == 3 {
            append("←".into(), 0.0, 91.0);
            append("→".into(), 91.0, 91.0);
        } else {
            append(["↑", "space", "delete"][row].into(), 0.0, 182.0);
        }
        for col in 0..7 {
            let index = row * 7 + col;
            let mut text = letters
                .get(index)
                .map(char::to_string)
                .unwrap_or_else(|| if col == 5 { "-" } else { "_" }.into());
            if state.shift || state.caps {
                text = text.to_uppercase();
            }
            append(text, 215.0 + col as f64 * 91.0, 91.0);
        }
        for col in 0..3 {
            append(
                if row == 3 {
                    ["@", ".", "0"][col].into()
                } else {
                    (row * 3 + col + 1).to_string()
                },
                885.0 + col as f64 * 91.0,
                91.0,
            );
        }
        append(
            ["caps", "ABC123", "symbols", "accents"][row].into(),
            1190.0,
            182.0,
        );
    }
    result
}
pub(super) fn initialize(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    if !has(node, "Keyboard") && !has(node, "MiniKeyboard") {
        return Ok(());
    }
    let edit = create_node(vm, "TextEditBox")?.expect("TextEditBox");
    with_node_mut(&edit, |n| n.parent = Some(Rc::downgrade(node)));
    set_field(vm, node, "textEditBox", Value::Object(edit))?;
    track(vm, node).dirty = true;
    Ok(())
}
fn edit(node: &ObjectRef) -> Option<ObjectRef> {
    with_node(node, |n| {
        n.field("textEditBox").and_then(|f| node_ref(&f.value))
    })
}
pub(super) fn changed(vm: &mut Vm, node: &ObjectRef, field: &str) -> Result<()> {
    if has(node, "Keyboard") || has(node, "MiniKeyboard") {
        track(vm, node).dirty = true;
        if field.eq_ignore_ascii_case("text") {
            if let Some(edit) = edit(node) {
                let text = with_node(node, |n| n.text("text"));
                set_field(vm, &edit, "text", Value::string(text.clone()))?;
                set_field(
                    vm,
                    &edit,
                    "cursorPosition",
                    Value::Integer(text.chars().count() as i32),
                )?;
            }
        }
    } else if has(node, "TextEditBox") {
        if let Some(parent) = with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade)) {
            if has(&parent, "Keyboard") || has(&parent, "MiniKeyboard") {
                if field.eq_ignore_ascii_case("text") {
                    let text = with_node(node, |n| n.text("text"));
                    set_field(vm, &parent, "text", Value::string(text))?;
                }
                track(vm, &parent).dirty = true;
            }
        }
    }
    Ok(())
}
pub(super) fn key(vm: &mut Vm, node: &ObjectRef, key: &str, pressed: bool) -> Result<Option<bool>> {
    if !has(node, "Keyboard") && !has(node, "MiniKeyboard") {
        return Ok(None);
    }
    if matches!(key, "up" | "down" | "left" | "right") {
        if pressed {
            let state = &mut track(vm, node).keyboard;
            let layout = keys(state);
            let current = &layout[state.selected.min(layout.len() - 1)];
            let (x, y) = (current.x + current.width / 2.0, current.y + 42.0);
            let next = layout
                .iter()
                .enumerate()
                .filter_map(|(index, candidate)| {
                    let (cx, cy) = (candidate.x + candidate.width / 2.0, candidate.y + 42.0);
                    let eligible = match key {
                        "left" => cx < x && cy == y,
                        "right" => cx > x && cy == y,
                        "up" => cy < y,
                        "down" => cy > y,
                        _ => false,
                    };
                    eligible.then_some((index, (cx - x).abs() + (cy - y).abs() * 10.0))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((index, _)) = next {
                state.selected = index;
            } else if matches!(key, "up" | "down") {
                return Ok(Some(false));
            }
            track(vm, node).consumed_keys.insert(key.to_string());
            track(vm, node).dirty = true;
            return Ok(Some(true));
        }
        return Ok(Some(track(vm, node).consumed_keys.remove(key)));
    }
    if key == "options" {
        if pressed {
            let state = &mut track(vm, node).keyboard;
            state.caps = !state.caps;
            state.shift = false;
            track(vm, node).dirty = true;
        }
        return Ok(Some(pressed));
    }
    if key != "OK" {
        return Ok(Some(false));
    }
    if !pressed {
        return Ok(Some(false));
    }
    let text = {
        let state = &track(vm, node).keyboard;
        let layout = keys(state);
        layout[state.selected.min(layout.len() - 1)].text.clone()
    };
    match text.as_str() {
        "↑" => {
            let state = &mut track(vm, node).keyboard;
            state.shift = !state.shift;
        }
        "caps" => {
            let state = &mut track(vm, node).keyboard;
            state.caps = !state.caps;
        }
        "ABC123" => track(vm, node).keyboard.mode = 0,
        "symbols" => track(vm, node).keyboard.mode = 1,
        "accents" => track(vm, node).keyboard.mode = 2,
        _ => {
            if let Some(edit) = edit(node) {
                let (old, cursor, max) = with_node(&edit, |n| {
                    (
                        n.text("text"),
                        n.number("cursorPosition").max(0.0) as usize,
                        n.number("maxTextLength").max(0.0) as usize,
                    )
                });
                let mut chars: Vec<char> = old.chars().collect();
                let mut cursor = cursor.min(chars.len());
                match text.as_str() {
                    "←" => cursor = cursor.saturating_sub(1),
                    "→" => cursor = (cursor + 1).min(chars.len()),
                    "delete" => {
                        if cursor > 0 {
                            cursor -= 1;
                            chars.remove(cursor);
                        }
                    }
                    _ => {
                        if chars.len() < max {
                            chars.insert(
                                cursor,
                                if text == "space" {
                                    ' '
                                } else {
                                    text.chars().next().unwrap_or(' ')
                                },
                            );
                            cursor += 1;
                            track(vm, node).keyboard.shift = false;
                        }
                    }
                }
                set_field(
                    vm,
                    node,
                    "text",
                    Value::string(chars.into_iter().collect::<String>()),
                )?;
                set_field(vm, &edit, "cursorPosition", Value::Integer(cursor as i32))?;
            }
        }
    }
    track(vm, node).dirty = true;
    Ok(Some(true))
}
/// HD channels get the device's own HD keyboard artwork instead of the FHD
/// layout scaled by 2/3 (widgets/keyboard-input-hd). Panels as (FHD key x,
/// FHD width, HD output x from the keyboard origin, HD output width).
const HD_PANELS: [(f64, f64, f64, f64); 4] = [
    (0.0, 182.0, 15.0, 122.0),
    (215.0, 637.0, 158.0, 423.0),
    (885.0, 273.0, 603.0, 181.0),
    (1190.0, 182.0, 806.0, 120.0),
];
/// HD text box: output x and width from the keyboard origin.
const HD_TEXT_BOX: (f64, f64) = (15.0, 912.0);

fn is_hd(vm: &Vm) -> bool {
    super::design_scale(vm) != 1.0
}

/// Key cell in FHD design units relative to the keyboard origin: (x, width).
fn cell(vm: &Vm, key: &Key) -> (f64, f64) {
    if !is_hd(vm) {
        return (22.0 + key.x, key.width);
    }
    let scale = super::design_scale(vm);
    let (fhd_x, fhd_width, hd_x, hd_width) = HD_PANELS
        .iter()
        .copied()
        .rev()
        .find(|(start, ..)| key.x >= *start)
        .unwrap_or(HD_PANELS[0]);
    let ratio = hd_width / fhd_width;
    // HD output px -> FHD design units (keyboard_* helpers scale back down).
    let x = (hd_x + (key.x - fhd_x) * ratio) / scale;
    (x, key.width * ratio / scale)
}

pub(super) fn render(vm: &mut Vm, node: &ObjectRef, state: &mut State) -> Result<()> {
    let value = with_node(node, |n| n.text("text"));
    let hd = is_hd(vm);
    // Device keyboard: text box at (22,0), key grid at (22,94), design px.
    let (box_x, box_width) = if hd {
        let scale = super::design_scale(vm);
        (HD_TEXT_BOX.0 / scale, HD_TEXT_BOX.1 / scale)
    } else {
        (22.0, 1372.0)
    };
    keyboard_rectangle(vm, state, box_x, 0.0, box_width, 72.0, 0xFFFF_FF26)?;
    // Text label inside the box (app-ui.xml): FHD {16,17} 1340x38, HD {24,11} 864x26.
    let text_bounds = if hd {
        let scale = super::design_scale(vm);
        (
            box_x + 24.0 / scale,
            11.0 / scale,
            864.0 / scale,
            26.0 / scale,
        )
    } else {
        (box_x + 16.0, 17.0, 1340.0, 38.0)
    };
    keyboard_label(vm, state, value, text_bounds, false)?;
    let layout = keys(&state.keyboard);
    let mut focused_key = None;
    for (index, key) in layout.into_iter().enumerate() {
        let focused = with_node(node, |n| n.has_focus) && index == state.keyboard.selected;
        let (x, width) = cell(vm, &key);
        let y = 94.0 + key.y;
        // FHD cells sit inside their bitmap margins; HD cells are separated
        // by a 1 output px line.
        let (inset, gap) = if hd {
            (0.0, 1.5)
        } else if key.x < 885.0 {
            (6.0, 2.0)
        } else {
            (3.0, 2.0)
        };
        keyboard_rectangle(
            vm,
            state,
            x + inset,
            y + 3.0,
            width - gap,
            // HD rows are 84 FHD units apart with a 1 output px separator.
            if hd { 84.0 - gap } else { 82.0 },
            0xFFFF_FF26,
        )?;
        let key = Key { width, ..key };
        if focused {
            focused_key = Some((key, x));
            continue;
        }
        draw_key_label(vm, state, &key, x, y, false)?;
    }
    if let Some((key, x)) = focused_key {
        let y = 94.0 + key.y;
        // Focus feedback paints above neighboring cells and includes margins.
        keyboard_rectangle(
            vm,
            state,
            x - 11.0,
            y - 13.0,
            key.width + 24.0,
            108.0,
            0x3232_32FF,
        )?;
        // Measured keyboard focus bitmap: grayscale ramp, about 224 -> 151.
        // Generate geometry rather than redistribute the device's artwork.
        for band in 0..35 {
            let shade = (224.0 - 73.0 * band as f64 / 34.0).round() as u32;
            let color = (shade << 24) | (shade << 16) | (shade << 8) | 255;
            keyboard_rectangle(
                vm,
                state,
                x - 9.5,
                y - 11.5 + band as f64 * 3.0,
                key.width + 21.0,
                3.0,
                color,
            )?;
        }
        draw_key_label(vm, state, &key, x, y, true)?;
    }
    Ok(())
}
fn draw_key_label(
    vm: &mut Vm,
    state: &mut State,
    key: &Key,
    x: f64,
    y: f64,
    focused: bool,
) -> Result<()> {
    let scale = super::design_scale(vm);
    if key.x >= 1190.0 && key.text != "caps" {
        let caption = match key.text.as_str() {
            "symbols" => "!?#$%&",
            "accents" => "áéíóú",
            other => other,
        };
        for (text, offset, width, size) in [
            (if key.text == "ABC123" { "◉" } else { "○" }, 20.0, 36.0, 36),
            (caption, 66.0, 116.0, 24),
        ] {
            let label = create_node(vm, "Label")?.expect("Label");
            set_field(vm, &label, "text", Value::string(text))?;
            set_field(
                vm,
                &label,
                "translation",
                super::super::float_array(&[
                    (x + offset) * scale,
                    (y + if size == 24 { 32.0 } else { 24.0 }) * scale,
                ]),
            )?;
            set_field(vm, &label, "width", Value::Float((width * scale) as f32))?;
            set_field(vm, &label, "height", Value::Float((38.0 * scale) as f32))?;
            if size == 24 {
                let font = create_node(vm, "Font")?.expect("Font");
                set_field(vm, &font, "uri", Value::string("font:MediumBoldSystemFont"))?;
                set_field(
                    vm,
                    &font,
                    "size",
                    Value::Integer((24.0 * scale).round() as i32),
                )?;
                set_field(vm, &label, "font", Value::Object(font))?;
            }
            set_field(
                vm,
                &label,
                "color",
                Value::Integer(if focused {
                    0x2626_26FF
                } else {
                    0xFEFE_FEFFu32 as i32
                }),
            )?;
            state.rendered.push(label);
        }
        return Ok(());
    }
    let text = match key.text.as_str() {
        "space" => "␣",
        "delete" => "⌫",
        "caps" => "☑↑",
        "symbols" => "○ !?#$%&",
        "accents" => "○ áéíóú",
        "ABC123" => "◉ ABC123",
        other => other,
    };
    let label = create_node(vm, "Label")?.expect("Label");
    set_field(
        vm,
        &label,
        "font",
        Value::string("font:MediumBoldSystemFont"),
    )?;
    set_field(vm, &label, "text", Value::string(text))?;
    set_field(
        vm,
        &label,
        "translation",
        super::super::float_array(&[x * scale, (y + 26.0) * scale]),
    )?;
    set_field(
        vm,
        &label,
        "width",
        Value::Float((key.width * scale) as f32),
    )?;
    set_field(vm, &label, "height", Value::Float((38.0 * scale) as f32))?;
    set_field(vm, &label, "horizAlign", Value::string("center"))?;
    if key.x >= 1190.0 {
        set_field(
            vm,
            &label,
            "font",
            Value::string("font:SmallestBoldSystemFont"),
        )?;
    }
    set_field(
        vm,
        &label,
        "color",
        Value::Integer(if focused {
            0x2626_26FF
        } else {
            0xFEFE_FEFFu32 as i32
        }),
    )?;
    state.rendered.push(label);
    Ok(())
}

fn keyboard_rectangle(
    vm: &mut Vm,
    state: &mut State,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: u32,
) -> Result<()> {
    let scale = super::design_scale(vm);
    super::add_rectangle(vm, state, x * scale, y * scale, w * scale, h * scale, color)
}
fn keyboard_label(
    vm: &mut Vm,
    state: &mut State,
    text: String,
    bounds: (f64, f64, f64, f64),
    bold: bool,
) -> Result<()> {
    let scale = super::design_scale(vm);
    let (x, y, w, h) = bounds;
    super::add_label(
        vm,
        state,
        text,
        (x * scale, y * scale, w * scale, h * scale),
        bold,
    )
}
