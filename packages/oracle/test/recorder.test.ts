import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { REPO_ROOT } from "../src/config.js";
import { SCRATCH_DIR, scratchDirFor } from "../src/recorder.js";

describe("scratchDirFor", () => {
  it("mirrors probes inside the repo", () => {
    expect(scratchDirFor(join(REPO_ROOT, "fidelity/language/hello"))).toBe(
      join(SCRATCH_DIR, "fidelity/language/hello"),
    );
  });

  it("confines probes outside the repo to .oracle/external", () => {
    for (const probe of ["/tmp/x/hello", join(REPO_ROOT, "../../etc"), REPO_ROOT]) {
      const dir = scratchDirFor(probe);
      expect(dir.startsWith(join(SCRATCH_DIR, "external") + "/")).toBe(true);
    }
  });
});
