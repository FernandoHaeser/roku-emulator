//! What the channel sees of the device around it: manifest, package files,
//! the persistent registry and the reference device profile.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::value::{AssocArray, Object, Value};

/// Reference device (fidelity/runtime/device-info golden; AOC H302X,
/// Roku OS 16.0.4). Values that identify the physical unit are replaced by
/// stable emulator values.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    pub model: &'static str,
    pub model_display_name: &'static str,
    pub model_type: &'static str,
    pub os_major: &'static str,
    pub os_minor: &'static str,
    pub os_revision: &'static str,
    pub os_build: &'static str,
    pub locale: &'static str,
    pub country: &'static str,
    pub ui_width: u32,
    pub ui_height: u32,
    pub ui_name: &'static str,
    pub display_mode: &'static str,
    pub display_type: &'static str,
    pub aspect_ratio: &'static str,
    pub video_mode: &'static str,
    pub time_zone: &'static str,
    /// Minutes east of UTC (time-zone-offset in device-info is -180).
    pub utc_offset_minutes: i32,
}

impl Default for DeviceProfile {
    fn default() -> DeviceProfile {
        DeviceProfile {
            model: "H000X",
            model_display_name: "43S5135/78G",
            model_type: "TV",
            os_major: "16",
            os_minor: "0",
            os_revision: "4",
            os_build: "853",
            locale: "pt_BR",
            country: "BR",
            ui_width: 1280,
            ui_height: 720,
            ui_name: "HD",
            display_mode: "720p",
            display_type: "HDTV",
            aspect_ratio: "16x9",
            video_mode: "1080p",
            time_zone: "America/Sao_Paulo",
            utc_offset_minutes: -180,
        }
    }
}

/// Registry: section -> key -> value. Persisted by the host between runs.
pub type RegistryData = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Default)]
pub struct Environment {
    pub manifest: BTreeMap<String, String>,
    pub files: Rc<BTreeMap<String, Vec<u8>>>,
    pub profile: DeviceProfile,
    pub registry: RegistryData,
    /// Set when the registry changed since the host last persisted it.
    pub registry_dirty: bool,
}

impl Environment {
    pub fn manifest_value(&self, key: &str) -> String {
        self.manifest.get(key).cloned().unwrap_or_default()
    }

    /// Package file by device path ("pkg:/images/a.png").
    pub fn package_file(&self, uri: &str) -> Option<&Vec<u8>> {
        let path = uri.strip_prefix("pkg:/")?.trim_start_matches('/');
        self.files.get(path)
    }

    /// Registry as JSON (`{"section":{"key":"value"}}`) for host persistence.
    pub fn registry_json(&self) -> String {
        let mut root = AssocArray::new();
        for (section, entries) in &self.registry {
            let mut values = AssocArray::new();
            for (key, value) in entries {
                values.set(key, Value::string(value.as_str()));
            }
            root.set(section, Value::object(Object::AssocArray(values)));
        }
        crate::json::format(&Value::object(Object::AssocArray(root))).unwrap_or_default()
    }

    pub fn load_registry_json(&mut self, text: &str) {
        let Ok(Value::Object(root)) = crate::json::parse(text) else {
            return;
        };
        let Object::AssocArray(root) = &*root.borrow() else {
            return;
        };
        self.registry.clear();
        for (section, entries) in root.sorted_entries() {
            let Value::Object(entries) = entries else {
                continue;
            };
            let Object::AssocArray(entries) = &*entries.borrow() else {
                continue;
            };
            let section_map = self.registry.entry(section).or_default();
            for (key, value) in entries.sorted_entries() {
                if let Some(text) = value.as_str() {
                    section_map.insert(key, text.to_string());
                }
            }
        }
    }
}

/// Width/height of a PNG, JPEG or GIF from its header (Poster bitmap size).
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
        let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some((width, height));
    }
    if bytes.starts_with(b"GIF8") && bytes.len() >= 10 {
        let width = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let height = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        return Some((width, height));
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut position = 2;
        while position + 9 < bytes.len() {
            if bytes[position] != 0xFF {
                position += 1;
                continue;
            }
            let marker = bytes[position + 1];
            let length = u16::from_be_bytes([bytes[position + 2], bytes[position + 3]]) as usize;
            if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
                let height = u16::from_be_bytes([bytes[position + 5], bytes[position + 6]]) as u32;
                let width = u16::from_be_bytes([bytes[position + 7], bytes[position + 8]]) as u32;
                return Some((width, height));
            }
            position += 2 + length;
        }
    }
    None
}
