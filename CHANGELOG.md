# Changelog

All notable changes to this project. Versions follow the roadmap phases in the
README; the project is pre-1.0 and every release is a beta.

## [Unreleased]

### Added (F4: renderer fidelity)

- Screenshot diff suite against device goldens with per-probe budgets; new
  probes `scenegraph/render-rects-{fhd,hd}`, `render-composite`, calibration
  probes `label-metrics(-hd)`, `font-advances(-hd)`, `font-kerning` and the
  `calibration` probe kind.
- 720p output surface (device UI resolution), rectangle/poster edges snapped
  like the device (floor start, floor size, no antialiasing).
- Label metrics measured on the device (advances, bearings, kerning, line
  heights; HD = FHD x 2/3) shared by the engine (`boundingRect`) and the
  canvas (wrap, ellipsis, alignment); `ExtraLargeSystemFont` (55 px).
- Bundled substitute font (Inter, SIL OFL 1.1) for OS-independent rendering,
  weight and position calibrated to device screenshots.
- Poster: `pkg:/` bitmap size from PNG/JPEG headers, `loadStatus`,
  `bitmapWidth/Height` in design pixels, natural-size drawing, `blendColor`.
- Package fonts via XML `role="font"`, Rustybuzz shaping, engine-owned lines
  shared by Canvas and VS Code, wrap/ellipsis and device lineSpacing.
- Remote Poster header requests/results in native and WASM hosts, with
  render-thread loadStatus callbacks, cached and detached-node loads.
- Nine-patch borders/stretch markers, preserving corners in output pixels.
- Animation/interpolator scheduling and device-recorded animation probe.
- Component function calls preserve the component's `m`; internal context
  references are excluded from debugger reference counts.
- `rotation` around `scaleRotateCenter`; the Scene's implicit background
  Poster reads `uri` as "" like the device.

### Added (F10: VS Code extension, unpublished)

- `apps/vscode`: runtime in a Node Worker, embedded emulator webview, start/
  stop/restart/configure/select-runtime commands, `roku-emulator` debug type
  over the binary protocol, local VSIX packaging and an isolated VS Code test.
- Marketplace listing (README, icon, changelog, compatibility table), VSIX
  install/upgrade/uninstall test, CI on Linux/Windows/macOS and a
  `vscode-v*` tag release workflow with a gated Marketplace publish job.

### Added (F7: deploy and console)

- Local developer installer with Digest authentication, ECP queries and remote
  keys, TCP console with CRLF output, and a Vite bridge to the browser runtime.
- Channel lifecycle console beacons and VS Code deployment configuration guide.
- Integration tests for authenticated upload, queued packages, ECP and console;
  browser tests for installer → WASM → TCP output and real RokuCommunity tooling.
- Resumable Main/Task telnet debugger: STOP, Ctrl+C, continue, step into/over/out,
  backtrace, selected thread, local listings and read-only local/field lookup.
  Browser keeps paused channels alive and forwards exact partial console output.
- Integration with `roku-debug` 0.24.7 / `roku-deploy` 4.0.0-alpha.10 is tested.
- Binary debug protocol (8081): breakpoints, threads, stack, variables, evaluate
  and setVariable. Expression evaluation and assignment in telnet. Shared-memory
  debug mailbox suspends synchronous SceneGraph callbacks without losing state.
- Probe `system/app-exit`.

### Added (F2: runtime components)

- `roList`, `roByteArray`, `roMessagePort` + `wait()`, `roDateTime` (pt_BR
  names), `roAppInfo`, `roDeviceInfo` (profile H000X, HD), `roRegistry` /
  `roRegistrySection`, `roXMLElement` / `roXMLList` (`@attr`), boxed
  `roInt`/`roFloat`/`roString`/`roBoolean`, `ParseJson`/`FormatJson`, `Box`,
  `GetInterface`, `Substitute`, `Tr`.
- Probes: `runtime/{message-port,json,appinfo,registry,datetime,bytearray,list-misc,xml,device-info}`.
- Network through the local proxy: `roUrlTransfer` sync/async, `roUrlEvent`,
  headers, cookies, Digest, cancellation; `roHttpAgent`, `roFileSystem`,
  `roPath`, `roRegex`, `roEVPDigest`, `roHMAC`, `roEVPCipher`, `roInput`
  (port only). Probe `runtime/extended`.

### Added (F3: SceneGraph core)

- XML components with `extends` chains (one function scope per chain),
  `<interface>` fields/functions, `<children>`, built-in node schemas with the
  device's field order.
- Threads in the VM: main, render and one per `Task`, round-robin scheduler,
  rendezvous on field sets, synchronous callbacks (`init`, observers,
  `callFunc`, `onKeyEvent`).
- `roSGNode` API, `observeField` (callback and port), `alwaysNotify`, focus
  chain + `onKeyEvent` (press/release), `Timer`, `Task`, `roSGScreen`,
  `m.global`, `boundingRect`.
- Display list (`sg::render`) for host renderers.
- Probes: `scenegraph/{node-api,observers,lifecycle,focus-keys,task-timer}`.

### Added (license)

- Apache License 2.0 (`LICENSE`, `NOTICE`), with its explicit patent grant;
  `license` set in `Cargo.toml` and every `package.json`.

### Added (branding)

- Project logo (`docs/assets/logo.svg`), also the web shell favicon and header
  mark; README redesigned with badges, screenshots, fidelity diagram, roadmap
  and FAQ.

### Added (web shell)

- Home screen (original launcher with the device's interaction model): tile
  grid, OK/Back/Options/Home navigation, about panel, single developer channel
  slot that persists across reloads (IndexedDB) together with its registry
  (localStorage).
- Canvas2D renderer of the display list in the worker (Rectangle, Label with
  wrap/ellipsis, Poster from the package or HTTP, clipping).
- Remote keys reach the channel; Home leaves it. Channel exit or crash returns
  to the home screen.
- wasm ABI: `brs_key`, `brs_display_list`, `brs_package_file`,
  `brs_inspect_zip`, registry load/take, `brs_unload`, wall clock offset,
  status `5` (waiting).
- e2e: home screen and `scenegraph/focus-keys` driven by the keyboard against
  the device golden.

### Added (F1: BrightScript language)

- Rust workspace with `brs-core` (lexer, parser, compiler, resumable bytecode
  VM, values, builtins), `brs-cli` (`brs run`, `brs conformance`) and `brs-wasm`
  (C-ABI WebAssembly build).
- Non-SceneGraph channels run natively and in the browser. The web shell can
  open a sideload `.zip` ("Abrir canal (.zip)") and streams its console.
- Device-exact print formatting, numeric promotion, string functions,
  arrays/associative arrays, `TRY/CATCH/THROW` and the BrightScript Micro
  Debugger crash dump (including the source digest).
- `roArray`, `roAssociativeArray`, `roTimespan`.
- Language probes recorded on the device: `boolean-ops`, `collections`,
  `control-flow`, `functions`, `numbers`, `print-zones`, `runtime-error`,
  `strings`.
- `pnpm conformance`, `pnpm test:rust`, `pnpm build:wasm`, `pnpm e2e`
  (Playwright fidelity tests in the browser).
- `make setup` / `scripts/setup.sh`: installs Node 22 (nvm/fnm), pnpm
  (corepack), rustup + wasm target, dependencies and `.env`. `Makefile`
  targets run through `scripts/run.sh`, so no manual `nvm use` is needed.
- Docs: `docs/architecture.md`, `docs/brightscript-semantics.md`,
  `docs/fidelity-workflow.md`.

### Changed

- Oracle: precise crash detection (non-fatal `ERROR` diagnostics are not
  crashes), 2 s settle for the crash dump, END marker must be a full line,
  `packageMd5` stored in `meta.json`.
- Gitflow: `develop` (default), `release` (pre-prod), `master` (prod); CI runs
  on the three branches and their PRs; `branch-policy` workflow enforces the
  promotion path (`docs/git-workflow.md`).
- CI runs `cargo fmt`, `clippy -D warnings`, Rust tests, conformance and e2e.
- Oracle: `OK` maps to ECP `Select`; key steps wait for `@@PROBE_BEGIN` plus
  `startDelayMs` (new `probe.json` field, default 1000).
- `brs conformance` runs SceneGraph probes (no longer skipped).
- PRINT column zones count UTF-8 bytes; `invalid` compares with any type via
  `=`/`<>`.

### Known gaps

- `FOR EACH` over an associative array uses insertion order; the device uses
  its internal hash order (`language/collections` fails).
- Float-literal promotion to Double uses a provisional 10-digit threshold.

## [0.1.0-beta]

### Added (F0: foundation)

- pnpm monorepo: React web shell, typed worker protocol, Vitest, ESLint,
  Prettier, CI.
- Fidelity oracle: sideload via the developer installer (Digest auth), telnet
  console capture, ECP keypress/app-ui/device-info, screenshots, text/XML/pixel
  diffs, redacted device profile (`profiles/H302X-16.0.4.json`).
- First probes and goldens (`language/hello`, `bench/throughput`,
  `scenegraph/label-basic`).
