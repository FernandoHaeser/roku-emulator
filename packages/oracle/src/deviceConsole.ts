import { Socket } from "node:net";

/**
 * Reader for the BrightScript console (telnet port 8085). The device streams
 * plain text; we accumulate it and let callers wait for a marker.
 */
export class DeviceConsole {
  private buffer = "";
  private readonly socket = new Socket();
  private waiters: Array<() => void> = [];
  private closed = false;

  private constructor() {
    this.socket.setEncoding("utf8");
    this.socket.on("data", (chunk: string) => {
      this.buffer += chunk;
      this.notify();
    });
    this.socket.on("close", () => {
      this.closed = true;
      this.notify();
    });
  }

  static connect(host: string, port: number, timeoutMs = 5_000): Promise<DeviceConsole> {
    const deviceConsole = new DeviceConsole();
    return new Promise((resolvePromise, reject) => {
      const timer = setTimeout(() => {
        deviceConsole.socket.destroy();
        reject(new Error(`Timed out connecting to console at ${host}:${port}`));
      }, timeoutMs);
      deviceConsole.socket.once("error", (error) => {
        clearTimeout(timer);
        reject(error);
      });
      deviceConsole.socket.connect(port, host, () => {
        clearTimeout(timer);
        resolvePromise(deviceConsole);
      });
    });
  }

  private notify(): void {
    const waiters = this.waiters;
    this.waiters = [];
    for (const wake of waiters) wake();
  }

  /** Everything received so far. */
  get text(): string {
    return this.buffer;
  }

  /** Resolves true once `predicate(text)` holds, or false on timeout / socket close. */
  async waitFor(predicate: (text: string) => boolean, timeoutMs: number): Promise<boolean> {
    const deadline = Date.now() + timeoutMs;
    while (!predicate(this.buffer)) {
      const remaining = deadline - Date.now();
      if (remaining <= 0 || this.closed) return false;
      await new Promise<void>((wake) => {
        const timer = setTimeout(wake, remaining);
        this.waiters.push(() => {
          clearTimeout(timer);
          wake();
        });
      });
    }
    return true;
  }

  close(): void {
    this.socket.destroy();
  }
}
