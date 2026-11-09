import { describe, expect, it } from "vitest";
import { buildDigestAuthorization, parseDigestChallenge } from "../src/digest.js";

describe("digest auth", () => {
  it("parses a Roku-style challenge", () => {
    const challenge = parseDigestChallenge(
      'Digest qop="auth", realm="rokudev", nonce="1700000000"',
    );
    expect(challenge).toEqual({ qop: "auth", realm: "rokudev", nonce: "1700000000" });
  });

  it("rejects non-digest schemes and unsupported algorithms", () => {
    expect(() => parseDigestChallenge('Basic realm="x"')).toThrow(/Digest/);
    expect(() => parseDigestChallenge('Digest realm="x", nonce="n", algorithm=SHA-256')).toThrow(
      /Unsupported/,
    );
  });

  it("matches the RFC 2617 reference response", () => {
    // Reference vector from RFC 2617 section 3.5.
    const header = buildDigestAuthorization(
      {
        realm: "testrealm@host.com",
        nonce: "dcd98b7102dd2f0e8b11d0f600bfb0c093",
        qop: "auth,auth-int",
        opaque: "5ccc069c403ebaf9f0171e9517f40e41",
      },
      { user: "Mufasa", password: "Circle Of Life" },
      "GET",
      "/dir/index.html",
      "0a4f113b",
      1,
    );
    expect(header).toContain('response="6629fae49393a05397450978507c4ef1"');
    expect(header).toContain("nc=00000001");
    expect(header).toContain('opaque="5ccc069c403ebaf9f0171e9517f40e41"');
  });
});
