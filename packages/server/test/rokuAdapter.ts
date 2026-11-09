/** Real RokuCommunity adapter; used by browser integration tests. */
import { logger } from "roku-debug/dist/logging.js";
import { TelnetAdapter } from "roku-debug/dist/adapters/TelnetAdapter.js";
import { RendezvousTracker } from "roku-debug/dist/RendezvousTracker.js";

export function createRokuAdapter(consolePort: number, ecpPort: number) {
  logger.logLevel = "error";
  const tracker = new RendezvousTracker(
    {} as ConstructorParameters<typeof RendezvousTracker>[0],
    {} as ConstructorParameters<typeof RendezvousTracker>[1],
  );
  return new TelnetAdapter(
    {
      device: { host: "127.0.0.1" },
      brightScriptConsolePort: consolePort,
      remotePort: ecpPort,
      stopOnEntry: true,
      enableDebuggerAutoRecovery: false,
    },
    tracker,
  );
}

export { rokuDeploy } from "roku-deploy";

export { DebugProtocolClient } from "roku-debug/dist/debugProtocol/client/DebugProtocolClient.js";
