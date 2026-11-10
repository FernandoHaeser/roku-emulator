import { createServer } from "node:http";
import { once } from "node:events";
import { expect, it } from "vitest";
import { digestFetch } from "../src/digest.js";
import { DigestAuthenticator } from "../../server/src/digestAuth.js";

it("authenticates before uploading a large package to a device that challenges early", async () => {
  const auth = new DigestAuthenticator("rokudev", "test-password");
  let unauthenticatedUploads = 0,
    received = 0,
    probes = 0;
  const server = createServer((request, response) => {
    if (!auth.verify(request.method!, request.url!, request.headers.authorization)) {
      if (request.method === "POST") unauthenticatedUploads++;
      else probes++;
      request.resume();
      response.writeHead(401, { "WWW-Authenticate": auth.challenge() }).end();
      return;
    }
    request.on("data", (chunk: Buffer) => {
      received += chunk.length;
    });
    request.on("end", () => response.end("installed"));
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const base = `http://127.0.0.1:${(server.address() as { port: number }).port}`;
  try {
    const bytes = new Uint8Array(4 * 1024 * 1024);
    const result = await digestFetch(
      `${base}/plugin_install`,
      () => ({ method: "POST", body: bytes }),
      { user: "rokudev", password: "test-password" },
      { challengeUrl: base },
    );
    expect(await result.text()).toBe("installed");
    expect(probes).toBe(1);
    expect(unauthenticatedUploads).toBe(0);
    expect(received).toBe(bytes.length);
    await expect(
      digestFetch(
        `${base}/plugin_install`,
        () => ({ method: "POST" }),
        { user: "u", password: "p" },
        { challengeUrl: "http://example.invalid/" },
      ),
    ).rejects.toThrow("share the request origin");
  } finally {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});
