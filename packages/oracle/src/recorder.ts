/** Records a probe on the device: package, sideload, run key steps, capture console/app-ui/screenshot, write the trace. */
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { basename, isAbsolute, join, relative, resolve } from "node:path";
import { REPO_ROOT, type OracleConfig } from "./config.js";
import { DevInstaller } from "./devInstaller.js";
import { DeviceConsole } from "./deviceConsole.js";
import { EcpClient } from "./ecp.js";
import { packChannel } from "./packager.js";
import {
  extractProbeOutput,
  hasCrashed,
  hasProbeBegun,
  hasProbeEnded,
  loadProbeSpec,
} from "./probe.js";
import { parseDeviceInfo, profileId } from "./profile.js";
import type { Trace } from "./trace.js";

/** Time to let the Micro Debugger finish its crash dump before capturing. */
const CRASH_DUMP_SETTLE_MS = 2_000;

const sleep = (ms: number) => new Promise((wake) => setTimeout(wake, ms));

/** Raw console logs go here for debugging; this directory is git-ignored. */
export const SCRATCH_DIR = join(REPO_ROOT, ".oracle");

/**
 * Scratch folder for a probe, always confined to SCRATCH_DIR. Probes inside the
 * repo mirror their relative path; probes elsewhere get "external/<name>-<hash>"
 * so a path like "../../tmp/x" can never escape the scratch directory.
 */
export function scratchDirFor(probeDir: string): string {
  const absolute = resolve(probeDir);
  const fromRepo = relative(REPO_ROOT, absolute);
  if (fromRepo !== "" && !fromRepo.startsWith("..") && !isAbsolute(fromRepo)) {
    return join(SCRATCH_DIR, fromRepo);
  }
  const hash = createHash("sha256").update(absolute).digest("hex").slice(0, 8);
  return join(SCRATCH_DIR, "external", `${basename(absolute)}-${hash}`);
}

export interface RecordResult {
  trace: Trace;
  rawConsolePath: string;
}

/**
 * Runs one probe on the real device and returns its trace:
 * delete dev channel -> sideload (auto-launches) -> replay keys -> wait for END marker
 * -> capture app-ui / screenshot -> exit to Home.
 */
export async function recordProbe(config: OracleConfig, probeDir: string): Promise<RecordResult> {
  const spec = loadProbeSpec(probeDir);
  const archive = packChannel(probeDir);
  const installer = new DevInstaller(config);
  const ecp = new EcpClient(config);

  // Waiting here also absorbs the Home screen reload left by the previous probe.
  const profile = profileId(parseDeviceInfo(await ecp.waitUntilReady()));
  // Connect before installing so no output from the launch is missed.
  const deviceConsole = await DeviceConsole.connect(config.host, config.consolePort);
  const startedAt = Date.now();

  try {
    await installer.remove();
    const outputStart = deviceConsole.text.length;
    await installer.install(archive);

    if (spec.steps.length > 0) {
      // Keys sent before the channel is listening are dropped by the device.
      await deviceConsole.waitFor(
        (text) => hasProbeBegun(text.slice(outputStart)),
        config.probeTimeoutMs,
      );
      await sleep(spec.startDelayMs);
    }
    for (const step of spec.steps) {
      await ecp.keypress(step.key);
      await sleep(step.waitMs);
    }
    await deviceConsole.waitFor(
      (text) => hasProbeEnded(text.slice(outputStart)),
      config.probeTimeoutMs,
    );
    const durationMs = Date.now() - startedAt;
    if (hasCrashed(deviceConsole.text.slice(outputStart))) {
      // The debugger keeps printing (backtrace, locals) after the error line.
      await sleep(CRASH_DUMP_SETTLE_MS);
    }
    const rawConsole = deviceConsole.text.slice(outputStart);
    await sleep(spec.settleMs);

    const { status, output } = extractProbeOutput(rawConsole);
    const trace: Trace = {
      meta: {
        kind: spec.kind,
        status,
        profile,
        recordedAt: new Date().toISOString(),
        durationMs,
        packageMd5: createHash("md5").update(archive).digest("hex"),
      },
      console: output,
    };
    if (status === "ok" && spec.capture.appUi) trace.appUi = await ecp.queryAppUi();
    if (status === "ok" && spec.capture.screenshot) trace.screenshot = await installer.screenshot();

    const scratch = scratchDirFor(probeDir);
    mkdirSync(scratch, { recursive: true });
    const rawConsolePath = join(scratch, "raw-console.log");
    writeFileSync(rawConsolePath, rawConsole);

    return { trace, rawConsolePath };
  } finally {
    deviceConsole.close();
    // Leave the device on the Home screen so the next probe starts from a known state.
    await ecp.keypress("Home").catch(() => undefined);
  }
}
