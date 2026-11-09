/**
 * Vite dev-server plugin: starts the device server next to the web shell and
 * mounts the bridge routes on the same origin. `make dev` is then all it takes
 * for the VS Code BrightScript extension to deploy to the emulator.
 *
 * Typed structurally so this package does not depend on Vite.
 */
import type { IncomingMessage, ServerResponse } from "node:http";
import { loadConfig } from "./config";
import { startDeviceServer, type RunningDeviceServer } from "./deviceServer";

interface DevServerLike {
  middlewares: {
    use(
      handler: (
        request: IncomingMessage,
        response: ServerResponse,
        next: (error?: unknown) => void,
      ) => void,
    ): void;
  };
  httpServer: { once(event: "close", listener: () => void): unknown } | null;
  config: { logger: { info(message: string): void; warn(message: string): void } };
}

export function rokuDevicePlugin() {
  let running: RunningDeviceServer | null = null;
  return {
    name: "roku-emu-device-server",
    apply: "serve" as const,
    async configureServer(server: DevServerLike) {
      const { logger } = server.config;
      try {
        const config = loadConfig();
        running = await startDeviceServer(config);
        server.middlewares.use(running.middleware);
        logger.info(
          `  roku device: ECP + dev installer http://${config.host}:${running.ecpPort}, console telnet ${config.host} ${running.consolePort}`,
        );
      } catch (error) {
        // Never block the shell: without the device server only VS Code deploys are unavailable.
        logger.warn(
          `  roku device server disabled: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
      server.httpServer?.once("close", () => {
        void running?.close();
      });
    },
  };
}
