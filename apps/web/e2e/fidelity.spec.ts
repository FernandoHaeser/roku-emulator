import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

/**
 * End-to-end fidelity: sideload a probe in the browser build (Worker + WASM
 * engine) and compare the console with the trace recorded on the real device.
 */

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const PROBES = ["language/hello", "language/numbers", "language/runtime-error"];

/** Same extraction rule as the oracle: text between the probe markers. */
function probeOutput(consoleLines: string[]): string {
  const begin = consoleLines.lastIndexOf("@@PROBE_BEGIN");
  const after = consoleLines.slice(begin + 1);
  const end = after.indexOf("@@PROBE_END");
  return end === -1 ? after.join("\n") : `${after.slice(0, end).join("\n")}\n`;
}

for (const probe of PROBES) {
  test(`${probe} matches the device golden`, async ({ page }) => {
    const probeDir = join(repoRoot, "fidelity", probe);
    const golden = readFileSync(join(probeDir, "golden", "console.txt"), "utf8");

    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await page.locator('input[type="file"]').setInputFiles({
      name: `${probe.replace("/", "-")}.zip`,
      mimeType: "application/zip",
      buffer: Buffer.from(packChannel(probeDir)),
    });
    await expect(page.getByText(/^(Finalizado|Crash)$/)).toBeVisible();

    const consoleLines = await page.locator('[role="log"] > div').allTextContents();
    expect(probeOutput(consoleLines)).toBe(golden);
  });
}
