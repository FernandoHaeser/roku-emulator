import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

test("AA enumeration matches native growth, collisions, clear and UTF-8 keys", async ({ page }) => {
  const directory = join(
    dirname(fileURLToPath(import.meta.url)),
    "../../../fidelity/language/aa-enumeration",
  );
  const expected = readFileSync(join(directory, "golden/console.txt"), "utf8");
  await page.goto("/");
  await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
  await page.locator('input[type="file"]').setInputFiles({
    name: "aa-enumeration.zip",
    mimeType: "application/zip",
    buffer: Buffer.from(packChannel(directory)),
  });
  const output = page.locator('[role="log"] > div');
  await expect.poll(() => output.allTextContents()).toContain("@@PROBE_END");
  const lines = await output.allTextContents();
  expect(
    lines
      .slice(lines.lastIndexOf("@@PROBE_BEGIN") + 1, lines.lastIndexOf("@@PROBE_END"))
      .join("\n") + "\n",
  ).toBe(expected);
});
