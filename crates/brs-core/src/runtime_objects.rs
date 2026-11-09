//! Nonvisual Roku components, shared by native and browser hosts.
mod filesystem;
mod network;
mod regex_crypto;

use crate::error::RuntimeError;
use crate::value::{AssocArray, Object, ObjectRef, Value};
use crate::vm::Vm;
pub use network::{complete_http, PendingHttp};

type ResultValue = Result<Value, RuntimeError>;
fn argument(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Invalid).unboxed()
}
fn text(args: &[Value], i: usize) -> String {
    argument(args, i).as_str().unwrap_or_default().to_string()
}
fn number(args: &[Value], i: usize) -> i64 {
    argument(args, i).as_i64().unwrap_or(0)
}
fn native(class: &'static str, state: AssocArray) -> Value {
    Value::object(Object::Native { class, state })
}
fn state(object: &ObjectRef) -> AssocArray {
    match &*object.borrow() {
        Object::Native { state, .. } => state.clone(),
        _ => unreachable!(),
    }
}
fn store(object: &ObjectRef, key: &str, value: Value) {
    if let Object::Native { state, .. } = &mut *object.borrow_mut() {
        state.set(key, value);
    }
}
fn get_str(state: &AssocArray, key: &str) -> String {
    state
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
fn get_int(state: &AssocArray, key: &str) -> i64 {
    state.get(key).and_then(Value::as_i64).unwrap_or(0)
}
fn ok(value: bool) -> ResultValue {
    Ok(Value::Boolean(value))
}
fn byte_array(bytes: Vec<u8>) -> Value {
    Value::object(Object::ByteArray(bytes))
}
fn bytes(value: &Value) -> Vec<u8> {
    match value {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Object(o) => match &*o.borrow() {
            Object::ByteArray(v) => v.clone(),
            _ => vec![],
        },
        _ => vec![],
    }
}
fn list(strings: impl IntoIterator<Item = String>) -> Value {
    Value::object(Object::List(
        strings.into_iter().map(Value::string).collect(),
    ))
}

pub fn create(vm: &mut Vm, class: &str, args: &[Value]) -> Option<ResultValue> {
    let class = match class {
        "rourltransfer" => "roUrlTransfer",
        "rohttpagent" => "roHttpAgent",
        "roinput" => "roInput",
        "roappmanager" => "roAppManager",
        "rofilesystem" => "roFileSystem",
        "ropath" => "roPath",
        "rolocalization" => "roLocalization",
        "roregex" => "roRegex",
        "roevpdigest" => "roEVPDigest",
        "rohmac" => "roHMAC",
        "roevpcipher" => "roEVPCipher",
        _ => return None,
    };
    let mut state = AssocArray::new();
    vm.env.next_identity += 1;
    state.set("identity", Value::Integer(vm.env.next_identity as i32));
    if class == "roPath" {
        state.set("path", Value::string(text(args, 1)));
    }
    if class == "roRegex" {
        let pattern = text(args, 1);
        let flags = text(args, 2);
        if regex_crypto::regex(&pattern, &flags).is_err() {
            return Some(Ok(Value::Invalid));
        }
        state.set("pattern", Value::string(pattern));
        state.set("flags", Value::string(flags));
    }
    Some(Ok(native(class, state)))
}

pub fn call(vm: &mut Vm, object: &ObjectRef, method: &str, args: &[Value]) -> ResultValue {
    let class = object.borrow().class_name();
    match class {
        "roUrlTransfer" | "roHttpAgent" | "roUrlEvent" => network::call(vm, object, method, args),
        "roFileSystem" | "roPath" => filesystem::call(vm, object, method, args),
        "roRegex" | "roEVPDigest" | "roHMAC" | "roEVPCipher" => {
            regex_crypto::call(vm, object, method, args)
        }
        "roInput" => match method {
            "setmessageport" | "setport" => {
                store(object, "port", argument(args, 0));
                if let Value::Object(port) = argument(args, 0) {
                    if let Object::MessagePort(port) = &*port.borrow() {
                        if !vm.env.input_ports.contains(port) {
                            vm.env.input_ports.push(*port);
                        }
                    }
                }
                Ok(Value::Invalid)
            }
            "enabletransportevents" => ok(true),
            "getmessageport" => Ok(state(object).get("port").cloned().unwrap_or(Value::Invalid)),
            _ => Err(RuntimeError::member_not_found()),
        },
        "roInputEvent" => match method {
            "isinput" => ok(true),
            "getinfo" => Ok(state(object).get("info").cloned().unwrap_or(Value::Invalid)),
            _ => Err(RuntimeError::member_not_found()),
        },
        "roLocalization" => match method {
            "getcurrentlocale" => Ok(Value::string(vm.env.profile.locale)),
            "getlocalizedasset" => Ok(Value::string(text(args, 0))),
            "getlocalizedstring" => Ok(Value::string(text(args, 0))),
            _ => Err(RuntimeError::member_not_found()),
        },
        "roAppManager" => match method {
            "settheme"
            | "updatetheme"
            | "setthemeattribute"
            | "clearthemeattribute"
            | "setdisplaymode"
            | "setuserpreset" => Ok(Value::Invalid),
            "gettheme" => Ok(Value::object(Object::AssocArray(AssocArray::new()))),
            _ => network::call(vm, object, method, args),
        },
        _ => Err(RuntimeError::member_not_found()),
    }
}

pub fn deliver_input(vm: &mut Vm, info: Value) {
    let mut state = AssocArray::new();
    state.set("info", info);
    let event = native("roInputEvent", state);
    for port in vm.env.input_ports.clone() {
        vm.post_message(port, event.clone());
    }
}
