//! Animation, SequentialAnimation, ParallelAnimation and field interpolators.
//!
//! Device behavior (fidelity/scenegraph/animation):
//! - an interpolator applies when its `fraction` changes; fractions outside
//!   the key range clamp to the first/last keyValue; `reverse` uses 1 - f.
//! - ColorFieldInterpolator interpolates in HSV with whole-degree hue,
//!   linear alpha and truncated channels (red -> 0x00FF80 at 0.5 = 0xBFFF00).
//! - state events: children report "running" before their parent;
//!   Sequential stops after its last child, Parallel before its last child.

use std::rc::Rc;

use super::runtime::{ancestors, find_node, set_field};
use super::{with_node, Node};
use crate::error::RuntimeError;
use crate::value::{Object, ObjectRef, Value};
use crate::vm::Vm;

/// A started top-level animation and the clock time it started.
#[derive(Clone)]
pub struct Running {
    pub node: std::rc::Weak<std::cell::RefCell<Object>>,
    pub start_ms: f64,
}

/// Frame interval used to advance running animations.
pub const FRAME_MS: f64 = 1000.0 / 60.0;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Single,
    Sequential,
    Parallel,
}

fn animation_kind(node: &Node) -> Option<Kind> {
    let is = |t: &str| node.ancestry.iter().any(|a| &**a == t);
    if is("SequentialAnimation") {
        Some(Kind::Sequential)
    } else if is("ParallelAnimation") {
        Some(Kind::Parallel)
    } else if is("Animation") {
        Some(Kind::Single)
    } else {
        None
    }
}

fn is_interpolator(node: &Node) -> bool {
    node.ancestry
        .iter()
        .any(|t| t.ends_with("FieldInterpolator"))
}

fn children(object: &ObjectRef) -> Vec<ObjectRef> {
    with_node(object, |n| n.children.clone())
}

/// Length in seconds of one run, delay included.
fn total(object: &ObjectRef) -> f64 {
    let (kind, delay, duration) = with_node(object, |n| {
        (
            animation_kind(n),
            n.number("delay").max(0.0),
            n.number("duration"),
        )
    });
    let animations = || {
        children(object)
            .into_iter()
            .filter(|c| with_node(c, |n| animation_kind(n).is_some()))
    };
    delay
        + match kind {
            Some(Kind::Single) => duration.max(0.0),
            Some(Kind::Sequential) => animations().map(|c| total(&c)).sum(),
            Some(Kind::Parallel) => animations().map(|c| total(&c)).fold(0.0, f64::max),
            None => 0.0,
        }
}

/// `control` field side effect for animation nodes.
pub fn control(vm: &mut Vm, object: &ObjectRef, control: &str) -> Result<(), RuntimeError> {
    let started = |vm: &Vm| {
        vm.sg
            .animations
            .iter()
            .position(|r| r.node.upgrade().is_some_and(|n| Rc::ptr_eq(&n, object)))
    };
    match control {
        "start" => {
            if let Some(index) = started(vm) {
                vm.sg.animations.remove(index);
            }
            reset(vm, object)?;
            let start_ms = vm.now_ms();
            vm.sg.animations.push(Running {
                node: Rc::downgrade(object),
                start_ms,
            });
            advance(vm, object, 0.0)?;
            set_state(vm, object, "running")
        }
        "stop" => {
            if let Some(index) = started(vm) {
                vm.sg.animations.remove(index);
            }
            stop_tree(vm, object)
        }
        "finish" => {
            if let Some(index) = started(vm) {
                vm.sg.animations.remove(index);
            }
            let end = total(object);
            advance(vm, object, end)?;
            finish(vm, object)
        }
        _ => Ok(()),
    }
}

fn set_state(vm: &mut Vm, object: &ObjectRef, state: &str) -> Result<(), RuntimeError> {
    if with_node(object, |n| n.text("state")) != state {
        set_field(vm, object, "state", Value::string(state))?;
    }
    Ok(())
}

/// Child animations go back to "stopped" before a new run.
fn reset(vm: &mut Vm, object: &ObjectRef) -> Result<(), RuntimeError> {
    for child in children(object) {
        if with_node(&child, |n| animation_kind(n).is_some()) {
            set_state(vm, &child, "stopped")?;
            reset(vm, &child)?;
        }
    }
    Ok(())
}

fn stop_tree(vm: &mut Vm, object: &ObjectRef) -> Result<(), RuntimeError> {
    for child in children(object) {
        if with_node(&child, |n| animation_kind(n).is_some()) {
            stop_tree(vm, &child)?;
        }
    }
    set_state(vm, object, "stopped")
}

/// End of a run: final states in device order.
fn finish(vm: &mut Vm, object: &ObjectRef) -> Result<(), RuntimeError> {
    let kind = with_node(object, animation_kind);
    let animations: Vec<ObjectRef> = children(object)
        .into_iter()
        .filter(|c| with_node(c, |n| animation_kind(n).is_some()))
        .collect();
    if kind == Some(Kind::Parallel) {
        set_state(vm, object, "stopped")?;
        for child in &animations {
            finish(vm, child)?;
        }
        Ok(())
    } else {
        for child in &animations {
            finish(vm, child)?;
        }
        set_state(vm, object, "stopped")
    }
}

/// Applies the animation tree at `elapsed` seconds into the current run.
fn advance(vm: &mut Vm, object: &ObjectRef, elapsed: f64) -> Result<(), RuntimeError> {
    let (kind, delay, duration, ease) = with_node(object, |n| {
        (
            animation_kind(n),
            n.number("delay").max(0.0),
            n.number("duration").max(0.0),
            n.text("easeFunction"),
        )
    });
    let local = elapsed - delay;
    if local < 0.0 {
        return Ok(());
    }
    match kind {
        Some(Kind::Single) => {
            let linear = if duration > 0.0 {
                (local / duration).min(1.0)
            } else {
                1.0
            };
            let fraction = ease_value(&ease, linear);
            for child in children(object) {
                if with_node(&child, is_interpolator) {
                    set_field(vm, &child, "fraction", Value::Float(fraction as f32))?;
                }
            }
        }
        Some(Kind::Sequential) | Some(Kind::Parallel) => {
            let own_length = total(object) - delay;
            let mut offset = 0.0;
            for child in children(object) {
                if !with_node(&child, |n| animation_kind(n).is_some()) {
                    continue;
                }
                let length = total(&child);
                let start = if kind == Some(Kind::Sequential) {
                    offset
                } else {
                    0.0
                };
                if local >= start {
                    let state = with_node(&child, |n| n.text("state"));
                    if local < start + length || length == 0.0 {
                        advance(vm, &child, local - start)?;
                        if state != "running" && local < start + length {
                            set_state(vm, &child, "running")?;
                        }
                    } else if state == "running" {
                        // Window passed: land on the final values, then stop.
                        // A Parallel child ending with its parent stops after it
                        // (see `finish`).
                        advance(vm, &child, length)?;
                        if kind == Some(Kind::Sequential) || start + length < own_length {
                            finish(vm, &child)?;
                        }
                    }
                }
                offset += length;
            }
        }
        None => {}
    }
    Ok(())
}

/// Advances every running animation; called by the render-thread service.
pub fn tick(vm: &mut Vm) -> Result<(), RuntimeError> {
    let now = vm.now_ms();
    let running: Vec<Running> = vm.sg.animations.clone();
    for entry in running {
        let Some(object) = entry.node.upgrade() else {
            continue;
        };
        let length = total(&object);
        let repeat = with_node(&object, |n| n.flag("repeat"));
        let mut elapsed = (now - entry.start_ms) / 1000.0;
        if repeat && length > 0.0 && elapsed >= length {
            elapsed %= length;
        }
        if elapsed >= length && !repeat {
            vm.sg
                .animations
                .retain(|r| !r.node.upgrade().is_some_and(|n| Rc::ptr_eq(&n, &object)));
            advance(vm, &object, length)?;
            finish(vm, &object)?;
        } else {
            advance(vm, &object, elapsed)?;
        }
    }
    Ok(())
}

// ---- easing ---------------------------------------------------------------------------

/// Roku easeFunction names; unknown names behave like the default (outCubic).
pub fn ease_value(name: &str, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let in_out = |f: fn(f64) -> f64| {
        if t < 0.5 {
            f(t * 2.0) / 2.0
        } else {
            1.0 - f((1.0 - t) * 2.0) / 2.0
        }
    };
    let power = |p: i32| move |x: f64| x.powi(p);
    let expo = |x: f64| {
        if x <= 0.0 {
            0.0
        } else {
            2f64.powf(10.0 * (x - 1.0))
        }
    };
    match name.to_ascii_lowercase().as_str() {
        "linear" => t,
        "inquad" => power(2)(t),
        "outquad" => 1.0 - power(2)(1.0 - t),
        "inoutquad" => in_out(|x| x * x),
        "incubic" => power(3)(t),
        "inoutcubic" => in_out(|x| x.powi(3)),
        "inquartic" => power(4)(t),
        "outquartic" => 1.0 - power(4)(1.0 - t),
        "inoutquartic" => in_out(|x| x.powi(4)),
        "inquintic" => power(5)(t),
        "outquintic" => 1.0 - power(5)(1.0 - t),
        "inoutquintic" => in_out(|x| x.powi(5)),
        "inexpo" => expo(t),
        "outexpo" => 1.0 - expo(1.0 - t),
        "inoutexpo" => in_out(expo),
        _ => 1.0 - (1.0 - t).powi(3),
    }
}

// ---- interpolators --------------------------------------------------------------------

fn numbers(value: &Value) -> Vec<f64> {
    match value {
        Value::Object(object) => match &*object.borrow() {
            Object::Array(items) => items.iter().filter_map(Value::as_f64).collect(),
            _ => Vec::new(),
        },
        other => other.as_f64().into_iter().collect(),
    }
}

fn list(value: &Value) -> Vec<Value> {
    match value {
        Value::Object(object) => match &*object.borrow() {
            Object::Array(items) => items.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Segment index and local t for `fraction` over `keys` (clamped).
fn segment(keys: &[f64], fraction: f64) -> Option<(usize, f64)> {
    let last = keys.len().checked_sub(1)?;
    if fraction <= keys[0] {
        return Some((0, 0.0));
    }
    if fraction >= keys[last] || last == 0 {
        return Some((last.saturating_sub(1), if last == 0 { 0.0 } else { 1.0 }));
    }
    let index = keys.windows(2).position(|w| fraction < w[1])?;
    let span = keys[index + 1] - keys[index];
    Some((
        index,
        if span > 0.0 {
            (fraction - keys[index]) / span
        } else {
            1.0
        },
    ))
}

/// `fraction` field side effect for interpolators.
pub fn interpolate(vm: &mut Vm, interpolator: &ObjectRef) -> Result<(), RuntimeError> {
    let (subtype, keys, values, target, fraction, reverse) = with_node(interpolator, |n| {
        (
            n.subtype.to_string(),
            numbers(&n.raw("key")),
            list(&n.raw("keyValue")),
            n.text("fieldToInterp"),
            n.number("fraction"),
            n.flag("reverse"),
        )
    });
    let Some((node_id, field)) = target.split_once('.') else {
        return Ok(());
    };
    if keys.is_empty() || values.len() < keys.len() {
        return Ok(());
    }
    let fraction = if reverse { 1.0 - fraction } else { fraction };
    let Some((index, t)) = segment(&keys, fraction) else {
        return Ok(());
    };
    let a = &values[index];
    let b = values.get(index + 1).unwrap_or(a);
    let value = match subtype.as_str() {
        "Vector2DFieldInterpolator" => {
            let (pa, pb) = (numbers(a), numbers(b));
            let lerp = |i: usize| {
                let x = pa.get(i).copied().unwrap_or(0.0);
                x + (pb.get(i).copied().unwrap_or(0.0) - x) * t
            };
            Value::object(Object::Array(vec![
                Value::Float(lerp(0) as f32),
                Value::Float(lerp(1) as f32),
            ]))
        }
        "ColorFieldInterpolator" => Value::Integer(lerp_color(color_of(a), color_of(b), t) as i32),
        _ => {
            let x = a.as_f64().unwrap_or(0.0);
            Value::Float((x + (b.as_f64().unwrap_or(0.0) - x) * t) as f32)
        }
    };
    let Some(target) = resolve_target(interpolator, node_id) else {
        return Ok(());
    };
    set_field(vm, &target, field, value)
}

/// `fieldToInterp` node: searched from the outermost ancestor down.
fn resolve_target(interpolator: &ObjectRef, id: &str) -> Option<ObjectRef> {
    ancestors(interpolator)
        .iter()
        .find_map(|ancestor| find_node(ancestor, id))
}

fn color_of(value: &Value) -> u32 {
    match value {
        Value::String(text) => {
            let text = text.trim_start_matches("0x").trim_start_matches('#');
            u32::from_str_radix(text, 16).unwrap_or(0)
        }
        other => other.as_f64().map_or(0, |v| v as i64 as u32),
    }
}

/// RGB (0..=255) to whole-degree hue, saturation and value (0..=1).
fn to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta) + 120.0
    } else {
        60.0 * ((r - g) / delta) + 240.0
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    (hue.floor(), saturation, max / 255.0)
}

fn to_rgb(hue: f64, saturation: f64, value: f64) -> (f64, f64, f64) {
    let c = value * saturation;
    let h = hue.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = value - c;
    ((r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0)
}

fn lerp_color(a: u32, b: u32, t: f64) -> u32 {
    let channel = |c: u32, shift: u32| f64::from((c >> shift) & 0xFF);
    let (ha, sa, va) = to_hsv(channel(a, 24), channel(a, 16), channel(a, 8));
    let (hb, sb, vb) = to_hsv(channel(b, 24), channel(b, 16), channel(b, 8));
    let mix = |x: f64, y: f64| x + (y - x) * t;
    let (r, g, bl) = to_rgb(mix(ha, hb).floor(), mix(sa, sb), mix(va, vb));
    let alpha = mix(channel(a, 0), channel(b, 0));
    let byte = |v: f64| v.clamp(0.0, 255.0) as u32;
    byte(r) << 24 | byte(g) << 16 | byte(bl) << 8 | byte(alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_interpolates_in_hsv_like_the_device() {
        assert_eq!(lerp_color(0xFF00_0000, 0x00FF_80FF, 0.5), 0xBFFF_007F);
        assert_eq!(lerp_color(0xFF00_0000, 0x00FF_80FF, 1.0), 0x00FF_7FFF);
    }

    #[test]
    fn segments_clamp_and_split() {
        let keys = [0.0, 0.5, 1.0];
        assert_eq!(segment(&keys, -0.5), Some((0, 0.0)));
        assert_eq!(segment(&keys, 0.25), Some((0, 0.5)));
        assert_eq!(segment(&keys, 0.75), Some((1, 0.5)));
        assert_eq!(segment(&keys, 1.5), Some((1, 1.0)));
    }

    #[test]
    fn easing_endpoints() {
        for name in ["linear", "inQuad", "outCubic", "inOutExpo", "unknown"] {
            assert_eq!(ease_value(name, 0.0), 0.0, "{name}");
            assert!((ease_value(name, 1.0) - 1.0).abs() < 1e-9, "{name}");
        }
    }
}
