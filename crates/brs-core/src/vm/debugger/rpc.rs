//! Structured inspection shared by the socket debugger and development tools.
use super::*;
use crate::value::ObjectRef;
use serde_json::{json, Value as Json};

pub(in crate::vm) struct Breakpoint {
    pub file: String,
    pub line: u32,
    pub ignore: u64,
    pub hits: u64,
}

impl Vm {
    pub(in crate::vm) fn debug_at_breakpoint(&mut self) -> bool {
        if self.debug_eval_active {
            return false;
        }
        let Some(pos) = self.debug_position(true) else {
            return false;
        };
        let key = (self.current, pos.function, pos.line);
        if self.debug_last_position == Some(key) {
            return false;
        }
        self.debug_last_position = Some(key);
        let file = &self.program.functions[pos.function as usize].file;
        for bp in self.debug_breakpoints.values_mut() {
            if bp.file.eq_ignore_ascii_case(file) && bp.line == pos.line {
                bp.hits += 1;
                if bp.hits > bp.ignore {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn debug_rpc(&mut self, text: &str) {
        let request: Json = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return,
        };
        let id = request["id"].as_str().unwrap_or("");
        let previous = self.current;
        let result = self.debug_rpc_result(&request);
        if previous == NO_THREAD {
            self.park();
        } else {
            self.load(previous);
        }
        let response = match result {
            Ok(result) => json!({"id": id, "result": result}),
            Err(error) => json!({"id": id, "error": error}),
        };
        self.write(&format!("\n@@ROKU_EMU_RPC {response}\n"));
        if self.debug_thread.is_some() {
            self.write(PROMPT);
        }
    }

    fn debug_rpc_result(&mut self, request: &Json) -> Result<Json, String> {
        let op = request["op"].as_str().unwrap_or("");
        match op {
            "scene" => return Ok(self.debug_scene()),
            "file" => {
                use base64::Engine;
                return Ok(self
                    .env
                    .read_file(request["path"].as_str().unwrap_or(""))
                    .map(|bytes| json!(base64::engine::general_purpose::STANDARD.encode(bytes)))
                    .unwrap_or(Json::Null));
            }
            "breakpoints" => {
                for entry in request["remove"].as_array().into_iter().flatten() {
                    if let Some(id) = entry.as_u64() {
                        self.debug_breakpoints.remove(&(id as u32));
                    }
                }
                for bp in request["add"].as_array().into_iter().flatten() {
                    let file = bp["file"]
                        .as_str()
                        .ok_or("Missing breakpoint file")?
                        .to_string();
                    let line = bp["line"].as_u64().ok_or("Missing breakpoint line")? as u32;
                    let id = bp["id"].as_u64().ok_or("Missing breakpoint id")? as u32;
                    if !self
                        .program
                        .functions
                        .iter()
                        .any(|f| f.file.eq_ignore_ascii_case(&file) && f.lines.contains(&line))
                    {
                        return Err("No executable source at breakpoint".into());
                    }
                    self.debug_breakpoints.insert(
                        id,
                        Breakpoint {
                            file,
                            line,
                            ignore: bp["ignore"].as_u64().unwrap_or(0),
                            hits: 0,
                        },
                    );
                }
                return Ok(json!({"count": self.debug_breakpoints.len()}));
            }
            "threads" => {
                let threads: Vec<_> = (0..self.threads.len()).map(|id| {
                    let frames = if id == self.current { &self.frames } else { &self.threads[id].frames };
                    let frame = frames.last();
                    let location = frame.map(|frame| {
                        let proto = &self.program.functions[frame.function as usize];
                        let line = if self.debug_thread == Some(id) { self.debug_location.map(|p| p.line).unwrap_or(proto.first_line) } else { proto.lines.get(frame.ip.saturating_sub(1)).copied().unwrap_or(proto.first_line) };
                        json!({"file": proto.file.as_ref(), "function": proto.name, "line": line, "source": self.program.sources.get(&proto.file).and_then(|s| s.lines().nth(line.saturating_sub(1) as usize)).unwrap_or("")})
                    }).unwrap_or(json!({"file":"??", "function":"", "line":0,"source":""}));
                    json!({"id":id, "primary":self.debug_thread == Some(id), "active": !frames.is_empty(), "location":location})
                }).collect();
                return Ok(json!(threads));
            }
            _ => {}
        }
        if self.debug_thread.is_none() {
            return Err("Not paused".into());
        }
        let thread = request["thread"]
            .as_u64()
            .map(|v| v as usize)
            .or(self.debug_thread)
            .unwrap_or(0);
        if thread >= self.threads.len() {
            return Err("Invalid thread".into());
        }
        self.load(thread);
        self.debug_frame = request["frame"].as_u64().unwrap_or(0) as usize;
        if self.debug_frame >= self.frames.len() {
            return Err("Invalid frame".into());
        }
        match op {
            "stack" => Ok(json!(self
                .frames
                .iter()
                .rev()
                .enumerate()
                .map(|(depth, frame)| {
                    let proto = &self.program.functions[frame.function as usize];
                    let line = if depth == 0 && self.debug_thread == Some(thread) {
                        self.debug_location
                            .map(|p| p.line)
                            .unwrap_or(proto.first_line)
                    } else {
                        proto
                            .lines
                            .get(frame.ip.saturating_sub(1))
                            .copied()
                            .unwrap_or(proto.first_line)
                    };
                    json!({"file":proto.file.as_ref(),"function":proto.name,"line":line})
                })
                .collect::<Vec<_>>())),
            "variables" => {
                let path = request["path"].as_array().cloned().unwrap_or_default();
                if path.is_empty() {
                    let frame = &self.frames[self.frames.len() - 1 - self.debug_frame];
                    let proto = &self.program.functions[frame.function as usize];
                    let mut vars = vec![variable("m", &frame.m, false)];
                    for (slot, name) in proto
                        .locals
                        .iter()
                        .enumerate()
                        .filter(|(_, n)| !n.starts_with('$'))
                    {
                        vars.push(variable(name, &self.stack[frame.base + slot], false));
                    }
                    Ok(json!(vars))
                } else {
                    let name = path[0].as_str().ok_or("Invalid variable path")?;
                    let mut expression = name.to_string();
                    for key in path.iter().skip(1) {
                        let key = key.as_str().ok_or("Invalid variable key")?;
                        // Arrays use numeric indexes; associative arrays retain string keys.
                        let current = self.debug_eval(&format!("return {expression}"))?;
                        let indexed = matches!(&current, Value::Object(o) if matches!(&*o.borrow(), Object::Array(_) | Object::List(_) | Object::ByteArray(_) | Object::XmlList(_)));
                        if indexed {
                            let index = key.parse::<usize>().map_err(|_| "Invalid array index")?;
                            expression.push_str(&format!("[{index}]"));
                        } else {
                            expression.push_str(&format!("[\"{}\"]", key.replace('"', "\"\"")));
                        }
                    }
                    let result = self.debug_eval(&format!("return {expression}"))?;
                    Ok(json!([variable(
                        name,
                        &result,
                        request["children"].as_bool().unwrap_or(false)
                    )]))
                }
            }
            "evaluate" => {
                let code = request["code"].as_str().ok_or("Missing expression")?;
                let result = self.debug_eval(code)?;
                Ok(variable("", &result, true))
            }
            _ => Err("Unknown debugger operation".into()),
        }
    }
}

fn variable(name: &str, value: &Value, include_children: bool) -> Json {
    let mut kind = value.type_name();
    let mut children = Vec::new();
    let mut container = false;
    let val = match value {
        Value::Boolean(v) => json!(v),
        Value::Integer(v) => json!(v),
        Value::LongInteger(v) => json!(v.to_string()),
        Value::Float(v) => json!(v),
        Value::Double(v) => json!(v),
        Value::String(v) => json!(v.as_ref()),
        Value::Object(object) => {
            if let Ok(object) = object.try_borrow() {
                match &*object {
                    Object::Array(items) | Object::List(items) | Object::XmlList(items) => {
                        container = true;
                        for (i, v) in items.iter().take(10_000).enumerate() {
                            children.push((i.to_string(), v.clone()));
                        }
                    }
                    Object::AssocArray(aa) => {
                        container = true;
                        children = aa.sorted_entries().into_iter().take(10_000).collect();
                    }
                    Object::ByteArray(bytes) => {
                        container = true;
                        for (i, v) in bytes.iter().take(10_000).enumerate() {
                            children.push((i.to_string(), Value::Integer(i32::from(*v))));
                        }
                    }
                    Object::Node(node) => {
                        container = true;
                        for field in &node.fields {
                            children.push((field.name.to_string(), field.value.clone()));
                        }
                    }
                    Object::Boxed(inner) => return variable(name, inner, include_children),
                    _ => {}
                }
            }
            json!(value.type_name())
        }
        _ => Json::Null,
    };
    if matches!(value, Value::Uninitialized) {
        kind = "Uninitialized".into();
    }
    json!({"name":name, "type":kind,"value":val,"container":container,"count":children.len(),"children": if include_children { children.iter().map(|(name,v)| variable(name,v,false)).collect::<Vec<_>>() } else { vec![] }})
}

impl Vm {
    fn debug_scene(&self) -> Json {
        fn node(vm: &Vm, object: &ObjectRef, depth: usize) -> Json {
            if depth > 64 {
                return Json::Null;
            }
            let borrowed = object.borrow();
            let Object::Node(data) = &*borrowed else {
                return Json::Null;
            };
            let fields = data
                .fields
                .iter()
                .filter_map(|f| {
                    crate::json::format(&f.value)
                        .ok()
                        .and_then(|v| serde_json::from_str::<Json>(&v).ok())
                        .map(|v| (f.name.to_string(), v))
                })
                .collect::<serde_json::Map<String, Json>>();
            let children = data.children.clone();
            let name = data.subtype.to_string();
            let ancestry = data
                .ancestry
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>();
            let focus = data.has_focus;
            drop(borrowed);
            let rect = crate::sg::render::scene_bounds(vm, object);
            json!({"type":name,"ancestry":ancestry,"focused":focus,"fields":fields,"bounds":[rect.x,rect.y,rect.width,rect.height],"children":children.iter().map(|c|node(vm,c,depth+1)).collect::<Vec<_>>()})
        }
        let roots = self
            .sg
            .screens
            .iter()
            .filter(|s| s.shown && !s.closed)
            .filter_map(|s| s.scene.as_ref())
            .map(|root| node(self, root, 0))
            .collect::<Vec<_>>();
        json!({"roots":roots,"threads":self.threads.iter().filter(|t|!t.frames.is_empty()).count()+usize::from(!self.frames.is_empty()),"timeMs":self.now_ms()})
    }
}
