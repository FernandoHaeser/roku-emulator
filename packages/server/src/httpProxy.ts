/** Local, bounded HTTP transport for sandboxed channel code. Never executes URL schemes. */
import { Agent, fetch as transportFetch } from "undici";
import { createHash, randomBytes } from "node:crypto";
import { parseDigestHeader } from "./digestAuth";
export interface HttpRequest {
  url: string;
  method?: string;
  headers?: Record<string, string>;
  body?: string;
  timeout?: number;
  username?: string;
  password?: string;
  ca?: string;
  peerVerify?: boolean;
  hostVerify?: boolean;
}
export interface HttpResponse {
  status: number;
  headers: Record<string, string>;
  headersArray: Record<string, string>[];
  body: string;
  failure: string;
}
const MAX_BODY = 8 * 1024 * 1024;
function validateUrl(value: string): URL {
  const url = new URL(value);
  if (
    !["http:", "https:"].includes(url.protocol) ||
    url.username ||
    url.password ||
    /^(169\.254\.|\[?fe80:|metadata\.google\.internal$)/i.test(url.hostname)
  )
    throw new Error("Unsupported URL");
  return url;
}
/** Fetch wraps resolver errors in cause (or AggregateError for multiple addresses). */
function isDnsFailure(error: unknown): boolean {
  const pending: unknown[] = [error];
  const visited = new Set<unknown>();
  while (pending.length) {
    const current = pending.pop();
    if (!current || typeof current !== "object" || visited.has(current)) continue;
    visited.add(current);
    const detail = current as { code?: unknown; cause?: unknown; errors?: unknown };
    if (detail.code === "ENOTFOUND" || detail.code === "EAI_AGAIN") return true;
    pending.push(detail.cause);
    if (Array.isArray(detail.errors)) pending.push(...detail.errors);
  }
  return false;
}

export async function httpRequest(
  input: unknown,
  cancellation?: AbortSignal,
): Promise<HttpResponse> {
  let request: HttpRequest;
  try {
    if (!input || typeof input !== "object" || typeof (input as HttpRequest).url !== "string")
      throw new Error("Invalid HTTP request");
    request = input as HttpRequest;
    validateUrl(request.url);
  } catch {
    return {
      status: -3,
      headers: {},
      headersArray: [],
      body: "",
      failure: "Malformed or unsupported HTTP URL",
    };
  }
  const timeout = Number.isFinite(request.timeout)
    ? Math.max(1, Math.min(60_000, request.timeout!))
    : 30_000;
  const signal = cancellation
    ? AbortSignal.any([cancellation, AbortSignal.timeout(timeout)])
    : AbortSignal.timeout(timeout);
  let url = validateUrl(request.url);
  const originalOrigin = url.origin;
  let method = (request.method || "GET").toUpperCase();
  let body = Buffer.from(typeof request.body === "string" ? request.body : "", "base64");
  if (body.length > MAX_BODY || !/^[A-Z]+$/.test(method))
    return {
      status: -3,
      headers: {},
      headersArray: [],
      body: "",
      failure: "Invalid HTTP request body or method",
    };
  const headers = new Headers();
  for (const [name, value] of Object.entries(request.headers ?? {})) {
    if (
      typeof value === "string" &&
      !/^(host|connection|transfer-encoding|content-length|upgrade|proxy-.*)$/i.test(name)
    )
      headers.set(name, value);
  }
  const dispatcher = new Agent({
    connect: {
      ...(typeof request.ca === "string" && request.ca ? { ca: request.ca } : {}),
      rejectUnauthorized: request.peerVerify !== false,
      ...(request.hostVerify === false ? { checkServerIdentity: () => undefined } : {}),
    },
  });
  try {
    let authenticated = false;
    for (let redirect = 0; redirect <= 10; redirect++) {
      const response = await transportFetch(url, {
        dispatcher,
        method,
        headers: Object.fromEntries(headers),
        ...(method === "GET" || method === "HEAD" ? {} : { body }),
        redirect: "manual",
        signal,
      });
      if (
        response.status === 401 &&
        !authenticated &&
        request.username &&
        url.origin === originalOrigin
      ) {
        const challenge = response.headers.get("www-authenticate") || "";
        if (/^Digest /i.test(challenge)) {
          const auth = parseDigestHeader(challenge);
          const algorithm = (auth.get("algorithm") || "MD5").toUpperCase();
          const hash = (s: string) =>
            createHash(algorithm.startsWith("SHA-256") ? "sha256" : "md5")
              .update(s)
              .digest("hex");
          const realm = auth.get("realm") || "",
            nonce = auth.get("nonce") || "",
            uri = url.pathname + url.search,
            cnonce = randomBytes(12).toString("hex"),
            nc = "00000001";
          const qop = (auth.get("qop") || "")
            .split(",")
            .map((s) => s.trim())
            .includes("auth")
            ? "auth"
            : "";
          const ha1 = hash(`${request.username}:${realm}:${request.password || ""}`),
            ha2 = hash(`${method}:${uri}`);
          const digest = qop
            ? hash(`${ha1}:${nonce}:${nc}:${cnonce}:${qop}:${ha2}`)
            : hash(`${ha1}:${nonce}:${ha2}`);
          const quote = (s: string) => `"${s.replace(/["\\\r\n]/g, "")}"`;
          headers.set(
            "Authorization",
            `Digest username=${quote(request.username)}, realm=${quote(realm)}, nonce=${quote(nonce)}, uri=${quote(uri)}, response=${quote(digest)}${qop ? `, qop=auth, nc=${nc}, cnonce=${quote(cnonce)}` : ""}${auth.get("opaque") ? `, opaque=${quote(auth.get("opaque")!)}` : ""}`,
          );
          authenticated = true;
          await response.body?.cancel();
          continue;
        }
      }
      if ([301, 302, 303, 307, 308].includes(response.status) && response.headers.has("location")) {
        const next = validateUrl(new URL(response.headers.get("location")!, url).href);
        if (next.origin !== url.origin) {
          headers.delete("Authorization");
          headers.delete("Cookie");
        }
        if (
          response.status === 303 ||
          ((response.status === 301 || response.status === 302) && method === "POST")
        ) {
          method = "GET";
          body = Buffer.alloc(0);
        }
        url = next;
        await response.body?.cancel();
        continue;
      }
      const parts: Uint8Array[] = [];
      let size = 0;
      if (response.body) {
        const reader = response.body.getReader();
        for (;;) {
          const part = await reader.read();
          if (part.done) break;
          size += part.value.length;
          if (size > MAX_BODY) {
            await reader.cancel();
            throw new Error("HTTP response exceeds 8 MiB");
          }
          parts.push(part.value);
        }
      }
      const result: HttpResponse = {
        status: response.status,
        headers: Object.fromEntries(response.headers),
        headersArray: [],
        body: Buffer.concat(parts).toString("base64"),
        failure: "",
      };
      for (const cookie of response.headers.getSetCookie())
        result.headersArray.push({ "set-cookie": cookie });
      for (const [name, value] of response.headers)
        if (name !== "set-cookie") result.headersArray.push({ [name]: value });
      return result;
    }
    throw new Error("Too many HTTP redirects");
  } catch (error) {
    return {
      status: signal.aborted ? -28 : isDnsFailure(error) ? -6 : -7,
      headers: {},
      headersArray: [],
      body: "",
      failure: error instanceof Error ? error.message : "HTTP transport failed",
    };
  } finally {
    await dispatcher.close();
  }
}
