mod cookies;
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value as Json};
use std::cell::RefCell;
use std::rc::Weak;

#[derive(Debug)]
pub struct PendingHttp {
    object: Weak<RefCell<Object>>,
    port: Option<usize>,
    file: Option<String>,
    source: i32,
}
fn port(state: &AssocArray) -> Option<usize> {
    state.get("port").and_then(|v| match v {
        Value::Object(o) => match &*o.borrow() {
            Object::MessagePort(p) => Some(*p),
            _ => None,
        },
        _ => None,
    })
}
fn headers(state: &AssocArray) -> Json {
    let value = state.get("headers").cloned().unwrap_or(Value::Invalid);
    crate::json::format(&value)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .filter(Json::is_object)
        .unwrap_or(json!({}))
}
pub(crate) fn request_headers(vm: &Vm, object: &ObjectRef, url: &str) -> Json {
    let state = state(object);
    let mut result = headers(&state);
    if let Some(cookie) = cookies::header(vm, object, url) {
        result["Cookie"] = json!(cookie);
    }
    if !get_str(&state, "authorization").is_empty() {
        result["Authorization"] = json!(get_str(&state, "authorization"));
    }
    if !get_str(&state, "useragent").is_empty() {
        result["User-Agent"] = json!(get_str(&state, "useragent"));
    }
    result
}

fn event(state: &AssocArray, result: &Json) -> Value {
    let mut state = state.clone();
    state.set(
        "code",
        Value::Integer(result["status"].as_i64().unwrap_or(-7) as i32),
    );
    let response_bytes = STANDARD
        .decode(result["body"].as_str().unwrap_or(""))
        .unwrap_or_default();
    let retain = result["status"].as_i64().unwrap_or(-7) < 400
        || matches!(state.get("retainbody"), Some(Value::Boolean(true)));
    state.set(
        "body",
        Value::string(if retain {
            String::from_utf8_lossy(&response_bytes).into_owned()
        } else {
            String::new()
        }),
    );
    state.set(
        "failure",
        Value::string(result["failure"].as_str().unwrap_or("")),
    );
    let headerjson = result["headers"].to_string();
    state.set(
        "responseheaders",
        crate::json::parse(&headerjson).unwrap_or(Value::Invalid),
    );
    state.set(
        "responseheadersarray",
        crate::json::parse(&result["headersArray"].to_string())
            .unwrap_or_else(|_| Value::object(Object::Array(vec![]))),
    );
    native("roUrlEvent", state)
}
pub fn complete_http(vm: &mut Vm, id: u32, response: &str) {
    let Some(pending) = vm.env.http_pending.remove(&id) else {
        // Not a roUrlTransfer: maybe a Video metadata request.
        crate::sg::video::complete_http(vm, id, response);
        return;
    };
    let Some(object) = pending.object.upgrade() else {
        return;
    };
    store(&object, "busy", Value::Boolean(false));
    let result: Json = serde_json::from_str(response)
        .unwrap_or(json!({"status":-7,"failure":"Invalid HTTP host response"}));
    cookies::receive(vm, &object, &result);
    if let Some(file) = pending.file {
        vm.env.write_file(
            &file,
            STANDARD
                .decode(result["body"].as_str().unwrap_or(""))
                .unwrap_or_default(),
        );
    }
    let mut state = state(&object);
    state.set("identity", Value::Integer(pending.source));
    if let Some(port) = pending.port {
        vm.post_message(port, event(&state, &result));
    }
}
fn percent(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

pub(super) fn call(vm: &mut Vm, object: &ObjectRef, method: &str, args: &[Value]) -> ResultValue {
    let state = state(object);
    let class = object.borrow().class_name();
    if class == "roUrlEvent" {
        return Ok(match method {
            "getresponsecode" => Value::Integer(get_int(&state, "code") as i32),
            "getstring" => Value::string(get_str(&state, "body")),
            "getfailurereason" => Value::string(get_str(&state, "failure")),
            "getsourceidentity" => Value::Integer(get_int(&state, "identity") as i32),
            "gettargetipaddress" => Value::string(""),
            "getint" => Value::Integer(1),
            "getresponseheaders" => state
                .get("responseheaders")
                .cloned()
                .unwrap_or(Value::Invalid),
            "getresponseheadersarray" => state
                .get("responseheadersarray")
                .cloned()
                .unwrap_or_else(|| Value::object(Object::Array(vec![]))),
            _ => return Err(RuntimeError::member_not_found()),
        });
    }
    match method {
        "seturl" | "setrequest" | "setcertificatesfile" | "setuseragent" => {
            let key = match method {
                "seturl" => "url",
                "setrequest" => "method",
                "setuseragent" => "useragent",
                _ => "certificates",
            };
            store(object, key, argument(args, 0));
            ok(true)
        }
        "getrequest" => Ok(Value::string(get_str(&state, "method"))),
        "getuseragent" => Ok(Value::string(get_str(&state, "useragent"))),
        "geturl" => Ok(Value::string(get_str(&state, "url"))),
        "getidentity" => Ok(Value::Integer(get_int(&state, "identity") as i32)),
        "setmessageport" | "setport" => {
            store(object, "port", argument(args, 0));
            Ok(Value::Invalid)
        }
        "getmessageport" => Ok(state.get("port").cloned().unwrap_or(Value::Invalid)),
        "addheader" | "setheaders" => {
            let mut header = match state.get("headers") {
                Some(Value::Object(o)) => match &*o.borrow() {
                    Object::AssocArray(a) => a.clone(),
                    _ => AssocArray::new(),
                },
                _ => AssocArray::new(),
            };
            if method == "setheaders" {
                if let Value::Object(o) = argument(args, 0) {
                    if let Object::AssocArray(a) = &*o.borrow() {
                        header = a.clone();
                    }
                }
            } else {
                header.set(&text(args, 0), Value::string(text(args, 1)));
            }
            store(object, "headers", Value::object(Object::AssocArray(header)));
            ok(true)
        }
        "clearheaders" => {
            store(
                object,
                "headers",
                Value::object(Object::AssocArray(AssocArray::new())),
            );
            Ok(Value::Invalid)
        }
        "setuserandpassword" => {
            store(object, "username", argument(args, 0));
            store(object, "password", argument(args, 1));
            ok(true)
        }
        "enablehostverification" | "enablepeerverification" => {
            store(
                object,
                if method == "enablehostverification" {
                    "hostverify"
                } else {
                    "peerverify"
                },
                argument(args, 0),
            );
            ok(true)
        }
        "retainbodyonerror" => {
            store(object, "retainbody", argument(args, 0));
            ok(true)
        }
        "setminimumtransferrate"
        | "setmaximumtransferrate"
        | "setconnecttimeout"
        | "settimeout" => {
            if method == "settimeout" || method == "setconnecttimeout" {
                store(
                    object,
                    "timeout",
                    Value::Integer((number(args, 0) * 1000) as i32),
                );
            }
            Ok(Value::Invalid)
        }
        "initclientcerts"
        | "initclientcertificates"
        | "enableencodings"
        | "enablefreshconnection"
        | "enableresume"
        | "enablesepevents"
        | "enablesplitevents"
        | "sethttpversion" => ok(true),
        "enablecookies" => {
            store(object, "cookiesenabled", Value::Boolean(true));
            Ok(Value::Invalid)
        }
        "getcookies" => Ok(cookies::get(
            vm,
            object,
            &text(args, 0),
            &text(args, 1),
            matches!(args.get(2), Some(Value::Boolean(true))),
        )),
        "addcookies" => {
            store(object, "cookies", argument(args, 0));
            ok(true)
        }
        "clearcookies" => {
            store(object, "cookies", Value::object(Object::Array(vec![])));
            Ok(Value::Invalid)
        }
        "escape" | "urlencode" => Ok(Value::string(percent(&text(args, 0)))),
        "unescape" => {
            let input = text(args, 0);
            let bytes = input.as_bytes();
            let mut out = Vec::new();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'%' && i + 2 < bytes.len() {
                    if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                        if let Ok(byte) = u8::from_str_radix(hex, 16) {
                            out.push(byte);
                            i += 3;
                            continue;
                        }
                    }
                }
                out.push(bytes[i]);
                i += 1;
            }
            Ok(Value::string(String::from_utf8_lossy(&out).as_ref()))
        }
        "asynccancel" => {
            let token = get_int(&state, "token") as u32;
            vm.env.http_pending.remove(&token);
            vm.host_http_cancel(token);
            store(object, "busy", Value::Boolean(false));
            ok(true)
        }
        "getfailurereason" => Ok(Value::string(get_str(&state, "failure"))),
        "gettostring"
        | "gettofile"
        | "postfromstring"
        | "postfromfile"
        | "postfromfiletofile"
        | "head"
        | "asyncgettostring"
        | "asyncgettofile"
        | "asyncpostfromstring"
        | "asyncpostfromfile"
        | "asyncpostfromfiletofile"
        | "asynchead" => {
            let asynchronous = method.starts_with("async");
            let operation = method.strip_prefix("async").unwrap_or(method);
            if matches!(state.get("busy"), Some(Value::Boolean(true))) {
                return if asynchronous {
                    ok(false)
                } else {
                    Ok(Value::Invalid)
                };
            }
            let post = operation.starts_with("post");
            let mut verb = get_str(&state, "method");
            if verb.is_empty() {
                verb = if operation == "head" {
                    "HEAD"
                } else if post {
                    "POST"
                } else {
                    "GET"
                }
                .into();
            }
            let body = if operation == "postfromstring" {
                text(args, 0).into_bytes()
            } else if post {
                vm.env.read_file(&text(args, 0)).unwrap_or_default()
            } else {
                vec![]
            };
            let file = if operation == "gettofile" {
                Some(text(args, 0))
            } else if operation == "postfromfiletofile" {
                Some(text(args, 1))
            } else {
                None
            };
            let headers = request_headers(vm, object, &get_str(&state, "url"));
            let request=json!({"url":get_str(&state,"url"),"method":verb,"headers":headers,"body":STANDARD.encode(body),"username":get_str(&state,"username"),"password":get_str(&state,"password"),"peerVerify":!matches!(state.get("peerverify"),Some(Value::Boolean(false))),"hostVerify":!matches!(state.get("hostverify"),Some(Value::Boolean(false))),"ca":vm.env.read_file(&get_str(&state,"certificates")).map(|b|String::from_utf8_lossy(&b).into_owned()),"timeout": if get_int(&state,"timeout")>0{get_int(&state,"timeout")}else{30000}}).to_string();
            if asynchronous {
                vm.env.next_identity += 1;
                let token = vm.env.next_identity;
                vm.env.http_pending.insert(
                    token,
                    PendingHttp {
                        object: std::rc::Rc::downgrade(object),
                        port: port(&state),
                        file,
                        source: get_int(&state, "identity") as i32,
                    },
                );
                store(object, "token", Value::Integer(token as i32));
                store(object, "busy", Value::Boolean(true));
                let started = vm.host_http_start(token, &request);
                if !started {
                    vm.env.http_pending.remove(&token);
                    store(object, "busy", Value::Boolean(false));
                }
                ok(started)
            } else {
                let response = vm.host_http(&request);
                let result: Json = serde_json::from_str(&response)
                    .unwrap_or(json!({"status":-7,"failure":"Invalid host response"}));
                store(
                    object,
                    "failure",
                    Value::string(result["failure"].as_str().unwrap_or("")),
                );
                cookies::receive(vm, object, &result);
                let status = result["status"].as_i64().unwrap_or(-7) as i32;
                let data = STANDARD
                    .decode(result["body"].as_str().unwrap_or(""))
                    .unwrap_or_default();
                if let Some(file) = file {
                    vm.env.write_file(&file, data);
                    Ok(Value::Integer(status))
                } else if operation == "head" {
                    Ok(event(&state, &result))
                } else if post {
                    Ok(Value::Integer(status))
                } else {
                    Ok(Value::string(
                        if status < 400
                            || matches!(state.get("retainbody"), Some(Value::Boolean(true)))
                        {
                            String::from_utf8_lossy(&data).into_owned()
                        } else {
                            String::new()
                        },
                    ))
                }
            }
        }
        _ => Err(RuntimeError::member_not_found()),
    }
}
