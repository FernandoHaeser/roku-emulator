# Roku Emulator (beta)

Roku OS emulator aiming at near device-level fidelity, running in the browser
(WebAssembly) and natively. A real Roku in Developer Mode acts as the
**oracle**: every behavior is validated by diffing the emulator against traces
recorded on the device.

## Status

The emulator runs **BrightScript and SceneGraph channels** in the browser and
natively. Console output is byte-identical to the reference device for the
recorded corpus (23 of 24 probes; see known gaps). The browser build renders
the SceneGraph scene, takes remote input from the keyboard and boots into a
home screen with the developer channel.

| Area                                    | State                                                                                        |
| --------------------------------------- | -------------------------------------------------------------------------------------------- |
| Fidelity oracle (`packages/oracle`)     | Working against a real device (H302X, OS 16.0.4), including remote key steps                 |
| BrightScript engine (`crates/brs-core`) | Lexer, parser, compiler, resumable bytecode VM with threads (main, render, Tasks)            |
| Native runner (`crates/brs-cli`)        | `brs run`, `brs conformance` (injects probe key steps)                                       |
| Browser build (`crates/brs-wasm` + web) | Home screen, sideload, Canvas2D renderer, remote keys, persistent registry                   |
| `ro*` components (F2)                   | roArray, roAssociativeArray, roList, roByteArray, roMessagePort, roTimespan, roDateTime,     |
|                                         | roAppInfo, roDeviceInfo, roRegistry(Section), roXMLElement, boxed roInt/roFloat/roString/... |
| SceneGraph core (F3)                    | XML components + extends, fields/observers, focus + onKeyEvent, Task, Timer, roSGScreen      |
| Renderer (F4)                           | Provisional: Rectangle, Label (wrap/ellipsis), Poster, clipping; system font approximated    |
| Pending                                 | roUrlTransfer (needs the proxy), roInput, animations, widgets (F5), Video (F6)               |

Known gaps (tracked by failing conformance, never masked):

- `FOR EACH` over an associative array follows the device's internal hash
  order; the emulator uses insertion order (`language/collections`).
- Float-literal promotion to Double uses a provisional digit threshold.
- Font metrics and widget geometry are provisional until F4/F5 probes.

## Architecture

The emulator core is written in **Rust** and compiled to:

- **WebAssembly** for the browser (loaded by a Web Worker in `apps/web`), and
- **native** binaries (`brs` CLI today, a desktop app for Linux, Windows and
  macOS later).

One core, two targets: no rewrite when moving to desktop. The web shell (React)
and the oracle (Node) stay in TypeScript; the oracle's goldens are the shared
conformance suite for both targets. Details in
[docs/architecture.md](docs/architecture.md).

### Repository layout

| Path                | Contents                                                          |
| ------------------- | ----------------------------------------------------------------- |
| `crates/brs-core`   | BrightScript engine: lexer, parser, compiler, VM, values, runtime |
| `crates/brs-cli`    | Native `brs` binary: run a channel, run conformance               |
| `crates/brs-wasm`   | C-ABI WebAssembly exports used by the browser worker              |
| `apps/web`          | React shell, Web Worker runtime host, Playwright e2e tests        |
| `packages/protocol` | Typed messages between the shell and the worker; remote keys      |
| `packages/oracle`   | Device client: sideload, console capture, ECP, screenshots, diff  |
| `fidelity/`         | Probe channels and their device-recorded `golden/` traces         |
| `profiles/`         | Reference device profile (identifiers redacted)                   |
| `scripts/`          | `cargo.sh` (rustup toolchain wrapper), `build-wasm.sh`            |
| `docs/`             | Architecture, device-verified semantics, fidelity workflow        |

### Documentation

- [docs/architecture.md](docs/architecture.md): engine pipeline, VM, hosts,
  wasm ABI, worker loop.
- [docs/brightscript-semantics.md](docs/brightscript-semantics.md): language
  behaviors confirmed on the device, with the probe that proves each one.
- [docs/fidelity-workflow.md](docs/fidelity-workflow.md): how to add a probe,
  record it on the device and make the emulator match.
- [docs/git-workflow.md](docs/git-workflow.md): gitflow branches
  (`develop` default, `release` pre-prod, `master` prod), PRs and releases.
- [CHANGELOG.md](CHANGELOG.md)

## Requirements

- macOS or Linux with `make`, `curl` and nvm (or fnm). `make setup` installs
  the rest: Node 22, pnpm 9, Rust via rustup (pinned by `rust-toolchain.toml`,
  with the `wasm32-unknown-unknown` target). Homebrew's `rust` formula is not
  enough (no wasm target); the scripts prefer rustup's toolchain even when it
  is on PATH.
- A Roku device with Developer Mode enabled, and
  _Settings > System > Advanced system settings > Control by mobile apps_ set to **Permissive**

### Troubleshooting

- `EHOSTUNREACH` from Node while `curl` works: macOS Local Network privacy is
  blocking that Node binary. Allow your terminal in _System Settings > Privacy &
  Security > Local Network_ (toggle off/on if already enabled). A newly installed
  Node version may trigger a permission prompt on first LAN access.
- `zsh: correct 'pnpm' to 'npm'`: pnpm is not active in your shell (usually
  an older default Node). Use the `make` targets, or `source scripts/env.sh`.
- Connect timeouts right after a probe: the device stalls while the Home screen
  reloads. The oracle retries connection-phase failures automatically.

## Setup

```sh
make setup        # or ./scripts/setup.sh
```

`setup` checks each prerequisite and installs what is missing (it asks first;
`./scripts/setup.sh --yes` skips the questions, `make setup-e2e` also installs
the Playwright browser):

1. Node 22 via nvm or fnm (from `.nvmrc`).
2. pnpm via corepack (version from `package.json`).
3. rustup, the pinned toolchain and the `wasm32-unknown-unknown` target
   (Homebrew on macOS, the official rustup installer elsewhere).
4. `pnpm install --frozen-lockfile`, Rust crates and the wasm build.
5. `.env` from `.env.example` when missing. Fill `ROKU_HOST` and
   `ROKU_DEV_PASSWORD` to use the oracle (never commit `.env`).

No `nvm use` is needed afterwards: every `make` target runs through
`scripts/run.sh`, which activates the right Node, pnpm and Rust. To use the raw
tools in your own shell, `source scripts/env.sh`. `make doctor` prints the
resolved versions.

## Running a channel

- **Browser:** `make dev`, open http://127.0.0.1:5173. The TV boots into the
  home screen. Click _Abrir canal (.zip)_ (or OK on _Instalar canal_) and pick a
  sideload zip: it becomes the developer channel and launches right away.
  Console output appears in the console panel.
- **Native:** `make run CHANNEL=<dir|zip>` (wraps `brs run`)
  prints the channel's console output to stdout.

Remote control (keyboard, layout independent):

| Key             | Remote                   |
| --------------- | ------------------------ |
| Arrows          | Up/Down/Left/Right       |
| Enter           | OK                       |
| Backspace / Esc | Back                     |
| H / Home        | Home (leave the channel) |
| O               | Options (`*`)            |
| Space / , / .   | Play / Rev / Fwd         |
| R               | Instant replay           |

On the home screen: OK opens the focused tile, Options opens the channel menu
(open / remove). The developer channel and its registry survive page reloads.

`CreateObject` of a class the engine does not implement yet prints the
device's `BRIGHTSCRIPT: ERROR: Runtime: unknown classname "X"` diagnostic and
returns `invalid`.

## Commands

| Command                               | What it does                                         |
| ------------------------------------- | ---------------------------------------------------- |
| `make help`                           | List every target                                    |
| `make dev`                            | Build wasm + web shell at http://127.0.0.1:5173      |
| `make run CHANNEL=<dir\|zip>`         | Run a channel natively, console to stdout            |
| `make test`                           | Rust + TypeScript unit tests                         |
| `make conformance`                    | Run probes in the engine and diff with goldens       |
| `make e2e`                            | Browser fidelity tests (Playwright)                  |
| `make check`                          | Every CI gate except e2e                             |
| `make format`                         | Prettier + rustfmt                                   |
| `make build` / `make wasm`            | Production build / wasm only                         |
| `make record PROBE=fidelity/language` | Run probes on the device and write `golden/`         |
| `make oracle ARGS="device-info"`      | Oracle CLI (`profile`, `list`, `screenshot`, `diff`) |
| `make clean`                          | Remove build outputs                                 |

The `pnpm` scripts behind them (`pnpm dev`, `pnpm conformance`,
`pnpm oracle ...`) work too when Node 22 and pnpm are active in your shell.

## Probes

A probe is a minimal channel under `fidelity/<area>/<name>/` that prints its
observable output between `@@PROBE_BEGIN` and `@@PROBE_END`. Optional
`probe.json` adds remote key steps and app-ui/screenshot capture:

```json
{
  "kind": "conformance",
  "steps": [{ "key": "Down", "waitMs": 300 }],
  "startDelayMs": 1000,
  "settleMs": 500,
  "capture": { "appUi": true, "screenshot": true }
}
```

Key steps start `startDelayMs` after `@@PROBE_BEGIN` (default 1000), both on
the device and in the emulator. `kind: "bench"` probes produce metrics used to calibrate device-mode timing and
are never diffed. Goldens in `golden/` are committed; raw device logs go to the
git-ignored `.oracle/`. Full workflow in
[docs/fidelity-workflow.md](docs/fidelity-workflow.md).
