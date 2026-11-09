/**
 * ECP `query/device-info` for the emulator: the reference device's profile
 * (profiles/*.json, identifiers redacted) with emulator values in place of the
 * redacted fields, so tools see a plausible device that is clearly not real
 * hardware.
 */
import { readFileSync } from "node:fs";

export type DeviceProfile = Record<string, string>;

const REDACTED = "<redacted>";
const EMULATOR_NAME = "Roku Emulator";

/** Replacements for fields the committed profile redacts. */
const EMULATOR_VALUES: Readonly<Record<string, string>> = {
  udn: "00000000-0000-4000-8000-000000000000",
  "serial-number": "EMULATOR0000",
  "device-id": "EMULATOR0000",
  "advertising-id": "00000000-0000-4000-8000-000000000000",
  "wifi-mac": "00:00:00:00:00:00",
  "ethernet-mac": "00:00:00:00:00:00",
  "network-name": "emulator",
  "friendly-device-name": EMULATOR_NAME,
  "friendly-model-name": EMULATOR_NAME,
  "default-device-name": EMULATOR_NAME,
  "user-device-name": EMULATOR_NAME,
  "user-device-location": "",
  "keyed-developer-id": "",
};

export function loadReferenceProfile(): DeviceProfile {
  return JSON.parse(
    readFileSync(new URL("../../../profiles/H302X-16.0.4.json", import.meta.url), "utf8"),
  ) as DeviceProfile;
}

function escapeXml(text: string): string {
  return text.replace(/[<>&"']/g, (char) => `&#${char.charCodeAt(0)};`);
}

/** Emulator device-info fields, in the profile's order. */
export function emulatorDeviceInfo(profile: DeviceProfile, uptimeSeconds: number): DeviceProfile {
  const info: DeviceProfile = {};
  for (const [key, value] of Object.entries(profile)) {
    info[key] = value === REDACTED ? (EMULATOR_VALUES[key] ?? "") : value;
  }
  // The emulator is always a developer device with ECP fully enabled.
  info["developer-enabled"] = "true";
  info["ecp-setting-mode"] = "permissive";
  info["network-type"] = "ethernet";
  // Volatile fields the profile drops.
  info["power-mode"] = "PowerOn";
  info.uptime = String(Math.floor(uptimeSeconds));
  return info;
}

export function deviceInfoXml(info: DeviceProfile): string {
  const lines = Object.entries(info).map(
    ([key, value]) => `\t<${key}>${escapeXml(value)}</${key}>`,
  );
  return `<?xml version="1.0" encoding="UTF-8" ?>\n<device-info>\n${lines.join("\n")}\n</device-info>\n`;
}
