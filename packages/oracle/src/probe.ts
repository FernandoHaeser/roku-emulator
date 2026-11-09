/** Probe definition: `probe.json` spec, console markers and crash detection. */
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { isRemoteKey, type RemoteKey } from "@roku-emu/protocol/keys";

/**
 * A probe is a tiny channel that exercises one behavior and prints its
 * observable output between two markers on the BrightScript console.
 */

export const PROBE_BEGIN_MARKER = "@@PROBE_BEGIN";
export const PROBE_END_MARKER = "@@PROBE_END";

/**
 * Text the device prints when a channel crashes into the debugger. Non-fatal
 * diagnostics such as "BRIGHTSCRIPT: ERROR: roArray.Join: ..." must NOT match.
 */
const CRASH_PATTERN =
  /BrightScript Micro Debugger\.|\(runtime error &h[0-9a-f]+\)|\(compile error &h[0-9a-f]+\)/i;

/** END marker only counts once its line is complete (TCP can split the newline off). */
const END_LINE_PATTERN = new RegExp(`${PROBE_END_MARKER}\r?\n`);

export type ProbeKind = "conformance" | "bench";

export interface ProbeStep {
  key: RemoteKey;
  /** Delay after the keypress before the next step. */
  waitMs: number;
}

export interface ProbeSpec {
  /** conformance = golden must match exactly; bench = metrics, never diffed. */
  kind: ProbeKind;
  steps: ProbeStep[];
  /** Wait after the BEGIN marker before the first step (channel UI settling). */
  startDelayMs: number;
  /** Wait after the last step before capturing app-ui / screenshot. */
  settleMs: number;
  capture: { appUi: boolean; screenshot: boolean };
}

const DEFAULT_SPEC: ProbeSpec = {
  kind: "conformance",
  steps: [],
  startDelayMs: 1000,
  settleMs: 0,
  capture: { appUi: false, screenshot: false },
};

export class ProbeSpecError extends Error {
  override name = "ProbeSpecError";
}

export function parseProbeSpec(raw: unknown): ProbeSpec {
  if (raw === undefined) return structuredClone(DEFAULT_SPEC);
  if (typeof raw !== "object" || raw === null)
    throw new ProbeSpecError("probe.json must be an object");
  const input = raw as Record<string, unknown>;

  const kind = input.kind ?? DEFAULT_SPEC.kind;
  if (kind !== "conformance" && kind !== "bench") {
    throw new ProbeSpecError(`Invalid probe kind: ${String(kind)}`);
  }

  const stepsInput = input.steps ?? [];
  if (!Array.isArray(stepsInput)) throw new ProbeSpecError("steps must be an array");
  const steps = stepsInput.map((step: unknown, index): ProbeStep => {
    const { key, waitMs = 300 } = (step ?? {}) as { key?: unknown; waitMs?: unknown };
    if (typeof key !== "string" || !isRemoteKey(key)) {
      throw new ProbeSpecError(`steps[${index}].key is not a valid remote key: ${String(key)}`);
    }
    if (typeof waitMs !== "number" || waitMs < 0) {
      throw new ProbeSpecError(`steps[${index}].waitMs must be a non-negative number`);
    }
    return { key, waitMs };
  });

  const settleMs = input.settleMs ?? DEFAULT_SPEC.settleMs;
  if (typeof settleMs !== "number" || settleMs < 0) {
    throw new ProbeSpecError("settleMs must be a non-negative number");
  }

  const startDelayMs = input.startDelayMs ?? DEFAULT_SPEC.startDelayMs;
  if (typeof startDelayMs !== "number" || startDelayMs < 0) {
    throw new ProbeSpecError("startDelayMs must be a non-negative number");
  }

  const capture = (input.capture ?? {}) as Record<string, unknown>;
  return {
    kind,
    steps,
    startDelayMs,
    settleMs,
    capture: { appUi: capture.appUi === true, screenshot: capture.screenshot === true },
  };
}

export function loadProbeSpec(probeDir: string): ProbeSpec {
  const specPath = join(probeDir, "probe.json");
  if (!existsSync(specPath)) return parseProbeSpec(undefined);
  return parseProbeSpec(JSON.parse(readFileSync(specPath, "utf8")));
}

/** Normalizes device line endings (CRLF) so goldens are platform independent. */
export function normalizeConsoleText(text: string): string {
  return text.replace(/\r\n?/g, "\n");
}

export function hasProbeBegun(consoleText: string): boolean {
  return normalizeConsoleText(consoleText).includes(`${PROBE_BEGIN_MARKER}\n`);
}

export function hasProbeEnded(consoleText: string): boolean {
  return END_LINE_PATTERN.test(consoleText) || hasCrashed(consoleText);
}

export function hasCrashed(consoleText: string): boolean {
  return CRASH_PATTERN.test(consoleText);
}

export type ProbeOutput =
  { status: "ok"; output: string } | { status: "crashed" | "incomplete"; output: string };

/**
 * Extracts the text printed between the last BEGIN marker and the END marker
 * that follows it. Marker lines themselves are excluded.
 */
export function extractProbeOutput(rawConsole: string): ProbeOutput {
  const text = normalizeConsoleText(rawConsole);
  const beginIndex = text.lastIndexOf(`${PROBE_BEGIN_MARKER}\n`);
  if (beginIndex === -1) {
    return { status: CRASH_PATTERN.test(text) ? "crashed" : "incomplete", output: text };
  }
  const start = beginIndex + PROBE_BEGIN_MARKER.length + 1;
  const endIndex = text.indexOf(`${PROBE_END_MARKER}\n`, start);
  if (endIndex === -1) {
    const output = text.slice(start);
    return { status: CRASH_PATTERN.test(output) ? "crashed" : "incomplete", output };
  }
  return { status: "ok", output: text.slice(start, endIndex) };
}
