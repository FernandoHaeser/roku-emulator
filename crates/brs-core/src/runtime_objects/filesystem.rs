use super::*;
use crate::environment::Environment;

pub(super) fn call(vm: &mut Vm, object: &ObjectRef, method: &str, args: &[Value]) -> ResultValue {
    let path = text(args, 0);
    if object.borrow().class_name() == "roPath" {
        let mut saved = get_str(&state(object), "path");
        return match method {
            "setpath" => {
                store(object, "path", Value::string(path.clone()));
                ok(Environment::normalize_path(&path).is_some())
            }
            "appendpath" => {
                saved = format!(
                    "{}/{}",
                    saved.trim_end_matches('/'),
                    path.trim_start_matches('/')
                );
                let normalized = Environment::normalize_path(&saved);
                if let Some(path) = &normalized {
                    store(object, "path", Value::string(path.as_str()));
                }
                ok(normalized.is_some())
            }
            "tostr" | "getpath" => Ok(Value::string(saved)),
            "split" => {
                let mut parts = AssocArray::new();
                let (volume, rest) = saved.split_once(":/").unwrap_or(("", &saved));
                let (directory, file) = rest.rsplit_once('/').unwrap_or(("", rest));
                let (base, extension) = file.rsplit_once('.').unwrap_or((file, ""));
                parts.set("phy", Value::string(format!("{volume}:")));
                parts.set("dir", Value::string(format!("/{directory}")));
                parts.set("base", Value::string(base));
                parts.set("ext", Value::string(extension));
                Ok(Value::object(Object::AssocArray(parts)))
            }
            _ => Err(RuntimeError::member_not_found()),
        };
    }
    match method {
        "getvolumelist" => Ok(list([
            "pkg:".into(),
            "tmp:".into(),
            "cachefs:".into(),
            "common:".into(),
        ])),
        "getdirectorylisting" => Ok(list(vm.env.directory_listing(&path))),
        "find" | "findrecurse" => {
            let regex = super::regex_crypto::regex(&text(args, 1), "")
                .map_err(|_| RuntimeError::type_mismatch("Invalid regex".into()))?;
            Ok(list(
                vm.env
                    .directory_listing(&path)
                    .into_iter()
                    .filter(|name| regex.is_match(name).unwrap_or(false)),
            ))
        }
        "exists" => {
            let normalized = Environment::normalize_path(&path);
            ok(vm.env.read_file(&path).is_some()
                || normalized.is_some_and(|p| {
                    p.ends_with(":/")
                        || vm
                            .env
                            .directories
                            .contains(&format!("{}/", p.trim_end_matches('/')))
                        || !vm.env.directory_listing(&p).is_empty()
                }))
        }
        "stat" => {
            let normalized = Environment::normalize_path(&path).unwrap_or_default();
            let data = vm.env.read_file(&path);
            let directory = normalized.ends_with(":/")
                || vm
                    .env
                    .directories
                    .contains(&format!("{}/", normalized.trim_end_matches('/')))
                || !vm.env.directory_listing(&normalized).is_empty();
            if data.is_none() && !directory {
                return Ok(Value::Invalid);
            }
            let mut stat = AssocArray::new();
            stat.set(
                "type",
                Value::string(if directory { "directory" } else { "file" }),
            );
            stat.set("size", Value::Integer(data.map_or(0, |v| v.len()) as i32));
            stat.set("hidden", Value::Boolean(false));
            stat.set(
                "permissions",
                Value::string(if normalized.starts_with("pkg:") {
                    "r"
                } else {
                    "rw"
                }),
            );
            Ok(Value::object(Object::AssocArray(stat)))
        }
        "createdirectory" => {
            let path = Environment::normalize_path(&path);
            if let Some(path) =
                path.filter(|p| p.starts_with("tmp:/") || p.starts_with("cachefs:/"))
            {
                vm.env
                    .directories
                    .insert(format!("{}/", path.trim_end_matches('/')));
                ok(true)
            } else {
                ok(false)
            }
        }
        "deletedirectory" => {
            let Some(path) = Environment::normalize_path(&path) else {
                return ok(false);
            };
            if !vm.env.directory_listing(&path).is_empty() {
                return ok(false);
            }
            ok(vm
                .env
                .directories
                .remove(&format!("{}/", path.trim_end_matches('/'))))
        }
        "delete" => {
            let Some(path) = Environment::normalize_path(&path) else {
                return ok(false);
            };
            ok(vm.env.writable_files.remove(&path).is_some())
        }
        "copyfile" | "rename" => {
            if method == "rename"
                && !Environment::normalize_path(&path)
                    .is_some_and(|p| p.starts_with("tmp:/") || p.starts_with("cachefs:/"))
            {
                return ok(false);
            }
            let Some(data) = vm.env.read_file(&path) else {
                return ok(false);
            };
            let success = vm.env.write_file(&text(args, 1), data);
            if success && method == "rename" {
                if let Some(path) = Environment::normalize_path(&path) {
                    if path.starts_with("pkg:/") {
                        return ok(false);
                    }
                    vm.env.writable_files.remove(&path);
                }
            }
            ok(success)
        }
        "getvolumeinfo" => {
            let mut info = AssocArray::new();
            info.set("blocksize", Value::Integer(4096));
            info.set("blocks", Value::Integer(16384));
            info.set(
                "freeblocks",
                Value::Integer(
                    ((64 * 1024 * 1024
                        - vm.env.writable_files.values().map(Vec::len).sum::<usize>())
                        / 4096) as i32,
                ),
            );
            Ok(Value::object(Object::AssocArray(info)))
        }
        "setmessageport" | "setport" => Ok(Value::Invalid),
        _ => Err(RuntimeError::member_not_found()),
    }
}
