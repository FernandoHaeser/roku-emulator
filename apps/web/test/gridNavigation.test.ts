import { describe, expect, it } from "vitest";
import { moveFocus } from "../src/home/gridNavigation";

describe("moveFocus", () => {
  // 5 tiles, 3 columns:  0 1 2 / 3 4
  it("moves within rows and stops at edges", () => {
    expect(moveFocus(0, "Left", 5, 3)).toBe(0);
    expect(moveFocus(0, "Right", 5, 3)).toBe(1);
    expect(moveFocus(2, "Right", 5, 3)).toBe(2);
    expect(moveFocus(4, "Right", 5, 3)).toBe(4);
  });

  it("moves between rows, clamping into a short last row", () => {
    expect(moveFocus(1, "Down", 5, 3)).toBe(4);
    expect(moveFocus(2, "Down", 5, 3)).toBe(4);
    expect(moveFocus(4, "Down", 5, 3)).toBe(4);
    expect(moveFocus(3, "Up", 5, 3)).toBe(0);
    expect(moveFocus(0, "Up", 5, 3)).toBe(0);
  });

  it("ignores other keys and empty grids", () => {
    expect(moveFocus(1, "OK", 5, 3)).toBe(1);
    expect(moveFocus(3, "Down", 0, 3)).toBe(0);
  });
});
