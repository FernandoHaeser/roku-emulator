import { createServer } from "node:http";
import { once } from "node:events";
import { mkdtemp, cp, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";
import { DigestAuthenticator } from "../../../packages/server/src/digestAuth";

test("channel HTTP sync/async, files, cookies, Digest and cancellation use the local proxy", async ({
  page,
}) => {
  const auth = new DigestAuthenticator("user", "test-password");
  const server = createServer((req, res) => {
    if (req.url === "/digest" && !auth.verify(req.method!, req.url, req.headers.authorization)) {
      req.resume();
      res.writeHead(401, { "WWW-Authenticate": auth.challenge() });
      res.end();
      return;
    }
    if (req.url === "/slow") {
      req.resume();
      const timer = setTimeout(() => res.end("late"), 1000);
      res.on("close", () => clearTimeout(timer));
      return;
    }
    const parts: Buffer[] = [];
    req.on("data", (part: Buffer) => parts.push(part));
    req.on("end", () => {
      if (req.url === "/post") {
        expect(Buffer.concat(parts).toString()).toBe("payload");
        expect(req.headers["x-test"]).toBe("channel");
        res.writeHead(201);
        res.end("created");
      } else if (req.url === "/set-cookie") {
        res.writeHead(200, { "Set-Cookie": "session=abc; Path=/" });
        res.end("ok");
      } else if (req.url === "/cookie") res.end(req.headers.cookie || "");
      else if (req.url === "/digest") res.end("authorized");
      else res.end("hello");
    });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address() as { port: number };
  const scratch = await mkdtemp(join(tmpdir(), "roku-net-"));
  try {
    const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
    await cp(join(root, "apps/web/e2e/fixtures/network"), scratch, { recursive: true });
    const source = join(scratch, "source/main.brs");
    await writeFile(
      source,
      (await readFile(source, "utf8")).replace("HTTP_BASE_URL", `http://127.0.0.1:${address.port}`),
    );
    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await page.locator('input[type="file"]').setInputFiles({
      name: "network.zip",
      mimeType: "application/zip",
      buffer: Buffer.from(packChannel(scratch)),
    });
    const log = page.locator('[role="log"]');
    await expect(log).toContainText("@@NETWORK_DONE");
    for (const line of [
      "sync=hello",
      "post= 201",
      "async=true",
      "event=roUrlEvent",
      "code= 200",
      "body=hello",
      "identity=true",
      "file= 200",
      "stored=hello",
      "cookie=session=abc",
      "digest=authorized",
      "busy=false",
      "cancel=true",
      "cancelled=true",
    ])
      await expect(log).toContainText(line);
  } finally {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
    await rm(scratch, { recursive: true, force: true });
  }
});
