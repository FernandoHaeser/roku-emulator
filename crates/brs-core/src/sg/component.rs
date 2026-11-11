//! XML component definitions (`components/**/*.xml`): `<interface>` fields
//! and functions, `<script>` includes, `<children>` and the `extends` chain.

use std::rc::Rc;

use super::FieldKind;
use crate::channel::Channel;
use crate::xml::{self, XmlElement};

#[derive(Debug, Clone)]
pub struct InterfaceField {
    pub id: String,
    pub kind: FieldKind,
    /// Initial value as written in the XML (converted when the node is built).
    pub value: Option<String>,
    pub on_change: Option<String>,
    pub always_notify: bool,
    /// `alias="child.field"` links the interface to a child field before init.
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Script {
    /// Device path ("pkg:/components/Foo.brs" or the XML path for inline code).
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ComponentDef {
    pub name: String,
    pub extends: String,
    pub file: String,
    pub fields: Vec<InterfaceField>,
    pub functions: Vec<String>,
    pub scripts: Vec<Script>,
    pub children: Option<Rc<XmlElement>>,
    /// Filled after compilation.
    pub scope: u32,
    pub init: Option<u32>,
}

/// Parses every component XML in the package. Errors are device-style
/// diagnostics (the device skips broken components and keeps going).
pub fn load_components(channel: &Channel) -> (Vec<ComponentDef>, Vec<String>) {
    let mut components = Vec::new();
    let mut errors = Vec::new();
    for (path, data) in &channel.files {
        if !path.starts_with("components/") || !path.to_lowercase().ends_with(".xml") {
            continue;
        }
        let text = String::from_utf8_lossy(data);
        let device_path = format!("pkg:/{path}");
        match parse_component(channel, path, &device_path, &text) {
            Ok(Some(def)) => components.push(def),
            Ok(None) => {}
            Err(message) => errors.push(message),
        }
    }
    (components, errors)
}

fn parse_component(
    channel: &Channel,
    path: &str,
    device_path: &str,
    text: &str,
) -> Result<Option<ComponentDef>, String> {
    let root = xml::parse(text).map_err(|e| {
        format!(
            "BRIGHTSCRIPT: ERROR: XML parse error in {device_path}({}): {}",
            e.line, e.message
        )
    })?;
    if !root.name.eq_ignore_ascii_case("component") {
        return Ok(None);
    }
    let name = root.attribute("name").unwrap_or_default().to_string();
    if name.is_empty() {
        return Err(format!(
            "BRIGHTSCRIPT: ERROR: Component in {device_path} has no name"
        ));
    }
    let extends = root.attribute("extends").unwrap_or("Group").to_string();
    let base_dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);

    let mut fields = Vec::new();
    let mut functions = Vec::new();
    if let Some(interface) = root.first_element("interface") {
        for element in interface.elements() {
            if element.name.eq_ignore_ascii_case("field") {
                let Some(id) = element.attribute_ci("id") else {
                    continue;
                };
                fields.push(InterfaceField {
                    id: id.to_string(),
                    kind: FieldKind::parse(element.attribute_ci("type").unwrap_or("string")),
                    value: element.attribute_ci("value").map(str::to_string),
                    on_change: element.attribute_ci("onChange").map(str::to_string),
                    always_notify: element
                        .attribute_ci("alwaysNotify")
                        .is_some_and(|v| v.eq_ignore_ascii_case("true")),
                    alias: element.attribute_ci("alias").map(str::to_string),
                });
            } else if element.name.eq_ignore_ascii_case("function") {
                if let Some(function) = element.attribute_ci("name") {
                    functions.push(function.to_string());
                }
            }
        }
    }

    let mut scripts = Vec::new();
    for element in root
        .elements()
        .filter(|e| e.name.eq_ignore_ascii_case("script"))
    {
        match element.attribute_ci("uri") {
            Some(uri) => {
                let relative = resolve_uri(base_dir, uri);
                let data = channel.files.get(&relative).ok_or_else(|| {
                    format!(
                        "BRIGHTSCRIPT: ERROR: Unable to load script {uri} for component {name}: {device_path}"
                    )
                })?;
                scripts.push(Script {
                    path: format!("pkg:/{relative}"),
                    text: String::from_utf8_lossy(data).into_owned(),
                });
            }
            None => {
                let inline = element.text();
                if !inline.trim().is_empty() {
                    scripts.push(Script {
                        path: device_path.to_string(),
                        text: inline,
                    });
                }
            }
        }
    }

    Ok(Some(ComponentDef {
        name,
        extends,
        file: device_path.to_string(),
        fields,
        functions,
        scripts,
        children: root.first_element("children").cloned(),
        scope: 0,
        init: None,
    }))
}

/// "pkg:/a/b.brs" -> "a/b.brs"; relative paths resolve against `base_dir`.
fn resolve_uri(base_dir: &str, uri: &str) -> String {
    let uri = uri.trim();
    if let Some(absolute) = uri.strip_prefix("pkg:/") {
        return absolute.trim_start_matches('/').to_string();
    }
    let mut parts: Vec<&str> = base_dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in uri.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_script_uris() {
        assert_eq!(resolve_uri("components", "Foo.brs"), "components/Foo.brs");
        assert_eq!(
            resolve_uri("components/a", "../b/C.brs"),
            "components/b/C.brs"
        );
        assert_eq!(
            resolve_uri("components", "pkg:/source/x.brs"),
            "source/x.brs"
        );
    }
}
