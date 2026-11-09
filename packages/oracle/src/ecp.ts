import type { RemoteKey } from "@roku-emu/protocol/keys";
import type { OracleConfig } from "./config.js";
import { isConnectError, withConnectRetry } from "./retry.js";

/** External Control Protocol client (port 8060): remote keys, launch and queries. */

/** Keys whose ECP name differs from the onKeyEvent/remote name. */
const ECP_KEY_NAMES: Partial<Record<RemoteKey, string>> = {
  OK: "Select",
  Options: "Info",
  Replay: "InstantReplay",
};

export class EcpError extends Error {
  override name = "EcpError";
}

export class EcpClient {
  constructor(private readonly config: OracleConfig) {}

  private url(path: string): string {
    return `http://${this.config.host}:${this.config.ecpPort}${path}`;
  }

  private async request(method: "GET" | "POST", path: string): Promise<string> {
    const response = await withConnectRetry(() => fetch(this.url(path), { method }));
    const body = await response.text();
    if (!response.ok) {
      throw new EcpError(
        `${method} ${path} failed with HTTP ${response.status}` +
          (response.status === 403
            ? " (enable Settings > System > Advanced > Control by mobile apps > Permissive)"
            : ""),
      );
    }
    return body;
  }

  keypress(key: RemoteKey): Promise<string> {
    return this.request("POST", `/keypress/${ECP_KEY_NAMES[key] ?? key}`);
  }

  /** Launches the sideloaded channel, optionally with deep-link params. */
  launchDev(params: Record<string, string> = {}): Promise<string> {
    const query = new URLSearchParams(params).toString();
    return this.request("POST", `/launch/dev${query ? `?${query}` : ""}`);
  }

  /**
   * Polls device-info until the device answers. After a channel exits, the Home
   * screen reload can leave ECP unresponsive for several seconds.
   */
  async waitUntilReady(timeoutMs = 30_000): Promise<string> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      try {
        return await fetch(this.url("/query/device-info"), {
          signal: AbortSignal.timeout(3_000),
        }).then(async (response) => {
          if (!response.ok) throw new EcpError(`device-info failed with HTTP ${response.status}`);
          return response.text();
        });
      } catch (error) {
        const retryable = isConnectError(error) || (error as Error).name === "TimeoutError";
        if (!retryable || Date.now() >= deadline) throw error;
        await new Promise((wake) => setTimeout(wake, 1_000));
      }
    }
  }

  queryDeviceInfo(): Promise<string> {
    return this.request("GET", "/query/device-info");
  }

  /** SceneGraph tree of the running dev channel, with field values. */
  queryAppUi(): Promise<string> {
    return this.request("GET", "/query/app-ui");
  }
}
