//! SceneGraph: nodes, typed fields, observers, XML components, focus, keys,
//! Task threads, timers and the display list consumed by host renderers.
//!
//! Behaviors follow traces recorded on the reference device
//! (fidelity/scenegraph): see docs/brightscript-semantics.md.

pub mod animation;
pub mod bitmap;
pub mod component;
mod font_metrics;
pub mod render;
pub mod runtime;
pub mod schema;
pub mod text;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::value::{deep_copy, to_display_string, AssocArray, Object, ObjectRef, Value};

pub use component::ComponentDef;

/// Declared type of a node field. Controls conversion on set and the value
/// returned on get.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    String,
    Uri,
    Integer,
    LongInteger,
    Float,
    Double,
    Time,
    Boolean,
    Color,
    Vector2D,
    Rect2D,
    AssocArray,
    Array,
    Node,
    NodeArray,
    Font,
    /// Notifies on every set; carries no meaningful value (Timer.fire).
    Trigger,
}

impl FieldKind {
    pub fn parse(name: &str) -> FieldKind {
        match name.to_ascii_lowercase().as_str() {
            "uri" => FieldKind::Uri,
            "integer" | "int" => FieldKind::Integer,
            "longinteger" => FieldKind::LongInteger,
            "float" => FieldKind::Float,
            "double" => FieldKind::Double,
            "time" => FieldKind::Time,
            "boolean" | "bool" => FieldKind::Boolean,
            "color" => FieldKind::Color,
            "vector2d" => FieldKind::Vector2D,
            "rect2d" => FieldKind::Rect2D,
            "assocarray" | "roassociativearray" => FieldKind::AssocArray,
            "node" | "rosgnode" => FieldKind::Node,
            "nodearray" => FieldKind::NodeArray,
            "font" => FieldKind::Font,
            "array" | "roarray" | "intarray" | "floatarray" | "stringarray" | "boolarray"
            | "colorarray" | "vector2darray" | "rect2darray" | "timearray" => FieldKind::Array,
            _ => FieldKind::String,
        }
    }

    /// Value a field holds when declared without one.
    pub fn default_value(self) -> Value {
        match self {
            FieldKind::String | FieldKind::Uri => Value::string(""),
            FieldKind::Integer | FieldKind::Color => Value::Integer(0),
            FieldKind::LongInteger => Value::LongInteger(0),
            FieldKind::Float => Value::Float(0.0),
            FieldKind::Double | FieldKind::Time => Value::Double(0.0),
            FieldKind::Boolean => Value::Boolean(false),
            FieldKind::Vector2D => float_array(&[0.0, 0.0]),
            FieldKind::Rect2D => rect_value(0.0, 0.0, 0.0, 0.0),
            _ => Value::Invalid,
        }
    }

    /// Converts a value being stored. `None` means the type is incompatible.
    pub fn convert(self, value: &Value) -> Option<Value> {
        let value = value.unboxed();
        Some(match self {
            FieldKind::String | FieldKind::Uri | FieldKind::Font => match &value {
                Value::String(_) => value,
                v if v.is_numeric() => Value::string(to_display_string(v)),
                Value::Boolean(_) => Value::string(to_display_string(&value)),
                Value::Object(_) if self == FieldKind::Font => value,
                _ => return None,
            },
            FieldKind::Integer => match &value {
                Value::String(text) => Value::Integer(parse_int(text) as i32),
                v if v.is_numeric() => Value::Integer(v.as_f64()? as i32),
                _ => return None,
            },
            FieldKind::Color => match &value {
                Value::String(text) => Value::Integer(parse_color(text)?),
                v if v.is_numeric() => Value::Integer(v.as_i64()? as i32),
                _ => return None,
            },
            FieldKind::LongInteger => match &value {
                Value::String(text) => Value::LongInteger(parse_int(text)),
                v if v.is_numeric() => Value::LongInteger(v.as_f64()? as i64),
                _ => return None,
            },
            FieldKind::Float => match &value {
                Value::String(text) => Value::Float(text.trim().parse::<f32>().unwrap_or(0.0)),
                v if v.is_numeric() => Value::Float(v.as_f64()? as f32),
                _ => return None,
            },
            FieldKind::Double | FieldKind::Time => match &value {
                Value::String(text) => Value::Double(text.trim().parse::<f64>().unwrap_or(0.0)),
                v if v.is_numeric() => Value::Double(v.as_f64()?),
                _ => return None,
            },
            FieldKind::Boolean => match &value {
                Value::Boolean(_) => value,
                Value::String(text) => Value::Boolean(text.trim().eq_ignore_ascii_case("true")),
                _ => return None,
            },
            FieldKind::Vector2D => match &value {
                Value::String(text) => float_array(&parse_number_list(text)),
                Value::Object(_) => float_array(&numbers_of(&value)?),
                _ => return None,
            },
            FieldKind::Rect2D => match &value {
                Value::String(text) => {
                    let n = parse_number_list(text);
                    let at = |i: usize| n.get(i).copied().unwrap_or(0.0);
                    rect_value(at(0), at(1), at(2), at(3))
                }
                Value::Object(_) => deep_copy(&value),
                _ => return None,
            },
            FieldKind::AssocArray | FieldKind::Array | FieldKind::NodeArray => match &value {
                Value::Invalid => Value::Invalid,
                Value::String(text) if self == FieldKind::Array => {
                    crate::json::parse(&hex_to_decimal(text)).unwrap_or(Value::Invalid)
                }
                Value::Object(_) => deep_copy(&value),
                _ => return None,
            },
            FieldKind::Node => match &value {
                Value::Invalid => Value::Invalid,
                Value::Object(object) if matches!(&*object.borrow(), Object::Node(_)) => value,
                _ => return None,
            },
            FieldKind::Trigger => value,
        })
    }

    /// Reads hand intrinsic values back boxed (roInt, roString...) and
    /// containers as copies, as the device does.
    pub fn read(self, value: &Value) -> Value {
        match value {
            // A node that is currently borrowed is the node being updated
            // (e.g. focusedChild pointing at itself).
            Value::Object(object) => match object.try_borrow() {
                Ok(borrowed) if !matches!(&*borrowed, Object::Node(_)) => {
                    drop(borrowed);
                    deep_copy(value)
                }
                _ => value.clone(),
            },
            Value::Invalid => Value::Invalid,
            other => Value::boxed(other.clone()),
        }
    }
}

fn parse_int(text: &str) -> i64 {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).unwrap_or(0);
    }
    let digits: String = text
        .chars()
        .enumerate()
        .take_while(|(i, c)| c.is_ascii_digit() || (*i == 0 && (*c == '-' || *c == '+')))
        .map(|(_, c)| c)
        .collect();
    digits.parse().unwrap_or(0)
}

/// "0xRRGGBBAA", "#RRGGBB" or "#RRGGBBAA" -> signed 32-bit RGBA.
pub fn parse_color(text: &str) -> Option<i32> {
    let text = text.trim();
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .or_else(|| text.strip_prefix('#'))?;
    let value = match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok()? << 8 | 0xFF,
        8 => u32::from_str_radix(hex, 16).ok()?,
        _ => return None,
    };
    Some(value as i32)
}

pub fn parse_number_list(text: &str) -> Vec<f64> {
    text.trim_matches(|c| c == '[' || c == ']' || c == '{' || c == '}' || char::is_whitespace(c))
        .split(',')
        .filter_map(|part| part.trim().parse::<f64>().ok())
        .collect()
}

fn numbers_of(value: &Value) -> Option<Vec<f64>> {
    let Value::Object(object) = value else {
        return None;
    };
    match &*object.borrow() {
        Object::Array(items) | Object::List(items) => {
            Some(items.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect())
        }
        _ => None,
    }
}

pub fn float_array(values: &[f64]) -> Value {
    Value::object(Object::Array(
        values.iter().map(|v| Value::Float(*v as f32)).collect(),
    ))
}

pub fn rect_value(x: f64, y: f64, width: f64, height: f64) -> Value {
    let mut aa = AssocArray::new();
    aa.set("height", Value::Float(height as f32));
    aa.set("width", Value::Float(width as f32));
    aa.set("x", Value::Float(x as f32));
    aa.set("y", Value::Float(y as f32));
    Value::object(Object::AssocArray(aa))
}

/// Where a field change is delivered.
#[derive(Debug, Clone)]
pub enum Observer {
    /// `observeField(name, "function")`: called on the render thread in the
    /// scope (and with the `m`) of the component that registered it.
    Callback {
        scope: u32,
        function: String,
        m: Value,
    },
    /// `observeField(name, port)`: a roSGNodeEvent is posted to the port.
    Port { port: usize },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub kind: FieldKind,
    pub value: Value,
    pub always_notify: bool,
    /// `onChange` from the component's `<interface>`.
    pub on_change: Option<String>,
    pub observers: Vec<Observer>,
}

/// Script state of a node whose type is an XML component.
#[derive(Debug, Clone)]
pub struct ComponentInstance {
    /// Scope of the most derived component (functions, callFunc, onKeyEvent).
    pub scope: u32,
    /// The component's `m` (holds `top` and `global`).
    pub m: Value,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub subtype: Rc<str>,
    /// Type chain, most derived first, ending with "Node".
    pub ancestry: Rc<Vec<Rc<str>>>,
    pub fields: Vec<Field>,
    index: HashMap<String, usize>,
    pub children: Vec<ObjectRef>,
    pub parent: Option<Weak<RefCell<Object>>>,
    pub component: Option<ComponentInstance>,
    pub has_focus: bool,
    /// Interface functions callable through callFunc (lowercased).
    pub functions: Rc<Vec<String>>,
    /// Hidden internal child (Scene background) excluded from app-ui listings.
    pub internal: bool,
}

impl Node {
    pub fn new(subtype: Rc<str>, ancestry: Rc<Vec<Rc<str>>>, fields: Vec<Field>) -> Node {
        let index = fields
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.to_lowercase(), i))
            .collect();
        Node {
            subtype,
            ancestry,
            fields,
            index,
            children: Vec::new(),
            parent: None,
            component: None,
            has_focus: false,
            functions: Rc::new(Vec::new()),
            internal: false,
        }
    }

    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.index.get(&name.to_lowercase()).copied()
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.field_index(name).map(|i| &self.fields[i])
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut Field> {
        self.field_index(name).map(move |i| &mut self.fields[i])
    }

    /// Raw stored value (no boxing/copy), for engine-internal reads.
    pub fn raw(&self, name: &str) -> Value {
        self.field(name).map_or(Value::Invalid, |f| f.value.clone())
    }

    pub fn number(&self, name: &str) -> f64 {
        self.field(name)
            .and_then(|f| f.value.as_f64())
            .unwrap_or(0.0)
    }

    pub fn text(&self, name: &str) -> String {
        match self.field(name).map(|f| &f.value) {
            Some(Value::String(text)) => text.to_string(),
            _ => String::new(),
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        matches!(
            self.field(name).map(|f| &f.value),
            Some(Value::Boolean(true))
        )
    }

    pub fn vector(&self, name: &str) -> (f64, f64) {
        let numbers = numbers_of(&self.raw(name)).unwrap_or_default();
        (
            numbers.first().copied().unwrap_or(0.0),
            numbers.get(1).copied().unwrap_or(0.0),
        )
    }

    pub fn id(&self) -> String {
        self.text("id")
    }

    pub fn add_field(&mut self, field: Field) -> bool {
        let key = field.name.to_lowercase();
        if self.index.contains_key(&key) {
            return false;
        }
        self.index.insert(key, self.fields.len());
        self.fields.push(field);
        true
    }

    pub fn remove_field(&mut self, name: &str) -> bool {
        let Some(position) = self.index.remove(&name.to_lowercase()) else {
            return false;
        };
        self.fields.remove(position);
        for slot in self.index.values_mut() {
            if *slot > position {
                *slot -= 1;
            }
        }
        true
    }

    /// `type` names this node is a strict subtype of (ancestry without itself).
    pub fn is_subtype(&self, name: &str) -> bool {
        self.ancestry
            .iter()
            .skip(1)
            .any(|t| t.eq_ignore_ascii_case(name))
    }

    /// PRINT format (fidelity/scenegraph/node-api).
    pub fn print_string(&self) -> String {
        let mut text = format!("<Component: roSGNode:{}> =\n{{\n", self.subtype);
        for field in &self.fields {
            text.push_str(&format!(
                "    {}: {}\n",
                field.name,
                nested_node_value(&field.value)
            ));
        }
        text.push('}');
        text
    }
}

/// Field values inside a printed node: strings quoted, invalid and containers
/// shown as components.
fn nested_node_value(value: &Value) -> String {
    match value {
        Value::String(text) => format!("\"{text}\""),
        Value::Invalid => "<Component: roInvalid>".into(),
        Value::Object(object) => match &*object.borrow() {
            Object::Node(node) => format!("<Component: roSGNode:{}>", node.subtype),
            other => format!("<Component: {}>", other.class_name()),
        },
        other => to_display_string(other),
    }
}

#[derive(Debug, Clone)]
pub struct NodeEvent {
    pub field: String,
    pub data: Value,
    pub node: Value,
    pub node_id: String,
}

/// Handle to a screen; its state lives in `SceneGraph::screens`.
#[derive(Debug, Clone, Copy)]
pub struct ScreenHandle {
    pub id: usize,
}

#[derive(Debug, Default)]
pub struct ScreenState {
    pub port: Option<usize>,
    pub scene: Option<ObjectRef>,
    pub shown: bool,
    pub closed: bool,
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub node: Weak<RefCell<Object>>,
    pub due_ms: f64,
}

/// Per-channel SceneGraph state owned by the VM.
#[derive(Default)]
pub struct SceneGraph {
    pub components: HashMap<String, Rc<ComponentDef>>,
    pub global: Option<ObjectRef>,
    pub screens: Vec<ScreenState>,
    /// Nodes created by CreateScene whose children/init still have to run on
    /// the render thread.
    pub pending_scenes: Vec<ObjectRef>,
    pub focus: Option<Weak<RefCell<Object>>>,
    pub timers: Vec<Timer>,
    /// Started top-level animations (see `animation.rs`).
    pub animations: Vec<animation::Running>,
    /// Remote Poster images: load state by URI and hosts' pending work
    /// (see `bitmap.rs`).
    pub images: bitmap::Images,
    /// Font files read from the package, by `pkg:/` URI (`None` = unreadable).
    pub fonts: HashMap<String, Option<Rc<Vec<u8>>>>,
    /// Remote key events waiting for the render thread: (key, pressed).
    pub keys: std::collections::VecDeque<(String, bool)>,
    /// Design resolution from the manifest (`ui_resolutions`).
    pub design_width: f64,
    pub design_height: f64,
    /// Incremented whenever something visible may have changed.
    pub revision: u64,
}

impl SceneGraph {
    pub fn active_screen(&self) -> Option<&ScreenState> {
        self.screens.iter().rev().find(|s| s.shown && !s.closed)
    }
}

/// Node behind a value, if it is one.
pub fn node_ref(value: &Value) -> Option<ObjectRef> {
    match value {
        Value::Object(object) if matches!(&*object.borrow(), Object::Node(_)) => {
            Some(object.clone())
        }
        _ => None,
    }
}

pub fn with_node<T>(object: &ObjectRef, f: impl FnOnce(&Node) -> T) -> T {
    match &*object.borrow() {
        Object::Node(node) => f(node),
        _ => unreachable!("node expected"),
    }
}

pub fn with_node_mut<T>(object: &ObjectRef, f: impl FnOnce(&mut Node) -> T) -> T {
    match &mut *object.borrow_mut() {
        Object::Node(node) => f(node),
        _ => unreachable!("node expected"),
    }
}

/// XML array attributes may hold `0x` color literals (e.g. an interpolator's
/// keyValue); they become signed 32-bit integers like the color fields.
fn hex_to_decimal(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("0x").or_else(|| rest.find("0X")) {
        out.push_str(&rest[..start]);
        let digits: String = rest[start + 2..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        match u32::from_str_radix(&digits, 16) {
            Ok(value) if !digits.is_empty() => out.push_str(&(value as i32).to_string()),
            _ => out.push_str(&rest[start..start + 2 + digits.len()]),
        }
        rest = &rest[start + 2 + digits.len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod hex_tests {
    #[test]
    fn converts_color_literals() {
        assert_eq!(
            super::hex_to_decimal("[0xFF000000, 0x00FF80FF]"),
            "[-16777216, 16744703]"
        );
    }
}
