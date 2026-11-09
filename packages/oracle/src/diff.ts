/** Trace comparison: exact text, XML tree (app-ui) and pixel diff with tolerance. */
import jpeg from "jpeg-js";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";
import type { Trace } from "./trace.js";

/** Compares an emulator trace against a golden trace recorded on the device. */

export interface TextMismatch {
  /** 1-based line where the texts first diverge. */
  line: number;
  expected: string | undefined;
  actual: string | undefined;
}

export interface ImageMismatch {
  reason: "size" | "pixels";
  detail: string;
}

export interface DiffReport {
  console: TextMismatch | null;
  appUi: TextMismatch | null;
  screenshot: ImageMismatch | null;
}

export interface DiffOptions {
  /** Max fraction (0..1) of differing pixels still considered a match. */
  pixelTolerance: number;
  /** Per-pixel color distance threshold passed to pixelmatch (0..1). */
  pixelThreshold: number;
}

export const DEFAULT_DIFF_OPTIONS: DiffOptions = { pixelTolerance: 0.001, pixelThreshold: 0.1 };

export function diffText(expected: string, actual: string): TextMismatch | null {
  if (expected === actual) return null;
  const expectedLines = expected.split("\n");
  const actualLines = actual.split("\n");
  const length = Math.max(expectedLines.length, actualLines.length);
  for (let index = 0; index < length; index++) {
    if (expectedLines[index] !== actualLines[index]) {
      return { line: index + 1, expected: expectedLines[index], actual: actualLines[index] };
    }
  }
  return null;
}

/** Removes formatting-only whitespace between tags so layout changes do not count. */
export function normalizeXml(xml: string): string {
  return xml.replace(/>\s+</g, ">\n<").trim();
}

interface RgbaImage {
  width: number;
  height: number;
  data: Uint8Array;
}

function decodeImage(data: Buffer, extension: "png" | "jpg"): RgbaImage {
  if (extension === "png") return PNG.sync.read(data);
  return jpeg.decode(data, { useTArray: true, formatAsRGBA: true });
}

export function diffImages(
  expected: NonNullable<Trace["screenshot"]>,
  actual: NonNullable<Trace["screenshot"]>,
  options: DiffOptions = DEFAULT_DIFF_OPTIONS,
): ImageMismatch | null {
  const a = decodeImage(expected.data, expected.extension);
  const b = decodeImage(actual.data, actual.extension);
  if (a.width !== b.width || a.height !== b.height) {
    return { reason: "size", detail: `${a.width}x${a.height} vs ${b.width}x${b.height}` };
  }
  const differing = pixelmatch(a.data, b.data, undefined, a.width, a.height, {
    threshold: options.pixelThreshold,
  });
  const ratio = differing / (a.width * a.height);
  if (ratio <= options.pixelTolerance) return null;
  return {
    reason: "pixels",
    detail: `${differing} pixels differ (${(ratio * 100).toFixed(3)}%, tolerance ${(options.pixelTolerance * 100).toFixed(3)}%)`,
  };
}

export function diffTraces(
  expected: Trace,
  actual: Trace,
  options: DiffOptions = DEFAULT_DIFF_OPTIONS,
): DiffReport {
  return {
    console: diffText(expected.console, actual.console),
    appUi:
      expected.appUi === undefined
        ? null
        : diffText(normalizeXml(expected.appUi), normalizeXml(actual.appUi ?? "")),
    screenshot:
      expected.screenshot === undefined
        ? null
        : actual.screenshot === undefined
          ? { reason: "size", detail: "emulator produced no screenshot" }
          : diffImages(expected.screenshot, actual.screenshot, options),
  };
}

export function isMatch(report: DiffReport): boolean {
  return report.console === null && report.appUi === null && report.screenshot === null;
}
