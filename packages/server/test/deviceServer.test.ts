import { createHash } from "node:crypto";
import { createServer, get } from "node:http";
import { connect } from "node:net";
import { once } from "node:events";
import { afterEach, expect, it } from "vitest";
import { packChannel } from "../../oracle/src/packager";
import { startDeviceServer, type RunningDeviceServer } from "../src/deviceServer";
import { DEFAULT_CONFIG } from "../src/config";
import { parseDigestHeader } from "../src/digestAuth";
import type { BridgeEvent } from "@roku-emu/protocol/bridge";

let device: RunningDeviceServer | undefined;
afterEach(async () => {
  await device?.close();
});
const md5 = (text: string) => createHash("md5").update(text).digest("hex");

it("accepts authenticated sideload, routes ECP, and streams console over TCP", async () => {
  device = await startDeviceServer({
    ...DEFAULT_CONFIG,
    ecpPort: 0,
    consolePort: 0,
    debugPort: 0,
    sceneConsolePort: 0,
    installerPort: 0,
  });
  const base = `http://127.0.0.1:${device.ecpPort}`;
  const events: BridgeEvent[] = [];
  const challenge = await fetch(`${base}/plugin_install`);
  expect(challenge.status).toBe(401);
  const fields = parseDigestHeader(challenge.headers.get("www-authenticate")!);
  const nonce = fields.get("nonce")!;
  const uri = "/plugin_install";
  const response = md5(
    `${md5("rokudev:rokudev:rokudev")}:${nonce}:00000001:test:auth:${md5(`POST:${uri}`)}`,
  );
  const auth = `Digest username="rokudev", realm="rokudev", nonce="${nonce}", uri="${uri}", qop=auth, nc=00000001, cnonce="test", response="${response}"`;
  const form = new FormData();
  form.set("mysubmit", "Install");
  form.set(
    "archive",
    new Blob([new Uint8Array(packChannel("fidelity/language/hello"))]),
    "hello.zip",
  );
  const installed = await fetch(`${base}${uri}`, {
    method: "POST",
    headers: { Authorization: auth },
    body: form,
  });
  expect(await installed.text()).toContain("Install Success.");
  // Uploads made before a browser connects must be delivered when it opens.
  const disconnect = device.bridge.connectShell((event) => events.push(event));
  expect(events[0]).toMatchObject({ type: "install", fileName: "hello.zip" });
  await fetch(`${base}/keypress/Select`, { method: "POST" });
  expect(events.slice(-2)).toEqual([
    { type: "key", key: "OK", pressed: true },
    { type: "key", key: "OK", pressed: false },
  ]);
  device.bridge.setShellState({ runningTitle: "Hello & test" });
  expect(await (await fetch(`${base}/query/active-app`)).text()).toContain("Hello &#38; test");
  const socket = connect(device.consolePort, "127.0.0.1");
  await once(socket, "connect");
  let output = "";
  socket.on("data", (chunk) => {
    output += String(chunk);
  });
  // TCP connect can precede the server's accept callback: wait for its sink.
  await expect
    .poll(() => {
      device!.bridge.writeConsole("hello\nworld\r\n");
      return output;
    })
    .toContain("hello\r\nworld\r\n");
  socket.destroy();
  disconnect();
});

it("serves bridge packages and rejects foreign hosts and unmarked posts", async () => {
  device = await startDeviceServer({
    ...DEFAULT_CONFIG,
    ecpPort: 0,
    consolePort: 0,
    debugPort: 0,
    sceneConsolePort: 0,
    installerPort: 0,
  });
  const server = createServer((req, res) =>
    device!.middleware(req, res, () => {
      res.writeHead(404);
      res.end();
    }),
  );
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address() as { port: number };
  const base = `http://127.0.0.1:${address.port}/__roku-emu`;
  try {
    const installed = device.bridge.install("test.zip", Buffer.from("package"));
    expect(await (await fetch(`${base}/package/${installed.id}`)).text()).toBe("package");
    expect((await fetch(`${base}/state`, { method: "POST", body: "{}" })).status).toBe(403);
    const foreignStatus = await new Promise<number | undefined>((resolve, reject) => {
      get(`${base}/package/${installed.id}`, { headers: { Host: "evil.example" } }, (response) => {
        response.resume();
        resolve(response.statusCode);
      }).on("error", reject);
    });
    expect(foreignStatus).toBe(403);
    expect(
      (
        await fetch(`${base}/state`, {
          method: "POST",
          headers: { "x-roku-emu-bridge": "1" },
          body: "null",
        })
      ).status,
    ).toBe(400);
  } finally {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});
