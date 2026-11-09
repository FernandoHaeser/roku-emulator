import { runTests } from "@vscode/test-electron";
import { mkdir, mkdtemp, writeFile, cp, rm } from "node:fs/promises";
import { cliRunner, resolveVSCode } from "./vscode-cli.mjs";
import { tmpdir } from "node:os";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scratch = await mkdtemp(join(tmpdir(), "roku-vscode-test-"));
const workspace = join(scratch, "channel");
await mkdir(join(workspace, "source"), { recursive: true });
await mkdir(join(workspace, ".vscode"));
await writeFile(
  join(workspace, "manifest"),
  "title=VS Code Integration\nmajor_version=1\nminor_version=0\nbuild_version=0\n",
);
await writeFile(
  join(workspace, "source/main.brs"),
  "sub Main()\n answer = 42\n print answer\n while true\n sleep(100)\n end while\nend sub\n",
);
await writeFile(
  join(workspace, ".vscode/launch.json"),
  '{\n // preserve\n "version":"0.2.0", "configurations":[{"name":"Physical Roku"}]\n}\n',
);
const extensions = join(root, ".vscode-test/extensions");
await mkdir(extensions, { recursive: true });
const vscodeExecutable = await resolveVSCode();
const runCli = cliRunner(vscodeExecutable);
try {
  const located = (
    await runCli(["--locate-extension", "rokucommunity.brightscript"])
  ).stdout.trim();
  if (!located) throw new Error("BrightScript extension not found");
  await cp(located, join(extensions, "rokucommunity.brightscript"), { recursive: true });
} catch {
  await runCli([
    "--install-extension",
    "rokucommunity.brightscript",
    "--extensions-dir",
    extensions,
    "--user-data-dir",
    join(scratch, "install-profile"),
  ]);
}
try {
  await runTests({
    vscodeExecutablePath: vscodeExecutable,
    extensionDevelopmentPath: root,
    extensionTestsPath: join(root, "dist/electron-test.cjs"),
    launchArgs: [
      workspace,
      "--user-data-dir",
      join(scratch, "profile"),
      "--extensions-dir",
      extensions,
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
    ],
    extensionTestsEnv: { ROKU_EXTENSION_TEST: "1" },
  });
} finally {
  await rm(join(root, ".vscode-test/last-run-logs"), { recursive: true, force: true });
  try {
    await cp(join(scratch, "profile/logs"), join(root, ".vscode-test/last-run-logs"), {
      recursive: true,
    });
  } catch {
    /* VS Code may exit before creating logs. */
  }
  await rm(scratch, { recursive: true, force: true });
}
