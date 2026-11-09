// Install -> upgrade -> uninstall check for the packaged VSIX, using the VS Code
// CLI against a throwaway profile and extensions directory. Run after `build`.
import { createVSIX } from "@vscode/vsce";
import { access, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { cliRunner, resolveVSCode } from "./vscode-cli.mjs";
import * as assert from "node:assert/strict";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = join(root, "package.json");
const original = await readFile(manifestPath, "utf8");
const manifest = JSON.parse(original);
const id = `${manifest.publisher}.${manifest.name}`.toLowerCase();
const [major, minor, patch] = manifest.version.split(".").map(Number);
const nextVersion = `${major}.${minor}.${patch + 1}`;

const scratch = await mkdtemp(join(tmpdir(), "roku-vscode-install-"));
const extensionsDir = join(scratch, "extensions");
const userDataDir = join(scratch, "profile");

const runCli = cliRunner(await resolveVSCode());
const code = (...args) =>
  runCli([...args, "--extensions-dir", extensionsDir, "--user-data-dir", userDataDir]);

async function installedVersion() {
  const { stdout } = await code("--list-extensions", "--show-versions");
  const line = stdout.split(/\r?\n/).find((entry) => entry.toLowerCase().startsWith(`${id}@`));
  return line?.split("@")[1];
}

async function installedFolder() {
  const entries = await readdir(extensionsDir).catch(() => []);
  return entries.find((entry) => entry.toLowerCase().startsWith(`${id}-`));
}

async function pack(version, packagePath) {
  await writeFile(manifestPath, JSON.stringify({ ...manifest, version }, null, 2) + "\n");
  try {
    await createVSIX({ cwd: root, packagePath, dependencies: false, allowMissingRepository: true });
  } finally {
    await writeFile(manifestPath, original);
  }
}

try {
  const first = join(scratch, `roku-emulator-${manifest.version}.vsix`);
  const second = join(scratch, `roku-emulator-${nextVersion}.vsix`);
  await pack(manifest.version, first);
  await pack(nextVersion, second);

  await code("--install-extension", first, "--force");
  assert.equal(await installedVersion(), manifest.version, "fresh install");
  const folder = await installedFolder();
  assert.ok(folder, "extension folder exists");
  for (const file of ["dist/extension.cjs", "runtime/worker.mjs", "runtime/web/index.html"]) {
    await access(join(extensionsDir, folder, file));
  }

  await code("--install-extension", second, "--force");
  assert.equal(await installedVersion(), nextVersion, "upgrade");

  await code("--uninstall-extension", id);
  assert.equal(await installedVersion(), undefined, "uninstall");
  console.log(`VSIX install ${manifest.version} -> upgrade ${nextVersion} -> uninstall passed.`);
} finally {
  await writeFile(manifestPath, original);
  await rm(scratch, { recursive: true, force: true });
}
