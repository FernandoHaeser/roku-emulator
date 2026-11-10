import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

/**
 * F8 shell tools: the clickable remote drives a device probe exactly like the
 * keyboard, and the node inspector shows the live SceneGraph tree.
 */

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../../..");

/** Remote key -> accessible name of its on-screen button. */
const REMOTE_BUTTON: Record<string, string> = {
  Up: "Para cima",
  Down: "Para baixo",
  Left: "Para a esquerda",
  Right: "Para a direita",
  OK: "OK",
  Back: "Voltar",
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

test("clickable remote and node inspector on scenegraph/focus-keys", async ({ page }) => {
  const probe = "scenegraph/focus-keys";
  const probeDir = join(repoRoot, "fidelity", probe);
  const golden = readFileSync(join(probeDir, "golden", "console.txt"), "utf8");
  const spec = JSON.parse(readFileSync(join(probeDir, "probe.json"), "utf8")) as ProbeSpec;

  await page.goto("/");
  await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
  await page.locator('input[type="file"]').setInputFiles({
    name: "focus-keys.zip",
    mimeType: "application/zip",
    buffer: Buffer.from(packChannel(probeDir)),
  });
  await expect(page.getByLabel("Tela inicial")).toBeHidden();

  // Same steps and timing as the device recording, through the remote.
  const remote = page.getByRole("region", { name: "Controle remoto" });
  await expect.poll(async () => (await consoleLines(page)).includes("@@PROBE_BEGIN")).toBe(true);
  await page.waitForTimeout(spec.startDelayMs ?? 1000);
  for (const step of spec.steps) {
    await remote.getByRole("button", { name: REMOTE_BUTTON[step.key]!, exact: true }).click();
    await page.waitForTimeout(step.waitMs);
  }
  await expect.poll(async () => (await consoleLines(page)).includes("@@PROBE_END")).toBe(true);
  expect(probeOutput(await consoleLines(page))).toBe(golden);

  // Node inspector: live tree, fields of the selected node, highlight.
  const linesBefore = (await consoleLines(page)).length;
  await page.getByRole("tab", { name: "Nodes" }).click();
  const tree = page.getByRole("tree", { name: "Árvore de nodes" });
  await expect(tree.getByText("FocusScene")).toBeVisible();
  await tree.getByRole("button", { name: /outer/ }).click();
  const fields = page.getByLabel("Campos do node");
  await expect(fields.getByRole("rowheader", { name: "id", exact: true })).toBeVisible();
  await expect(fields.getByText('"outer"')).toBeVisible();
  // Branches below the root start collapsed.
  const outerItem = tree.locator("li", { has: page.getByRole("button", { name: /outer/ }) }).last();
  await outerItem.getByRole("button", { name: "Expandir" }).first().click();
  await tree.getByRole("button", { name: /inner/ }).click();
  await expect(fields.getByText("Group › Node")).toBeVisible();

  // Filter keeps matches and their ancestors only.
  await page.getByLabel("Filtrar nodes").fill("inner");
  await expect(tree.getByRole("button", { name: /inner/ })).toBeVisible();
  await expect(tree.getByText("FocusScene")).toBeVisible();

  // Polling (1 s) must not write to the console.
  await page.waitForTimeout(2_500);
  expect((await consoleLines(page)).length).toBe(linesBefore);

  // Home through the remote returns to the launcher.
  await remote.getByRole("button", { name: "Início", exact: true }).click();
  await expect(page.getByLabel("Tela inicial")).toBeVisible();
});

test("registry tools edit, persist and delete entries of a running channel", async ({ page }) => {
  const zip = Buffer.from(packChannel(join(repoRoot, "fidelity/scenegraph/focus-keys")));
  const load = async () => {
    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await page
      .locator('input[type="file"]')
      .setInputFiles({ name: "registry.zip", mimeType: "application/zip", buffer: zip });
    await expect(page.getByLabel("Tela inicial")).toBeHidden();
    await page.getByRole("tab", { name: "Registro", exact: true }).click();
  };
  await load();
  const registry = page.getByRole("region", { name: "Registro", exact: true });
  await registry.getByLabel("Seção", { exact: true }).fill("e2e");
  await registry.getByLabel("Chave", { exact: true }).fill("demo");
  await registry.getByLabel("Valor", { exact: true }).fill("private-test-value");
  await registry.getByRole("button", { name: "Salvar no registro" }).click();
  await expect(registry.getByLabel("Valor de e2e/demo", { exact: true })).toHaveValue(
    "private-test-value",
  );
  await expect(registry.getByLabel("Valor de e2e/demo", { exact: true })).toHaveAttribute(
    "type",
    "password",
  );
  await expect
    .poll(() =>
      page.evaluate(() =>
        Object.values(localStorage).some((value) => value.includes("private-test-value")),
      ),
    )
    .toBe(true);
  await load();
  await expect(registry.getByLabel("Valor de e2e/demo", { exact: true })).toHaveValue(
    "private-test-value",
  );
  await registry.getByRole("button", { name: "Excluir e2e/demo", exact: true }).click();
  await expect(registry.getByLabel("Valor de e2e/demo", { exact: true })).toHaveCount(0);
});
