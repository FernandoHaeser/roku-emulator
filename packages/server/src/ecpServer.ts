/**
 * HTTP server answering like a device's ECP (port 8060) and developer
 * installer (port 80). Both live on one port here; the VS Code extension and
 * roku-deploy take them as `remotePort` / `packagePort`.
 */
import { appUiXml, sceneNodes, type SceneInspection } from "./sceneInspection";
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { DeviceBridge } from "./bridge";
import type { DeviceServerConfig } from "./config";
import { deviceInfoXml, emulatorDeviceInfo, type DeviceProfile } from "./deviceInfo";
import { DigestAuthenticator } from "./digestAuth";
import { remoteKeyFromEcp } from "./ecpKeys";
import { installerPage, type InstallerMessage } from "./installerPage";
import { boundaryOf, parseMultipart } from "./multipart";
import { zipEntryNames } from "./zip";

/** Same cap as the engine's package reader (crates/brs-core/src/channel.rs). */
const MAX_UPLOAD_BYTES = 256 * 1024 * 1024;

const XML_TYPE = 'text/xml; charset="utf-8"';

function escapeXml(text: string): string {
  return text.replace(/[<>&"']/g, (char) => `&#${char.charCodeAt(0)};`);
}

function send(response: ServerResponse, status: number, body = "", type = "text/plain"): void {
  response.writeHead(status, { "Content-Type": type, "Content-Length": Buffer.byteLength(body) });
  response.end(body);
}

/** Reads the request body, rejecting anything larger than `limit`. */
function readBody(request: IncomingMessage, limit: number): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0;
    request.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > limit) {
        reject(new Error("payload too large"));
        request.destroy();
        return;
      }
      chunks.push(chunk);
    });
    request.on("end", () => resolve(Buffer.concat(chunks)));
    request.on("error", reject);
  });
}

export class EcpServer {
  private readonly server: Server;
  private readonly startedAt = Date.now();
  private screenshot: Buffer | null = null;
  private readonly auth: DigestAuthenticator;

  constructor(
    private readonly bridge: DeviceBridge,
    config: DeviceServerConfig,
    private readonly profile: DeviceProfile,
  ) {
    this.auth = new DigestAuthenticator(config.username, config.password);
    this.server = createServer((request, response) => {
      this.handle(request, response).catch(() => {
        if (!response.headersSent) send(response, 500, "Internal error");
        else response.destroy();
      });
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
    return new Promise((resolve) => {
      this.server.closeAllConnections();
      this.server.close(() => resolve());
    });
  }

  private async handle(request: IncomingMessage, response: ServerResponse): Promise<void> {
    const url = new URL(request.url ?? "/", "http://device");
    const path = url.pathname;
    const method = request.method ?? "GET";

    if (path === "/pkgs/dev.jpg" && method === "GET") {
      if (!this.screenshot) {
        send(response, 404);
        return;
      }
      response.writeHead(200, {
        "Content-Type": "image/jpeg",
        "Content-Length": this.screenshot.length,
      });
      response.end(this.screenshot);
      return;
    }
    if (method === "GET" && path === "/query/app-ui") {
      const scene = await this.bridge.rpc<SceneInspection>({ op: "scene" });
      send(response, 200, appUiXml(scene, this.bridge.shellState.runningTitle ?? "Roku"), XML_TYPE);
      return;
    }
    if (method === "GET" && path === "/query/media-player") {
      send(response, 200, '<player state="close"><error>0</error></player>', XML_TYPE);
      return;
    }
    if (method === "GET" && path === "/query/chanperf") {
      const scene = await this.bridge.rpc<SceneInspection>({ op: "scene" });
      send(
        response,
        200,
        `<chanperf><nodes>${sceneNodes(scene).length}</nodes><threads>${scene.threads}</threads><uptime>${scene.timeMs}</uptime></chanperf>`,
        XML_TYPE,
      );
      return;
    }
    if (path === "/" || path === "/plugin_install" || path === "/plugin_inspect") {
      await this.handleInstaller(request, response, method);
      return;
    }
    if (method === "GET" && path === "/query/device-info") {
      const info = emulatorDeviceInfo(this.profile, (Date.now() - this.startedAt) / 1000);
      send(response, 200, deviceInfoXml(info), XML_TYPE);
      return;
    }
    if (method === "GET" && path === "/query/active-app") {
      send(response, 200, this.activeAppXml(), XML_TYPE);
      return;
    }
    if (method === "GET" && path === "/query/apps") {
      send(response, 200, this.appsXml(), XML_TYPE);
      return;
    }
    const keyRoute = /^\/(keypress|keydown|keyup)\/([^/]+)$/.exec(path);
    if (method === "POST" && keyRoute) {
      const key = remoteKeyFromEcp(decodeURIComponent(keyRoute[2]!));
      if (key) {
        const kind = keyRoute[1];
        if (kind === "keypress" || kind === "keydown") this.bridge.key(key, true);
        if (kind === "keypress" || kind === "keyup") this.bridge.key(key, false);
      }
      // The device answers 200 even for keys it ignores.
      send(response, 200);
      return;
    }
    if (method === "POST" && path === "/input") {
      this.bridge.input(Object.fromEntries(url.searchParams));
      send(response, 200);
      return;
    }
    const launchRoute = /^\/launch\/([^/]+)$/.exec(path);
    if (method === "POST" && launchRoute) {
      const launched =
        launchRoute[1] === "dev" && this.bridge.launchDev(Object.fromEntries(url.searchParams));
      send(response, launched ? 200 : 404);
      return;
    }
    send(response, 404);
  }

  private activeAppXml(): string {
    const title = this.bridge.shellState.runningTitle;
    const app =
      title === null
        ? "\t<app>Roku</app>"
        : `\t<app id="dev" type="appl" version="1.0.0">${escapeXml(title)}</app>`;
    return `<?xml version="1.0" encoding="UTF-8" ?>\n<active-app>\n${app}\n</active-app>\n`;
  }

  private appsXml(): string {
    const installed = this.bridge.package;
    const app = installed
      ? `\t<app id="dev" type="appl" version="1.0.0">${escapeXml(installed.fileName)}</app>\n`
      : "";
    return `<?xml version="1.0" encoding="UTF-8" ?>\n<apps>\n${app}</apps>\n`;
  }

  private async handleInstaller(
    request: IncomingMessage,
    response: ServerResponse,
    method: string,
  ): Promise<void> {
    if (!this.auth.verify(method, request.url ?? "", request.headers.authorization)) {
      // Never read an unauthenticated body; drain it so the socket stays usable.
      request.resume();
      response.writeHead(401, {
        "WWW-Authenticate": this.auth.challenge(),
        "Content-Type": "text/plain",
        "Content-Length": 0,
      });
      response.end();
      return;
    }
    if (method === "HEAD" || method === "GET") {
      send(response, 200, method === "GET" ? installerPage([]) : "", "text/html");
      return;
    }
    if (method !== "POST") {
      send(response, 405);
      return;
    }
    const boundary = boundaryOf(request.headers["content-type"]);
    let body: Buffer;
    try {
      body = await readBody(request, MAX_UPLOAD_BYTES);
    } catch {
      send(response, 413, "Package too large");
      return;
    }
    const form = boundary ? parseMultipart(body, boundary) : new Map();
    const action = form.get("mysubmit")?.data.toString("utf8") ?? "";
    if (action === "Screenshot") {
      try {
        this.screenshot = Buffer.from(
          await this.bridge.rpc<string>({ op: "screenshot" }),
          "base64",
        );
        send(response, 200, '<html><img src="pkgs/dev.jpg"></html>', "text/html");
      } catch {
        send(
          response,
          200,
          installerPage([{ type: "error", text: "Screenshot unavailable" }]),
          "text/html",
        );
      }
      return;
    }
    const messages = this.runInstallerAction(
      action,
      form.get("archive"),
      form.get("remotedebug")?.data.toString() === "1",
    );
    send(response, 200, installerPage(messages), "text/html");
  }

  private runInstallerAction(
    action: string,
    archive: { filename: string | null; data: Buffer } | undefined,
    debug = false,
  ): InstallerMessage[] {
    switch (action) {
      case "Install":
      case "Replace": {
        if (!archive || archive.data.length === 0) {
          return [{ type: "error", text: "Failure: No file selected." }];
        }
        const names = zipEntryNames(archive.data);
        if (!names) {
          return [
            {
              type: "error",
              text: "Install Failure: Unzip failed. Invalid or corrupt zip archive.  Unloading.",
            },
          ];
        }
        if (!names.some((name) => name.replace(/^\.?\//, "") === "manifest")) {
          return [{ type: "error", text: "Install Failure: No manifest. Invalid package." }];
        }
        this.bridge.install(archive.filename || "dev.zip", archive.data, debug);
        return [
          { type: "info", text: `Application Received: ${archive.data.length} bytes stored.` },
          { type: "success", text: "Install Success." },
        ];
      }
      case "Delete":
      case "DeleteAll":
        this.bridge.deletePackage();
        return [{ type: "success", text: "Delete Succeeded." }];
      default:
        return [{ type: "error", text: `Unsupported installer action: ${action || "(none)"}` }];
    }
  }
}
