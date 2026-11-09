import { afterEach, expect, it, vi } from "vitest";
import { EcpClient } from "../src/ecp.js";
import { parseOracleConfig } from "../src/config.js";
afterEach(() => vi.unstubAllGlobals());
it("translates remote key aliases to device ECP names", async () => {
  const fetch = vi.fn<typeof globalThis.fetch>(async () => new Response(""));
  vi.stubGlobal("fetch", fetch);
  const client = new EcpClient(
    parseOracleConfig({ ROKU_HOST: "127.0.0.1", ROKU_DEV_PASSWORD: "unused" }),
  );
  for (const key of ["OK", "Options", "Replay", "Down"] as const) await client.keypress(key);
  expect(fetch.mock.calls.map((call) => call[0])).toEqual([
    "http://127.0.0.1:8060/keypress/Select",
    "http://127.0.0.1:8060/keypress/Info",
    "http://127.0.0.1:8060/keypress/InstantReplay",
    "http://127.0.0.1:8060/keypress/Down",
  ]);
});
