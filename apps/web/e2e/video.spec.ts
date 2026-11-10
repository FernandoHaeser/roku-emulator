import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

/**
 * Video (F6): each fidelity/video probe runs with the shell's real player
 * (engine in external mode, <video> + Shaka) and must print the device's
 * state sequence exactly. The probes stream public test media, so they need
 * network access.
 *
 * Headless Chromium has no audio sink and stalls the clock of audible media
 * (currentTime never advances), so the test mutes media before playback.
 */
const root = join(dirname(fileURLToPath(import.meta.url)), "../../../fidelity/video");

for (const probe of readdirSync(root).sort()) {
  test(`video/${probe} matches the device playback events`, async ({ page }) => {
    test.setTimeout(90_000);
    const directory = join(root, probe);
    const golden = readFileSync(join(directory, "golden/console.txt"), "utf8");
    await page.addInitScript(() => {
      const play = HTMLMediaElement.prototype.play;
      HTMLMediaElement.prototype.play = function (this: HTMLMediaElement) {
        this.muted = true;
        return play.call(this);
      };
    });
    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await page.locator('input[type="file"]').setInputFiles({
      name: `${probe}.zip`,
      mimeType: "application/zip",
      buffer: Buffer.from(packChannel(directory)),
    });
    const output = page.locator('[role="log"] > div');
    await expect.poll(() => output.allTextContents(), { timeout: 60_000 }).toContain("@@PROBE_END");
    const lines = await output.allTextContents();
    expect(
      lines
        .slice(lines.lastIndexOf("@@PROBE_BEGIN") + 1, lines.lastIndexOf("@@PROBE_END"))
        .join("\n") + "\n",
    ).toBe(golden);
  });
}
