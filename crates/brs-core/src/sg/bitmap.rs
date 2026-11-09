//! Poster bitmap loading: dimensions from the PNG/JPEG header and the
//! device's load fields.
//!
//! Device behavior:
//! - `scenegraph/render-composite`: bitmaps are shown 1:1 in output pixels,
//!   so `bitmapWidth/Height` are in design coordinates (a 124x84 PNG in an FHD
//!   channel on the 720p UI reports 186x126).
//! - `scenegraph/poster-ninepatch`: a `.9.png` reports its size without the
//!   1 px marker border.
//! - `scenegraph/poster-remote`: http(s) images report `loading` (observers
//!   run during the `uri` assignment), then `ready` with the bitmap size or
//!   `failed`. A URI that already finished loading goes `loading` -> result
//!   within the assignment.
//!
//! `pkg:/` images load synchronously. Remote ones are fetched by the host:
//! it drains [`take_requests`] and answers with [`complete`]; results are
//! applied by the render thread ([`apply_results`]) so observers run there.

use std::collections::{HashMap, VecDeque};
use std::rc::Weak;

use super::runtime::set_field;
use super::with_node;
use crate::error::RuntimeError;
use crate::value::{Object, ObjectRef, Value};
use crate::vm::Vm;

/// Remote image state shared by every Poster using the same URI.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Remote {
    Loading,
    /// Pixel size of the file.
    Ready(u32, u32),
    Failed,
}

#[derive(Default)]
pub struct Images {
    remote: HashMap<String, Remote>,
    /// URIs the host still has to fetch.
    requests: Vec<String>,
    /// Host answers waiting for the render thread.
    results: VecDeque<(String, Option<(u32, u32)>)>,
    /// Posters showing a URI that is still loading.
    waiting: Vec<(Weak<std::cell::RefCell<Object>>, String)>,
}

impl Images {
    pub fn has_results(&self) -> bool {
        !self.results.is_empty()
    }
}

/// URIs to fetch since the last call.
pub fn take_requests(vm: &mut Vm) -> Vec<String> {
    std::mem::take(&mut vm.sg.images.requests)
}

/// Host result for a remote image: its pixel size, or `None` on failure.
pub fn complete(vm: &mut Vm, uri: &str, size: Option<(u32, u32)>) {
    vm.sg.images.results.push_back((uri.to_string(), size));
}

/// Applies host results to the Posters waiting for them (render thread).
pub fn apply_results(vm: &mut Vm) -> Result<(), RuntimeError> {
    while let Some((uri, size)) = vm.sg.images.results.pop_front() {
        let state = size.map_or(Remote::Failed, |(w, h)| Remote::Ready(w, h));
        vm.sg.images.remote.insert(uri.clone(), state);
        let mut posters = Vec::new();
        vm.sg.images.waiting.retain(|(poster, waiting)| {
            if *waiting != uri {
                return poster.strong_count() > 0;
            }
            posters.extend(poster.upgrade());
            false
        });
        for poster in posters {
            // Skip Posters whose uri changed while the image was loading.
            if with_node(&poster, |n| n.text("uri")) == uri {
                finish(vm, &poster, &uri, state)?;
            }
        }
    }
    Ok(())
}

/// Width and height in pixels from a PNG or baseline/progressive JPEG header.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.get(12..16) == Some(b"IHDR") {
        let read = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
        return Some((read(16)?, read(20)?));
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut at = 2;
        while at + 9 < bytes.len() {
            if bytes[at] != 0xFF {
                return None;
            }
            let marker = bytes[at + 1];
            let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
            // SOF0..SOF15 except DHT (C4), JPG (C8) and DAC (CC).
            if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
                let height = u16::from_be_bytes([bytes[at + 5], bytes[at + 6]]);
                let width = u16::from_be_bytes([bytes[at + 7], bytes[at + 8]]);
                return Some((u32::from(width), u32::from(height)));
            }
            at += 2 + length;
        }
    }
    None
}

/// Design pixels per output pixel (vertical), e.g. 1.5 for FHD on a 720p UI.
pub(super) fn design_per_output(vm: &Vm) -> f64 {
    let design = if vm.sg.design_height > 0.0 {
        vm.sg.design_height
    } else {
        1080.0
    };
    design / f64::from(vm.env.profile.ui_height)
}

/// Bitmap size without the 1 px 9-patch marker border.
fn content_size(uri: &str, (width, height): (u32, u32)) -> (u32, u32) {
    if is_nine_patch(uri) {
        (width.saturating_sub(2), height.saturating_sub(2))
    } else {
        (width, height)
    }
}

pub fn is_nine_patch(uri: &str) -> bool {
    let path = uri.split(['?', '#']).next().unwrap_or(uri);
    path.to_ascii_lowercase().ends_with(".9.png")
}

fn set_size(
    vm: &mut Vm,
    poster: &ObjectRef,
    uri: &str,
    size: Option<(u32, u32)>,
) -> Result<(), RuntimeError> {
    let scale = design_per_output(vm);
    let (width, height) = size.map_or((0.0, 0.0), |size| {
        let (w, h) = content_size(uri, size);
        (f64::from(w) * scale, f64::from(h) * scale)
    });
    set_field(vm, poster, "bitmapWidth", Value::Float(width as f32))?;
    set_field(vm, poster, "bitmapHeight", Value::Float(height as f32))
}

fn finish(vm: &mut Vm, poster: &ObjectRef, uri: &str, state: Remote) -> Result<(), RuntimeError> {
    match state {
        Remote::Ready(w, h) => {
            set_size(vm, poster, uri, Some((w, h)))?;
            set_field(vm, poster, "loadStatus", Value::string("ready"))
        }
        Remote::Failed => set_field(vm, poster, "loadStatus", Value::string("failed")),
        Remote::Loading => Ok(()),
    }
}

/// Updates `loadStatus` and `bitmapWidth/Height` after `uri` changes.
pub fn load(vm: &mut Vm, poster: &ObjectRef, uri: &str) -> Result<(), RuntimeError> {
    if uri.is_empty() {
        set_size(vm, poster, uri, None)?;
        return set_field(vm, poster, "loadStatus", Value::string("none"));
    }
    if uri.starts_with("pkg:/") {
        let size = vm.env.read_file(uri).as_deref().and_then(image_size);
        set_size(vm, poster, uri, size)?;
        let status = if size.is_some() { "ready" } else { "failed" };
        return set_field(vm, poster, "loadStatus", Value::string(status));
    }
    set_size(vm, poster, uri, None)?;
    set_field(vm, poster, "loadStatus", Value::string("loading"))?;
    match vm.sg.images.remote.get(uri).copied() {
        Some(Remote::Loading) => {}
        Some(state) => return finish(vm, poster, uri, state),
        None => {
            vm.sg.images.remote.insert(uri.to_string(), Remote::Loading);
            vm.sg.images.requests.push(uri.to_string());
        }
    }
    vm.sg
        .images
        .waiting
        .push((std::rc::Rc::downgrade(poster), uri.to_string()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{content_size, image_size, is_nine_patch};

    #[test]
    fn nine_patch_size_drops_the_border() {
        assert!(is_nine_patch("pkg:/images/panel.9.png"));
        assert!(is_nine_patch("https://example.com/a.9.PNG?v=2"));
        assert!(!is_nine_patch("pkg:/images/panel.png"));
        assert_eq!(content_size("pkg:/images/panel.9.png", (50, 50)), (48, 48));
        assert_eq!(content_size("pkg:/images/panel.png", (50, 50)), (50, 50));
    }

    #[test]
    fn reads_png_header() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fidelity/scenegraph/render-composite/images/quad.png"
        );
        assert_eq!(image_size(&std::fs::read(path).unwrap()), Some((124, 84)));
    }

    #[test]
    fn reads_jpeg_sof() {
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0, 0];
        jpeg.extend([0xFF, 0xC0, 0x00, 0x11, 0x08, 0x02, 0xD0, 0x05, 0x00, 0x03]);
        jpeg.extend([0; 16]);
        assert_eq!(image_size(&jpeg), Some((1280, 720)));
    }
}
