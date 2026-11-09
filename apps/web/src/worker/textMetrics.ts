/**
 * Device text metrics for the Canvas2D host, mirroring
 * crates/brs-core/src/sg/text.rs (same generated tables), so line breaks,
 * ellipsis and alignment follow the device even though glyphs come from a
 * substitute font.
 */
import {
  ADVANCE_20,
  CHARS,
  END_OFFSET,
  FONTS,
  KERN_BOLD,
  KERN_REGULAR,
  KERN_SIZE,
} from "./fontMetrics.generated";

const MEDIUM = 4;
const MEDIUM_BOLD = 5;
const FALLBACK_COLUMN = CHARS.indexOf("n");
const COLUMNS = new Map([...CHARS].map((char, index) => [char.codePointAt(0)!, index]));
const KERNING = {
  regular: new Map(KERN_REGULAR),
  bold: new Map(KERN_BOLD),
};

export interface TextFont {
  /** System font id, or -1 for a font with an explicit size. */
  id: number;
  /** Size in design px (already multiplied by the node's scale). */
  size: number;
  bold: boolean;
  /** Design px per calibrated FHD px. */
  scale: number;
}

function resolve(font: TextFont): { id: number; fhdSize: number } {
  if (font.id >= 0 && font.id < FONTS.length) return { id: font.id, fhdSize: FONTS[font.id]![1] };
  return { id: font.bold ? MEDIUM_BOLD : MEDIUM, fhdSize: font.size / font.scale };
}

/** Advance width of one line in design px. */
export function measureText(font: TextFont, text: string): number {
  if (!text) return 0;
  const { id, fhdSize } = resolve(font);
  const ratio = fhdSize / FONTS[id]![1];
  const kerning = font.bold ? KERNING.bold : KERNING.regular;
  let width = 0;
  let previous = -1;
  let lastColumn = FALLBACK_COLUMN;
  for (const char of text) {
    const code = char.codePointAt(0)!;
    lastColumn = COLUMNS.get(code) ?? FALLBACK_COLUMN;
    width += (ADVANCE_20[id]![lastColumn]! / 20) * ratio;
    if (previous >= 0)
      width += ((kerning.get(previous * 65536 + code) ?? 0) / 2) * (fhdSize / KERN_SIZE);
    previous = code;
  }
  width += END_OFFSET[id]![lastColumn]! * ratio;
  return Math.round(Math.max(0, width)) * font.scale;
}

/** Line height in design px. */
export function lineHeight(font: TextFont): number {
  if (font.id >= 0 && font.id < FONTS.length) {
    const fhd = FONTS[font.id]![2];
    // HD channels: the device rounds the scaled line height up.
    return font.scale < 1 ? Math.ceil(fhd * font.scale - 1e-6) : fhd * font.scale;
  }
  return Math.round((font.size * 46) / 45);
}
