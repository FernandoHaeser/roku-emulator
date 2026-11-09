/** Client for the device's developer web installer: `/plugin_install` (install, remove) and `/plugin_inspect` (screenshot). */
import type { OracleConfig } from "./config.js";
import { digestFetch } from "./digest.js";
import { withConnectRetry } from "./retry.js";

/**
 * Client for the Developer Mode web installer (port 80): sideload, delete and
 * screenshot the dev channel. All endpoints require Digest auth.
 */

export class DevInstallerError extends Error {
  override name = "DevInstallerError";
}

export interface Screenshot {
  data: Buffer;
  extension: "png" | "jpg";
}

const INSTALL_OK = /Install Success|Identical to previous version|Application Received/i;

function stripHtml(html: string): string {
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

export class DevInstaller {
  constructor(private readonly config: OracleConfig) {}

  private get baseUrl(): string {
    return `http://${this.config.host}`;
  }

  private async submit(path: string, fields: Record<string, string>, archive?: Uint8Array) {
    const response = await withConnectRetry(() =>
      digestFetch(
        `${this.baseUrl}${path}`,
        () => {
          const form = new FormData();
          for (const [key, value] of Object.entries(fields)) form.append(key, value);
          // The installer form always carries an "archive" part, even when empty.
          form.append(
            "archive",
            new Blob([archive ?? new Uint8Array()], { type: "application/zip" }),
            archive ? "channel.zip" : "",
          );
          return { method: "POST", body: form };
        },
        { user: this.config.devUser, password: this.config.devPassword },
      ),
    );
    const body = await response.text();
    if (response.status === 401) {
      throw new DevInstallerError("Device rejected credentials (check ROKU_DEV_PASSWORD)");
    }
    if (!response.ok) {
      throw new DevInstallerError(
        `${path} failed with HTTP ${response.status}: ${stripHtml(body)}`,
      );
    }
    return body;
  }

  /** Removes the sideloaded channel (also wipes its registry, giving probes a clean state). */
  async remove(): Promise<void> {
    await this.submit("/plugin_install", { mysubmit: "Delete" });
  }

  /** Sideloads a package. The device launches the channel automatically on success. */
  async install(archive: Uint8Array): Promise<void> {
    const body = await this.submit("/plugin_install", { mysubmit: "Install" }, archive);
    if (!INSTALL_OK.test(body)) {
      throw new DevInstallerError(`Install failed: ${stripHtml(body).slice(0, 500)}`);
    }
  }

  /** Captures the current screen. Format (png/jpg) depends on the firmware. */
  async screenshot(): Promise<Screenshot> {
    const body = await this.submit("/plugin_inspect", { mysubmit: "Screenshot" });
    const match = /pkgs\/dev\.(png|jpg)/i.exec(body);
    if (!match?.[1]) {
      throw new DevInstallerError(
        "Screenshot failed: no image reference (is a dev channel running?)",
      );
    }
    const extension = match[1].toLowerCase() as Screenshot["extension"];
    const response = await withConnectRetry(() =>
      digestFetch(
        `${this.baseUrl}/pkgs/dev.${extension}?time=${Date.now()}`,
        () => ({ method: "GET" }),
        { user: this.config.devUser, password: this.config.devPassword },
      ),
    );
    if (!response.ok) {
      throw new DevInstallerError(`Screenshot download failed with HTTP ${response.status}`);
    }
    return { data: Buffer.from(await response.arrayBuffer()), extension };
  }
}
