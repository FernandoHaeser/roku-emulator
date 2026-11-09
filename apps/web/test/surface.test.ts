import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { DEVICE_UI_RESOLUTION } from "../src/config/surface";

it("surface follows the reference device profile's UI resolution", () => {
  const profile = JSON.parse(
    readFileSync(new URL("../../../profiles/H302X-16.0.4.json", import.meta.url), "utf8"),
  ) as Record<string, string>;
  expect(DEVICE_UI_RESOLUTION).toBe(profile["ui-resolution"]);
});
