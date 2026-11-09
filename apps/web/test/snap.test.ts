import { describe, expect, it } from "vitest";
import { snapToOutputPixels } from "../src/worker/canvasRenderer";

// Values measured on the device (fidelity/scenegraph/render-rects-*).
describe("snapToOutputPixels", () => {
  const fhd = 2 / 3;
  it.each([
    [40, 100, 26, 66],
    [241, 100, 160, 66],
    [640, 101, 426, 67],
    [1240, 102, 826, 68],
  ])("FHD x=%d w=%d -> %d+%d", (x, w, outX, outW) => {
    const box = snapToOutputPixels(x, 0, w, 0, fhd, fhd);
    expect([box.x, box.width]).toEqual([outX, outW]);
  });
  it("keeps HD fractional starts and sizes floored", () => {
    const box = snapToOutputPixels(705 + 2 / 3, 345, 100 + 1 / 3, 80 + 1 / 3, 1, 1);
    expect(box).toEqual({ x: 705, y: 345, width: 100, height: 80 });
  });
});
