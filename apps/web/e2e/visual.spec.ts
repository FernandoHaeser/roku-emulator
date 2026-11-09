import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { compareImages } from "../../../packages/oracle/src/diff";
import { packChannel } from "../../../packages/oracle/src/packager";
import { loadProbeSpec } from "../../../packages/oracle/src/probe";

/**
 * Renderer fidelity (F4): run every probe that has a device screenshot, grab
 * the channel canvas and pixel-diff it with the golden. expected/actual/diff
 * PNGs land in test-results/visual/<probe>/ for inspection.
 */

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const fidelityRoot = join(repoRoot, "fidelity");
const outputRoot = join(repoRoot, "apps/web/test-results/visual");

/** Remote key -> host keyboard key (see apps/web/src/keyboard.ts). */
const KEYBOARD: Record<string, string> = {
  Up: "ArrowUp",
  Down: "ArrowDown",
  Left: "ArrowLeft",
  Right: "ArrowRight",
  OK: "Enter",
  Back: "Backspace",
};

function findVisualProbes(dir: string): string[] {
  const found: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const path = join(dir, entry.name);
    const golden = join(path, "golden");
    if (existsSync(golden) && readdirSync(golden).some((f) => /^screenshot\.(png|jpg)$/.test(f))) {
      found.push(relative(fidelityRoot, path).split("\\").join("/"));
    } else if (!existsSync(join(path, "manifest"))) {
      found.push(...findVisualProbes(path));
    }
  }
  return found.sort();
}

async function consoleLines(page: Page): Promise<string[]> {
  return page.locator('[role="log"] > div').allTextContents();
}

/** Copies the channel canvas at the golden's size and returns PNG bytes. */
async function captureCanvas(page: Page, width: number, height: number): Promise<Buffer> {
  const base64 = await page.evaluate(
    ([w, h]) => {
      const source = document.querySelector<HTMLCanvasElement>(
        '[aria-label="Tela do dispositivo"] canvas',
      );
      if (!source) throw new Error("No channel canvas");
      const copy = document.createElement("canvas");
      copy.width = w;
      copy.height = h;
      const context = copy.getContext("2d")!;
      context.imageSmoothingQuality = "high";
      context.drawImage(source, 0, 0, w, h);
      return copy.toDataURL("image/png").split(",")[1]!;
    },
    [width, height] as const,
  );
  return Buffer.from(base64, "base64");
}

for (const probe of findVisualProbes(fidelityRoot)) {
  test(`${probe} renders like the device screenshot`, async ({ page }) => {
    const probeDir = join(fidelityRoot, probe);
    const spec = loadProbeSpec(probeDir);
    const goldenFile = readdirSync(join(probeDir, "golden")).find((f) =>
      /^screenshot\.(png|jpg)$/.test(f),
    )!;
    const expected = {
      data: readFileSync(join(probeDir, "golden", goldenFile)),
      extension: goldenFile.endsWith("png") ? ("png" as const) : ("jpg" as const),
    };

    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await page.locator('input[type="file"]').setInputFiles({
      name: `${probe.replaceAll("/", "-")}.zip`,
      mimeType: "application/zip",
      buffer: Buffer.from(packChannel(probeDir)),
    });
    await expect.poll(async () => (await consoleLines(page)).includes("@@PROBE_BEGIN")).toBe(true);
    await page.waitForTimeout(spec.startDelayMs);
    for (const step of spec.steps) {
      await page.keyboard.press(KEYBOARD[step.key] ?? step.key);
      await page.waitForTimeout(step.waitMs);
    }
    await page.waitForTimeout(spec.settleMs);

    // Golden size comes from the JPEG/PNG header via a decode in compareImages;
    // the device profile is 720p, so screenshots are 1280x720.
    const actual = { data: await captureCanvas(page, 1280, 720), extension: "png" as const };
    const result = compareImages(expected, actual, spec.visual.threshold);

    const out = join(outputRoot, probe);
    mkdirSync(out, { recursive: true });
    writeFileSync(join(out, `expected.${expected.extension}`), expected.data);
    writeFileSync(join(out, "actual.png"), actual.data);
    writeFileSync(join(out, "diff.png"), result.diffPng);
    test.info().annotations.push({
      type: "visual",
      description: `${(result.ratio * 100).toFixed(3)}% differ (budget ${(spec.visual.tolerance * 100).toFixed(3)}%)`,
    });
    console.log(`[visual] ${probe}: ${(result.ratio * 100).toFixed(3)}% differ`);
    expect(result.ratio).toBeLessThanOrEqual(spec.visual.tolerance);
  });
}
