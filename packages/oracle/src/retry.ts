/** Retry for connection-phase failures only (the device stalls while the Home screen reloads). */
import { errorCode } from "./errors.js";

/**
 * Errors raised while *establishing* the TCP connection. The request never
 * reached the device, so retrying is safe even for non-idempotent calls such
 * as keypress or install. Errors after the request was sent (e.g. ECONNRESET)
 * are deliberately excluded: retrying them could press a key twice.
 */
const CONNECT_ERROR_CODES = new Set([
  "UND_ERR_CONNECT_TIMEOUT",
  "ECONNREFUSED",
  "EHOSTUNREACH",
  "ENETUNREACH",
]);

export function isConnectError(error: unknown): boolean {
  const code = errorCode(error);
  return code !== undefined && CONNECT_ERROR_CODES.has(code);
}

export interface RetryOptions {
  attempts: number;
  baseDelayMs: number;
}

export const DEFAULT_RETRY: RetryOptions = { attempts: 4, baseDelayMs: 1_000 };

const sleep = (ms: number) => new Promise((wake) => setTimeout(wake, ms));

/** Retries `operation` on connection-phase failures with linear backoff. */
export async function withConnectRetry<T>(
  operation: () => Promise<T>,
  options: RetryOptions = DEFAULT_RETRY,
): Promise<T> {
  for (let attempt = 1; ; attempt++) {
    try {
      return await operation();
    } catch (error) {
      if (attempt >= options.attempts || !isConnectError(error)) throw error;
      await sleep(options.baseDelayMs * attempt);
    }
  }
}
