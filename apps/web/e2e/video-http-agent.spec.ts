import { createServer } from "node:http";
import { once } from "node:events";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

for (const mode of ["agent", "content"] as const) {
  test(`Video ${mode} headers match the device on HLS manifests and segments`, async ({ page }) => {
    const fixture = join(dirname(fileURLToPath(import.meta.url)), "fixtures/video-http-agent");
    const playlist = await readFile(join(fixture, "media/playlist.m3u8"));
    const paths: string[] = [];
    const rejected: string[] = [];
    const server = createServer(async (req, res) => {
      const path = req.url ?? "";
      paths.push(path);
      if (
        mode === "agent"
          ? req.headers["x-emulator-probe"] !== "agent" ||
            req.headers["x-emulator-content"] !== undefined
          : req.headers["x-emulator-content"] !== "content" ||
            req.headers["x-emulator-probe"] !== undefined
      ) {
        rejected.push(path);
        res.writeHead(401).end();
        return;
      }
      if (path === "/media/playlist.m3u8") {
        res.writeHead(200, { "Content-Type": "application/vnd.apple.mpegurl" }).end(playlist);
      } else if (/^\/media\/part-\d+\.m2ts$/.test(path)) {
        try {
          const segment = await readFile(join(fixture, path));
          res.writeHead(200, { "Content-Type": "video/mp2t" }).end(segment);
        } catch {
          res.writeHead(404).end();
        }
      } else {
        res.writeHead(404).end();
      }
    });
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    const address = server.address() as { port: number };
    const scratch = await mkdtemp(join(tmpdir(), "roku-video-headers-"));
    try {
      await cp(fixture, scratch, { recursive: true });
      await writeFile(join(scratch, "header-mode.txt"), mode);
      await writeFile(join(scratch, "base-url.txt"), `http://127.0.0.1:${address.port}`);
      await page.goto("/");
      await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
      await page.locator('input[type="file"]').setInputFiles({
        name: "http-agent.zip",
        mimeType: "application/zip",
        buffer: Buffer.from(packChannel(scratch)),
      });
      const output = page.locator('[role="log"] > div');
      await expect.poll(() => output.allTextContents()).toContain("@@PROBE_END");
      const lines = await output.allTextContents();
      expect(lines).toContain("playing");
      expect(lines.some((line) => /runtime error|unreachable/.test(line))).toBe(false);
      expect(paths).toContain("/media/playlist.m3u8");
      expect(paths.some((path) => path.endsWith(".m2ts"))).toBe(true);
      expect(rejected).toEqual([]);
    } finally {
      server.closeAllConnections();
      await new Promise<void>((resolve) => server.close(() => resolve()));
      await rm(scratch, { recursive: true, force: true });
    }
  });
}
