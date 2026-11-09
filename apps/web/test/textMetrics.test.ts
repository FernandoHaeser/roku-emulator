import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { FONTS } from "../src/worker/fontMetrics.generated";
import { lineHeight, measureText } from "../src/worker/textMetrics";

/** Same bounds as crates/brs-core/src/sg/text.rs tests, on the same goldens. */
function rows(probe: string) {
  const text = readFileSync(
    new URL(`../../../fidelity/scenegraph/${probe}/golden/console.txt`, import.meta.url),
    "utf8",
  );
  return [...text.matchAll(/^(\w+SystemFont) \| (.+) \|\s+(\d+)\s+x\s+(\d+)/gm)].map((m) => ({
    id: FONTS.findIndex(([name]) => name === m[1]),
    text: m[2]!,
    width: Number(m[3]),
    height: Number(m[4]),
  }));
}

describe.each([
  ["label-metrics", 1],
  ["label-metrics-hd", 2 / 3],
])("device text metrics (%s)", (probe, scale) => {
  it("predicts widths and line heights", () => {
    const data = rows(probe);
    expect(data).toHaveLength(60);
    let withinOne = 0;
    for (const row of data) {
      const font = { id: row.id, size: FONTS[row.id]![1] * scale, bold: false, scale };
      font.bold = FONTS[row.id]![0].includes("Bold");
      expect(lineHeight(font)).toBe(row.height);
      const error = Math.abs(Math.round(measureText(font, row.text)) - row.width);
      expect(error).toBeLessThanOrEqual(6);
      if (error <= 1) withinOne += 1;
    }
    expect(withinOne).toBeGreaterThanOrEqual(40);
  });
});
