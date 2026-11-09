# Roku Emulator (beta)

Web-based Roku OS emulator aiming at near device-level fidelity. A real Roku in
Developer Mode acts as the **oracle**: every behavior is validated by diffing the
emulator against traces recorded on the device.

## Status: v0.1.0-beta (foundation)

This beta is the **foundation**, not a working emulator yet. It does **not** run
channels.

| Area                                        | State                                            |
| ------------------------------------------- | ------------------------------------------------ |
| Fidelity oracle (`packages/oracle`)         | Working against a real device (H302X, OS 16.0.4) |
| Probes + goldens (`fidelity/`)              | Language, SceneGraph and benchmark seeds         |
| Web shell (`apps/web`)                      | Worker + WebGL2 surface, console, remote keys    |
| BrightScript interpreter, runtime, renderer | Not started (next: F1)                           |

## Architecture direction

The emulator core (interpreter, `ro*` runtime, SceneGraph, renderer) will be
written in **Rust** and compiled to:

- **WebAssembly** for the browser build (loaded by the `apps/web` Worker), and
- **native** binaries for a desktop app on Linux, Windows and macOS.

One core, two targets: no rewrite when moving to desktop. The web shell (React)
and the oracle (Node) stay in TypeScript; the oracle's goldens are the shared
conformance suite for both targets.

## Requirements

- Node 22 (`nvm use`), pnpm 9 (`corepack enable pnpm`)
- A Roku device with Developer Mode enabled, and
  _Settings > System > Advanced system settings > Control by mobile apps_ set to **Permissive**

### Troubleshooting

- `EHOSTUNREACH` from Node while `curl` works: macOS Local Network privacy is
  blocking that Node binary. Allow your terminal in _System Settings > Privacy &
  Security > Local Network_ (toggle off/on if already enabled). A newly installed
  Node version may trigger a permission prompt on first LAN access.
- Connect timeouts right after a probe: the device stalls while the Home screen
  reloads. The oracle retries connection-phase failures automatically.

## Setup

```sh
pnpm install
cp .env.example .env   # fill ROKU_HOST and ROKU_DEV_PASSWORD (never commit .env)
```

## Commands

| Command                                | What it does                                        |
| -------------------------------------- | --------------------------------------------------- |
| `pnpm dev`                             | Web shell at http://127.0.0.1:5173                  |
| `pnpm test` / `lint` / `typecheck`     | Quality gates (also run in CI)                      |
| `pnpm oracle device-info`              | Device profile, identifiers redacted                |
| `pnpm oracle profile`                  | Save profile to `profiles/<model>-<os>.json`        |
| `pnpm oracle list`                     | List probes under `fidelity/`                       |
| `pnpm oracle record fidelity/language` | Run probes on the device and write `golden/` traces |
| `pnpm oracle screenshot`               | Screenshot of the running dev channel               |
| `pnpm oracle diff <golden> <actual>`   | Compare two trace directories                       |

## Probes

A probe is a minimal channel under `fidelity/<area>/<name>/` that prints its
observable output between `@@PROBE_BEGIN` and `@@PROBE_END`. Optional
`probe.json` adds remote key steps and app-ui/screenshot capture:

```json
{
  "kind": "conformance",
  "steps": [{ "key": "Down", "waitMs": 300 }],
  "settleMs": 500,
  "capture": { "appUi": true, "screenshot": true }
}
```

`kind: "bench"` probes produce metrics used to calibrate device-mode timing and
are never diffed. Goldens in `golden/` are committed; raw device logs go to the
git-ignored `.oracle/`.
