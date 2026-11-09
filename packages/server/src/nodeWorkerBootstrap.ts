/** Node Worker bootstrap for the browser-independent WASM runtime loop. */
import { parentPort } from "node:worker_threads";
import { readFile } from "node:fs/promises";
import { basename } from "node:path";
import type { ShellToWorkerMessage } from "@roku-emu/protocol";
const port = parentPort!;
const state = {
  onmessage: null as ((event: { data: ShellToWorkerMessage }) => void) | null,
  postMessage: (message: unknown) => port.postMessage(message),
};
(globalThis as unknown as { self: typeof state }).self = state;
const originalFetch = globalThis.fetch;
globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
  if (typeof input === "string" && input.endsWith(".wasm")) {
    const bytes = await readFile(new URL(`./${basename(input)}`, import.meta.url));
    return new Response(bytes, { headers: { "Content-Type": "application/wasm" } });
  }
  return originalFetch(input, init);
}) as typeof fetch;
const pending: ShellToWorkerMessage[] = [];
port.on("message", (message: ShellToWorkerMessage) => {
  if (state.onmessage) state.onmessage({ data: message });
  else pending.push(message);
});
const runtime = new URL("./runtime.worker.mjs", import.meta.url);
await import(runtime.href);
for (const message of pending) state.onmessage?.({ data: message });
