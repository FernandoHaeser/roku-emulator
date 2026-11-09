import { describe, expect, it } from "vitest";
import { LineBuffer } from "../src/worker/lineBuffer";

describe("LineBuffer", () => {
  it("emits only complete lines and keeps the partial tail", () => {
    const buffer = new LineBuffer();
    expect(buffer.push(" 1")).toEqual([]);
    expect(buffer.push("               2\nab")).toEqual([" 1               2"]);
    expect(buffer.push("\n\n")).toEqual(["ab", ""]);
    expect(buffer.push("Brightscript Debugger> ")).toEqual([]);
    expect(buffer.flush()).toEqual(["Brightscript Debugger> "]);
    expect(buffer.flush()).toEqual([]);
  });
});
