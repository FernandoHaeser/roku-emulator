/**
 * Turns low-level errors into actionable messages. Node's fetch reports every
 * network failure as "fetch failed" and hides the real reason in `cause`.
 */

const NETWORK_HINTS: Readonly<Record<string, string>> = {
  EHOSTUNREACH:
    "macOS may be blocking Local Network access for this Node binary. Allow your terminal " +
    "under System Settings > Privacy & Security > Local Network (toggle it off/on if it is " +
    "already enabled), then retry. Also check that ROKU_HOST is on the same network.",
  ECONNREFUSED:
    "The device refused the connection. Check that Developer Mode is enabled and that " +
    "'Control by mobile apps' is set to Permissive.",
  UND_ERR_CONNECT_TIMEOUT:
    "The device did not accept the connection in time. Roku devices often stall for a few " +
    "seconds while the Home screen reloads; if it persists, check ROKU_HOST and Wi-Fi.",
  ETIMEDOUT: "The device did not answer in time. Check ROKU_HOST and that the TV is powered on.",
  ENETUNREACH: "No route to the device network. Check Wi-Fi/VPN and ROKU_HOST.",
};

export function errorCode(error: unknown): string | undefined {
  if (typeof error !== "object" || error === null) return undefined;
  const { code, cause } = error as { code?: unknown; cause?: unknown };
  if (typeof code === "string") return code;
  return errorCode(cause);
}

export function describeError(error: unknown): string {
  if (!(error instanceof Error)) return String(error);
  const parts = [`${error.name}: ${error.message}`];
  if (error.cause instanceof Error && error.cause.message !== error.message) {
    parts.push(`  cause: ${error.cause.message}`);
  }
  const code = errorCode(error);
  const hint = code === undefined ? undefined : NETWORK_HINTS[code];
  if (hint) parts.push(`  hint: ${hint}`);
  return parts.join("\n");
}
