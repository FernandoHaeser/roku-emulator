/** Development console on port 8080, backed by live VM inspection. */
import { createServer, type Socket } from "node:net";
import { ConsoleInput } from "./consoleInput";
import { toCrlf } from "./consoleServer";
import { sceneNodes, type SceneInspection } from "./sceneInspection";
import type { DeviceBridge } from "./bridge";
export class SceneConsole {
  private readonly sockets = new Set<Socket>();
  private readonly server;
  constructor(bridge: DeviceBridge) {
    this.server = createServer((socket) => {
      this.sockets.add(socket);
      socket.on("error", () => socket.destroy());
      socket.on("close", () => this.sockets.delete(socket));
      let queue = Promise.resolve();
      const input = new ConsoleInput((command) => {
        queue = queue.then(async () => {
          try {
            const scene = await bridge.rpc<SceneInspection>({ op: "scene" });
            const nodes = sceneNodes(scene);
            let output: string;
            switch (command.trim().split(/\s/)[0]?.toLowerCase()) {
              case "sgnodes":
                output = nodes
                  .map(
                    (n, i) =>
                      `${i}: ${n.type} id=${String(n.fields.id ?? "")} children=${n.children.length} focused=${n.focused}`,
                  )
                  .join("\n");
                break;
              case "sgperf":
              case "chanperf":
                output = `nodes=${nodes.length} threads=${scene.threads} uptimeMs=${Math.round(scene.timeMs)}`;
                break;
              case "r2d2_bitmaps":
                output = nodes
                  .filter((n) => n.type === "Poster")
                  .map((n) => String(n.fields.uri ?? ""))
                  .join("\n");
                break;
              case "help":
                output = "sgnodes all, sgperf, chanperf, r2d2_bitmaps";
                break;
              default:
                output = "Unknown command. Type help.";
            }
            socket.write(toCrlf(`${output}\nRoku Debugger> `));
          } catch (error) {
            socket.write(toCrlf(`Inspection unavailable: ${String(error)}\nRoku Debugger> `));
          }
        });
      });
      socket.on("data", (data: Buffer) => input.push(data));
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
