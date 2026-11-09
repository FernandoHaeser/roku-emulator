/** Device server settings, from the environment with device-like defaults. */

export interface DeviceServerConfig {
  /** Bind address. Loopback by default: the emulator is never exposed on the LAN unless asked. */
  host: string;
  /** ECP and dev installer (one HTTP server). A device uses 8060 and 80. */
  ecpPort: number;
  /** BrightScript console (telnet), like the device's port 8085. */
  consolePort: number;
  debugPort: number;
  sceneConsolePort: number;
  installerPort: number;
  /** Dev installer credentials (Digest auth), like the device's `rokudev` account. */
  username: string;
  password: string;
}

export const DEFAULT_CONFIG: DeviceServerConfig = {
  host: "127.0.0.1",
  ecpPort: 8060,
  consolePort: 8085,
  debugPort: 8081,
  sceneConsolePort: 8080,
  installerPort: 8060,
  username: "rokudev",
  password: "rokudev",
};

const LOOPBACK_HOSTS = new Set(["127.0.0.1", "::1", "localhost"]);

function parsePort(value: string | undefined, fallback: number, name: string): number {
  if (value === undefined || value === "") return fallback;
  const port = Number(value);
  if (!Number.isInteger(port) || port < 0 || port > 65_535) {
    throw new Error(`${name} must be a port number (0-65535), got "${value}"`);
  }
  return port;
}

/**
 * Reads `ROKU_EMU_*` variables. Binding outside loopback requires
 * `ROKU_EMU_ALLOW_LAN=1` and a non-default password: the installer accepts
 * arbitrary packages, so it must not be reachable with well-known credentials.
 */
export function loadConfig(env: NodeJS.ProcessEnv = process.env): DeviceServerConfig {
  const host = env.ROKU_EMU_HOST || DEFAULT_CONFIG.host;
  const password = env.ROKU_EMU_DEV_PASSWORD || DEFAULT_CONFIG.password;
  if (!LOOPBACK_HOSTS.has(host)) {
    if (env.ROKU_EMU_ALLOW_LAN !== "1") {
      throw new Error(
        `ROKU_EMU_HOST=${host} is not loopback; set ROKU_EMU_ALLOW_LAN=1 to allow it`,
      );
    }
    if (password === DEFAULT_CONFIG.password) {
      throw new Error(
        "Set ROKU_EMU_DEV_PASSWORD to a non-default value before exposing the emulator on the network",
      );
    }
  }
  return {
    host,
    ecpPort: parsePort(env.ROKU_EMU_ECP_PORT, DEFAULT_CONFIG.ecpPort, "ROKU_EMU_ECP_PORT"),
    consolePort: parsePort(
      env.ROKU_EMU_CONSOLE_PORT,
      DEFAULT_CONFIG.consolePort,
      "ROKU_EMU_CONSOLE_PORT",
    ),
    installerPort: parsePort(
      env.ROKU_EMU_INSTALLER_PORT,
      DEFAULT_CONFIG.installerPort,
      "ROKU_EMU_INSTALLER_PORT",
    ),
    sceneConsolePort: parsePort(
      env.ROKU_EMU_SCENE_CONSOLE_PORT,
      DEFAULT_CONFIG.sceneConsolePort,
      "ROKU_EMU_SCENE_CONSOLE_PORT",
    ),
    debugPort: parsePort(env.ROKU_EMU_DEBUG_PORT, DEFAULT_CONFIG.debugPort, "ROKU_EMU_DEBUG_PORT"),
    username: DEFAULT_CONFIG.username,
    password,
  };
}
