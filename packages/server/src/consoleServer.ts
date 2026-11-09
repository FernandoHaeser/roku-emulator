/**
 * BrightScript console over TCP, like the device's port 8085: every client
 * receives the channel's console output with CRLF line endings (what the
 * device sends). Commands are framed and forwarded to the browser runtime.
 */
import { createServer, type Server, type Socket } from "node:net";
import { ConsoleInput } from "./consoleInput";
import type { DeviceBridge } from "./bridge";

export function toCrlf(text: string): string {
  return text.replace(/\r?\n/g, "\r\n");
}

export class ConsoleServer {
  private readonly server: Server;
  private readonly sockets = new Set<Socket>();

  constructor(bridge: DeviceBridge) {
    this.server = createServer((socket) => {
      this.sockets.add(socket);
      const disconnect = bridge.connectConsole((text) => {
        if (!socket.destroyed) socket.write(toCrlf(text));
      });
      const cleanup = () => {
        disconnect();
        this.sockets.delete(socket);
      };
      socket.on("close", cleanup);
      socket.on("error", cleanup);
      const input = new ConsoleInput((command) => bridge.debuggerCommand(command));
      socket.on("data", (chunk: Buffer) => input.push(chunk));
    });
  }

  listen(port: number, host: string): Promise<number> {
    return new Promise((resolve, reject) => {
      this.server.once("error", reject);
      this.server.listen(port, host, () => {
        this.server.off("error", reject);
        const address = this.server.address();
        resolve(typeof address === "object" && address ? address.port : port);
      });
    });
  }

  close(): Promise<void> {
    for (const socket of this.sockets) socket.destroy();
    return new Promise((resolve) => this.server.close(() => resolve()));
  }
}
