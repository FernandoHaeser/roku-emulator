import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { ProbeKind, ProbeOutput } from "./probe.js";

/**
 * A trace is everything observable from one probe run. Goldens are traces
 * recorded on the real device, stored under `<probe>/golden/`.
 */

export interface TraceMeta {
  kind: ProbeKind;
  status: ProbeOutput["status"];
  /** Profile id of the device (or emulator profile) that produced the trace. */
  profile: string;
  recordedAt: string;
  durationMs: number;
}

export interface Trace {
  meta: TraceMeta;
  console: string;
  appUi?: string;
  screenshot?: { data: Buffer; extension: "png" | "jpg" };
}

const CONSOLE_FILE = "console.txt";
const APP_UI_FILE = "app-ui.xml";
const META_FILE = "meta.json";

export function goldenDir(probeDir: string): string {
  return join(probeDir, "golden");
}

export function writeTrace(dir: string, trace: Trace): void {
  // Start clean so stale artifacts (e.g. an old screenshot) never linger.
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, META_FILE), `${JSON.stringify(trace.meta, null, 2)}\n`);
  writeFileSync(join(dir, CONSOLE_FILE), trace.console);
  if (trace.appUi !== undefined) writeFileSync(join(dir, APP_UI_FILE), trace.appUi);
  if (trace.screenshot) {
    writeFileSync(join(dir, `screenshot.${trace.screenshot.extension}`), trace.screenshot.data);
  }
}

export function readTrace(dir: string): Trace {
  if (!existsSync(join(dir, META_FILE))) throw new Error(`No trace found in ${dir}`);
  const trace: Trace = {
    meta: JSON.parse(readFileSync(join(dir, META_FILE), "utf8")) as TraceMeta,
    console: readFileSync(join(dir, CONSOLE_FILE), "utf8"),
  };
  if (existsSync(join(dir, APP_UI_FILE)))
    trace.appUi = readFileSync(join(dir, APP_UI_FILE), "utf8");
  const screenshotFile = readdirSync(dir).find((name) => /^screenshot\.(png|jpg)$/.test(name));
  if (screenshotFile) {
    trace.screenshot = {
      data: readFileSync(join(dir, screenshotFile)),
      extension: screenshotFile.endsWith("png") ? "png" : "jpg",
    };
  }
  return trace;
}
