import { createServer } from "node:net";
import { once } from "node:events";
import { expect, it } from "vitest";
import { httpRequest } from "../src/httpProxy";

it("reports Roku's resolver error separately from connection refusal", async () => {
  // .invalid is reserved: this must fail in resolution, before HTTP or TLS.
  const unresolved = await httpRequest({
    url: "https://roku-emulator-conformance.invalid/auth",
    method: "POST",
    timeout: 3000,
  });
  expect(unresolved.status).toBe(-6);
  expect(unresolved.body).toBe("");
  expect(unresolved.headers).toEqual({});
  const server = createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (!address || typeof address === "string") throw Error("Expected a TCP port");
  await new Promise<void>((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
  const refused = await httpRequest({ url: `http://127.0.0.1:${address.port}/`, timeout: 3000 });
  expect(refused.status).toBe(-7);
});
