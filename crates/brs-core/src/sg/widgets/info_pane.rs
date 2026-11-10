//! InfoPane geometry recorded on the physical device (fidelity/widgets/info-pane).
//!
//! Roku OS reports an InfoPane as subtype `InfoLabel` with five children:
//! the 9-patch background Poster, an unused Poster, the primary Label, a
//! `BulletText` group and the secondary Label. Blocks are stacked from a 24 px
//! top inset with a 12 px gap and the panel keeps a 29 px bottom inset; the
//! requested height is a minimum, never a clip.
use super::*;

/// Design-resolution metrics (HD/720p units, multiplied by `design_scale`).
const SIDE_INSET: f64 = 19.0;
const TOP_INSET: f64 = 24.0;
const BOTTOM_INSET: f64 = 29.0;
const BLOCK_GAP: f64 = 12.0;
const HORIZONTAL_PADDING: f64 = 32.0;
const BULLET_INDENT: f64 = 29.0;
const BULLET_GLYPH_WIDTH: f64 = 14.0;
const LINE_SPACING: f64 = 12.0;
const BULLET_GLYPH: &str = "\u{2022}";

const PRIMARY: usize = 2;
const BULLETS: usize = 3;
const SECONDARY: usize = 4;

pub(super) fn initialize(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    if !has(node, "InfoPane") {
        return Ok(());
    }
    // subtype() reports InfoLabel and isSubtype("InfoLabel") is true on the
    // device; the InfoPane entry stays first so widget dispatch still finds it.
    with_node_mut(node, |n| {
        n.subtype = "InfoLabel".into();
        n.ancestry = Rc::new(vec![
            "InfoPane".into(),
            "InfoLabel".into(),
            "Group".into(),
            "Node".into(),
        ]);
    });
    let scale = design_scale(vm);
    set_field(vm, node, "width", Value::Float((780.0 * scale) as f32))?;
    set_field(vm, node, "height", Value::Float((300.0 * scale) as f32))?;
    let content_width = (780.0 - HORIZONTAL_PADDING) * scale;
    for (index, kind) in ["Poster", "Poster", "Label", "Group", "Label"]
        .iter()
        .enumerate()
    {
        let child = if *kind == "Label" {
            create_label(vm, content_width)?
        } else {
            create_node(vm, kind)?.expect("native pane child")
        };
        with_node_mut(&child, |n| n.parent = Some(Rc::downgrade(node)));
        if index == 0 {
            with_node_mut(&child, |n| {
                n.native_paint = Some(super::super::NativePaint::InfoPanel);
                n.field_mut("uri").unwrap().value =
                    Value::string("/RokuOS/Artwork/SceneGraph/GenevaTheme/Base/HD/infoPanel.9.png");
                n.field_mut("loadStatus").unwrap().value = Value::string("ready");
            });
        }
        if index == PRIMARY {
            set_translation(vm, &child, SIDE_INSET * scale, TOP_INSET * scale)?;
        }
        if index == SECONDARY {
            // The device positions the secondary label horizontally up front
            // and only moves it vertically once it has text.
            set_translation(vm, &child, SIDE_INSET * scale, 0.0)?;
            set_field(vm, &child, "visible", Value::Boolean(false))?;
        }
        if index == BULLETS {
            with_node_mut(&child, |n| {
                n.subtype = "BulletText".into();
                n.ancestry = Rc::new(vec!["BulletText".into(), "Group".into(), "Node".into()]);
            });
        }
        with_node_mut(node, |n| n.children.push(child));
    }
    track(vm, node).dirty = true;
    Ok(())
}

/// Internal labels report subtype `Label` and answer isSubtype("Label") with
/// true, unlike a Label created by the channel (whose own type is excluded).
/// They keep the Label default font (uri "", size 34), as on the device.
fn create_label(vm: &mut Vm, width: f64) -> Result<ObjectRef> {
    let scale = design_scale(vm);
    let label = create_node(vm, "Label")?.expect("Label");
    with_node_mut(&label, |n| {
        n.ancestry = Rc::new(vec![
            "InfoPaneLabel".into(),
            "Label".into(),
            "Group".into(),
            "Node".into(),
        ]);
    });
    set_field(vm, &label, "wrap", Value::Boolean(true))?;
    set_field(vm, &label, "width", Value::Float(width as f32))?;
    set_field(
        vm,
        &label,
        "lineSpacing",
        Value::Float((LINE_SPACING * scale) as f32),
    )?;
    Ok(label)
}

fn set_translation(vm: &mut Vm, node: &ObjectRef, x: f64, y: f64) -> Result<()> {
    set_field(vm, node, "translation", super::super::float_array(&[x, y]))
}

pub(super) fn changed(vm: &mut Vm, node: &ObjectRef, field: &str) {
    // infoText2BottomAlign alone does not trigger a relayout on the device;
    // it takes effect on the next change of one of these fields.
    if has(node, "InfoPane")
        && matches!(
            field.to_ascii_lowercase().as_str(),
            "width"
                | "height"
                | "infotext"
                | "infotext2"
                | "bullettext"
                | "textcolor"
                | "infotext2color"
        )
    {
        track(vm, node).dirty = true;
    }
}

/// Sets a label's text, width and color and returns its measured height.
fn set_label(
    vm: &mut Vm,
    label: &ObjectRef,
    text: String,
    width: f64,
    color: Value,
) -> Result<f64> {
    // A height left from the previous layout would clip the measurement.
    set_field(vm, label, "height", Value::Float(0.0))?;
    set_field(vm, label, "text", Value::string(text))?;
    set_field(vm, label, "width", Value::Float(width as f32))?;
    set_field(vm, label, "color", color)?;
    Ok(measure(vm, label))
}

fn measure(vm: &Vm, label: &ObjectRef) -> f64 {
    with_node(label, |n| {
        super::super::render::label_layout(vm, n).1.height()
    })
}

/// Rebuilds the BulletText children (text label, then glyph label, per item)
/// and returns the group height.
fn layout_bullets(
    vm: &mut Vm,
    group: &ObjectRef,
    items: &[String],
    width: f64,
    color: &Value,
) -> Result<f64> {
    let scale = design_scale(vm);
    let old = with_node_mut(group, |n| std::mem::take(&mut n.children));
    for child in old {
        with_node_mut(&child, |n| n.parent = None);
    }
    let text_width = width - BULLET_INDENT * scale;
    let mut y = 0.0;
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            y += BLOCK_GAP * scale;
        }
        let text = create_label(vm, text_width)?;
        let height = set_label(vm, &text, item.clone(), text_width, color.clone())?;
        set_translation(vm, &text, BULLET_INDENT * scale, y)?;
        let glyph = create_label(vm, BULLET_GLYPH_WIDTH * scale)?;
        set_label(
            vm,
            &glyph,
            BULLET_GLYPH.into(),
            BULLET_GLYPH_WIDTH * scale,
            color.clone(),
        )?;
        set_field(vm, &glyph, "wrap", Value::Boolean(false))?;
        set_translation(vm, &glyph, 0.0, y)?;
        for child in [text, glyph] {
            with_node_mut(&child, |n| n.parent = Some(Rc::downgrade(group)));
            with_node_mut(group, |n| n.children.push(child));
        }
        y += height;
    }
    Ok(y)
}

pub(super) fn render(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    let children = with_node(node, |n| n.children.clone());
    if children.len() < 5 {
        return Ok(());
    }
    let scale = design_scale(vm);
    let width = with_node(node, |n| n.number("width")).max(HORIZONTAL_PADDING * scale);
    let content_width = width - HORIZONTAL_PADDING * scale;
    let (text, footer, color, footer_color, bottom) = with_node(node, |n| {
        (
            n.text("infoText"),
            n.text("infoText2"),
            n.raw("textColor"),
            n.raw("infoText2Color"),
            n.flag("infoText2BottomAlign"),
        )
    });
    let bullets = strings(node, "bulletText");

    let primary = set_label(vm, &children[PRIMARY], text, content_width, color.clone())?;
    set_field(
        vm,
        &children[PRIMARY],
        "height",
        Value::Float(primary as f32),
    )?;
    let mut cursor = TOP_INSET * scale + primary;

    let bullet_height = layout_bullets(vm, &children[BULLETS], &bullets, content_width, &color)?;
    if !bullets.is_empty() {
        // An emptied BulletText keeps its previous translation on the device.
        cursor += BLOCK_GAP * scale;
        set_translation(vm, &children[BULLETS], SIDE_INSET * scale, cursor)?;
        cursor += bullet_height;
    }

    // The secondary label always tracks the pane width, even when empty.
    let secondary = set_label(
        vm,
        &children[SECONDARY],
        footer.clone(),
        content_width,
        footer_color,
    )?;
    let has_footer = !footer.is_empty();
    let footer_top = cursor + BLOCK_GAP * scale;
    if has_footer {
        cursor = footer_top + secondary;
    }

    let requested = with_node(node, |n| n.number("height"));
    let height = requested.max(cursor + BOTTOM_INSET * scale);
    set_field(vm, &children[0], "width", Value::Float(width as f32))?;
    set_field(vm, &children[0], "height", Value::Float(height as f32))?;

    set_field(
        vm,
        &children[SECONDARY],
        "visible",
        Value::Boolean(has_footer),
    )?;
    if has_footer {
        let y = if bottom {
            height - secondary - BOTTOM_INSET * scale
        } else {
            footer_top
        };
        set_translation(vm, &children[SECONDARY], SIDE_INSET * scale, y)?;
        set_field(
            vm,
            &children[SECONDARY],
            "height",
            Value::Float(secondary as f32),
        )?;
    }
    Ok(())
}
