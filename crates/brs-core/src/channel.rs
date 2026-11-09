//! Channel packages: manifest + files, loaded from a sideload zip or from a
//! directory listing supplied by the host.

use std::collections::BTreeMap;
use std::fmt;

use crate::md5::md5_hex;

/// Upper bounds that protect hosts from zip bombs and oversized packages.
const MAX_ENTRIES: usize = 10_000;
const MAX_TOTAL_UNCOMPRESSED: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct PackageError(pub String);

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Channel {
    /// manifest key -> value
    pub manifest: BTreeMap<String, String>,
    /// Normalized relative path ("source/main.brs") -> contents.
    pub files: BTreeMap<String, Vec<u8>>,
    /// MD5 of the original zip, when loaded from one.
    pub package_md5: Option<String>,
}

impl Channel {
    pub fn from_files(files: BTreeMap<String, Vec<u8>>) -> Result<Channel, PackageError> {
        let mut normalized = BTreeMap::new();
        for (path, data) in files {
            normalized.insert(normalize_path(&path)?, data);
        }
        let manifest_bytes = normalized
            .get("manifest")
            .ok_or_else(|| PackageError("Package has no manifest".into()))?;
        let manifest = parse_manifest(&String::from_utf8_lossy(manifest_bytes));
        Ok(Channel {
            manifest,
            files: normalized,
            package_md5: None,
        })
    }

    pub fn from_zip(bytes: &[u8]) -> Result<Channel, PackageError> {
        let files = read_zip(bytes)?;
        let mut channel = Channel::from_files(files)?;
        channel.package_md5 = Some(md5_hex(bytes));
        Ok(channel)
    }

    pub fn title(&self) -> String {
        self.manifest.get("title").cloned().unwrap_or_default()
    }

    /// "major.minor.build", as shown by the device.
    pub fn version(&self) -> String {
        let part = |key: &str| {
            self.manifest
                .get(key)
                .cloned()
                .unwrap_or_else(|| "0".into())
        };
        format!(
            "{}.{}.{}",
            part("major_version"),
            part("minor_version"),
            part("build_version")
        )
    }

    /// BrightScript sources under source/, in path order.
    pub fn source_files(&self) -> Vec<(String, String)> {
        self.files
            .iter()
            .filter(|(path, _)| {
                path.starts_with("source/") && path.to_lowercase().ends_with(".brs")
            })
            .map(|(path, data)| (path.clone(), String::from_utf8_lossy(data).into_owned()))
            .collect()
    }
}

pub fn parse_manifest(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

/// Rejects absolute paths and parent traversal (zip-slip); normalizes slashes.
pub fn normalize_path(path: &str) -> Result<String, PackageError> {
    let unified = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in unified.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(PackageError(format!("Unsafe path in package: {path}"))),
            other if other.contains(':') => {
                return Err(PackageError(format!("Unsafe path in package: {path}")))
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() || unified.starts_with('/') {
        return Err(PackageError(format!("Unsafe path in package: {path}")));
    }
    Ok(parts.join("/"))
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<usize, PackageError> {
    bytes
        .get(offset..offset + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
        .ok_or_else(corrupt)
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<usize, PackageError> {
    bytes
        .get(offset..offset + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or_else(corrupt)
}

fn corrupt() -> PackageError {
    PackageError("Invalid or corrupt zip archive".into())
}

/// Minimal zip reader (stored + deflate), driven by the central directory.
pub fn read_zip(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    const END_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    let search_start = bytes.len().saturating_sub(22 + 65_535);
    let end = (search_start..=bytes.len().saturating_sub(22))
        .rev()
        .find(|&i| bytes[i..i + 4] == END_SIGNATURE)
        .ok_or_else(corrupt)?;
    let entry_count = u16_at(bytes, end + 10)?;
    let mut cursor = u32_at(bytes, end + 16)?;
    if entry_count > MAX_ENTRIES {
        return Err(PackageError("Package has too many entries".into()));
    }

    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for _ in 0..entry_count {
        if u32_at(bytes, cursor)? != 0x0201_4b50 {
            return Err(corrupt());
        }
        let method = u16_at(bytes, cursor + 10)?;
        let compressed_size = u32_at(bytes, cursor + 20)?;
        let uncompressed_size = u32_at(bytes, cursor + 24)?;
        let name_length = u16_at(bytes, cursor + 28)?;
        let extra_length = u16_at(bytes, cursor + 30)?;
        let comment_length = u16_at(bytes, cursor + 32)?;
        let local_offset = u32_at(bytes, cursor + 42)?;
        let name_bytes = bytes
            .get(cursor + 46..cursor + 46 + name_length)
            .ok_or_else(corrupt)?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        cursor += 46 + name_length + extra_length + comment_length;

        if name.ends_with('/') {
            continue; // directory entry
        }
        total = total.saturating_add(uncompressed_size);
        if total > MAX_TOTAL_UNCOMPRESSED {
            return Err(PackageError(
                "Package exceeds the uncompressed size limit".into(),
            ));
        }
        if u32_at(bytes, local_offset)? != 0x0403_4b50 {
            return Err(corrupt());
        }
        let data_start = local_offset
            + 30
            + u16_at(bytes, local_offset + 26)?
            + u16_at(bytes, local_offset + 28)?;
        let data = bytes
            .get(data_start..data_start + compressed_size)
            .ok_or_else(corrupt)?;
        let contents = match method {
            0 => data.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, uncompressed_size)
                .map_err(|_| corrupt())?,
            other => {
                return Err(PackageError(format!(
                    "Unsupported zip compression method {other}"
                )))
            }
        };
        files.insert(normalize_path(&name)?, contents);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zip_slip_paths() {
        assert!(normalize_path("../evil").is_err());
        assert!(normalize_path("source/../../evil").is_err());
        assert!(normalize_path("/etc/passwd").is_err());
        assert!(normalize_path("C:/x").is_err());
        assert_eq!(
            normalize_path("source\\main.brs").unwrap(),
            "source/main.brs"
        );
    }

    #[test]
    fn manifest_parsing_ignores_comments() {
        let manifest = parse_manifest("# c\ntitle=My App\nmajor_version = 2\n\n");
        assert_eq!(manifest["title"], "My App");
        assert_eq!(manifest["major_version"], "2");
    }

    #[test]
    fn reads_a_stored_zip() {
        // Single stored entry "manifest" containing "title=x".
        let name = b"manifest";
        let data = b"title=x";
        let mut zip = Vec::new();
        zip.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        zip.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        zip.extend_from_slice(&(data.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(data.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(name);
        zip.extend_from_slice(data);
        let central = zip.len();
        zip.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        zip.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        zip.extend_from_slice(&(data.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(data.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&[0; 12]);
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(name);
        let central_size = zip.len() - central;
        zip.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        zip.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
        zip.extend_from_slice(&(central_size as u32).to_le_bytes());
        zip.extend_from_slice(&(central as u32).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());

        let channel = Channel::from_zip(&zip).unwrap();
        assert_eq!(channel.title(), "x");
        assert_eq!(channel.package_md5.as_deref().map(str::len), Some(32));
    }
}
