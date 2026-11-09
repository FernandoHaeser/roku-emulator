/** Main thread writes commands; only the runtime Worker blocks on Atomics. */
export const DEBUG_MAILBOX_BYTES = 65_544;
export function writeDebugCommand(buffer: SharedArrayBuffer, command: string): void {
  const bytes = new TextEncoder().encode(command);
  if (bytes.length > buffer.byteLength - 8) throw new Error("Debugger command too large");
  new Uint8Array(buffer, 8).set(bytes);
  const header = new Int32Array(buffer, 0, 2);
  Atomics.store(header, 1, bytes.length);
  Atomics.store(header, 0, 1);
  Atomics.notify(header, 0);
}
export function waitDebugCommand(buffer: SharedArrayBuffer, ready: () => void): string {
  const header = new Int32Array(buffer, 0, 2);
  Atomics.store(header, 0, 0);
  ready();
  while (Atomics.load(header, 0) === 0) Atomics.wait(header, 0, 0);
  return new TextDecoder().decode(new Uint8Array(buffer, 8, Atomics.load(header, 1)).slice());
}
