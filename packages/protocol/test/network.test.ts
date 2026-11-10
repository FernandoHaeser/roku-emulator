import { describe, expect, it } from "vitest";
import { networkStart, networkFinish } from "../src/network";
describe("network-tool metadata", () => {
  it("does not retain account credentials, payloads, query values or fragments", () => {
    const entry = networkStart(
      1,
      JSON.stringify({
        url: "https://user:secret@example.com/api?token=private&token=again&q=search#fragment",
        method: "post",
        headers: { Authorization: "Bearer secret" },
        body: "sensitive",
      }),
      10,
    );
    expect(entry.method).toBe("POST");
    expect(entry.url).toContain("example.com/api?");
    for (const secret of ["user", "secret", "private", "again", "search", "fragment", "sensitive"])
      expect(JSON.stringify(entry)).not.toContain(secret);
  });
  it("reports response bytes, errors and elapsed time without retaining bodies", () => {
    const entry = networkStart(2, '{"url":"https://example.com"}', 100);
    const response = networkFinish(entry, '{"status":404,"body":"dGVzdA=="}', 125);
    expect(response).toMatchObject({ status: 404, responseBytes: 4, durationMs: 25 });
    expect(networkFinish(entry, "invalid", 90)).toMatchObject({ status: -7, durationMs: 0 });
    expect(networkStart(3, "invalid", 0).url).toBe("Invalid request");
  });
});
