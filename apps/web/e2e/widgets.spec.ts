import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";
import { loadProbeSpec } from "../../../packages/oracle/src/probe";
const root = join(dirname(fileURLToPath(import.meta.url)), "../../../fidelity/widgets");
const keys: Record<string, string> = {
  Up: "ArrowUp",
  Down: "ArrowDown",
  Left: "ArrowLeft",
  Right: "ArrowRight",
  OK: "Enter",
  Back: "Backspace",
  Options: "o",
};
for (const probe of readdirSync(root).sort()) {
  test(`widgets/${probe} matches device navigation and binding events`, async ({ page }) => {
    const directory = join(root, probe);
    const spec = loadProbeSpec(directory);
    const golden = readFileSync(join(directory, "golden/console.txt"), "utf8");
    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await page.locator('input[type="file"]').setInputFiles({
      name: `${probe}.zip`,
      mimeType: "application/zip",
      buffer: Buffer.from(packChannel(directory)),
    });
    const output = page.locator('[role="log"] > div');
    await expect.poll(() => output.allTextContents()).toContain("@@PROBE_BEGIN");
    await page.waitForTimeout(spec.startDelayMs);
    for (const step of spec.steps) {
      await page.keyboard.press(keys[step.key] ?? step.key);
      await page.waitForTimeout(step.waitMs);
    }
    await expect.poll(() => output.allTextContents()).toContain("@@PROBE_END");
    const lines = await output.allTextContents();
    expect(
      lines
        .slice(lines.lastIndexOf("@@PROBE_BEGIN") + 1, lines.lastIndexOf("@@PROBE_END"))
        .join("\n") + "\n",
    ).toBe(golden);
  });
}
