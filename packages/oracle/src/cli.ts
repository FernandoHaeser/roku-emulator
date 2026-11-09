import { existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { loadEnvFile, parseOracleConfig, REPO_ROOT, type OracleConfig } from "./config.js";
import { DevInstaller } from "./devInstaller.js";
import { diffTraces, isMatch } from "./diff.js";
import { describeError } from "./errors.js";
import { EcpClient } from "./ecp.js";
import { parseDeviceInfo, profileId, redactProfile } from "./profile.js";
import { recordProbe, SCRATCH_DIR } from "./recorder.js";
import { goldenDir, readTrace, writeTrace } from "./trace.js";

const USAGE = `Usage: pnpm oracle <command>

Commands:
  device-info             Print the device profile (identifiers redacted)
  profile                 Save the device profile to profiles/<model>-<os>.json
  list [dir]              List probes under fidelity/ (or dir)
  record <probe|dir>...   Run probes on the device and write their golden traces
  screenshot [file]       Save a screenshot of the running dev channel
  diff <golden> <actual>  Compare two trace directories
`;

/** pnpm runs scripts inside the package dir; resolve user paths against the original cwd. */
const userPath = (path: string): string => resolve(process.env.INIT_CWD ?? process.cwd(), path);

function findProbes(root: string): string[] {
  if (existsSync(join(root, "manifest"))) return [root];
  if (!existsSync(root) || !statSync(root).isDirectory()) return [];
  return readdirSync(root)
    .sort()
    .filter((entry) => !entry.startsWith(".") && entry !== "golden")
    .flatMap((entry) => {
      const child = join(root, entry);
      return statSync(child).isDirectory() ? findProbes(child) : [];
    });
}

function deviceConfig(): OracleConfig {
  loadEnvFile();
  return parseOracleConfig(process.env);
}

async function commandRecord(targets: string[]): Promise<number> {
  const config = deviceConfig();
  const probes = targets.flatMap((target) => findProbes(userPath(target)));
  if (probes.length === 0) {
    console.error("No probes found (a probe is a directory with a manifest).");
    return 1;
  }
  let failures = 0;
  for (const probeDir of probes) {
    const name = relative(REPO_ROOT, probeDir);
    process.stdout.write(`record ${name} ... `);
    try {
      const { trace, rawConsolePath } = await recordProbe(config, probeDir);
      writeTrace(goldenDir(probeDir), trace);
      console.log(`${trace.meta.status} (${trace.meta.durationMs} ms)`);
      if (trace.meta.status !== "ok") {
        failures++;
        console.log(`  raw console: ${relative(REPO_ROOT, rawConsolePath)}`);
      }
      if (trace.meta.kind === "bench") console.log(indent(trace.console));
    } catch (error) {
      failures++;
      console.log(`error\n${indent(describeError(error))}`);
    }
  }
  return failures === 0 ? 0 : 1;
}

function commandDiff(goldenPath: string, actualPath: string): number {
  const report = diffTraces(readTrace(userPath(goldenPath)), readTrace(userPath(actualPath)));
  if (isMatch(report)) {
    console.log("match");
    return 0;
  }
  for (const [part, mismatch] of Object.entries(report)) {
    if (mismatch === null) continue;
    console.log(`${part}: mismatch`);
    console.log(indent(JSON.stringify(mismatch, null, 2)));
  }
  return 1;
}

const indent = (text: string): string =>
  text
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n");

async function main(argv: string[]): Promise<number> {
  const [command, ...args] = argv;
  switch (command) {
    case "device-info": {
      const ecp = new EcpClient(deviceConfig());
      const profile = redactProfile(parseDeviceInfo(await ecp.queryDeviceInfo()));
      console.log(JSON.stringify(profile, null, 2));
      return 0;
    }
    case "profile": {
      const ecp = new EcpClient(deviceConfig());
      const profile = redactProfile(parseDeviceInfo(await ecp.queryDeviceInfo()));
      const dir = join(REPO_ROOT, "profiles");
      mkdirSync(dir, { recursive: true });
      const file = join(dir, `${profileId(profile)}.json`);
      writeFileSync(file, `${JSON.stringify(profile, null, 2)}\n`);
      console.log(`wrote ${relative(REPO_ROOT, file)}`);
      return 0;
    }
    case "list": {
      const root = args[0] ? userPath(args[0]) : join(REPO_ROOT, "fidelity");
      for (const probe of findProbes(root)) console.log(relative(REPO_ROOT, probe));
      return 0;
    }
    case "record":
      if (args.length === 0) break;
      return commandRecord(args);
    case "screenshot": {
      const shot = await new DevInstaller(deviceConfig()).screenshot();
      const file = args[0] ? userPath(args[0]) : join(SCRATCH_DIR, `screenshot.${shot.extension}`);
      mkdirSync(resolve(file, ".."), { recursive: true });
      writeFileSync(file, shot.data);
      console.log(`wrote ${file}`);
      return 0;
    }
    case "diff":
      if (args.length !== 2 || !args[0] || !args[1]) break;
      return commandDiff(args[0], args[1]);
  }
  console.error(USAGE);
  return 2;
}

main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (error: unknown) => {
    console.error(describeError(error));
    process.exit(1);
  },
);
