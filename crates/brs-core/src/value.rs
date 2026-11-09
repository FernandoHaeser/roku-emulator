//! Runtime values and their textual representations. Formatting rules are
//! taken from traces recorded on a real device (see fidelity/language).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A user-defined function (index into the program's function table) or a
/// built-in global function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRef {
    User(u32),
    Builtin(u16),
}

pub type ObjectRef = Rc<RefCell<Object>>;

#[derive(Debug, Clone)]
pub enum Value {
    /// A variable that was never assigned (`<uninitialized>`), also used for
    /// parameters omitted by the caller.
    Uninitialized,
    Invalid,
    Boolean(bool),
    Integer(i32),
    LongInteger(i64),
    Float(f32),
    Double(f64),
    String(Rc<str>),
    Function(FunctionRef),
    Object(ObjectRef),
}

impl Value {
    pub fn string(text: impl Into<Rc<str>>) -> Value {
        Value::String(text.into())
    }

    pub fn object(object: Object) -> Value {
        Value::Object(Rc::new(RefCell::new(object)))
    }

    /// Name returned by `type()` and quoted in error messages.
    pub fn type_name(&self) -> String {
        match self {
            Value::Uninitialized => "<uninitialized>".into(),
            Value::Invalid => "Invalid".into(),
            Value::Boolean(_) => "Boolean".into(),
            Value::Integer(_) => "Integer".into(),
            Value::LongInteger(_) => "LongInteger".into(),
            Value::Float(_) => "Float".into(),
            Value::Double(_) => "Double".into(),
            Value::String(_) => "String".into(),
            Value::Function(_) => "Function".into(),
            Value::Object(object) => object.borrow().class_name().into(),
        }
    }

    pub fn is_numeric(&self) -> bool {
        match self {
            Value::Integer(_) | Value::LongInteger(_) | Value::Float(_) | Value::Double(_) => true,
            Value::Object(object) => object
                .try_borrow()
                .is_ok_and(|o| matches!(&*o, Object::Boxed(inner) if inner.is_numeric())),
            _ => false,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Integer(v) => Some(f64::from(*v)),
            Value::LongInteger(v) => Some(*v as f64),
            Value::Float(v) => Some(f64::from(*v)),
            Value::Double(v) => Some(*v),
            Value::Object(object) => match object.try_borrow().as_deref() {
                Ok(Object::Boxed(inner)) => inner.as_f64(),
                _ => None,
            },
            _ => None,
        }
    }

    /// Integer view used for indexes and integer arguments (truncates floats).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(v) => Some(i64::from(*v)),
            Value::LongInteger(v) => Some(*v),
            Value::Float(v) => Some(*v as i64),
            Value::Double(v) => Some(*v as i64),
            Value::Object(object) => match object.try_borrow().as_deref() {
                Ok(Object::Boxed(inner)) => inner.as_i64(),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Wraps an intrinsic value as roInt/roString/... (what node field reads
    /// and `Box()` return on the device).
    pub fn boxed(value: Value) -> Value {
        match value {
            Value::Object(_) | Value::Uninitialized => value,
            other => Value::object(Object::Boxed(other)),
        }
    }

    pub fn is_boxed(&self) -> bool {
        matches!(self, Value::Object(object)
            if object.try_borrow().is_ok_and(|o| matches!(&*o, Object::Boxed(_))))
    }

    /// The intrinsic value behind a boxed object; other values are cloned.
    pub fn unboxed(&self) -> Value {
        if let Value::Object(object) = self {
            if let Ok(borrowed) = object.try_borrow() {
                if let Object::Boxed(inner) = &*borrowed {
                    return inner.clone();
                }
            }
        }
        self.clone()
    }

    pub fn is_invalid(&self) -> bool {
        match self {
            Value::Invalid => true,
            Value::Object(object) => object
                .try_borrow()
                .is_ok_and(|o| matches!(&*o, Object::Boxed(Value::Invalid))),
            _ => false,
        }
    }
}

// ---- objects ---------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct AssocArray {
    entries: Vec<(String, Value)>,
    /// Lowercased key -> position in `entries` (lookups are case-insensitive).
    index: HashMap<String, usize>,
}

impl AssocArray {
    pub fn new() -> AssocArray {
        AssocArray::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.index
            .get(&key.to_lowercase())
            .map(|&i| &self.entries[i].1)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(&key.to_lowercase())
    }

    /// Inserts or replaces. An existing entry keeps its original key spelling.
    pub fn set(&mut self, key: &str, value: Value) {
        let lower = key.to_lowercase();
        match self.index.get(&lower) {
            Some(&i) => self.entries[i].1 = value,
            None => {
                self.index.insert(lower, self.entries.len());
                self.entries.push((key.to_string(), value));
            }
        }
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let Some(position) = self.index.remove(&key.to_lowercase()) else {
            return false;
        };
        self.entries.remove(position);
        for slot in self.index.values_mut() {
            if *slot > position {
                *slot -= 1;
            }
        }
        true
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn keys_in_insertion_order(&self) -> Vec<String> {
        self.entries.iter().map(|(key, _)| key.clone()).collect()
    }

    /// Entries in the order the device uses for `Keys()` and printing:
    /// sorted case-insensitively.
    pub fn sorted_entries(&self) -> Vec<(String, Value)> {
        let mut entries = self.entries.clone();
        entries.sort_by_key(|(key, _)| key.to_lowercase());
        entries
    }
}

#[derive(Debug, Clone)]
pub enum Object {
    Array(Vec<Value>),
    AssocArray(AssocArray),
    /// roList: same storage as roArray, different interfaces and printing.
    List(Vec<Value>),
    ByteArray(Vec<u8>),
    /// roInt, roFloat, roString... wrapping an intrinsic value.
    Boxed(Value),
    Timespan {
        mark_ms: f64,
    },
    /// Handle into the VM's port table (messages live in the VM).
    MessagePort(usize),
    DateTime {
        /// Milliseconds since the Unix epoch, UTC.
        epoch_ms: i64,
    },
    AppInfo,
    DeviceInfo,
    Registry,
    RegistrySection(String),
    XmlElement(Rc<crate::xml::XmlElement>),
    XmlList(Vec<Value>),
    Node(crate::sg::Node),
    NodeEvent(crate::sg::NodeEvent),
    Screen(crate::sg::ScreenHandle),
    ScreenEvent {
        closed: bool,
    },
    /// Hidden state of a FOR EACH loop.
    Iterator {
        items: Vec<Value>,
        position: usize,
    },
}

impl Object {
    pub fn class_name(&self) -> &'static str {
        match self {
            Object::Array(_) => "roArray",
            Object::AssocArray(_) => "roAssociativeArray",
            Object::List(_) => "roList",
            Object::ByteArray(_) => "roByteArray",
            Object::Boxed(inner) => match inner {
                Value::Integer(_) => "roInt",
                Value::LongInteger(_) => "roLongInteger",
                Value::Float(_) => "roFloat",
                Value::Double(_) => "roDouble",
                Value::String(_) => "roString",
                Value::Boolean(_) => "roBoolean",
                Value::Function(_) => "roFunction",
                _ => "roInvalid",
            },
            Object::Timespan { .. } => "roTimespan",
            Object::MessagePort(_) => "roMessagePort",
            Object::DateTime { .. } => "roDateTime",
            Object::AppInfo => "roAppInfo",
            Object::DeviceInfo => "roDeviceInfo",
            Object::Registry => "roRegistry",
            Object::RegistrySection(_) => "roRegistrySection",
            Object::XmlElement(_) => "roXMLElement",
            Object::XmlList(_) => "roXMLList",
            Object::Node(_) => "roSGNode",
            Object::NodeEvent(_) => "roSGNodeEvent",
            Object::Screen(_) => "roSGScreen",
            Object::ScreenEvent { .. } => "roSGScreenEvent",
            Object::Iterator { .. } => "roEnum",
        }
    }
}

/// Copy made when an AA or array crosses a node field boundary (the device
/// copies containers on field get/set). Nodes are shared, not copied.
pub fn deep_copy(value: &Value) -> Value {
    let Value::Object(object) = value else {
        return value.clone();
    };
    let copy = match &*object.borrow() {
        Object::Array(items) => Object::Array(items.iter().map(deep_copy).collect()),
        Object::List(items) => Object::List(items.iter().map(deep_copy).collect()),
        Object::AssocArray(aa) => {
            let mut copy = AssocArray::new();
            for key in aa.keys_in_insertion_order() {
                if let Some(item) = aa.get(&key) {
                    copy.set(&key, deep_copy(item));
                }
            }
            Object::AssocArray(copy)
        }
        _ => return value.clone(),
    };
    Value::object(copy)
}

// ---- number formatting -----------------------------------------------------

/// C `printf("%.{precision}g")`, which is what the device uses for floats.
pub fn format_general(value: f64, precision: usize) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    if value.is_infinite() {
        return if value > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    let precision = precision.max(1);
    let scientific = format!("{:.*e}", precision - 1, value);
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if exponent < -4 || exponent >= precision as i32 {
        let mantissa = strip_fraction_zeros(mantissa);
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", exponent.abs())
    } else {
        let decimals = (precision as i32 - 1 - exponent).max(0) as usize;
        strip_fraction_zeros(&format!("{value:.decimals$}")).to_string()
    }
}

fn strip_fraction_zeros(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}

pub const FLOAT_PRECISION: usize = 6;
pub const DOUBLE_PRECISION: usize = 14;

pub fn format_float(value: f32) -> String {
    format_general(f64::from(value), FLOAT_PRECISION)
}

pub fn format_double(value: f64) -> String {
    format_general(value, DOUBLE_PRECISION)
}

/// Representation used by `.ToStr()` and string concatenation contexts: no
/// sign padding, no trailing space.
pub fn to_display_string(value: &Value) -> String {
    match value {
        Value::Uninitialized => "<uninitialized>".into(),
        Value::Invalid => "invalid".into(),
        Value::Boolean(b) => if *b { "true" } else { "false" }.into(),
        Value::Integer(v) => v.to_string(),
        Value::LongInteger(v) => v.to_string(),
        Value::Float(v) => format_float(*v),
        Value::Double(v) => format_double(*v),
        Value::String(s) => s.to_string(),
        Value::Function(_) => "<Function>".into(),
        Value::Object(object) => match &*object.borrow() {
            Object::Boxed(inner) => to_display_string(inner),
            other => format!("<Component: {}>", other.class_name()),
        },
    }
}

/// Representation produced by the PRINT statement. Device rules:
/// numbers reserve a leading sign column (space when non-negative); Float and
/// Double are followed by a space; containers are expanded one level.
pub fn to_print_string(value: &Value) -> String {
    match value {
        Value::Integer(v) => signed(v.to_string(), *v >= 0),
        Value::LongInteger(v) => signed(v.to_string(), *v >= 0),
        Value::Float(v) => format!("{} ", signed(format_float(*v), !v.is_sign_negative())),
        Value::Double(v) => format!("{} ", signed(format_double(*v), !v.is_sign_negative())),
        Value::Object(object) => match &*object.borrow() {
            Object::Array(items) => {
                let mut text = String::from("<Component: roArray> =\n[\n");
                for item in items {
                    text.push_str(&format!("    {}\n", nested_print_string(item)));
                }
                text.push(']');
                text
            }
            Object::AssocArray(aa) => {
                let mut text = String::from("<Component: roAssociativeArray> =\n{\n");
                for (key, item) in aa.sorted_entries() {
                    text.push_str(&format!("    {key}: {}\n", nested_print_string(&item)));
                }
                text.push('}');
                text
            }
            Object::List(items) => {
                let mut text = String::from("<Component: roList> =\n(\n");
                for item in items {
                    text.push_str(&format!("    {}\n", nested_print_string(item)));
                }
                text.push(')');
                text
            }
            Object::Boxed(inner) => to_print_string(inner),
            Object::Node(node) => node.print_string(),
            other => format!("<Component: {}>", other.class_name()),
        },
        other => to_display_string(other),
    }
}

/// Values inside a printed container: strings quoted, numbers unpadded,
/// nested containers not expanded.
fn nested_print_string(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{s}\""),
        other => to_display_string(other),
    }
}

fn signed(text: String, non_negative: bool) -> String {
    if non_negative {
        format!(" {text}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_format_matches_device_samples() {
        // Samples from fidelity/language/numbers golden.
        assert_eq!(format_float(2f32.sqrt()), "1.41421");
        assert_eq!(format_float(1.0 / 3.0), "0.333333");
        assert_eq!(format_float(0.1 + 0.2), "0.3");
        assert_eq!(format_float(1e7), "1e+07");
        assert_eq!(format_float(123_456.7), "123457");
        assert_eq!(format_float(1_234_567.8), "1.23457e+06");
        assert_eq!(format_float(0.00001), "1e-05");
        assert_eq!(format_float(0.0001), "0.0001");
        assert_eq!(format_double(1.0 / 3.0), "0.33333333333333");
        // Device sample: the literal 3.14159265358979 (not PI itself).
        #[allow(clippy::approx_constant)]
        let sample = 3.14159265358979;
        assert_eq!(format_double(sample), "3.1415926535898");
        assert_eq!(format_double(2_147_483_648.0), "2147483648");
    }

    #[test]
    fn print_padding() {
        assert_eq!(to_print_string(&Value::Integer(3)), " 3");
        assert_eq!(to_print_string(&Value::Integer(-3)), "-3");
        assert_eq!(to_print_string(&Value::Float(-0.5)), "-0.5 ");
        assert_eq!(to_print_string(&Value::Float(5.0)), " 5 ");
    }

    #[test]
    fn assoc_array_is_case_insensitive_and_sorts_keys() {
        let mut aa = AssocArray::new();
        aa.set("b", Value::Integer(2));
        aa.set("D", Value::Integer(4));
        aa.set("a", Value::Integer(1));
        assert!(aa.contains("d"));
        let keys: Vec<_> = aa.sorted_entries().into_iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["a", "b", "D"]);
        assert!(aa.remove("B"));
        assert_eq!(aa.len(), 2);
        assert!(aa.get("a").is_some());
    }
}
