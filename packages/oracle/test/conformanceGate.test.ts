import { expect, it } from "vitest";
import { conformanceErrors } from "../src/conformanceGate.js";
const known = {
  "runtime/example": { status: "(ok)", detail: ["line 1", 'device:   "a"', 'emulator: "b"'] },
};
const failure =
  'FAIL  runtime/example (ok)\n      line 1\n      device:   "a"\n      emulator: "b"\n0 passed, 1 failed, 0 skipped\n';
it("accepts only the exact documented failure and runner status", () => {
  expect(conformanceErrors(failure, 1, ["runtime/example"], known)).toEqual([]);
  expect(conformanceErrors(failure.replace('"b"', '"c"'), 1, ["runtime/example"], known)).toContain(
    "Unexpected failure: runtime/example (ok)",
  );
  expect(conformanceErrors(failure, null, ["runtime/example"], known)).toContain(
    "Unexpected runner exit: null",
  );
});
it("rejects new failures, skipped and missing probes", () => {
  expect(conformanceErrors(failure, 1, ["runtime/example"], {})).toContain(
    "Unexpected failure: runtime/example (ok)",
  );
  expect(
    conformanceErrors(
      "SKIP  runtime/example (no golden)\n0 passed, 0 failed, 1 skipped",
      0,
      ["runtime/example"],
      {},
    ),
  ).toContain("Skipped probe: runtime/example");
  expect(conformanceErrors("0 passed, 0 failed, 0 skipped", 0, ["runtime/example"], {})).toContain(
    "Missing probe: runtime/example",
  );
});
it("requires review when a known failure is fixed and rejects truncated output", () => {
  expect(
    conformanceErrors(
      "PASS  runtime/example\n1 passed, 0 failed, 0 skipped",
      0,
      ["runtime/example"],
      known,
    ).length,
  ).toBeGreaterThan(0);
  expect(conformanceErrors("PASS  runtime/example", 0, ["runtime/example"], {})).toContain(
    "Runner summary is missing",
  );
});
