import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

test("physical text editing and virtual keyboard share the active field", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
  await page.locator('input[type="file"]').setInputFiles({
    name: "physical-input.zip",
    mimeType: "application/zip",
    buffer: Buffer.from(
      packChannel(join(dirname(fileURLToPath(import.meta.url)), "fixtures/physical-input")),
    ),
  });
  const lines = page.locator('[role="log"] > div');
  await expect.poll(() => lines.allTextContents()).toContain("INPUT_READY");
  // H/O/R, space and punctuation must type text instead of activating shortcuts.
  await page.keyboard.type("demo h.o r ");
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", key: "é", code: "KeyE", text: "é" });
  await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: "é", code: "KeyE" });
  await expect
    .poll(async () =>
      (await lines.allTextContents())
        .filter((line) => line.startsWith("text="))
        .map((line) => JSON.parse(line.slice(5))),
    )
    .toContain("demo h.o r é");
  await page.keyboard.press("Home");
  await page.keyboard.press("Delete");
  await page.keyboard.press("End");
  await page.keyboard.press("Backspace");
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.type("X");
  await expect.poll(() => lines.allTextContents()).toContain('text="emo h.o rX "');
  await page.keyboard.press("ArrowDown");
  await expect.poll(() => lines.allTextContents()).toContain("PASSWORD_READY");
  await page.keyboard.type("test");
  await expect.poll(() => lines.allTextContents()).toContain('text="test"');
  await page.keyboard.press("Enter");
  await expect.poll(() => lines.allTextContents()).toContain("DIALOG_READY");
  await page.keyboard.type("1");
  for (let i = 0; i < 4; i++) {
    await page.keyboard.press("ArrowDown");
    await page.waitForTimeout(100);
  }
  await page.keyboard.press("Enter");
  await expect.poll(() => lines.allTextContents()).toContain('confirmed="test1"');
  await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
});
