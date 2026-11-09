import { expect, it } from "vitest";
import { imageSize } from "../src/worker/imageHeaders";
it("reads PNG dimensions and rejects truncated or empty headers", () => {
  const png = new Uint8Array(24);
  png.set([137, 80, 78, 71, 13, 10, 26, 10]);
  const view = new DataView(png.buffer);
  view.setUint32(16, 124);
  view.setUint32(20, 84);
  expect(imageSize(png)).toEqual([124, 84]);
  expect(imageSize(png.subarray(0, 20))).toBeNull();
  view.setUint32(16, 0);
  expect(imageSize(png)).toBeNull();
});
it("reads JPEG dimensions after variable-length segments without looping on bad lengths", () => {
  expect(
    imageSize(Uint8Array.from([255, 216, 255, 224, 0, 4, 0, 0, 255, 192, 0, 7, 8, 0, 84, 0, 124])),
  ).toEqual([124, 84]);
  expect(imageSize(Uint8Array.from([255, 216, 255, 224, 0, 0]))).toBeNull();
});
