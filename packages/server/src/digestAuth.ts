/**
 * Server side of HTTP Digest authentication (RFC 7616 / 2617, MD5, qop=auth),
 * as used by the device's developer installer. roku-deploy first sends a
 * bodyless probe, reads the challenge, then posts the package with the
 * `Authorization` header.
 */
import { createHash, randomBytes, timingSafeEqual } from "node:crypto";

const REALM = "rokudev";
/** Nonces expire so a captured header cannot be replayed indefinitely. */
const NONCE_TTL_MS = 5 * 60_000;
const MAX_NONCES = 256;

const md5 = (value: string) => createHash("md5").update(value).digest("hex");

/** Parses `Digest key=value, key="value", ...`. */
export function parseDigestHeader(header: string): Map<string, string> {
  const fields = new Map<string, string>();
  const body = header.replace(/^Digest\s+/i, "");
  const pattern = /([a-z0-9_-]+)\s*=\s*(?:"((?:[^"\\]|\\.)*)"|([^\s,]+))/gi;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(body))) {
    fields.set(match[1]!.toLowerCase(), match[2] ?? match[3] ?? "");
  }
  return fields;
}

function safeEqual(a: string, b: string): boolean {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  return left.length === right.length && timingSafeEqual(left, right);
}

export class DigestAuthenticator {
  private readonly nonces = new Map<string, number>();

  constructor(
    private readonly username: string,
    private readonly password: string,
    private readonly now: () => number = Date.now,
  ) {}

  /** `WWW-Authenticate` header value with a fresh nonce. */
  challenge(): string {
    this.prune();
    const nonce = randomBytes(16).toString("hex");
    this.nonces.set(nonce, this.now());
    return `Digest realm="${REALM}", nonce="${nonce}", qop="auth"`;
  }

  /** True when `authorization` is a valid response for `method` on `requestUri`. */
  verify(method: string, requestUri: string, authorization: string | undefined): boolean {
    if (!authorization?.toLowerCase().startsWith("digest ")) return false;
    const fields = parseDigestHeader(authorization);
    const nonce = fields.get("nonce") ?? "";
    const issuedAt = this.nonces.get(nonce);
    if (issuedAt === undefined || this.now() - issuedAt > NONCE_TTL_MS) return false;
    const uri = fields.get("uri") ?? "";
    if (fields.get("username") !== this.username || fields.get("realm") !== REALM) return false;
    // The digest covers the URI: it must be the one actually requested.
    if (uri !== requestUri) return false;
    const ha1 = md5(`${this.username}:${REALM}:${this.password}`);
    const ha2 = md5(`${method.toUpperCase()}:${uri}`);
    const qop = fields.get("qop");
    const expected =
      qop === "auth"
        ? md5(`${ha1}:${nonce}:${fields.get("nc") ?? ""}:${fields.get("cnonce") ?? ""}:auth:${ha2}`)
        : md5(`${ha1}:${nonce}:${ha2}`);
    return safeEqual(expected, fields.get("response") ?? "");
  }

  private prune(): void {
    const now = this.now();
    for (const [nonce, issuedAt] of this.nonces) {
      if (now - issuedAt > NONCE_TTL_MS) this.nonces.delete(nonce);
    }
    while (this.nonces.size >= MAX_NONCES) {
      const oldest = this.nonces.keys().next().value;
      if (oldest === undefined) break;
      this.nonces.delete(oldest);
    }
  }
}
