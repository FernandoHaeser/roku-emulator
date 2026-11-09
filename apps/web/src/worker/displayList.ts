/**
 * Display list format produced by crates/brs-core/src/sg/render.rs, plus the
 * pure helpers the canvas renderer needs (colors, text layout). Kept free of
 * canvas types so it can be unit tested in Node.
 */

export type DisplayItem =
  /** Rectangle: x, y, w, h, rgba, opacity. */
  | ["r", number, number, number, number, number, number]
  /** Image (Poster): x, y, w, h, uri, opacity, blendRgba. */
  | ["i", number, number, number, number, string, number, number]
  /** Text (Label): x, y, w, h, text, size, bold, rgba, opacity, horizAlign, vertAlign, wrap, maxLines. */
  | [
      "t",
      number,
      number,
      number,
      number,
      string,
      number,
      boolean,
      number,
      number,
      string,
      string,
      boolean,
      number,
    ]
  /** Push clip rectangle: x, y, w, h. */
  | ["c", number, number, number, number]
  /** Pop clip. */
  | ["C"];

export interface DisplayList {
  /** Design resolution of the channel (ui_resolutions). */
  w: number;
  h: number;
  /** Scene backgroundColor (signed 0xRRGGBBAA). */
  bg: number;
  items: DisplayItem[];
}

export function parseDisplayList(json: string): DisplayList {
  return JSON.parse(json) as DisplayList;
}

/** SceneGraph colors are 0xRRGGBBAA stored as signed 32-bit integers. */
export function rgbaToCss(value: number, opacity = 1): string {
  const unsigned = value >>> 0;
  const red = unsigned >>> 24;
  const green = (unsigned >>> 16) & 0xff;
  const blue = (unsigned >>> 8) & 0xff;
  const alpha = ((unsigned & 0xff) / 255) * opacity;
  return `rgba(${red}, ${green}, ${blue}, ${Number(alpha.toFixed(4))})`;
}

const ELLIPSIS = "...";

/**
 * Splits `text` into the lines a Label shows.
 * - `wrap`: greedy word wrap to `width` (explicit newlines always break);
 *   `maxLines` > 0 caps the count and ellipsizes the last kept line.
 * - no wrap: one line, ellipsized when it overflows a positive `width`.
 */
export function layoutLines(
  text: string,
  width: number,
  wrap: boolean,
  maxLines: number,
  measure: (value: string) => number,
): string[] {
  if (!wrap) {
    const single = text.replace(/\n/g, " ");
    return [width > 0 ? ellipsize(single, width, measure) : single];
  }
  const lines: string[] = [];
  for (const paragraph of text.split("\n")) {
    let current = "";
    for (const word of paragraph.split(" ")) {
      const candidate = current ? `${current} ${word}` : word;
      if (current && width > 0 && measure(candidate) > width) {
        lines.push(current);
        current = word;
      } else {
        current = candidate;
      }
    }
    lines.push(current);
  }
  if (maxLines > 0 && lines.length > maxLines) {
    const kept = lines.slice(0, maxLines);
    kept[maxLines - 1] = ellipsize(`${kept[maxLines - 1]}${ELLIPSIS}`, width, measure);
    return kept;
  }
  return lines;
}

function ellipsize(text: string, width: number, measure: (value: string) => number): string {
  if (measure(text) <= width) return text;
  let end = text.endsWith(ELLIPSIS) ? text.length - ELLIPSIS.length : text.length;
  while (end > 0 && measure(`${text.slice(0, end).trimEnd()}${ELLIPSIS}`) > width) end -= 1;
  return `${text.slice(0, end).trimEnd()}${ELLIPSIS}`;
}
