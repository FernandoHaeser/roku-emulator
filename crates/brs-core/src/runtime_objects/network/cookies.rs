use super::*;
use url::Url;
fn entries(object: &ObjectRef) -> Vec<Value> {
    match state(object).get("cookies") {
        Some(Value::Object(o)) => match &*o.borrow() {
            Object::Array(a) => a.clone(),
            _ => vec![],
        },
        _ => vec![],
    }
}
fn values(value: &Value) -> Option<AssocArray> {
    match value {
        Value::Object(o) => match &*o.borrow() {
            Object::AssocArray(a) => Some(a.clone()),
            _ => None,
        },
        _ => None,
    }
}
fn matches(vm: &Vm, cookie: &AssocArray, domain: &str, path: &str, secure: bool) -> bool {
    let target = get_str(cookie, "domain");
    let target = target.trim_start_matches('.');
    let cookie_path = get_str(cookie, "path");
    (domain.is_empty()
        || domain == target
        || (!matches!(cookie.get("hostonly"), Some(Value::Boolean(true)))
            && domain.ends_with(&format!(".{target}"))))
        && (path == cookie_path
            || (path.starts_with(&cookie_path)
                && (cookie_path.ends_with('/')
                    || path.as_bytes().get(cookie_path.len()) == Some(&b'/'))))
        && (!matches!(cookie.get("secure"), Some(Value::Boolean(true))) || secure)
        && (get_int(cookie, "expiresms") == 0
            || get_int(cookie, "expiresms") > vm.epoch_ms() as i64)
}
pub(super) fn get(vm: &Vm, object: &ObjectRef, domain: &str, path: &str, secure: bool) -> Value {
    Value::object(Object::Array(
        entries(object)
            .into_iter()
            .filter(|value| values(value).is_some_and(|c| matches(vm, &c, domain, path, secure)))
            .collect(),
    ))
}
pub(super) fn header(vm: &Vm, object: &ObjectRef, url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let mut pairs = Vec::new();
    for entry in entries(object) {
        if let Some(cookie) = values(&entry) {
            if matches(
                vm,
                &cookie,
                url.host_str().unwrap_or(""),
                url.path(),
                url.scheme() == "https",
            ) {
                pairs.push(format!(
                    "{}={}",
                    get_str(&cookie, "name"),
                    get_str(&cookie, "value")
                ));
            }
        }
    }
    if pairs.is_empty() {
        None
    } else {
        Some(pairs.join("; "))
    }
}
pub(super) fn receive(vm: &Vm, object: &ObjectRef, result: &Json) {
    let state = state(object);
    if !matches!(state.get("cookiesenabled"), Some(Value::Boolean(true))) {
        return;
    }
    let Ok(url) = Url::parse(&get_str(&state, "url")) else {
        return;
    };
    let host = url.host_str().unwrap_or("");
    let mut jar = entries(object);
    'cookies: for entry in result["headersArray"].as_array().into_iter().flatten() {
        let Some(raw) = entry["set-cookie"].as_str() else {
            continue;
        };
        let mut parts = raw.split(';');
        let Some((name, value)) = parts.next().and_then(|s| s.trim().split_once('=')) else {
            continue;
        };
        let mut cookie = AssocArray::new();
        cookie.set("name", Value::string(name));
        cookie.set("value", Value::string(value));
        cookie.set("domain", Value::string(host));
        cookie.set("hostonly", Value::Boolean(true));
        cookie.set(
            "path",
            Value::string(url.path().rsplit_once('/').map_or("/", |(p, _)| {
                if p.is_empty() {
                    "/"
                } else {
                    p
                }
            })),
        );
        cookie.set("version", Value::Integer(0));
        for part in parts {
            let (name, value) = part.trim().split_once('=').unwrap_or((part.trim(), ""));
            match name.to_ascii_lowercase().as_str() {
                "domain" => {
                    let domain = value.trim_start_matches('.');
                    if host != domain && !host.ends_with(&format!(".{domain}")) {
                        continue 'cookies;
                    }
                    cookie.set("hostonly", Value::Boolean(false));
                    cookie.set("domain", Value::string(value));
                }
                "path" => cookie.set("path", Value::string(value)),
                "secure" => cookie.set("secure", Value::Boolean(true)),
                "max-age" => {
                    if let Ok(seconds) = value.parse::<i64>() {
                        cookie.set(
                            "expiresms",
                            Value::LongInteger(
                                (vm.epoch_ms() as i64).saturating_add(seconds.saturating_mul(1000)),
                            ),
                        );
                    }
                }
                "expires" => {
                    if let Ok(date) = httpdate::parse_http_date(value) {
                        if let Ok(time) = date.duration_since(std::time::UNIX_EPOCH) {
                            cookie.set("expiresms", Value::LongInteger(time.as_millis() as i64));
                        }
                    }
                }
                _ => {}
            }
        }
        jar.retain(|entry| {
            !values(entry).is_some_and(|c| {
                get_str(&c, "name") == get_str(&cookie, "name")
                    && get_str(&c, "domain") == get_str(&cookie, "domain")
                    && get_str(&c, "path") == get_str(&cookie, "path")
            })
        });
        jar.push(Value::object(Object::AssocArray(cookie)));
    }
    store(object, "cookies", Value::object(Object::Array(jar)));
}
