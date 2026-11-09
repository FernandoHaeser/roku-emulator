import { describe, expect, it } from "vitest";
import { layoutLines, rgbaToCss } from "../src/worker/displayList";

/** Monospace measure: 10 px per character. */
const measure = (value: string) => value.length * 10;

describe("rgbaToCss", () => {
  it("decodes signed 0xRRGGBBAA", () => {
    // 0xFF0000FF stored as a signed int32.
    expect(rgbaToCss(0xff0000ff | 0)).toBe("rgba(255, 0, 0, 1)");
    expect(rgbaToCss(0x00ff0080, 0.5)).toBe("rgba(0, 255, 0, 0.251)");
  });
});

describe("layoutLines", () => {
  it("keeps a single line and ellipsizes when it overflows", () => {
    expect(layoutLines("hello", 0, false, 0, measure)).toEqual(["hello"]);
    expect(layoutLines("hello world", 80, false, 0, measure)).toEqual(["hello..."]);
  });

  it("wraps words and caps maxLines", () => {
    expect(layoutLines("aa bb cc", 50, true, 0, measure)).toEqual(["aa bb", "cc"]);
    expect(layoutLines("aa bb cc dd ee", 50, true, 2, measure)).toEqual(["aa bb", "cc..."]);
    expect(layoutLines("a\nb", 100, true, 0, measure)).toEqual(["a", "b"]);
  });
});
