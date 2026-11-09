import { describe, expect, it } from "vitest";
import { extractProbeOutput, hasProbeEnded, parseProbeSpec } from "../src/probe.js";

describe("extractProbeOutput", () => {
  it("returns text between markers with normalized line endings", () => {
    const raw = "boot noise\r\n@@PROBE_BEGIN\r\nhello\r\n 3  3.5\r\n@@PROBE_END\r\ntrailing\r\n";
    expect(extractProbeOutput(raw)).toEqual({ status: "ok", output: "hello\n 3  3.5\n" });
  });

  it("uses the last BEGIN marker (re-launch after stale output)", () => {
    const raw = "@@PROBE_BEGIN\nold\n@@PROBE_BEGIN\nnew\n@@PROBE_END\n";
    expect(extractProbeOutput(raw).output).toBe("new\n");
  });

  it("waits for the END line to be complete", () => {
    expect(hasProbeEnded("@@PROBE_BEGIN\nx\n@@PROBE_END")).toBe(false);
    expect(hasProbeEnded("@@PROBE_BEGIN\nx\n@@PROBE_END\r\n")).toBe(true);
  });

  it("does not treat non-fatal diagnostics as crashes", () => {
    const text =
      "------ Compiling dev 'p' ------\n@@PROBE_BEGIN\nBRIGHTSCRIPT: ERROR: roArray.Join: non-string value\n";
    expect(hasProbeEnded(text)).toBe(false);
  });

  it("flags crashes and incomplete runs", () => {
    const crash =
      "@@PROBE_BEGIN\nType Mismatch. (runtime error &h18)\nBrightScript Micro Debugger.\n";
    expect(extractProbeOutput(crash).status).toBe("crashed");
    expect(hasProbeEnded(crash)).toBe(true);
    expect(extractProbeOutput("@@PROBE_BEGIN\npartial\n").status).toBe("incomplete");
  });
});

describe("parseProbeSpec", () => {
  it("defaults to a console-only conformance probe", () => {
    expect(parseProbeSpec(undefined)).toEqual({
      kind: "conformance",
      steps: [],
      startDelayMs: 1000,
      settleMs: 0,
      capture: { appUi: false, screenshot: false },
    });
  });

  it("validates remote keys", () => {
    expect(parseProbeSpec({ steps: [{ key: "Down" }] }).steps).toEqual([
      { key: "Down", waitMs: 300 },
    ]);
    expect(() => parseProbeSpec({ steps: [{ key: "Jump" }] })).toThrow(/remote key/);
  });
});
