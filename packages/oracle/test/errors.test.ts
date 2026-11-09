import { describe, expect, it } from "vitest";
import { describeError } from "../src/errors.js";

describe("describeError", () => {
  it("surfaces the hidden cause of fetch failures with a hint", () => {
    const cause = Object.assign(new Error("connect EHOSTUNREACH 192.0.2.10:8060"), {
      code: "EHOSTUNREACH",
    });
    const message = describeError(new TypeError("fetch failed", { cause }));
    expect(message).toContain("cause: connect EHOSTUNREACH");
    expect(message).toContain("Local Network");
  });

  it("handles plain errors and non-errors", () => {
    expect(describeError(new Error("boom"))).toBe("Error: boom");
    expect(describeError("x")).toBe("x");
  });
});
