//! Global functions (Len, Mid, CreateObject...) and member functions of
//! intrinsic values and components. Global string functions are 1-based;
//! `ifString` methods are 0-based (verified on device).

use crate::error::RuntimeError;
use crate::value::{format_double, format_float, to_display_string, AssocArray, Object, Value};
use crate::vm::Vm;

type BuiltinFn = fn(&mut Vm, Vec<Value>) -> Result<Value, RuntimeError>;

const GLOBALS: &[(&str, BuiltinFn)] = &[
    ("readasciifile", |vm, a| {
        arity(&a, 1, 1)?;
        Ok(Value::string(
            String::from_utf8_lossy(&vm.env.read_file(string_arg(&a, 0)?).unwrap_or_default())
                .as_ref(),
        ))
    }),
    ("writeasciifile", |vm, a| {
        arity(&a, 2, 2)?;
        let success = vm
            .env
            .write_file(string_arg(&a, 0)?, string_arg(&a, 1)?.as_bytes().to_vec());
        if !success {
            vm.write(&format!(
                "BRIGHTSCRIPT: ERROR: WriteAsciiFile: file open for write failed: {}\n",
                vm.current_location()
            ));
        }
        Ok(Value::Boolean(success))
    }),
    ("deletefile", |vm, a| {
        arity(&a, 1, 1)?;
        let path = crate::environment::Environment::normalize_path(string_arg(&a, 0)?);
        Ok(Value::Boolean(path.is_some_and(|p| {
            vm.env.writable_files.remove(&p).is_some()
        })))
    }),
    ("copyfile", |vm, a| {
        arity(&a, 2, 2)?;
        let source = vm.env.read_file(string_arg(&a, 0)?);
        Ok(Value::Boolean(source.is_some_and(|bytes| {
            vm.env
                .write_file(string_arg(&a, 1).unwrap_or_default(), bytes)
        })))
    }),
    ("listdir", |vm, a| {
        arity(&a, 1, 1)?;
        Ok(Value::object(Object::List(
            vm.env
                .directory_listing(string_arg(&a, 0)?)
                .into_iter()
                .map(Value::string)
                .collect(),
        )))
    }),
    ("abs", abs),
    ("box", |_, a| {
        arity(&a, 1, 1)?;
        Ok(Value::boxed(a[0].unboxed()))
    }),
    ("asc", asc),
    ("atn", |_, a| float_fn(a, f64::atan)),
    ("cdbl", cdbl),
    ("chr", chr),
    ("cint", cint),
    ("cos", |_, a| float_fn(a, f64::cos)),
    ("createobject", create_object),
    ("csng", csng),
    ("exp", |_, a| float_fn(a, f64::exp)),
    ("fix", fix),
    ("formatjson", format_json),
    ("getglobalaa", |vm, a| {
        arity(&a, 0, 0)?;
        Ok(vm.global_m())
    }),
    ("getinterface", |_, a| {
        arity(&a, 2, 2)?;
        Ok(get_interface(&Value::boxed(a[0].clone()), &a[1].unboxed()))
    }),
    ("instr", instr),
    ("int", int),
    ("lcase", |_, a| {
        arity(&a, 1, 1)?;
        Ok(Value::string(string_arg(&a, 0)?.to_lowercase()))
    }),
    ("left", left),
    ("len", |_, a| {
        arity(&a, 1, 1)?;
        Ok(Value::Integer(string_arg(&a, 0)?.chars().count() as i32))
    }),
    ("log", |_, a| float_fn(a, f64::ln)),
    ("mid", mid),
    ("parsejson", parse_json),
    ("right", right),
    ("rnd", rnd),
    ("sgn", sgn),
    ("sin", |_, a| float_fn(a, f64::sin)),
    ("sleep", |vm, a| {
        arity(&a, 1, 1)?;
        vm.sleep(number_arg(&a, 0)?);
        Ok(Value::Invalid)
    }),
    ("sqr", |_, a| float_fn(a, f64::sqrt)),
    ("str", str_fn),
    ("stri", stri),
    ("string", string_fn),
    ("substitute", substitute),
    ("tan", |_, a| float_fn(a, f64::tan)),
    ("tr", |_, a| {
        arity(&a, 1, 1)?;
        Ok(Value::string(string_arg(&a, 0)?))
    }),
    ("type", |_, a| {
        arity(&a, 1, 2)?;
        Ok(Value::string(a[0].type_name()))
    }),
    ("ucase", |_, a| {
        arity(&a, 1, 1)?;
        Ok(Value::string(string_arg(&a, 0)?.to_uppercase()))
    }),
    ("uptime", |vm, a| {
        arity(&a, 0, 1)?;
        Ok(Value::Float((vm.now_ms() / 1000.0) as f32))
    }),
    ("val", val),
    ("wait", |vm, a| {
        arity(&a, 2, 2)?;
        let timeout = number_arg(&a, 0)?;
        match &a[1] {
            Value::Object(object) => match &*object.borrow() {
                Object::MessagePort(port) => Ok(vm.wait_for_message(*port, timeout)),
                _ => Err(arg_mismatch(&a[1], "Object")),
            },
            other => Err(arg_mismatch(other, "Object")),
        }
    }),
];

/// Builtins that see boxed arguments as-is (everything else gets intrinsic
/// values, so roInt/roString from node fields behave like Integer/String).
const KEEPS_BOXED: &[&str] = &["type", "box", "getinterface", "formatjson"];

pub fn lookup(lowercase_name: &str) -> Option<u16> {
    GLOBALS
        .iter()
        .position(|(name, _)| *name == lowercase_name)
        .map(|i| i as u16)
}

pub fn name(index: u16) -> &'static str {
    GLOBALS.get(index as usize).map_or("", |(name, _)| name)
}

pub fn call(vm: &mut Vm, index: u16, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let (name, function) = GLOBALS
        .get(index as usize)
        .ok_or_else(RuntimeError::not_a_function)?;
    let args = if KEEPS_BOXED.contains(name) {
        args
    } else {
        args.iter().map(Value::unboxed).collect()
    };
    function(vm, args)
}

// ---- argument helpers ----------------------------------------------------------

fn arity(args: &[Value], min: usize, max: usize) -> Result<(), RuntimeError> {
    if args.len() < min || args.len() > max {
        return Err(RuntimeError::wrong_argument_count());
    }
    Ok(())
}

fn arg_mismatch(value: &Value, expected: &str) -> RuntimeError {
    RuntimeError::type_mismatch(format!(
        "Type Mismatch. Unable to cast \"{}\" to \"{expected}\".",
        value.type_name()
    ))
}

fn string_arg(args: &[Value], index: usize) -> Result<&str, RuntimeError> {
    args[index]
        .as_str()
        .ok_or_else(|| arg_mismatch(&args[index], "String"))
}

fn number_arg(args: &[Value], index: usize) -> Result<f64, RuntimeError> {
    args[index]
        .as_f64()
        .ok_or_else(|| arg_mismatch(&args[index], "Float"))
}

fn int_arg(args: &[Value], index: usize) -> Result<i64, RuntimeError> {
    args[index]
        .as_i64()
        .ok_or_else(|| arg_mismatch(&args[index], "Integer"))
}

fn char_slice(text: &str, start: usize, count: usize) -> String {
    text.chars().skip(start).take(count).collect()
}

// ---- global functions ----------------------------------------------------------

fn float_fn(args: Vec<Value>, f: fn(f64) -> f64) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    Ok(Value::Float(f(number_arg(&args, 0)?) as f32))
}

fn abs(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    Ok(match &args[0] {
        Value::Integer(v) => Value::Integer(v.wrapping_abs()),
        Value::LongInteger(v) => Value::LongInteger(v.wrapping_abs()),
        Value::Float(v) => Value::Float(v.abs()),
        Value::Double(v) => Value::Double(v.abs()),
        other => return Err(arg_mismatch(other, "Float")),
    })
}

fn asc(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    Ok(Value::Integer(
        string_arg(&args, 0)?.chars().next().map_or(0, |c| c as i32),
    ))
}

fn chr(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    let code = int_arg(&args, 0)?;
    Ok(Value::string(
        char::from_u32(code as u32)
            .map(String::from)
            .unwrap_or_default(),
    ))
}

fn cint(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    // Rounds half away from zero: cint(2.5) = 3.
    Ok(Value::Integer(number_arg(&args, 0)?.round() as i32))
}

fn cdbl(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    Ok(Value::Double(number_arg(&args, 0)?))
}

fn csng(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    Ok(Value::Float(number_arg(&args, 0)? as f32))
}

fn fix(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    Ok(Value::Integer(number_arg(&args, 0)?.trunc() as i32))
}

fn int(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    Ok(Value::Integer(number_arg(&args, 0)?.floor() as i32))
}

fn sgn(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    let value = number_arg(&args, 0)?;
    Ok(Value::Integer(if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }))
}

fn rnd(vm: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    let range = int_arg(&args, 0)?;
    let sample = vm.next_random();
    Ok(if range <= 0 {
        Value::Float(sample as f32)
    } else {
        Value::Integer((sample * range as f64) as i32 + 1)
    })
}

fn instr(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 3, 3)?;
    let start = int_arg(&args, 0)?.max(1) as usize;
    let text: Vec<char> = string_arg(&args, 1)?.chars().collect();
    let needle: Vec<char> = string_arg(&args, 2)?.chars().collect();
    Ok(Value::Integer(
        find_chars(&text, &needle, start - 1).map_or(0, |i| i as i32 + 1),
    ))
}

fn find_chars(text: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return (from <= text.len()).then_some(from);
    }
    (from..=text.len().saturating_sub(needle.len()))
        .find(|&i| text.get(i..i + needle.len()) == Some(needle))
}

fn left(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 2, 2)?;
    let count = int_arg(&args, 1)?.max(0) as usize;
    Ok(Value::string(char_slice(string_arg(&args, 0)?, 0, count)))
}

fn right(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 2, 2)?;
    let text = string_arg(&args, 0)?;
    let count = int_arg(&args, 1)?.max(0) as usize;
    let length = text.chars().count();
    Ok(Value::string(char_slice(
        text,
        length.saturating_sub(count),
        count,
    )))
}

fn mid(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 2, 3)?;
    let text = string_arg(&args, 0)?;
    let start = (int_arg(&args, 1)?.max(1) - 1) as usize;
    let count = if args.len() == 3 {
        int_arg(&args, 2)?.max(0) as usize
    } else {
        usize::MAX
    };
    Ok(Value::string(char_slice(text, start, count)))
}

/// Str(): numbers with the sign column, no trailing space (" 5", "-5", " 2.5").
fn str_fn(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 1)?;
    let (text, non_negative) = match &args[0] {
        Value::Integer(v) => (v.to_string(), *v >= 0),
        Value::LongInteger(v) => (v.to_string(), *v >= 0),
        Value::Float(v) => (format_float(*v), !v.is_sign_negative()),
        Value::Double(v) => (format_double(*v), !v.is_sign_negative()),
        other => return Err(arg_mismatch(other, "Float")),
    };
    Ok(Value::string(if non_negative {
        format!(" {text}")
    } else {
        text
    }))
}

fn stri(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 2)?;
    let value = int_arg(&args, 0)?;
    let radix = if args.len() == 2 {
        int_arg(&args, 1)?
    } else {
        10
    };
    if !(2..=36).contains(&radix) {
        return Ok(Value::string(""));
    }
    // Device: other radixes print the 32-bit two's complement, without the
    // sign column (StrI(&hFF0000FF, 16) = "ff0000ff", scenegraph/animation).
    if radix != 10 && i32::try_from(value).is_ok() {
        return Ok(Value::string(to_radix(
            i64::from(value as i32 as u32),
            radix as u32,
        )));
    }
    // Device: StrI(42) = " 42" (sign column, like Str).
    let digits = to_radix(value, radix as u32);
    Ok(Value::string(if value >= 0 {
        format!(" {digits}")
    } else {
        digits
    }))
}

fn to_radix(value: i64, radix: u32) -> String {
    if radix == 10 {
        return value.to_string();
    }
    let negative = value < 0;
    let mut magnitude = value.unsigned_abs();
    let mut digits = Vec::new();
    loop {
        digits.push(
            std::char::from_digit((magnitude % u64::from(radix)) as u32, radix).unwrap_or('0'),
        );
        magnitude /= u64::from(radix);
        if magnitude == 0 {
            break;
        }
    }
    if negative {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

fn string_fn(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 2, 2)?;
    let count = int_arg(&args, 0)?.max(0) as usize;
    let unit = match &args[1] {
        Value::String(s) => s.to_string(),
        other => char::from_u32(other.as_i64().unwrap_or(32) as u32)
            .map(String::from)
            .unwrap_or_default(),
    };
    Ok(Value::string(unit.repeat(count)))
}

/// Val(): parses the longest numeric prefix ("3 apples" -> 3); result is Float.
fn val(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 2)?;
    let text = string_arg(&args, 0)?;
    if args.len() == 2 {
        let radix = int_arg(&args, 1)? as u32;
        return Ok(Value::Integer(
            i64::from_str_radix(text.trim(), radix).unwrap_or(0) as i32,
        ));
    }
    Ok(Value::Float(parse_number_prefix(text) as f32))
}

fn parse_number_prefix(text: &str) -> f64 {
    let trimmed = text.trim_start();
    let mut end = 0;
    let mut seen_digit = false;
    let mut seen_dot = false;
    let mut seen_exponent = false;
    let bytes = trimmed.as_bytes();
    while end < bytes.len() {
        let c = bytes[end] as char;
        let accept = match c {
            '0'..='9' => {
                seen_digit = true;
                true
            }
            '+' | '-' => end == 0 || matches!(bytes[end - 1] as char, 'e' | 'E'),
            '.' if !seen_dot && !seen_exponent => {
                seen_dot = true;
                true
            }
            'e' | 'E' if seen_digit && !seen_exponent => {
                seen_exponent = true;
                true
            }
            _ => false,
        };
        if !accept {
            break;
        }
        end += 1;
    }
    let mut candidate = &trimmed[..end];
    while !candidate.is_empty() {
        if let Ok(value) = candidate.parse::<f64>() {
            return value;
        }
        candidate = &candidate[..candidate.len() - 1];
    }
    0.0
}

fn create_object(vm: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::wrong_argument_count());
    }
    let class_name = string_arg(&args, 0)?.to_string();
    let class = class_name.to_lowercase();
    if let Some(result) = crate::runtime_objects::create(vm, &class, &args) {
        return result;
    }
    Ok(match class.as_str() {
        "roarray" => Value::object(Object::Array(Vec::new())),
        "rolist" => Value::object(Object::List(Vec::new())),
        "roassociativearray" => Value::object(Object::AssocArray(AssocArray::new())),
        "rotimespan" => Value::object(Object::Timespan {
            mark_ms: vm.now_ms(),
        }),
        "romessageport" => {
            let port = vm.new_port();
            Value::object(Object::MessagePort(port))
        }
        "robytearray" => Value::object(Object::ByteArray(Vec::new())),
        "rostring" => Value::object(Object::Boxed(Value::string(""))),
        "roint" => Value::object(Object::Boxed(Value::Integer(0))),
        "rofloat" => Value::object(Object::Boxed(Value::Float(0.0))),
        "rodouble" => Value::object(Object::Boxed(Value::Double(0.0))),
        "roboolean" => Value::object(Object::Boxed(Value::Boolean(false))),
        "rolonginteger" => Value::object(Object::Boxed(Value::LongInteger(0))),
        "rodatetime" => Value::object(Object::DateTime {
            epoch_ms: vm.epoch_ms() as i64,
        }),
        "roappinfo" => Value::object(Object::AppInfo),
        "rodeviceinfo" => Value::object(Object::DeviceInfo),
        "roregistry" => Value::object(Object::Registry),
        "roregistrysection" => {
            arity(&args, 2, 2)?;
            Value::object(Object::RegistrySection(string_arg(&args, 1)?.to_string()))
        }
        "roxmlelement" => Value::object(Object::XmlElement(std::rc::Rc::new(
            crate::xml::XmlElement {
                name: String::new(),
                attributes: Vec::new(),
                children: Vec::new(),
            },
        ))),
        "rosgscreen" => crate::sg::runtime::new_screen(vm),
        "rosgnode" => {
            arity(&args, 2, 2)?;
            let type_name = string_arg(&args, 1)?.to_string();
            match crate::sg::runtime::create_node(vm, &type_name)? {
                Some(node) => Value::Object(node),
                None => {
                    let location = vm.current_location();
                    vm.write(&format!(
                        "BRIGHTSCRIPT: ERROR: roSGNode: Failed to create roSGNode with type {type_name}: {location}\n"
                    ));
                    Value::Invalid
                }
            }
        }
        _ => {
            let location = vm.current_location();
            vm.write(&format!(
                "BRIGHTSCRIPT: ERROR: Runtime: unknown classname \"{class_name}\": {location}\n"
            ));
            Value::Invalid
        }
    })
}

fn parse_json(vm: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 2)?;
    match crate::json::parse(string_arg(&args, 0)?) {
        Ok(value) => Ok(value),
        Err(message) => {
            let location = vm.current_location();
            vm.write(&format!(
                "BRIGHTSCRIPT: ERROR: ParseJSON: {message}: {location}\n"
            ));
            Ok(Value::Invalid)
        }
    }
}

fn format_json(vm: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 1, 2)?;
    match crate::json::format(&args[0]) {
        Ok(text) => Ok(Value::string(text)),
        Err(message) => {
            let location = vm.current_location();
            vm.write(&format!(
                "BRIGHTSCRIPT: ERROR: FormatJSON: {message}: {location}\n"
            ));
            Ok(Value::string(""))
        }
    }
}

fn substitute(_: &mut Vm, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity(&args, 2, 5)?;
    let mut text = string_arg(&args, 0)?.to_string();
    for (index, value) in args.iter().enumerate().skip(1) {
        let replacement = value.as_str().unwrap_or_default();
        text = text.replace(&format!("{{{}}}", index - 1), replacement);
    }
    Ok(Value::string(text))
}

/// Interfaces each class implements (for GetInterface).
fn interfaces(class: &str) -> &'static [&'static str] {
    match class {
        "roArray" => &[
            "ifArray",
            "ifArrayGet",
            "ifArraySet",
            "ifArrayJoin",
            "ifArraySort",
            "ifEnum",
        ],
        "roList" => &[
            "ifList",
            "ifArray",
            "ifArrayGet",
            "ifArraySet",
            "ifListToArray",
            "ifEnum",
        ],
        "roAssociativeArray" => &["ifAssociativeArray", "ifEnum"],
        "roString" => &["ifString", "ifStringOps", "ifToStr"],
        "roSGNode" => &[
            "ifSGNodeField",
            "ifSGNodeChildren",
            "ifSGNodeFocus",
            "ifSGNodeDict",
            "ifSGNodeBoundingRect",
            "ifSGNodeHttpAgentAccess",
            "ifHttpAgent",
        ],
        "roByteArray" => &["ifByteArray", "ifArray", "ifEnum"],
        "roMessagePort" => &["ifMessagePort"],
        "roDateTime" => &["ifDateTime"],
        "roUrlTransfer" => &[
            "ifUrlTransfer",
            "ifHttpAgent",
            "ifSetMessagePort",
            "ifGetMessagePort",
        ],
        "roHttpAgent" => &["ifHttpAgent"],
        "roUrlEvent" => &["ifUrlEvent", "ifSourceIdentity"],
        "roInput" => &["ifInput", "ifSetMessagePort"],
        "roInputEvent" => &["ifInputEvent"],
        "roFileSystem" => &["ifFileSystem"],
        "roPath" => &["ifPath", "ifToStr"],
        "roRegex" => &["ifRegex"],
        "roLocalization" => &["ifLocalization"],
        "roEVPDigest" => &["ifEVPDigest"],
        "roHMAC" => &["ifHMAC"],
        "roEVPCipher" => &["ifEVPCipher"],
        "roXMLElement" => &["ifXMLElement"],
        _ => &[],
    }
}

fn get_interface(object: &Value, name: &Value) -> Value {
    let name = name.as_str().unwrap_or_default();
    match object {
        Value::Object(o) => {
            let class = o.borrow().class_name();
            if interfaces(class)
                .iter()
                .any(|i| i.eq_ignore_ascii_case(name))
            {
                object.clone()
            } else {
                Value::Invalid
            }
        }
        _ => Value::Invalid,
    }
}

// ---- member functions ----------------------------------------------------------

pub fn call_method(
    vm: &mut Vm,
    receiver: &Value,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if let Value::Object(object) = receiver {
        let (is_node, screen, event) = match &*object.borrow() {
            Object::Node(_) => (true, None, None),
            Object::Screen(handle) => (false, Some(*handle), None),
            Object::NodeEvent(event) => (false, None, Some(event.clone())),
            _ => (false, None, None),
        };
        if is_node {
            return crate::sg::runtime::node_method(vm, object, name, args);
        }
        if let Some(handle) = screen {
            let args = args.iter().map(Value::unboxed).collect();
            return crate::sg::runtime::screen_method(vm, handle, name, args);
        }
        if let Some(event) = event {
            return crate::sg::runtime::node_event_method(&event, name);
        }
        let unboxed: Vec<Value> = args.iter().map(Value::unboxed).collect();
        if let Some(result) = crate::components::call(vm, object, name, &unboxed) {
            return result;
        }
        // Boxed intrinsics expose the intrinsic's methods (roString -> ifString).
        let inner = match &*object.borrow() {
            Object::Boxed(inner) => Some(inner.clone()),
            _ => None,
        };
        if let Some(inner) = inner {
            return call_method(vm, &inner, name, unboxed);
        }
        // roList also implements ifArray.
        let list_items = match &*object.borrow() {
            Object::List(items) => Some(items.clone()),
            _ => None,
        };
        if let Some(items) = list_items {
            let array = Value::object(Object::Array(items));
            let result = array_method(vm, &array, name, unboxed)?;
            if let (Value::Object(array), Object::List(items)) = (&array, &mut *object.borrow_mut())
            {
                if let Object::Array(updated) = &*array.borrow() {
                    *items = updated.clone();
                }
            }
            return Ok(result);
        }
        let args = unboxed;
        return call_method_intrinsic(vm, receiver, name, args);
    }
    let args = args.iter().map(Value::unboxed).collect();
    call_method_intrinsic(vm, receiver, name, args)
}

fn call_method_intrinsic(
    vm: &mut Vm,
    receiver: &Value,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    match receiver {
        Value::String(text) => string_method(text, name, args),
        Value::Integer(_)
        | Value::LongInteger(_)
        | Value::Float(_)
        | Value::Double(_)
        | Value::Boolean(_) => match name {
            "tostr" => {
                arity(&args, 0, 1)?;
                Ok(Value::string(to_display_string(receiver)))
            }
            _ => Err(RuntimeError::member_not_found()),
        },
        Value::Object(object) => {
            let class = object.borrow().class_name();
            match class {
                "roArray" => array_method(vm, receiver, name, args),
                "roAssociativeArray" => assoc_array_method(receiver, name, args),
                "roTimespan" => timespan_method(vm, receiver, name, args),
                _ => Err(RuntimeError::member_not_found()),
            }
        }
        _ => Err(RuntimeError::invalid_dot()),
    }
}

fn string_method(text: &str, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let chars: Vec<char> = text.chars().collect();
    Ok(match name {
        "len" => {
            arity(&args, 0, 0)?;
            Value::Integer(chars.len() as i32)
        }
        "left" => {
            arity(&args, 1, 1)?;
            Value::string(char_slice(text, 0, int_arg(&args, 0)?.max(0) as usize))
        }
        "right" => {
            arity(&args, 1, 1)?;
            let count = int_arg(&args, 0)?.max(0) as usize;
            Value::string(char_slice(text, chars.len().saturating_sub(count), count))
        }
        "mid" => {
            arity(&args, 1, 2)?;
            let start = int_arg(&args, 0)?.max(0) as usize;
            let count = if args.len() == 2 {
                int_arg(&args, 1)?.max(0) as usize
            } else {
                usize::MAX
            };
            Value::string(char_slice(text, start, count))
        }
        "instr" => {
            arity(&args, 1, 2)?;
            let (start, needle) = if args.len() == 2 {
                (int_arg(&args, 0)?.max(0) as usize, string_arg(&args, 1)?)
            } else {
                (0, string_arg(&args, 0)?)
            };
            let needle: Vec<char> = needle.chars().collect();
            Value::Integer(find_chars(&chars, &needle, start).map_or(-1, |i| i as i32))
        }
        "trim" => {
            arity(&args, 0, 0)?;
            Value::string(text.trim())
        }
        "toint" => {
            arity(&args, 0, 0)?;
            Value::Integer(parse_number_prefix(text) as i32)
        }
        "tofloat" => {
            arity(&args, 0, 0)?;
            Value::Float(parse_number_prefix(text) as f32)
        }
        "tostr" | "getstring" => {
            arity(&args, 0, 0)?;
            Value::string(text)
        }
        "isempty" => {
            arity(&args, 0, 0)?;
            Value::Boolean(text.is_empty())
        }
        "startswith" => {
            arity(&args, 1, 2)?;
            Value::Boolean(text.starts_with(string_arg(&args, 0)?))
        }
        "endswith" => {
            arity(&args, 1, 2)?;
            Value::Boolean(text.ends_with(string_arg(&args, 0)?))
        }
        "replace" => {
            arity(&args, 2, 2)?;
            let from = string_arg(&args, 0)?;
            if from.is_empty() {
                Value::string(text)
            } else {
                Value::string(text.replace(from, string_arg(&args, 1)?))
            }
        }
        "split" => {
            arity(&args, 1, 1)?;
            let separator = string_arg(&args, 0)?;
            let parts: Vec<Value> = if separator.is_empty() {
                chars.iter().map(|c| Value::string(c.to_string())).collect()
            } else {
                text.split(separator).map(Value::string).collect()
            };
            Value::object(Object::Array(parts))
        }
        "tokenize" => {
            arity(&args, 1, 1)?;
            let delimiters: Vec<char> = string_arg(&args, 0)?.chars().collect();
            let parts = text
                .split(|c| delimiters.contains(&c))
                .filter(|p| !p.is_empty())
                .map(Value::string)
                .collect();
            Value::object(Object::Array(parts))
        }
        _ => return Err(RuntimeError::member_not_found()),
    })
}

fn with_array<T>(receiver: &Value, f: impl FnOnce(&mut Vec<Value>) -> T) -> T {
    let Value::Object(object) = receiver else {
        unreachable!("array receiver")
    };
    let mut object = object.borrow_mut();
    let Object::Array(items) = &mut *object else {
        unreachable!("array receiver")
    };
    f(items)
}

fn array_method(
    vm: &mut Vm,
    receiver: &Value,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    Ok(match name {
        "count" => {
            arity(&args, 0, 0)?;
            Value::Integer(with_array(receiver, |items| items.len() as i32))
        }
        "isempty" => {
            arity(&args, 0, 0)?;
            Value::Boolean(with_array(receiver, |items| items.is_empty()))
        }
        "push" => {
            arity(&args, 1, 1)?;
            let value = args.into_iter().next().unwrap_or(Value::Invalid);
            with_array(receiver, |items| items.push(value));
            Value::Invalid
        }
        "pop" => {
            arity(&args, 0, 0)?;
            with_array(receiver, |items| items.pop()).unwrap_or(Value::Invalid)
        }
        "peek" => {
            arity(&args, 0, 0)?;
            with_array(receiver, |items| items.last().cloned()).unwrap_or(Value::Invalid)
        }
        "shift" => {
            arity(&args, 0, 0)?;
            with_array(receiver, |items| {
                if items.is_empty() {
                    Value::Invalid
                } else {
                    items.remove(0)
                }
            })
        }
        "unshift" => {
            arity(&args, 1, 1)?;
            let value = args.into_iter().next().unwrap_or(Value::Invalid);
            with_array(receiver, |items| items.insert(0, value));
            Value::Invalid
        }
        "clear" => {
            arity(&args, 0, 0)?;
            with_array(receiver, Vec::clear);
            Value::Invalid
        }
        "delete" => {
            arity(&args, 1, 1)?;
            let index = int_arg(&args, 0)?;
            Value::Boolean(with_array(receiver, |items| {
                if index >= 0 && (index as usize) < items.len() {
                    items.remove(index as usize);
                    true
                } else {
                    false
                }
            }))
        }
        "getentry" => {
            arity(&args, 1, 1)?;
            let index = int_arg(&args, 0)?;
            with_array(receiver, |items| {
                usize::try_from(index)
                    .ok()
                    .and_then(|i| items.get(i).cloned())
            })
            .unwrap_or(Value::Invalid)
        }
        "setentry" => {
            arity(&args, 2, 2)?;
            let index = int_arg(&args, 0)?.max(0) as usize;
            let value = args[1].clone();
            with_array(receiver, |items| {
                if index >= items.len() {
                    items.resize(index + 1, Value::Invalid);
                }
                items[index] = value;
            });
            Value::Invalid
        }
        "append" => {
            arity(&args, 1, 1)?;
            let extra = match &args[0] {
                Value::Object(other) => match &*other.borrow() {
                    Object::Array(items) => items.clone(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            with_array(receiver, |items| items.extend(extra));
            Value::Invalid
        }
        "reverse" => {
            arity(&args, 0, 0)?;
            with_array(receiver, |items| items.reverse());
            Value::Invalid
        }
        "join" => {
            arity(&args, 1, 1)?;
            let separator = string_arg(&args, 0)?.to_string();
            let joined = with_array(receiver, |items| {
                let mut parts = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    match item.as_str() {
                        Some(text) => parts.push(text.to_string()),
                        None => return Err(index),
                    }
                }
                Ok(parts.join(&separator))
            });
            match joined {
                Ok(text) => Value::string(text),
                Err(index) => {
                    // Non-fatal on device: diagnostic line, empty result.
                    let location = vm.current_location();
                    vm.write(&format!("BRIGHTSCRIPT: ERROR: roArray.Join: non-string value at index {index}: {location}\n"));
                    Value::string("")
                }
            }
        }
        "sort" => {
            arity(&args, 0, 1)?;
            let flags = if args.is_empty() {
                String::new()
            } else {
                string_arg(&args, 0)?.to_lowercase()
            };
            let case_insensitive = flags.contains('i');
            with_array(receiver, |items| {
                items.sort_by(|a, b| {
                    sort_key(a, case_insensitive)
                        .partial_cmp(&sort_key(b, case_insensitive))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                if flags.contains('r') {
                    items.reverse();
                }
            });
            Value::Invalid
        }
        _ => return Err(RuntimeError::member_not_found()),
    })
}

/// Sort order: numbers ascending, then strings, then everything else.
fn sort_key(value: &Value, case_insensitive: bool) -> (u8, f64, String) {
    match value {
        v if v.is_numeric() => (0, v.as_f64().unwrap_or(0.0), String::new()),
        Value::String(s) => (
            1,
            0.0,
            if case_insensitive {
                s.to_lowercase()
            } else {
                s.to_string()
            },
        ),
        _ => (2, 0.0, String::new()),
    }
}

fn with_aa<T>(receiver: &Value, f: impl FnOnce(&mut AssocArray) -> T) -> T {
    let Value::Object(object) = receiver else {
        unreachable!("aa receiver")
    };
    let mut object = object.borrow_mut();
    let Object::AssocArray(aa) = &mut *object else {
        unreachable!("aa receiver")
    };
    f(aa)
}

fn assoc_array_method(
    receiver: &Value,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    Ok(match name {
        "count" => {
            arity(&args, 0, 0)?;
            Value::Integer(with_aa(receiver, |aa| aa.len() as i32))
        }
        "isempty" => {
            arity(&args, 0, 0)?;
            Value::Boolean(with_aa(receiver, |aa| aa.is_empty()))
        }
        "keys" => {
            arity(&args, 0, 0)?;
            let keys = with_aa(receiver, |aa| {
                aa.sorted_entries()
                    .into_iter()
                    .map(|(k, _)| Value::string(k))
                    .collect()
            });
            Value::object(Object::Array(keys))
        }
        "items" => {
            arity(&args, 0, 0)?;
            let items = with_aa(receiver, |aa| {
                aa.sorted_entries()
                    .into_iter()
                    .map(|(key, value)| {
                        let mut item = AssocArray::new();
                        item.set("key", Value::string(key));
                        item.set("value", value);
                        Value::object(Object::AssocArray(item))
                    })
                    .collect()
            });
            Value::object(Object::Array(items))
        }
        "lookup" | "lookupci" => {
            arity(&args, 1, 1)?;
            let key = string_arg(&args, 0)?;
            with_aa(receiver, |aa| aa.get(key).cloned()).unwrap_or(Value::Invalid)
        }
        "doesexist" => {
            arity(&args, 1, 1)?;
            let key = string_arg(&args, 0)?;
            Value::Boolean(with_aa(receiver, |aa| aa.contains(key)))
        }
        "delete" => {
            arity(&args, 1, 1)?;
            let key = string_arg(&args, 0)?;
            Value::Boolean(with_aa(receiver, |aa| aa.remove(key)))
        }
        "clear" => {
            arity(&args, 0, 0)?;
            with_aa(receiver, AssocArray::clear);
            Value::Invalid
        }
        "addreplace" => {
            arity(&args, 2, 2)?;
            let key = string_arg(&args, 0)?.to_string();
            let value = args[1].clone();
            with_aa(receiver, |aa| aa.set(&key, value));
            Value::Invalid
        }
        "append" => {
            arity(&args, 1, 1)?;
            let extra = match &args[0] {
                Value::Object(other) => match &*other.borrow() {
                    Object::AssocArray(aa) => aa.sorted_entries(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            with_aa(receiver, |aa| {
                for (key, value) in extra {
                    aa.set(&key, value);
                }
            });
            Value::Invalid
        }
        "setmodecasesensitive" => {
            arity(&args, 0, 0)?;
            Value::Invalid
        }
        _ => return Err(RuntimeError::member_not_found()),
    })
}

fn timespan_method(
    vm: &mut Vm,
    receiver: &Value,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    arity(&args, 0, 0)?;
    let now = vm.now_ms();
    let Value::Object(object) = receiver else {
        unreachable!()
    };
    let mut object = object.borrow_mut();
    let Object::Timespan { mark_ms } = &mut *object else {
        unreachable!()
    };
    Ok(match name {
        "mark" => {
            *mark_ms = now;
            Value::Invalid
        }
        "totalmilliseconds" => Value::Integer((now - *mark_ms) as i32),
        "totalseconds" => Value::Integer(((now - *mark_ms) / 1000.0) as i32),
        _ => return Err(RuntimeError::member_not_found()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_prefix_parsing() {
        assert_eq!(parse_number_prefix("12.5"), 12.5);
        assert_eq!(parse_number_prefix("3 apples"), 3.0);
        assert_eq!(parse_number_prefix("abc"), 0.0);
        assert_eq!(parse_number_prefix("  -4e2x"), -400.0);
    }

    #[test]
    fn radix_conversion() {
        assert_eq!(to_radix(255, 16), "ff");
        assert_eq!(to_radix(-5, 2), "-101");
    }
}
