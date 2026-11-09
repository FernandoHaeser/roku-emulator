/** Device profile from `query/device-info`, with identifiers redacted and volatile fields dropped. */
import { XMLParser } from "fast-xml-parser";

/**
 * Device profile captured from ECP /query/device-info. Profiles are committed to
 * the repo, so every identifier that could single out the physical device or its
 * network is redacted before writing.
 */

export type DeviceProfile = Record<string, string>;

/** Fields whose names match any of these are replaced with REDACTED_VALUE. */
const SENSITIVE_FIELD_PATTERNS = [
  /^udn$/,
  /serial/,
  /device-id/,
  /advertising-id/,
  /mac$/,
  /network-name/,
  /ssid/,
  /friendly/,
  /^user-device/,
  /device-name/,
  /developer-id/,
  /uuid/,
  /(^|-)ip(-|$)/,
  /grandcentral/,
  /davinci/,
];

/**
 * Fields that change between captures without describing the device's
 * capabilities. Dropped so the committed profile only changes on real updates.
 */
const VOLATILE_FIELDS = new Set(["uptime", "power-mode", "headphones-connected"]);

/** Identifiers shorter than this are not used for value-based redaction (too many false hits). */
const MIN_IDENTIFIER_LENGTH = 6;

export const REDACTED_VALUE = "<redacted>";

export function parseDeviceInfo(xml: string): DeviceProfile {
  const parser = new XMLParser({ parseTagValue: false, trimValues: true });
  const document = parser.parse(xml) as { "device-info"?: Record<string, unknown> };
  const info = document["device-info"];
  if (info === undefined || typeof info !== "object") {
    throw new Error("Response is not an ECP device-info document");
  }
  const profile: DeviceProfile = {};
  for (const [key, value] of Object.entries(info)) {
    profile[key] = value === undefined || value === null ? "" : String(value);
  }
  return profile;
}

export function isSensitiveField(name: string): boolean {
  return SENSITIVE_FIELD_PATTERNS.some((pattern) => pattern.test(name));
}

/**
 * Redacts by field name AND by value: firmware embeds identifiers inside other
 * fields (e.g. default-device-name = "AOC•Roku TV - <serial>"), so any value that
 * contains a sensitive identifier is redacted too. Volatile fields are dropped.
 */
export function redactProfile(profile: DeviceProfile): DeviceProfile {
  const identifiers = Object.entries(profile)
    .filter(([key, value]) => isSensitiveField(key) && value.length >= MIN_IDENTIFIER_LENGTH)
    .map(([, value]) => value.toLowerCase());

  const containsIdentifier = (value: string): boolean => {
    const lower = value.toLowerCase();
    return identifiers.some((identifier) => lower.includes(identifier));
  };

  return Object.fromEntries(
    Object.entries(profile)
      .filter(([key]) => !VOLATILE_FIELDS.has(key))
      .map(([key, value]) => [
        key,
        isSensitiveField(key) || containsIdentifier(value) ? REDACTED_VALUE : value,
      ]),
  );
}

/** File-system-safe profile id, e.g. "3941X-14.0.4". */
export function profileId(profile: DeviceProfile): string {
  const model = profile["model-number"] ?? "unknown-model";
  const version = profile["software-version"] ?? "unknown-os";
  return `${model}-${version}`.replace(/[^a-zA-Z0-9._-]/g, "_");
}
