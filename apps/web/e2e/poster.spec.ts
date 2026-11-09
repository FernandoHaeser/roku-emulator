import { createServer } from "node:http";
import { once } from "node:events";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

test("remote Posters report load results through WASM, including cached and detached nodes", async ({
  page,
}) => {
  const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
  const image = await readFile(join(root, "fidelity/scenegraph/poster-ninepatch/images/panel.png"));
  let requests = 0;
  const server = createServer((req, res) => {
    requests++;
    if (req.url === "/missing.png") {
      res.writeHead(404);
      res.end();
    } else {
      res.writeHead(200, { "Content-Type": "image/png" });
      res.end(image);
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address() as { port: number };
  const scratch = await mkdtemp(join(tmpdir(), "roku-poster-"));
  try {
    await cp(join(root, "fidelity/scenegraph/poster-remote"), scratch, { recursive: true });
    const file = join(scratch, "components/ProbeScene.xml");
    await writeFile(
      file,
      (await readFile(file, "utf8")).replace(
        "https://raw.githubusercontent.com/FernandoHaeser/roku-emulator/develop/docs/assets/",
        `http://127.0.0.1:${address.port}/`,
      ),
    );
    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await page.locator('input[type="file"]').setInputFiles({
      name: "poster.zip",
      mimeType: "application/zip",
      buffer: Buffer.from(packChannel(scratch)),
    });
    const output = page.locator('[role="log"] > div');
    await expect.poll(() => output.allTextContents()).toContain("done");
    const lines = await output.allTextContents();
    expect(lines.some((line) => line.startsWith("first -> ready "))).toBe(true);
    expect(lines.some((line) => line.startsWith("missing -> failed "))).toBe(true);
    expect(lines.some((line) => line.startsWith("detached -> ready "))).toBe(true);
    expect(requests).toBeGreaterThanOrEqual(2);
  } finally {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
    await rm(scratch, { recursive: true, force: true });
  }
});
