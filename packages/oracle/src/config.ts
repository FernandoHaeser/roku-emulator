/** Oracle configuration from `.env` (device host, dev credentials, ports, timeouts). */
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export interface OracleConfig {
  host: string;
  devUser: string;
  devPassword: string;
  ecpPort: number;
  consolePort: number;
  probeTimeoutMs: number;
}

export class OracleConfigError extends Error {
  override name = "OracleConfigError";
}

/** Repository root, resolved from this file's location (packages/oracle/src). */
export const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

/**
 * Loads `<repo>/.env` into process.env (without overriding variables that are
 * already set) using Node's built-in loader, so no dotenv dependency is needed.
 */
export function loadEnvFile(path = resolve(REPO_ROOT, ".env")): void {
  if (existsSync(path)) process.loadEnvFile(path);
}

// Accepts IPv4 or a plain hostname. Rejects schemes, paths and ports so the value
// cannot be abused to point requests at an arbitrary URL.
const HOST_PATTERN =
  /^(?:\d{1,3}(?:\.\d{1,3}){3}|[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9-]+)*)$/;

function parsePort(raw: string | undefined, fallback: number, name: string): number {
  if (raw === undefined || raw.trim() === "") return fallback;
  const port = Number(raw);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new OracleConfigError(`${name} must be a valid TCP port, got "${raw}"`);
  }
  return port;
}

export function parseOracleConfig(env: NodeJS.ProcessEnv): OracleConfig {
  const host = env.ROKU_HOST?.trim() ?? "";
  if (host === "") {
    throw new OracleConfigError("ROKU_HOST is not set. Fill it in .env (see .env.example).");
  }
  if (!HOST_PATTERN.test(host)) {
    throw new OracleConfigError(`ROKU_HOST must be an IPv4 address or hostname, got "${host}"`);
  }
  const devPassword = env.ROKU_DEV_PASSWORD ?? "";
  if (devPassword === "") {
    throw new OracleConfigError("ROKU_DEV_PASSWORD is not set. Fill it in .env.");
  }
  const timeoutRaw = env.ORACLE_PROBE_TIMEOUT_MS?.trim();
  const probeTimeoutMs = timeoutRaw ? Number(timeoutRaw) : 20_000;
  if (!Number.isInteger(probeTimeoutMs) || probeTimeoutMs <= 0) {
    throw new OracleConfigError(`ORACLE_PROBE_TIMEOUT_MS must be a positive integer`);
  }
  return {
    host,
    devUser: env.ROKU_DEV_USER?.trim() || "rokudev",
    devPassword,
    ecpPort: parsePort(env.ROKU_ECP_PORT, 8060, "ROKU_ECP_PORT"),
    consolePort: parsePort(env.ROKU_CONSOLE_PORT, 8085, "ROKU_CONSOLE_PORT"),
    probeTimeoutMs,
  };
}
