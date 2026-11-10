/** HTTP Digest authentication (RFC 7616), used by the developer web installer. */
import { createHash, randomBytes } from "node:crypto";

/**
 * Minimal HTTP Digest auth (RFC 7616, MD5 + qop=auth), which is what the Roku
 * Developer Mode web installer requires. Node's fetch has no built-in support.
 */

export interface DigestChallenge {
  realm: string;
  nonce: string;
  qop?: string;
  opaque?: string;
  algorithm?: string;
}

export interface DigestCredentials {
  user: string;
  password: string;
}

const md5 = (value: string): string => createHash("md5").update(value).digest("hex");

export function parseDigestChallenge(header: string): DigestChallenge {
  if (!/^digest\s/i.test(header)) {
    throw new Error("Server did not offer Digest authentication");
  }
  const params: Record<string, string> = {};
  for (const match of header.slice(7).matchAll(/(\w+)=(?:"([^"]*)"|([^,\s]+))/g)) {
    const key = match[1];
    if (key) params[key.toLowerCase()] = match[2] ?? match[3] ?? "";
  }
  const { realm, nonce } = params;
  if (realm === undefined || nonce === undefined) {
    throw new Error("Malformed Digest challenge: missing realm or nonce");
  }
  const algorithm = params.algorithm;
  if (algorithm !== undefined && algorithm.toUpperCase() !== "MD5") {
    throw new Error(`Unsupported Digest algorithm: ${algorithm}`);
  }
  const challenge: DigestChallenge = { realm, nonce };
  if (params.qop !== undefined) challenge.qop = params.qop;
  if (params.opaque !== undefined) challenge.opaque = params.opaque;
  if (algorithm !== undefined) challenge.algorithm = algorithm;
  return challenge;
}

export function buildDigestAuthorization(
  challenge: DigestChallenge,
  credentials: DigestCredentials,
  method: string,
  uri: string,
  cnonce = randomBytes(8).toString("hex"),
  nonceCount = 1,
): string {
  const ha1 = md5(`${credentials.user}:${challenge.realm}:${credentials.password}`);
  const ha2 = md5(`${method}:${uri}`);
  const supportsAuthQop = challenge.qop?.split(",").some((q) => q.trim() === "auth") ?? false;
  const nc = nonceCount.toString(16).padStart(8, "0");
  const response = supportsAuthQop
    ? md5(`${ha1}:${challenge.nonce}:${nc}:${cnonce}:auth:${ha2}`)
    : md5(`${ha1}:${challenge.nonce}:${ha2}`);

  const parts = [
    `username="${credentials.user}"`,
    `realm="${challenge.realm}"`,
    `nonce="${challenge.nonce}"`,
    `uri="${uri}"`,
    `response="${response}"`,
  ];
  if (supportsAuthQop) parts.push(`qop=auth`, `nc=${nc}`, `cnonce="${cnonce}"`);
  if (challenge.opaque !== undefined) parts.push(`opaque="${challenge.opaque}"`);
  if (challenge.algorithm !== undefined) parts.push(`algorithm=${challenge.algorithm}`);
  return `Digest ${parts.join(", ")}`;
}

/**
 * Performs a request, answering a 401 Digest challenge once. `makeInit` is a
 * factory because request bodies (FormData streams) cannot be sent twice.
 */
export async function digestFetch(
  url: string,
  makeInit: () => RequestInit,
  credentials: DigestCredentials,
  options: { challengeUrl?: string; signal?: AbortSignal } = {},
): Promise<Response> {
  let challenge: DigestChallenge | undefined;
  if (options.challengeUrl) {
    if (new URL(options.challengeUrl).origin !== new URL(url).origin)
      throw new Error("Digest challenge URL must share the request origin");
    // Discover authentication without sending an archive that the device may
    // reject and close mid-upload (EPIPE) before fetch receives its 401.
    const probe = await fetch(
      options.challengeUrl,
      options.signal ? { signal: options.signal } : {},
    );
    const header = probe.headers.get("www-authenticate");
    if (probe.status === 401 && header) challenge = parseDigestChallenge(header);
    await probe.body?.cancel();
  }
  const send = (challenge?: DigestChallenge) => {
    const init = makeInit();
    if (!challenge) return fetch(url, init);
    const method = (init.method ?? "GET").toUpperCase();
    const { pathname, search } = new URL(url);
    const headers = new Headers(init.headers);
    headers.set(
      "authorization",
      buildDigestAuthorization(challenge, credentials, method, `${pathname}${search}`),
    );
    return fetch(url, { ...init, headers });
  };
  const first = await send(challenge);
  if (first.status !== 401) return first;
  const header = first.headers.get("www-authenticate");
  if (header === null) return first;
  await first.body?.cancel();
  return send(parseDigestChallenge(header));
}
