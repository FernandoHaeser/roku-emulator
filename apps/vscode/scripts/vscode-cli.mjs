// Locates a VS Code build for the test scripts and wraps its CLI.
import {
  downloadAndUnzipVSCode,
  resolveCliArgsFromVSCodeExecutablePath,
} from "@vscode/test-electron";
import { execFile } from "node:child_process";
import { access } from "node:fs/promises";
import { promisify } from "node:util";

const exists = (path) =>
  access(path).then(
    () => true,
    () => false,
  );
const macApp = "/Applications/Visual Studio Code.app/Contents/MacOS/Code";

export async function resolveVSCode() {
  if (process.env.VSCODE_EXECUTABLE) return process.env.VSCODE_EXECUTABLE;
  if (process.platform === "darwin" && (await exists(macApp))) return macApp;
  const downloaded = await downloadAndUnzipVSCode(process.env.VSCODE_VERSION || "stable");
  // Recent macOS builds name the binary `Code`; test-electron still returns `Electron`.
  if (!(await exists(downloaded)) && downloaded.endsWith("/MacOS/Electron")) {
    const renamed = downloaded.replace(/Electron$/, "Code");
    if (await exists(renamed)) return renamed;
  }
  return downloaded;
}

/** Runs the VS Code CLI; Windows ships it as a .cmd shim that needs a shell. */
export function cliRunner(executable) {
  const [cli, ...cliArgs] = resolveCliArgsFromVSCodeExecutablePath(executable, {
    reuseMachineInstall: true,
  });
  const shell = process.platform === "win32";
  const quote = (value) => (shell ? `"${value}"` : value);
  const execute = promisify(execFile);
  return (args) =>
    execute(quote(cli), [...cliArgs, ...args].map(quote), {
      shell,
      maxBuffer: 16 * 1024 * 1024,
    });
}
