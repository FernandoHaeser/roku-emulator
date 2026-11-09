import { describe, expect, it } from "vitest";
import { parseOracleConfig } from "../src/config.js";

const base = { ROKU_HOST: "192.168.0.42", ROKU_DEV_PASSWORD: "secret" };

describe("parseOracleConfig", () => {
  it("applies defaults", () => {
    expect(parseOracleConfig(base)).toEqual({
      host: "192.168.0.42",
      devUser: "rokudev",
      devPassword: "secret",
      ecpPort: 8060,
      consolePort: 8085,
      probeTimeoutMs: 20000,
    });
  });

  it("requires host and password", () => {
    expect(() => parseOracleConfig({ ROKU_DEV_PASSWORD: "x" })).toThrow(/ROKU_HOST/);
    expect(() => parseOracleConfig({ ROKU_HOST: "10.0.0.1" })).toThrow(/ROKU_DEV_PASSWORD/);
  });

  it("rejects hosts carrying a scheme, path or port", () => {
    for (const host of ["http://10.0.0.1", "10.0.0.1/evil", "10.0.0.1:80", "a b"]) {
      expect(() => parseOracleConfig({ ...base, ROKU_HOST: host })).toThrow(/ROKU_HOST/);
    }
  });

  it("validates ports", () => {
    expect(() => parseOracleConfig({ ...base, ROKU_ECP_PORT: "99999" })).toThrow(/port/);
  });
});
