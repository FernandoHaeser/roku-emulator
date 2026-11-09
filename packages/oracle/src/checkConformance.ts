import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { conformanceErrors, type KnownFailure } from "./conformanceGate.js";
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const fidelity = join(repo, "fidelity");
function discover(directory: string): string[] {
  if (existsSync(join(directory, "manifest")))
    return [relative(fidelity, directory).split("\\").join("/")];
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .flatMap((entry) => discover(join(directory, entry.name)));
}
const known = JSON.parse(readFileSync(join(fidelity, "known-failures.json"), "utf8")) as Record<
  string,
  KnownFailure
>;
const run = spawnSync(
  join(repo, "scripts/cargo.sh"),
  ["run", "--release", "-q", "-p", "brs-cli", "--", "conformance", "fidelity"],
  { cwd: repo, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
);
process.stdout.write(run.stdout ?? "");
process.stderr.write(run.stderr ?? "");
const errors = conformanceErrors(run.stdout ?? "", run.status, discover(fidelity), known);
if (run.error) errors.push(run.error.message);
for (const error of errors) console.error(error);
if (errors.length) process.exitCode = 1;
else
  console.log(
    `Conformance gate passed with ${Object.keys(known).length} exact, documented exceptions.`,
  );
