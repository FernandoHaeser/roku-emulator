import { mkdtempSync, mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { unzipSync } from "fflate";
import { PNG } from "pngjs";
import { describe, expect, it } from "vitest";
import { diffImages, diffText } from "../src/diff.js";
import { packChannel } from "../src/packager.js";

function solidPng(width: number, height: number, rgb: [number, number, number]): Buffer {
  const png = new PNG({ width, height });
  for (let i = 0; i < width * height; i++) png.data.set([...rgb, 255], i * 4);
  return PNG.sync.write(png);
}

describe("diffText", () => {
  it("reports the first diverging line", () => {
    expect(diffText("a\nb\nc", "a\nb\nc")).toBeNull();
    expect(diffText("a\nb\nc", "a\nX\nc")).toEqual({ line: 2, expected: "b", actual: "X" });
    expect(diffText("a", "a\nextra")).toEqual({ line: 2, expected: undefined, actual: "extra" });
  });
});

describe("diffImages", () => {
  it("matches identical images and flags size or pixel differences", () => {
    const red = { data: solidPng(4, 4, [255, 0, 0]), extension: "png" as const };
    const blue = { data: solidPng(4, 4, [0, 0, 255]), extension: "png" as const };
    const small = { data: solidPng(2, 2, [255, 0, 0]), extension: "png" as const };
    expect(diffImages(red, red)).toBeNull();
    expect(diffImages(red, blue)?.reason).toBe("pixels");
    expect(diffImages(red, small)?.reason).toBe("size");
  });
});

describe("packChannel", () => {
  const makeProbe = () => {
    const dir = mkdtempSync(join(tmpdir(), "probe-"));
    mkdirSync(join(dir, "source"));
    mkdirSync(join(dir, "golden"));
    writeFileSync(join(dir, "manifest"), "title=probe\n");
    writeFileSync(join(dir, "source", "main.brs"), "sub Main()\nend sub\n");
    writeFileSync(join(dir, "probe.json"), "{}");
    writeFileSync(join(dir, "golden", "console.txt"), "x");
    return dir;
  };

  it("zips channel files and skips oracle metadata", () => {
    const entries = Object.keys(unzipSync(packChannel(makeProbe()))).sort();
    // Directory entries are required by the Roku installer.
    expect(entries).toEqual(["manifest", "source/", "source/main.brs"]);
  });

  it("rejects symlinks and missing manifest", () => {
    const dir = makeProbe();
    symlinkSync("/etc/hosts", join(dir, "source", "leak.brs"));
    expect(() => packChannel(dir)).toThrow(/Symlinks/);
    expect(() => packChannel(mkdtempSync(join(tmpdir(), "empty-")))).toThrow(/manifest/);
  });
});
