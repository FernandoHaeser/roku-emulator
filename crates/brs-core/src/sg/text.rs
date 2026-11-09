//! Label text metrics and layout measured on the reference device.
//!
//! System fonts: the device lays text out at FHD size and scales it for HD
//! channels (`scenegraph/label-metrics-hd`: widths x 2/3 rounded, line heights
//! x 2/3 rounded up). Widths are the sum of per-character advances, plus the
//! last character's bearing and kerning pairs (scaled from 55 px). Fonts that
//! are not system fonts (explicit `size`) reuse the Medium tables scaled.
//!
//! Package fonts (`scenegraph/label-package-font`): the string is shaped with
//! kerning at the output size (design size / design px per output px), each
//! glyph advance is floored to whole output pixels and the sum is scaled back
//! to design px and rounded. Line height is `ceil(ascent) + ceil(descent) + 1`
//! at the design size.
//!
//! Wrapped labels (`scenegraph/label-wrap`): `n` lines take
//! `n * lineHeight + (n - 1) * lineSpacing` (lineSpacing defaults to 12); a
//! word wider than the label never starts a new line.

use std::fmt;
use std::rc::Rc;

use rustybuzz::{Face, UnicodeBuffer};

use super::font_metrics::{
    ADVANCE_20, CHARS, END_OFFSET, FONTS, KERN_BOLD, KERN_REGULAR, KERN_SIZE,
};

/// Medium regular/bold: base tables for fonts with an explicit size.
const MEDIUM: usize = 4;
const MEDIUM_BOLD: usize = 5;
/// Fallback for characters outside the calibrated set.
const FALLBACK_CHAR: char = 'n';

/// TrueType/OpenType file loaded from the channel package.
#[derive(Clone)]
pub struct PackageFont {
    pub uri: Rc<str>,
    pub data: Rc<Vec<u8>>,
}

impl fmt::Debug for PackageFont {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PackageFont({})", self.uri)
    }
}

impl PartialEq for PackageFont {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontSpec {
    /// System font id (index into the calibration tables), when known.
    pub id: Option<usize>,
    /// Size in design pixels.
    pub size: f64,
    pub bold: bool,
    /// Font file from the package; takes precedence over the system tables.
    pub package: Option<PackageFont>,
    /// Design pixels per output pixel (1.5 for an FHD channel on a 720p UI).
    pub output_scale: f64,
}

/// Resolves a `font:<Name>` system font URI.
pub fn system_font(name: &str) -> Option<usize> {
    let name = name.strip_prefix("font:").unwrap_or(name);
    FONTS
        .iter()
        .position(|(font, _, _)| font.eq_ignore_ascii_case(name))
}

/// FHD size of a system font.
pub fn system_size(id: usize) -> f64 {
    f64::from(FONTS[id].1)
}

fn column(c: char) -> usize {
    CHARS
        .chars()
        .position(|known| known == c)
        .or_else(|| CHARS.chars().position(|known| known == FALLBACK_CHAR))
        .unwrap_or(0)
}

fn kern(bold: bool, a: char, b: char) -> f64 {
    let table: &[(u32, i8)] = if bold { &KERN_BOLD } else { &KERN_REGULAR };
    let key = (a as u32) << 16 | b as u32;
    table
        .binary_search_by_key(&key, |&(k, _)| k)
        .map_or(0.0, |i| f64::from(table[i].1) / 2.0)
}

/// Unrounded width in FHD pixels using the tables of `id`, scaled to `size`.
fn fhd_width(id: usize, size: f64, bold: bool, text: &str) -> f64 {
    let scale = size / system_size(id);
    let mut width = 0.0;
    let mut previous: Option<char> = None;
    let mut last = None;
    for c in text.chars() {
        let col = column(c);
        width += f64::from(ADVANCE_20[id][col]) / 20.0 * scale;
        if let Some(p) = previous {
            width += kern(bold, p, c) * size / KERN_SIZE;
        }
        previous = Some(c);
        last = Some(col);
    }
    if let Some(col) = last {
        width += f64::from(END_OFFSET[id][col]) * scale;
    }
    width.max(0.0)
}

/// Single-line advance width in design pixels.
pub fn text_width(font: &FontSpec, text: &str, fhd: bool) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    if let Some(package) = &font.package {
        if let Some(width) = package_width(package, font.size, font.output_scale, text) {
            return width;
        }
    }
    let (id, size) = match font.id {
        Some(id) => (id, system_size(id)),
        None => (
            if font.bold { MEDIUM_BOLD } else { MEDIUM },
            if fhd { font.size } else { font.size * 1.5 },
        ),
    };
    let width = fhd_width(id, size, font.bold, text).round();
    if fhd {
        width
    } else {
        (width * 2.0 / 3.0).round()
    }
}

/// Line height in design pixels.
pub fn line_height(font: &FontSpec, fhd: bool) -> f64 {
    if let Some((ascent, descent)) = font
        .package
        .as_ref()
        .and_then(|p| package_extents(p, font.size))
    {
        return ascent + descent + 1.0;
    }
    match font.id {
        Some(id) => {
            let height = f64::from(FONTS[id].2);
            if fhd {
                height
            } else {
                (height * 2.0 / 3.0).ceil()
            }
        }
        None => (font.size * 46.0 / 45.0).round(),
    }
}

/// Ascent of the first line's glyphs below the line top, in design px (0 for
/// system fonts, which hosts center in the line box).
pub fn ascent(font: &FontSpec) -> f64 {
    font.package
        .as_ref()
        .and_then(|p| package_extents(p, font.size))
        .map_or(0.0, |(ascent, _)| ascent)
}

/// Whole design px above and below the baseline for a package font.
fn package_extents(font: &PackageFont, size: f64) -> Option<(f64, f64)> {
    let face = Face::from_slice(&font.data, 0)?;
    let scale = size / f64::from(face.units_per_em());
    Some((
        (f64::from(face.ascender()) * scale).ceil(),
        (-f64::from(face.descender()) * scale).ceil(),
    ))
}

fn package_width(font: &PackageFont, size: f64, output_scale: f64, text: &str) -> Option<f64> {
    let face = Face::from_slice(&font.data, 0)?;
    let pixels = size / output_scale / f64::from(face.units_per_em());
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    let glyphs = rustybuzz::shape(&face, &[], buffer);
    let output: f64 = glyphs
        .glyph_positions()
        .iter()
        .map(|p| (f64::from(p.x_advance) * pixels).floor())
        .sum();
    Some((output * output_scale).round())
}

const ELLIPSIS: &str = "\u{2026}";

/// A Label broken into lines, each with its width (design px).
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub lines: Vec<(String, f64)>,
    pub line_height: f64,
    pub line_spacing: f64,
}

impl Layout {
    /// Width of the widest line.
    pub fn width(&self) -> f64 {
        self.lines.iter().map(|(_, w)| *w).fold(0.0, f64::max)
    }

    /// Height of the text block.
    pub fn height(&self) -> f64 {
        let count = self.lines.len().max(1) as f64;
        count * self.line_height + (count - 1.0) * self.line_spacing
    }
}

/// Breaks `text` into lines for a Label `width` wide (0 = unbounded).
pub fn layout(
    font: &FontSpec,
    text: &str,
    fhd: bool,
    width: f64,
    wrap: bool,
    max_lines: usize,
    line_spacing: f64,
) -> Layout {
    let measure = |value: &str| text_width(font, value, fhd);
    let mut lines: Vec<String> = Vec::new();
    if !wrap || width <= 0.0 {
        let single = text.replace('\n', " ");
        lines.push(if width > 0.0 {
            ellipsize(&single, width, &measure)
        } else {
            single
        });
    } else {
        for paragraph in text.split('\n') {
            let mut current = String::new();
            for word in paragraph.split(' ') {
                let candidate = if current.is_empty() {
                    word.to_string()
                } else {
                    format!("{current} {word}")
                };
                // A line already wider than the label (one long word) keeps
                // growing: the device does not break it.
                if !current.is_empty() && measure(&candidate) > width && measure(&current) <= width
                {
                    lines.push(std::mem::replace(&mut current, word.to_string()));
                } else {
                    current = candidate;
                }
            }
            lines.push(current);
        }
        if max_lines > 0 && lines.len() > max_lines {
            lines.truncate(max_lines);
            let last = lines.pop().unwrap_or_default();
            lines.push(ellipsize(&format!("{last}{ELLIPSIS}"), width, &measure));
        }
    }
    Layout {
        lines: lines
            .into_iter()
            .map(|line| {
                let w = measure(&line);
                (line, w)
            })
            .collect(),
        line_height: line_height(font, fhd),
        line_spacing,
    }
}

fn ellipsize(text: &str, width: f64, measure: &dyn Fn(&str) -> f64) -> String {
    if measure(text) <= width {
        return text.to_string();
    }
    let body = text.strip_suffix(ELLIPSIS).unwrap_or(text);
    let mut chars: Vec<char> = body.chars().collect();
    loop {
        let candidate = format!("{}{ELLIPSIS}", chars.iter().collect::<String>().trim_end());
        if chars.is_empty() || measure(&candidate) <= width {
            return candidate;
        }
        chars.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Device widths from scenegraph/label-metrics(-hd); see the probe goldens.
    fn device_rows(probe: &str) -> Vec<(String, String, f64, f64)> {
        let path = format!(
            "{}/../../fidelity/scenegraph/{probe}/golden/console.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(" | ").collect();
                let [font, text, size] = parts[..] else {
                    return None;
                };
                let (w, h) = size.trim().split_once(" x ")?;
                Some((
                    font.to_string(),
                    text.to_string(),
                    w.trim().parse().ok()?,
                    h.trim().parse().ok()?,
                ))
            })
            .filter(|(font, ..)| system_font(font).is_some())
            .collect()
    }

    fn check(probe: &str, fhd: bool) {
        let rows = device_rows(probe);
        assert_eq!(rows.len(), 60);
        let mut within_one = 0;
        for (font, text, width, height) in &rows {
            let id = system_font(font).unwrap();
            let spec = system(id);
            assert_eq!(line_height(&spec, fhd), *height, "{font} line height");
            let error = (text_width(&spec, text, fhd) - width).abs();
            assert!(error <= 6.0, "{font} {text:?}: off by {error}");
            if error <= 1.0 {
                within_one += 1;
            }
        }
        // Without the system font's hinting, ~3/4 of the strings land within 1 px.
        assert!(within_one >= 40, "{probe}: {within_one}/60 within 1 px");
    }

    fn system(id: usize) -> FontSpec {
        FontSpec {
            id: Some(id),
            size: system_size(id),
            bold: FONTS[id].0.contains("Bold"),
            package: None,
            output_scale: 1.5,
        }
    }

    fn inter_bold(size: f64) -> FontSpec {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fidelity/scenegraph/label-package-font/fonts/inter-bold.ttf"
        );
        FontSpec {
            id: None,
            size,
            bold: false,
            package: Some(PackageFont {
                uri: "pkg:/fonts/inter-bold.ttf".into(),
                data: Rc::new(std::fs::read(path).unwrap()),
            }),
            output_scale: 1.5,
        }
    }

    /// scenegraph/label-package-font: widths and heights match exactly.
    #[test]
    fn matches_device_package_font() {
        for (size, text, width, height) in [
            (24.0, "Package font 24 AVATAR Wave", 341.0, 31.0),
            (36.0, "Package font 36 AVATAR Wave", 521.0, 45.0),
            (60.0, "Package font 60 AVATAR Wave", 888.0, 75.0),
            (90.0, "Kerning: To Wa Ty 0123456789", 1355.0, 111.0),
        ] {
            let font = inter_bold(size);
            assert_eq!(text_width(&font, text, true), width, "{text}");
            assert_eq!(line_height(&font, true), height, "{text}");
        }
    }

    /// scenegraph/label-wrap: block height grows by lineHeight + lineSpacing.
    #[test]
    fn wraps_like_the_device() {
        let medium = system(system_font("MediumSystemFont").unwrap());
        let words = |n: usize| vec!["Avatar"; n].join(" ");
        for (n, height) in [(1, 38.0), (2, 88.0), (3, 138.0), (4, 188.0)] {
            let layout = layout(&medium, &words(n), true, 176.0, true, 0, 12.0);
            assert_eq!(layout.height(), height, "{n} lines");
        }
        let spaced = layout(&medium, &words(3), true, 176.0, true, 0, 10.0);
        assert_eq!(spaced.height(), 134.0);
        let capped = layout(&medium, &words(3), true, 176.0, true, 2, 12.0);
        assert_eq!(capped.height(), 88.0);
        assert!(capped.lines[1].0.ends_with(ELLIPSIS));
        // Narrower than any word: no wrapping at all.
        let narrow = layout(&medium, &words(4), true, 10.0, true, 0, 12.0);
        assert_eq!(narrow.lines.len(), 1);
        let package = layout(&inter_bold(40.0), &words(3), true, 232.0, true, 0, 12.0);
        assert_eq!(package.height(), 174.0);
    }

    #[test]
    fn matches_device_metrics_fhd() {
        check("label-metrics", true);
    }

    #[test]
    fn matches_device_metrics_hd() {
        check("label-metrics-hd", false);
    }
}
