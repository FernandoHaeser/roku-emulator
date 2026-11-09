/** Starts the device-facing servers around one bridge. */
import { SceneConsole } from "./sceneConsole";
import { DebugProtocolServer } from "./debugProtocolServer";
import { DeviceBridge } from "./bridge";
import { createBridgeMiddleware } from "./bridgeMiddleware";
import type { DeviceServerConfig } from "./config";
import { ConsoleServer } from "./consoleServer";
import { loadReferenceProfile, type DeviceProfile } from "./deviceInfo";
import { EcpServer } from "./ecpServer";

export interface RunningDeviceServer {
  bridge: DeviceBridge;
  middleware: ReturnType<typeof createBridgeMiddleware>;
  ecpPort: number;
  consolePort: number;
  debugPort: number;
  sceneConsolePort: number;
  installerPort: number;
  close(): Promise<void>;
}

export async function startDeviceServer(
  config: DeviceServerConfig,
  profile: DeviceProfile = loadReferenceProfile(),
): Promise<RunningDeviceServer> {
  const bridge = new DeviceBridge();
  const ecp = new EcpServer(bridge, config, profile);
  const console = new ConsoleServer(bridge);
  const debug = new DebugProtocolServer(bridge);
  const scene = new SceneConsole(bridge);
  const installer = new EcpServer(bridge, config, profile);
  const ecpPort = await ecp.listen(config.ecpPort, config.host);
  let consolePort: number;
  try {
    consolePort = await console.listen(config.consolePort, config.host);
  } catch (error) {
    await ecp.close();
    throw error;
  }
  let debugPort: number;
  try {
    debug.ioPort = consolePort;
    debugPort = await debug.listen(config.debugPort, config.host);
  } catch (error) {
    await Promise.all([ecp.close(), console.close(), debug.close()]);
    throw error;
  }
  let sceneConsolePort: number;
  try {
    sceneConsolePort = await scene.listen(config.sceneConsolePort, config.host);
  } catch (error) {
    await Promise.all([ecp.close(), console.close(), debug.close()]);
    throw error;
  }
  let installerPort = ecpPort;
  if (config.installerPort !== config.ecpPort) {
    try {
      installerPort = await installer.listen(config.installerPort, config.host);
    } catch (error) {
      await Promise.all([ecp.close(), console.close(), debug.close(), scene.close()]);
      throw error;
    }
  }
  return {
    bridge,
    middleware: createBridgeMiddleware(bridge),
    ecpPort,
    consolePort,
    debugPort,
    sceneConsolePort,
    installerPort,
    close: async () => {
      await Promise.all([
        ecp.close(),
        console.close(),
        debug.close(),
        scene.close(),
        installer.close(),
      ]);
    },
  };
}
