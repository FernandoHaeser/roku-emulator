import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

/**
 * SceneGraph in the browser: home screen, sideload, remote keys routed
 * through the worker, and Home returning to the launcher.
 */

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../../..");

/** Remote key -> host keyboard key (see apps/web/src/keyboard.ts). */
const KEYBOARD: Record<string, string> = {
  Up: "ArrowUp",
  Down: "ArrowDown",
  Left: "ArrowLeft",
  Right: "ArrowRight",
  OK: "Enter",
  Back: "Escape",
};

interface ProbeSpec {
  steps: { key: string; waitMs: number }[];
  startDelayMs?: number;
}

function consoleLines(page: Page): Promise<string[]> {
  return page.locator('[role="log"] > div').allTextContents();
}

function probeOutput(lines: string[]): string {
  const begin = lines.lastIndexOf("@@PROBE_BEGIN");
  const after = lines.slice(begin + 1);
  const end = after.indexOf("@@PROBE_END");
  return end === -1 ? after.join("\n") : `${after.slice(0, end).join("\n")}\n`;
}

async function sideload(page: Page, probe: string): Promise<void> {
  await page.locator('input[type="file"]').setInputFiles({
    name: `${probe.replace("/", "-")}.zip`,
    mimeType: "application/zip",
    buffer: Buffer.from(packChannel(join(repoRoot, "fidelity", probe))),
  });
}

test("home screen is shown at startup", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
  await expect(page.getByLabel("Tela inicial")).toBeVisible();
  await expect(page.getByRole("button", { name: "Instalar canal" })).toBeVisible();
});

test("scenegraph/focus-keys matches the device golden and Home returns to the launcher", async ({
  page,
}) => {
  const probe = "scenegraph/focus-keys";
  const probeDir = join(repoRoot, "fidelity", probe);
  const golden = readFileSync(join(probeDir, "golden", "console.txt"), "utf8");
  const spec = JSON.parse(readFileSync(join(probeDir, "probe.json"), "utf8")) as ProbeSpec;

  await page.goto("/");
  await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
  await sideload(page, probe);
  await expect(page.getByLabel("Tela inicial")).toBeHidden();

  // Same timing as the oracle recorder: keys start after BEGIN + startDelayMs.
  await expect.poll(async () => (await consoleLines(page)).includes("@@PROBE_BEGIN")).toBe(true);
  await page.waitForTimeout(spec.startDelayMs ?? 1000);
  for (const step of spec.steps) {
    await page.keyboard.press(KEYBOARD[step.key]!);
    await page.waitForTimeout(step.waitMs);
  }
  await expect.poll(async () => (await consoleLines(page)).includes("@@PROBE_END")).toBe(true);
  expect(probeOutput(await consoleLines(page))).toBe(golden);

  // Home leaves the channel; the dev channel tile now carries the manifest title.
  await page.keyboard.press("KeyH");
  await expect(page.getByLabel("Tela inicial")).toBeVisible();
  await expect(page.getByText("probe-scenegraph-focus-keys").first()).toBeVisible();

  // OK on the focused dev tile relaunches it.
  await page.keyboard.press("Enter");
  await expect(page.getByLabel("Tela inicial")).toBeHidden();
});
