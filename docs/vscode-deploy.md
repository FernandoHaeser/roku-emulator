# Deploy from VS Code

Run `make dev` and keep http://127.0.0.1:5173 open. The browser runs the
channel; the local Node server supplies the developer installer and ECP at
`127.0.0.1:8060` and the TCP console at `127.0.0.1:8085`.

In the **channel project's** `.vscode/launch.json`, use the BrightScript
extension's telnet launch configuration:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Web emulator",
      "type": "brightscript",
      "request": "launch",
      "host": "127.0.0.1",
      "password": "rokudev",
      "rootDir": "${workspaceFolder}",
      "packagePort": 8060,
      "remotePort": 8060,
      "consolePort": 8085,
      "enableDebugProtocol": false
    }
  ]
}
```

## Recommended: the Roku Emulator extension (F10)

`apps/vscode` is a first-party extension (debug type `roku-emulator`). It
starts the bundled runtime in a Node Worker, embeds the emulator screen in a
webview, writes a launch configuration without touching other entries, and
debugs over the binary protocol (port 8081) through the RokuCommunity
`DebugProtocolClient`. Build a local package with
`pnpm --filter ./apps/vscode build && pnpm --filter ./apps/vscode package`
and install `apps/vscode/roku-emulator-0.1.0.vsix`. It is not published on
the Marketplace yet.

Commands: open, start, stop, restart, configure project, select runtime and
open in browser. Settings: `rokuEmulator.runtime`, `externalUrl`, `ecpPort`,
`consolePort`, `debugPort`.

Why a separate debug type: BrightScript extension 3.0.1 validates the password
on port 80 regardless of `packagePort`, which needs elevated privileges on
loopback. Keep the BrightScript extension for language services.

### Testing and releasing the extension

- `pnpm --filter ./apps/vscode test:install`: packages the current version and
  a patch bump, then installs, upgrades and uninstalls them with the VS Code
  CLI in a throwaway profile.
- `pnpm --filter ./apps/vscode test:electron`: real debug session in an
  isolated VS Code (downloads VS Code stable when no local install is found).
- CI (`.github/workflows/vscode.yml`) runs both on Linux, Windows and macOS.
- Release: bump `apps/vscode/package.json` and its `CHANGELOG.md`, merge to
  `master`, then push the tag `vscode-v<version>`. The workflow attaches the
  VSIX to a GitHub pre-release; the `publish` job waits for approval in the
  `marketplace` environment and publishes a Marketplace pre-release (see
  [marketplace-publishing.md](marketplace-publishing.md)).

## Marketplace publishing

Microsoft Entra ID through GitHub OIDC; no stored token. One-time setup, the
release procedure and troubleshooting:
[marketplace-publishing.md](marketplace-publishing.md).

## Supported debugger operations

- `STOP`, Ctrl+C/pause, continue, step into/over/out on Main, Task and
  synchronous SceneGraph callbacks (`init`, observers, `onKeyEvent`,
  `callFunc`). Hosts with a `debug_wait` mailbox (SharedArrayBuffer in the
  browser, a Node Worker in the extension) suspend only the engine thread.
- Line breakpoints set before or during a session, hit-count conditions.
- Threads, stack frames with frame selection, scopes, variable expansion,
  watch/hover evaluation and `setVariable`.
- Telnet (8085): `c`, `s`/`step`, `over`, `out`, `bt`, `var`, `threads`,
  `print <expr>`/`? <expr>` and assignments.

Covered by browser tests (`apps/web/e2e/device-server.spec.ts`, the real
`roku-debug` 0.24.7 telnet adapter and binary client) and by an isolated VS
Code run (`pnpm --filter ./apps/vscode test:electron`).

Known limits: expression breakpoint conditions are not supported; stepping
across multiple statements on one source line is not supported; wall-clock
deadlines are not shifted when resuming; a crash returns to the launcher.

To check the console independently, run `nc 127.0.0.1 8085` before deploying.
Uploads made before opening the browser are queued until it connects. The
installed package lives in server memory; browser storage retains the local
channel for manual relaunch. Use one emulator tab to avoid duplicate execution.

Settings are process environment variables: `ROKU_EMU_HOST`,
`ROKU_EMU_ECP_PORT`, `ROKU_EMU_CONSOLE_PORT`, `ROKU_EMU_DEV_PASSWORD`. Defaults are loopback, 8060, 8085 and
`rokudev` / `rokudev`. They are separate from the physical oracle's credentials.
For example: `ROKU_EMU_ECP_PORT=8061 make dev`. These variables are not loaded
from `.env` automatically. Binding outside loopback requires
`ROKU_EMU_ALLOW_LAN=1` and a non-default password; the web bridge currently
accepts loopback hosts only.

The Vite plugin runs only during development. A static production build does
not include the Node device server. If a port is occupied, the shell remains
available and Vite reports that device deployment is disabled.
