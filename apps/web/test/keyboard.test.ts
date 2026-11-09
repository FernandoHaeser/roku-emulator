import { describe, expect, it } from "vitest";
import { mapKeyboardCode } from "../src/keyboard";

describe("mapKeyboardCode", () => {
  it("maps arrows, OK and Back", () => {
    expect(mapKeyboardCode("ArrowUp")).toBe("Up");
    expect(mapKeyboardCode("Enter")).toBe("OK");
    expect(mapKeyboardCode("Backspace")).toBe("Back");
  });

  it("ignores unmapped keys", () => {
    expect(mapKeyboardCode("KeyZ")).toBeNull();
  });
});
