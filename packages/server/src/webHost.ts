/** Serves the built shell and same-origin bridge; also used by the VS Code extension. */
import { createServer } from "node:http";
import { readFile, realpath } from "node:fs/promises";
import { resolve, extname, sep } from "node:path";
import type { RunningDeviceServer } from "./deviceServer";
const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".wasm": "application/wasm",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
};
export async function startWebHost(root: string, device: RunningDeviceServer, port = 0) {
  const directory = await realpath(root);
  const server = createServer((request, response) => {
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    response.setHeader("Cross-Origin-Resource-Policy", "cross-origin");
    device.middleware(request, response, () => {
      void (async () => {
        const pathname = decodeURIComponent(
          new URL(request.url ?? "/", "http://localhost").pathname,
        );
        const file = await realpath(
          resolve(directory, pathname === "/" ? "index.html" : `.${pathname}`),
        );
        if (!file.startsWith(directory + sep)) {
          response.writeHead(403);
          response.end();
          return;
        }
        let data = await readFile(file);
        if (extname(file) === ".html" && device.bridge.runtimeReceiver)
          data = Buffer.from(
            data
              .toString()
              .replace(
                "<head>",
                '<head><script>globalThis.__ROKU_RUNTIME_HOST__="server";</script>',
              ),
          );

        response.writeHead(200, {
          "Content-Type": TYPES[extname(file)] ?? "application/octet-stream",
          "Content-Length": data.length,
          "Cache-Control": "no-cache",
        });
        response.end(data);
      })().catch(() => {
        if (!response.headersSent) response.writeHead(404);
        response.end();
      });
    });
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      server.off("error", reject);
      resolve();
    });
  });
  const address = server.address() as { port: number };
  return {
    url: `http://127.0.0.1:${address.port}/?device=1`,
    port: address.port,
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections();
        server.close(() => resolve());
      }),
  };
}
