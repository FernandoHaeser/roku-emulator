import { describe, expect, it } from "vitest";
import { parseDeviceInfo, profileId, REDACTED_VALUE, redactProfile } from "../src/profile.js";

const xml = `<?xml version="1.0" encoding="UTF-8" ?>
<device-info>
  <udn>29380007-0800-1025-80a4-d83154332d7e</udn>
  <serial-number>X00000000000</serial-number>
  <device-id>S00000000000</device-id>
  <advertising-id>aaaa-bbbb</advertising-id>
  <wifi-mac>d8:31:34:33:2d:7e</wifi-mac>
  <network-name>HomeWifi</network-name>
  <friendly-device-name>Living Room</friendly-device-name>
  <default-device-name>AOC Roku TV - X00000000000</default-device-name>
  <some-label>contains s00000000000 id</some-label>
  <uptime>408575</uptime>
  <model-number>3941X</model-number>
  <model-name>Roku Express</model-name>
  <software-version>14.0.4</software-version>
  <ui-resolution>1080p</ui-resolution>
  <developer-enabled>true</developer-enabled>
</device-info>`;

describe("device profile", () => {
  it("redacts identifiers but keeps capability fields", () => {
    const profile = redactProfile(parseDeviceInfo(xml));
    for (const key of [
      "udn",
      "serial-number",
      "device-id",
      "advertising-id",
      "wifi-mac",
      "network-name",
      "friendly-device-name",
    ]) {
      expect(profile[key]).toBe(REDACTED_VALUE);
    }
    // Identifiers embedded inside other fields are redacted by value (case-insensitive).
    expect(profile["default-device-name"]).toBe(REDACTED_VALUE);
    expect(profile["some-label"]).toBe(REDACTED_VALUE);
    // Volatile fields are dropped so the committed profile stays stable.
    expect(profile).not.toHaveProperty("uptime");
    expect(profile["model-number"]).toBe("3941X");
    expect(profile["software-version"]).toBe("14.0.4");
    expect(profile["ui-resolution"]).toBe("1080p");
    expect(profileId(profile)).toBe("3941X-14.0.4");
  });

  it("rejects non device-info documents", () => {
    expect(() => parseDeviceInfo("<foo/>")).toThrow(/device-info/);
  });
});
