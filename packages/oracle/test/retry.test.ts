import { describe, expect, it, vi } from "vitest";
import { isConnectError, withConnectRetry } from "../src/retry.js";

const connectError = () =>
  new TypeError("fetch failed", {
    cause: Object.assign(new Error("Connect Timeout Error"), { code: "UND_ERR_CONNECT_TIMEOUT" }),
  });

describe("withConnectRetry", () => {
  it("retries connection-phase failures until success", async () => {
    const operation = vi
      .fn<() => Promise<string>>()
      .mockRejectedValueOnce(connectError())
      .mockResolvedValueOnce("ok");
    await expect(withConnectRetry(operation, { attempts: 3, baseDelayMs: 1 })).resolves.toBe("ok");
    expect(operation).toHaveBeenCalledTimes(2);
  });

  it("never retries errors raised after the request was sent", async () => {
    const reset = Object.assign(new Error("socket hang up"), { code: "ECONNRESET" });
    expect(isConnectError(reset)).toBe(false);
    const operation = vi.fn<() => Promise<string>>().mockRejectedValue(reset);
    await expect(withConnectRetry(operation, { attempts: 3, baseDelayMs: 1 })).rejects.toBe(reset);
    expect(operation).toHaveBeenCalledTimes(1);
  });

  it("gives up after the configured attempts", async () => {
    const operation = vi.fn<() => Promise<string>>().mockRejectedValue(connectError());
    await expect(withConnectRetry(operation, { attempts: 2, baseDelayMs: 1 })).rejects.toThrow();
    expect(operation).toHaveBeenCalledTimes(2);
  });
});
