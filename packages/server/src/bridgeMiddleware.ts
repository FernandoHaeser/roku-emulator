/**
 * Same-origin HTTP routes the web shell uses to talk to the device server,
 * mounted on the shell's dev server (Connect-style middleware).
 *
 * Security: the routes only answer requests whose Host is loopback (blocks DNS
 * rebinding), send no CORS headers (other origins cannot read responses or the
 * event stream), and POSTs require a custom header (other origins cannot send
 * it without a CORS preflight, which is never granted).
 */
import { httpRequest } from "./httpProxy";
import type { IncomingMessage, ServerResponse } from "node:http";
import {
  BRIDGE_HEADER,
  BRIDGE_PATH,
  type BridgeEvent,
  type ConsolePost,
  type ShellState,
} from "@roku-emu/protocol/bridge";
import type { DeviceBridge } from "./bridge";

const MAX_POST_BYTES = 12 * 1024 * 1024;
const KEEPALIVE_MS = 15_000;
const LOOPBACK_HOST = /^(127\.0\.0\.1|localhost|\[::1\])(:\d+)?$/i;

type Next = (error?: unknown) => void;

function readJson(request: IncomingMessage): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0;
    request.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > MAX_POST_BYTES) {
        reject(new Error("payload too large"));
        request.destroy();
        return;
      }
      chunks.push(chunk);
    });
    request.on("end", () => {
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString("utf8")));
      } catch (error) {
        reject(error);
      }
    });
    request.on("error", reject);
  });
}

function status(response: ServerResponse, code: number): void {
  response.writeHead(code, { "Content-Length": 0 });
  response.end();
}

export function createBridgeMiddleware(bridge: DeviceBridge) {
  return (request: IncomingMessage, response: ServerResponse, next: Next): void => {
    const url = new URL(request.url ?? "/", "http://shell");
    if (!url.pathname.startsWith(`${BRIDGE_PATH}/`)) {
      next();
      return;
    }
    if (!LOOPBACK_HOST.test(request.headers.host ?? "")) {
      status(response, 403);
      return;
    }
    const route = url.pathname.slice(BRIDGE_PATH.length);
    const method = request.method ?? "GET";

    if (method === "POST" && route === "/runtime") {
      if (request.headers[BRIDGE_HEADER] !== "1" || !bridge.runtimeReceiver) {
        status(response, 403);
        return;
      }
      void (async () => {
        const header = request.headers["x-roku-emu-message"];
        if (typeof header !== "string") throw new Error("Missing message");
        const message = JSON.parse(decodeURIComponent(header)) as { type?: unknown };
        if (
          !["key", "debuggerCommand", "stopChannel", "inspectChannel", "launchChannel"].includes(
            String(message.type),
          )
        )
          throw new Error("Invalid message");
        const chunks: Buffer[] = [];
        let size = 0;
        for await (const part of request) {
          const chunk = Buffer.from(part as Uint8Array);
          size += chunk.length;
          if (size > 256 * 1024 * 1024) throw new Error("Package too large");
          chunks.push(chunk);
        }
        bridge.runtimeReceiver?.(message, size ? Buffer.concat(chunks) : undefined);
        status(response, 204);
      })().catch(() => status(response, 400));
      return;
    }
    if (method === "GET" && route === "/file") {
      void bridge
        .rpc<string | null>({ op: "file", path: url.searchParams.get("uri") ?? "" })
        .then((data) => {
          if (data === null) {
            status(response, 404);
            return;
          }
          const bytes = Buffer.from(data, "base64");
          response.writeHead(200, {
            "Content-Type": "application/octet-stream",
            "Content-Length": bytes.length,
          });
          response.end(bytes);
        })
        .catch(() => status(response, 404));
      return;
    }

    if (method === "POST" && route === "/debug") {
      if (request.headers[BRIDGE_HEADER] !== "1") {
        status(response, 403);
        return;
      }
      void readJson(request)
        .then((body) => {
          if (
            !body ||
            typeof body !== "object" ||
            !["evaluate", "variables", "stack", "threads", "scene", "file", "breakpoints"].includes(
              String((body as { op?: unknown }).op),
            )
          )
            throw new Error("Invalid debugger request");
          return bridge.rpc(body as Record<string, unknown>);
        })
        .then((result) => {
          const text = JSON.stringify({ result });
          response.writeHead(200, {
            "Content-Type": "application/json",
            "Content-Length": Buffer.byteLength(text),
          });
          response.end(text);
        })
        .catch((error) => {
          const text = JSON.stringify({ error: String(error) });
          response.writeHead(400, { "Content-Type": "application/json" });
          response.end(text);
        });
      return;
    }

    if (method === "POST" && route === "/http") {
      if (request.headers[BRIDGE_HEADER] !== "1") {
        status(response, 403);
        return;
      }
      const controller = new AbortController();
      response.on("close", () => controller.abort());
      void readJson(request)
        .then((body) => httpRequest(body, controller.signal))
        .then((result) => {
          const body = JSON.stringify(result);
          response.writeHead(200, {
            "Content-Type": "application/json",
            "Content-Length": Buffer.byteLength(body),
          });
          response.end(body);
        })
        .catch(() => status(response, 400));
      return;
    }

    if (method === "GET" && route === "/events") {
      response.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
      });
      response.write("retry: 2000\n\n");
      const disconnect = bridge.connectShell((event: BridgeEvent) => {
        response.write(`data: ${JSON.stringify(event)}\n\n`);
      });
      const keepalive = setInterval(() => response.write(": keepalive\n\n"), KEEPALIVE_MS);
      request.on("close", () => {
        clearInterval(keepalive);
        disconnect();
      });
      return;
    }

    const packageRoute = /^\/package\/([a-f0-9]+)$/.exec(route);
    if (method === "GET" && packageRoute) {
      const installed = bridge.package;
      if (!installed || installed.id !== packageRoute[1]) {
        status(response, 404);
        return;
      }
      response.writeHead(200, {
        "Content-Type": "application/zip",
        "Content-Length": installed.zip.length,
        "Cache-Control": "no-store",
      });
      response.end(installed.zip);
      return;
    }

    if (
      method === "POST" &&
      (route === "/console" || route === "/state" || route === "/response")
    ) {
      if (request.headers[BRIDGE_HEADER] !== "1") {
        status(response, 403);
        return;
      }
      readJson(request).then(
        (body) => {
          if (body === null || typeof body !== "object") {
            status(response, 400);
            return;
          }
          if (route === "/response") {
            const value = body as { id?: unknown; result?: unknown; error?: unknown };
            if (typeof value.id !== "string") {
              status(response, 400);
              return;
            }
            bridge.resolveRpc({
              id: value.id,
              result: value.result,
              ...(typeof value.error === "string" ? { error: value.error } : {}),
            });
          } else if (route === "/console") {
            const text = (body as Partial<ConsolePost>).text;
            if (typeof text === "string") bridge.writeConsole(text);
          } else {
            const title = (body as Partial<ShellState>).runningTitle;
            bridge.setShellState({
              runningTitle: typeof title === "string" ? title : null,
              paused: (body as ShellState).paused === true,
            });
          }
          status(response, 204);
        },
        () => status(response, 400),
      );
      return;
    }
    status(response, 404);
  };
}
