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
pub(super) fn is_keyboard(node: &ObjectRef) -> bool {
    has(node, "Keyboard") || has(node, "MiniKeyboard") || has(node, "DynamicKeyboard")
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
fn layout(node: &ObjectRef, state: &KeyboardState) -> Vec<Key> {
    if !has(node, "MiniKeyboard") {
        let mut result = keys(state);
        if has(node, "DynamicKeyboard") {
            for key in &mut result {
                key.x = if key.x >= 1190.0 {
                    1188.0
                } else if key.x >= 885.0 {
                    882.0 + ((key.x - 885.0) / 91.0).round() * 90.0
                } else if key.x >= 215.0 {
                    216.0 + ((key.x - 215.0) / 91.0).round() * 90.0
                } else {
                    key.x / 91.0 * 90.0
                };
                key.width = if key.width > 100.0 { 180.0 } else { 90.0 };
            }
        }
        return result;
    }
    let lower = with_node(node, |n| n.flag("lowerCase"));
    let mut result: Vec<Key> = "abcdefghijklmnopqrstuvwxyz1234567890"
        .chars()
        .enumerate()
        .map(|(index, c)| Key {
            text: if lower {
                c.to_string()
            } else {
                c.to_uppercase().to_string()
            },
            x: (index % 6) as f64 * 90.0,
            y: (index / 6) as f64 * 84.0,
            width: 87.0,
        })
        .collect();
    for (index, text) in ["clear", "space", "delete"].iter().enumerate() {
        result.push(Key {
            text: text.to_string(),
            x: index as f64 * 180.0,
            y: 504.0,
            width: 177.0,
        });
    }
    result
}
pub(super) fn initialize(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    if has(node, "TextEditBox") {
        track(vm, node).dirty = true;
    }
    if !is_keyboard(node) {
        return Ok(());
    }
    let edit = create_node(vm, "TextEditBox")?.expect("TextEditBox");
    set_field(
        vm,
        &edit,
        "maxTextLength",
        Value::Integer(if has(node, "MiniKeyboard") { 25 } else { 75 }),
    )?;
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
    if is_keyboard(node) {
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
        track(vm, node).dirty = true;
        if let Some(parent) = with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade)) {
            if is_keyboard(&parent) {
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
    if !is_keyboard(node) {
        return Ok(None);
    }
    if matches!(key, "up" | "down" | "left" | "right") {
        if pressed {
            let state = &mut track(vm, node).keyboard;
            let layout = layout(node, state);
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
            } else if has(node, "MiniKeyboard") || matches!(key, "up" | "down") {
                return Ok(Some(false));
            }
            track(vm, node).consumed_keys.insert(key.to_string());
            track(vm, node).dirty = true;
            return Ok(Some(true));
        }
        return Ok(Some(track(vm, node).consumed_keys.remove(key)));
    }
    if key == "options" && !has(node, "MiniKeyboard") {
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
        let layout = layout(node, state);
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
                    "clear" => {
                        chars.clear();
                        cursor = 0;
                    }
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
    if has(node, "MiniKeyboard") {
        return render_mini(vm, node, state);
    }
    if has(node, "DynamicKeyboard") {
        return render_dynamic(vm, node, state);
    }
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
    let layout = layout(node, &state.keyboard);
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
                with_node_mut(&font, |n| {
                    n.native_font = Some(super::super::NativeFont::Sized)
                });
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

/// MiniKeyboard is a distinct six-column widget. Measurements come from the
/// HD/FHD mini-keyboard probes, not from scaling the full keyboard template.
fn render_mini(vm: &mut Vm, node: &ObjectRef, state: &mut State) -> Result<()> {
    let scale = super::design_scale(vm);
    let hd = is_hd(vm);
    let (box_x, grid_x, grid_y) = if hd {
        (15.0, 22.5, 94.5)
    } else {
        (14.5, 22.0, 94.0)
    };
    if with_node(node, |n| n.flag("showTextEditBox")) {
        keyboard_rectangle(vm, state, box_x, 0.0, 555.0, 72.0, 0xFFFF_FF26)?;
        keyboard_label(
            vm,
            state,
            with_node(node, |n| n.text("text")),
            (
                box_x + 36.0,
                if hd { 16.5 } else { 17.0 },
                483.0,
                if hd { 39.0 } else { 38.0 },
            ),
            false,
        )?;
    }
    let focused = with_node(node, |n| n.has_focus);
    // The device uses one continuous keyboard bitmap, with subtle dividers;
    // empty black gutters between independent rectangles exaggerate the grid.
    keyboard_rectangle(
        vm,
        state,
        grid_x + 3.0,
        grid_y + 4.5,
        537.0,
        585.0,
        0x414141FF,
    )?;
    for row in 1..7 {
        keyboard_rectangle(
            vm,
            state,
            grid_x + 3.0,
            grid_y + 4.5 + row as f64 * 84.0 - 1.5,
            537.0,
            1.5,
            0x323232A0,
        )?;
    }
    for col in 1..6 {
        keyboard_rectangle(
            vm,
            state,
            grid_x + 3.0 + col as f64 * 90.0 - 1.5,
            grid_y + 4.5,
            1.5,
            504.0,
            0x32323260,
        )?;
    }
    for col in 1..3 {
        keyboard_rectangle(
            vm,
            state,
            grid_x + 3.0 + col as f64 * 180.0 - 1.5,
            grid_y + 508.5,
            1.5,
            81.0,
            0x32323260,
        )?;
    }
    for (index, key) in layout(node, &state.keyboard).into_iter().enumerate() {
        let x = grid_x + key.x;
        let y = grid_y + key.y;
        let selected = focused && index == state.keyboard.selected;
        if selected {
            mini_focus(vm, state, x - 13.5, y - 13.5, key.width + 25.5)?;
        }
        let icon_color = if selected {
            0x000000FFu32
        } else {
            with_node(node, |n| {
                n.number(if focused {
                    "focusedKeyColor"
                } else {
                    "keyColor"
                }) as i64 as u32
            })
        };
        if mini_icon(vm, state, &key, x, y, icon_color)? {
            continue;
        }
        let label = create_node(vm, "Label")?.expect("Label");
        let text = match key.text.as_str() {
            "clear" => "⌦",
            "space" => "␣",
            "delete" => "⌫",
            other => other,
        };
        set_field(vm, &label, "text", Value::string(text))?;
        set_field(
            vm,
            &label,
            "font",
            // Mini keyboard captures draw every key label bold, focused or not.
            Value::string("font:MediumBoldSystemFont"),
        )?;
        set_field(
            vm,
            &label,
            "translation",
            super::super::float_array(&[
                x * scale,
                (y + if key.text.chars().all(|c| c.is_ascii_digit()) {
                    25.5
                } else {
                    22.5
                }) * scale,
            ]),
        )?;
        set_field(
            vm,
            &label,
            "width",
            Value::Float((key.width * scale) as f32),
        )?;
        set_field(vm, &label, "height", Value::Float((38.0 * scale) as f32))?;
        set_field(vm, &label, "horizAlign", Value::string("center"))?;
        let color = if selected {
            Value::Integer(0x0000_00FF)
        } else {
            with_node(node, |n| {
                n.raw(if focused {
                    "focusedKeyColor"
                } else {
                    "keyColor"
                })
            })
        };
        set_field(vm, &label, "color", color)?;
        state.rendered.push(label);
    }
    Ok(())
}

pub(super) fn render_edit(vm: &mut Vm, node: &ObjectRef, state: &mut State) -> Result<()> {
    // Keyboard widgets render their internal textbox with their own layout.
    if with_node(node, |n| n.parent.as_ref().and_then(Weak::upgrade))
        .is_some_and(|parent| is_keyboard(&parent))
    {
        return Ok(());
    }
    let scale = super::design_scale(vm);
    let width = with_node(node, |n| n.number("width"));
    let width = if width > 0.0 { width / scale } else { 555.0 };
    keyboard_rectangle(vm, state, 0.0, 0.0, width, 72.0, 0xFFFFFF26)?;
    let text = with_node(node, |n| n.text("text"));
    let secure = with_node(node, |n| n.flag("secureMode"));
    let shown = if secure {
        "*".repeat(text.chars().count())
    } else {
        text.clone()
    };
    let value = if shown.is_empty() {
        with_node(node, |n| n.text("hintText"))
    } else {
        shown
    };
    keyboard_label(
        vm,
        state,
        value.clone(),
        (36.0, 17.0, width - 72.0, 38.0),
        false,
    )?;
    if let Some(label) = state.rendered.last() {
        let color = with_node(node, |n| {
            n.field(if text.is_empty() {
                "hintTextColor"
            } else {
                "textColor"
            })
            .unwrap()
            .value
            .clone()
        });
        set_field(vm, label, "color", color)?;
    }
    if with_node(node, |n| n.flag("active") || n.has_focus) {
        let font = super::super::render::label_font(vm, &Value::string("font:MediumSystemFont"));
        let prefix: String = value
            .chars()
            .take(with_node(node, |n| n.number("cursorPosition")) as usize)
            .collect();
        let x = 36.0 + super::super::text::text_width(&font, &prefix, !is_hd(vm)) / scale;
        keyboard_rectangle(vm, state, x.min(width - 36.0), 17.0, 3.0, 36.0, 0xFFFFFFFF)?;
    }
    Ok(())
}

fn render_dynamic(vm: &mut Vm, node: &ObjectRef, state: &mut State) -> Result<()> {
    keyboard_rectangle(vm, state, 0.0, 0.0, 1368.0, 72.0, 0xFFFFFF0D)?;
    let text = with_node(node, |n| n.text("text"));
    let secure = with_node(node, |n| n.text("keyboardDomain") == "password")
        || edit(node).is_some_and(|edit| with_node(&edit, |n| n.flag("secureMode")));
    keyboard_label(
        vm,
        state,
        if secure {
            "*".repeat(text.chars().count())
        } else {
            text
        },
        (36.0, 17.0, 1296.0, 38.0),
        false,
    )?;
    let mut selected = None;
    for (index, key) in layout(node, &state.keyboard).into_iter().enumerate() {
        let y = 100.0 + key.y;
        keyboard_rectangle(vm, state, key.x, y, key.width - 2.0, 82.0, 0xFFFFFF26)?;
        if with_node(node, |n| n.has_focus) && index == state.keyboard.selected {
            selected = Some(key);
        } else {
            draw_key_label(vm, state, &key, key.x, y, false)?;
        }
    }
    if let Some(key) = selected {
        keyboard_rectangle(
            vm,
            state,
            key.x - 4.0,
            96.0 + key.y,
            key.width + 8.0,
            90.0,
            0xEEEEEEFF,
        )?;
        draw_key_label(vm, state, &key, key.x, 100.0 + key.y, true)?;
    }
    Ok(())
}

/// Host text input is an emulator convenience; remote navigation stays separate.
pub(super) fn text_target(vm: &Vm) -> Option<ObjectRef> {
    if let Some(dialog) = &vm.sg.widgets.modal {
        return vm
            .sg
            .widgets
            .entries
            .get(&id(dialog))
            .and_then(|state| state.dialog_keyboard.as_ref())
            .and_then(edit);
    }
    if let Some(focused) = super::super::runtime::focused_node(vm) {
        if is_keyboard(&focused) {
            return edit(&focused);
        }
        if has(&focused, "TextEditBox") {
            return Some(focused);
        }
    }
    fn active(node: &ObjectRef) -> Option<ObjectRef> {
        if !with_node(node, |n| n.flag("visible")) {
            return None;
        }
        if has(node, "TextEditBox") && with_node(node, |n| n.flag("active")) {
            return Some(node.clone());
        }
        with_node(node, |n| n.children.clone())
            .iter()
            .rev()
            .find_map(active)
    }
    vm.sg
        .screens
        .iter()
        .rev()
        .find(|screen| screen.shown && !screen.closed)
        .and_then(|screen| screen.scene.as_ref())
        .and_then(active)
}
pub(super) fn host_input(vm: &mut Vm, text: &str, command: &str) -> Result<bool> {
    let Some(target) = text_target(vm) else {
        return Ok(false);
    };
    let (old, cursor, max) = with_node(&target, |n| {
        (
            n.text("text"),
            n.number("cursorPosition").max(0.0) as usize,
            n.number("maxTextLength").max(0.0) as usize,
        )
    });
    let mut chars: Vec<char> = old.chars().collect();
    let mut cursor = cursor.min(chars.len());
    match command {
        "Backspace" => {
            if cursor > 0 {
                cursor -= 1;
                chars.remove(cursor);
            }
        }
        "Delete" => {
            if cursor < chars.len() {
                chars.remove(cursor);
            }
        }
        "ArrowLeft" => cursor = cursor.saturating_sub(1),
        "ArrowRight" => cursor = (cursor + 1).min(chars.len()),
        "Home" => cursor = 0,
        "End" => cursor = chars.len(),
        "" => {
            for character in text.chars().filter(|c| !c.is_control()) {
                if chars.len() >= max {
                    break;
                }
                chars.insert(cursor, character);
                cursor += 1;
            }
        }
        _ => return Ok(false),
    }
    let new: String = chars.into_iter().collect();
    if new != old {
        set_field(vm, &target, "text", Value::string(new))?;
    }
    set_field(vm, &target, "cursorPosition", Value::Integer(cursor as i32))?;
    Ok(true)
}

/// Geometry for the native key symbols, independent of the host's font fallback.
/// Focus box of a MiniKeyboard key, from the HD/FHD captures: 1 px black
/// border, rounded corners (about 5 px) and a vertical gradient (224 -> 151)
/// in 2 px bands. Built from spans so the corners show whatever is behind.
fn mini_focus(vm: &mut Vm, state: &mut State, left: f64, top: f64, width: f64) -> Result<()> {
    const PX: f64 = 1.5; // one output pixel in FHD units
    const ROWS: usize = 72;
    const BORDER: u32 = 0x0000_00FF;
    let shade = |row: usize| {
        let band = (row.saturating_sub(1) / 2).min(34) as f64;
        let value = (224.0 - 73.0 * band / 34.0).round() as u32;
        (value << 24) | (value << 16) | (value << 8) | 255
    };
    let right = left + width;
    let y = |row: usize| top + row as f64 * PX;
    // Top/bottom edge rows: (border start, fill start) in output pixels.
    for (edge, border, fill) in [
        (0usize, 5.0, None),
        (1, 2.0, Some(4.0)),
        (2, 1.0, Some(2.0)),
    ] {
        for row in [edge, ROWS - 1 - edge] {
            match fill {
                None => keyboard_rectangle(
                    vm,
                    state,
                    left + border * PX,
                    y(row),
                    width - 2.0 * border * PX,
                    PX,
                    BORDER,
                )?,
                Some(fill) => {
                    let span = (fill - border) * PX;
                    keyboard_rectangle(vm, state, left + border * PX, y(row), span, PX, BORDER)?;
                    keyboard_rectangle(vm, state, right - fill * PX, y(row), span, PX, BORDER)?;
                    keyboard_rectangle(
                        vm,
                        state,
                        left + fill * PX,
                        y(row),
                        width - 2.0 * fill * PX,
                        PX,
                        shade(row),
                    )?;
                }
            }
        }
    }
    let side = (ROWS - 6) as f64 * PX;
    keyboard_rectangle(vm, state, left, y(3), PX, side, BORDER)?;
    keyboard_rectangle(vm, state, right - PX, y(3), PX, side, BORDER)?;
    for row in 3..ROWS - 3 {
        keyboard_rectangle(
            vm,
            state,
            left + PX,
            y(row),
            width - 2.0 * PX,
            PX,
            shade(row),
        )?;
    }
    Ok(())
}

fn mini_icon(
    vm: &mut Vm,
    state: &mut State,
    key: &Key,
    x: f64,
    y: f64,
    color: u32,
) -> Result<bool> {
    let cx = x + key.width / 2.0;
    let cy = y + 40.5;
    let mut strokes = Vec::new();
    // Geometry measured on the HD/FHD mini-keyboard captures, in FHD units
    // (output pixels x 1.5) around the key center: 2 px strokes, a 3 px
    // chevron and a 3 px cross, like the device artwork.
    match key.text.as_str() {
        "clear" => strokes.extend([
            (-6.0, -16.5, 10.5, 3.0),  // handle
            (-7.5, -15.0, 3.0, 4.5),   // handle, left post
            (3.0, -15.0, 3.0, 4.5),    // handle, right post
            (-13.5, -10.5, 25.5, 3.0), // lid
            (-12.0, -4.5, 3.0, 19.5),  // body, left side
            (7.5, -4.5, 3.0, 19.5),    // body, right side
            (-12.0, 12.0, 22.5, 3.0),  // body, bottom
            (-4.5, -4.5, 3.0, 13.5),   // inner bar
            (1.5, -4.5, 3.0, 13.5),    // inner bar
        ]),
        "space" => strokes.extend([
            (-15.0, 0.0, 30.0, 3.0),
            (-15.0, -3.0, 3.0, 6.0),
            (12.0, -3.0, 3.0, 6.0),
        ]),
        "delete" => {
            strokes.extend([
                (-7.5, -10.5, 24.0, 3.0), // top
                (-7.5, 9.0, 24.0, 3.0),   // bottom
                (13.5, -10.5, 3.0, 22.5), // right side
            ]);
            // Left chevron: one 3 px segment per output row.
            for (row, column) in [-6, -7, -8, -9, -9, -9, -9, -8, -8, -7, -6]
                .into_iter()
                .enumerate()
            {
                let dy = (row as f64 - 5.0) * 1.5;
                strokes.push((column as f64 * 1.5, dy, 4.5, 1.5));
            }
            // Cross: two 3 px diagonals over seven output rows.
            for k in 0..7 {
                let dy = (k as f64 - 3.0) * 1.5;
                strokes.push(((k as f64 - 1.0) * 1.5, dy, 4.5, 1.5));
                strokes.push(((4.0 - k as f64) * 1.5, dy, 4.5, 1.5));
            }
        }
        _ => return Ok(false),
    }
    // The delete artwork sits one output pixel left of the key center.
    let shift = if key.text == "delete" { -1.5 } else { 0.0 };
    for (dx, dy, w, h) in strokes {
        keyboard_rectangle(vm, state, cx + dx + shift, cy + dy, w, h, color)?;
    }
    Ok(true)
}
