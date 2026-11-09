# Architecture

## Overview

```
              ┌──────────────── crates/brs-core (Rust, host independent) ────────────────┐
 channel.zip ─▶ Channel ─▶ lexer ─▶ parser ─▶ AST ─▶ compiler ─▶ bytecode ─▶ Vm::run(budget)
              └──────────────────────────────────────────────────────────────┬──────────┘
                                                                Host trait  │ write / now_ms
                     ┌───────────────────────────────┬───────────────────────┴──────┐
              crates/brs-cli (native)        crates/brs-wasm (C ABI)       future desktop app
              stdout, conformance            apps/web Web Worker
                                                     │ postMessage (packages/protocol)
                                              React shell (apps/web)
```

The engine never touches the OS, the DOM or the clock directly. Everything
platform specific goes through the `Host` trait (`crates/brs-core/src/vm.rs`):

```rust
pub trait Host {
    fn write(&mut self, text: &str); // console output (device telnet port 8085)
    fn now_ms(&self) -> f64;         // monotonic clock for Sleep, roTimespan, UpTime
    fn epoch_ms(&self) -> f64;       // wall clock for roDateTime (default 0)
}
```

## Engine pipeline (`crates/brs-core`)

| Module               | Responsibility                                                                                                                                                                                                                |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `channel.rs`         | Reads the sideload zip (stored + deflate), parses `manifest`, normalizes paths. Rejects zip-slip paths; caps entry count and total size.                                                                                      |
| `lexer.rs`           | Tokens. Case-insensitive keywords, `rem`/`'` comments, numeric suffixes (`%`, `!`, `#`, `&`), hex literals.                                                                                                                   |
| `parser.rs`          | Recursive descent into `ast.rs`. Single-line and block `IF`, AA/array literals, `TRY/CATCH`, typed params.                                                                                                                    |
| `compiler.rs`        | Two passes over all files: collect function names, then emit bytecode. Locals are numbered slots per function.                                                                                                                |
| `bytecode.rs`        | Instruction set (stack machine), `FunctionProto`, `Program`.                                                                                                                                                                  |
| `vm.rs`              | Resumable interpreter, call frames, `TRY` handlers, crash dump.                                                                                                                                                               |
| `ops.rs`             | Operator semantics and numeric promotion.                                                                                                                                                                                     |
| `value.rs`           | `Value` enum, `AssocArray`, number formatting (`%g` rules).                                                                                                                                                                   |
| `builtins.rs`        | Global functions and member functions (`ifString`, `ifArray`, `ifAssociativeArray`, ...).                                                                                                                                     |
| `error.rs`           | Runtime errors with the device's codes and messages.                                                                                                                                                                          |
| `md5.rs`             | MD5, only to reproduce the crash dump's "Source Digest".                                                                                                                                                                      |
| `components.rs`      | Non-visual `ro*` objects: roList, roByteArray, roMessagePort, roDateTime, roAppInfo, roDeviceInfo, roRegistry(Section), roXMLElement/List.                                                                                    |
| `environment.rs`     | Device profile (model, locale, display), package files, registry (JSON round-trip for host persistence), image header sizes.                                                                                                  |
| `json.rs` / `xml.rs` | `ParseJson`/`FormatJson` with device ordering and escapes; small XML parser shared by roXMLElement and the component loader.                                                                                                  |
| `sg/`                | SceneGraph: `component.rs` (XML components), `schema.rs` (built-in node fields, device field order), `runtime.rs` (nodes, fields, observers, focus, keys, Task/Timer, roSGScreen), `render.rs` (boundingRect + display list). |

`launch(channel, host)` (`lib.rs`) compiles every `source/*.brs`, prints the
device's `------ Running dev '<title>' main ------` banner and starts `Main` or
`RunUserInterface` (passing an empty AA when the entry declares a parameter).

### Compiler notes

- Name resolution order: local variable, user function, builtin; an unknown
  identifier becomes a new local (BrightScript has no declarations).
- Hidden locals (loop limits, iterators) are prefixed `$` so user code cannot
  collide with them.
- `PRINT` evaluates every item first, then emits one `PrintStacked`, because the
  device prints nothing when a later item raises an error.

### Resumable VM

`Vm::run(budget)` executes at most `budget` instructions and returns:

| `Status`                | Meaning                                                                       |
| ----------------------- | ----------------------------------------------------------------------------- |
| `Running`               | Budget exhausted; call `run` again.                                           |
| `Sleeping { until_ms }` | `Sleep()` was called; resume after `until_ms` (host clock).                   |
| `Finished`              | Entry function returned.                                                      |
| `Crashed`               | Unhandled runtime error; the Micro Debugger dump was written.                 |
| `Stopped`               | `STOP` statement.                                                             |
| `Waiting`               | Every thread is blocked on a message port with no timer due; resume on input. |

All VM state lives in explicit frames, never in the Rust call stack, so the host
can pause between slices.

### Threads and SceneGraph

- Threads are register sets swapped in and out of the VM (`park`/`load`):
  `MAIN_THREAD` (0) runs `Main`, `RENDER_THREAD` (1) runs component code, and
  each `Task` gets its own thread. A round-robin scheduler gives each runnable
  thread a quantum of 20 000 instructions.
- Callbacks (`init`, observers, `callFunc`, `onKeyEvent`) run synchronously on
  the render thread via `call_sync`; a crash inside one unwinds with an
  internal "aborted" error after the dump.
- `wait(ms, port)` parks the thread; `service()` (between slices) creates
  pending scenes, finishes Tasks, fires Timers and dispatches queued remote keys
  through the focus chain.
- Every compiled component gets its own function scope (`Program.scopes`)
  containing the scripts of its whole `extends` chain.
- `sg.revision` increments whenever something visible may have changed; hosts
  re-read the display list only when it moves.

### Display list

`sg::render::display_list` flattens the shown scene into JSON in design
coordinates (`ui_resolutions`: FHD 1920x1080, else 1280x720):

```json
{"w":1920,"h":1080,"bg":<rgba>,"items":[
  ["r",x,y,w,h,rgba,opacity],
  ["i",x,y,w,h,uri,opacity,blendRgba],
  ["t",x,y,w,h,text,size,bold,rgba,opacity,horizAlign,vertAlign,wrap,maxLines],
  ["c",x,y,w,h], ["C"]
]}
```

`c`/`C` push and pop a clip (`clippingRect`). Colors are signed `0xRRGGBBAA`.

On an unhandled error, `crash` writes the same dump as the device: source window
around the failing line, `Source Digest(s)` (first 8 hex digits of the package
MD5), backtrace and local variables. See
[brightscript-semantics.md](brightscript-semantics.md#runtime-errors).

## Native host (`crates/brs-cli`)

- `brs run <dir|zip>`: runs a channel, echoes console output to stdout, honors
  `Sleep` with real waits.
- `brs conformance <dir>`: for each probe with a golden, runs it, extracts the
  text between `@@PROBE_BEGIN`/`@@PROBE_END` and diffs it against
  `golden/console.txt`. Uses `packageMd5` from `golden/meta.json` so crash dumps
  carry the device's digest. Bench probes print emulator vs device timings.
  Probes with `steps` get their remote keys (press + release) injected after
  `@@PROBE_BEGIN` + `startDelayMs`, like the recorder does on the device.

## Browser host (`crates/brs-wasm` + `apps/web`)

### WebAssembly ABI

Plain C ABI, no wasm-bindgen. Exports (`crates/brs-wasm/src/lib.rs`):

| Export                                        | Purpose                                                                                                          |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `brs_alloc(len) -> ptr`                       | Allocate a buffer for JS to copy a zip into (ownership returns to Rust).                                         |
| `brs_load_zip(ptr, len, now_ms, epoch_off)`   | Load and launch. `epoch_off = Date.now() - performance.now()` feeds roDateTime. `0` ok, `1` error.               |
| `brs_unload()`                                | Drop the running channel (Home).                                                                                 |
| `brs_inspect_zip(ptr, len)`                   | Manifest summary `{title, version, icon}` in the blob without launching; `brs_take_icon()` moves the icon bytes. |
| `brs_run(budget, now_ms) -> i32`              | Run a slice. `-1` not loaded, `0` running, `1` sleeping, `2` finished, `3` crashed, `4` stopped, `5` waiting.    |
| `brs_sleep_until() -> f64`                    | Wake-up time after status `1`.                                                                                   |
| `brs_key(pressed)`                            | Queue a remote key (name in the scratch buffer, ECP spelling).                                                   |
| `brs_revision() -> f64`                       | SceneGraph revision.                                                                                             |
| `brs_display_list(known) -> i32`              | Display list JSON in the blob; `-1` unchanged since `known`, `-2` no scene shown.                                |
| `brs_package_file() -> i32`                   | Package file (path in scratch, `pkg:/...`) in the blob; `-2` missing.                                            |
| `brs_registry_load()` / `brs_registry_take()` | Restore the registry from JSON (scratch) / take it when dirty (`-1` unchanged).                                  |
| `brs_scratch(len)`, `brs_blob_ptr/len()`      | Exchange buffers: JS -> wasm strings, wasm -> JS payloads. Valid until the next call.                            |
| `brs_take_output()`, `brs_output_len()`       | Pending console output.                                                                                          |

The status codes are mirrored in `apps/web/src/worker/engine.ts`
(`EngineStatus`); change both together.

### Worker loop (`apps/web/src/worker/runtime.worker.ts`)

1. The shell sends `launchChannel { channelId, name, zip, registry }`.
2. The worker bumps a generation counter (stale slices of a previous channel
   stop themselves), loads the zip, restores the registry and starts slicing.
3. Each slice calls `run(50_000)` repeatedly for up to 12 ms, flushes output
   through `lineBuffer` (complete lines only), then publishes state: a new
   display list (only when the revision moved) goes to `CanvasRenderer`, a
   dirty registry goes to the shell as `registry`.
4. `Sleeping` schedules the next slice at the wake-up time; `Waiting` schedules
   nothing until a `key` arrives; a final status unloads the channel, clears
   the screen and reports `channelStatus` (the shell then shows the home screen,
   like the device after a channel exits).
5. `stopChannel` (remote Home) unloads immediately and reports `idle`.

`CanvasRenderer` (`canvasRenderer.ts`) draws the display list on the
transferred `OffscreenCanvas` with Canvas2D, scaling design coordinates to the
surface. Poster images come from the package (`brs_package_file`) or over
HTTP, decoded with `createImageBitmap` and cached per URI. Text layout
(word wrap, `maxLines`, ellipsis) lives in the pure `displayList.ts`.
Provisional until F4: system font (not redistributable; a sans-serif stack is
used), `blendColor`, filtering.

### Shell and home screen (`apps/web`)

- `useEmulator` owns the worker and routes keys: on the home screen they drive
  the launcher; in a channel they go to the worker, except **Home**, which the
  OS handles (stop channel, show home). Only releases of keys whose press
  reached the channel are forwarded.
- `HomeScreen` is an original launcher (not the Roku OS UI, which is
  proprietary) with the device's interaction model: tile grid driven by the
  remote (`gridNavigation.ts`), OK opens, Options (`*`, key O) opens the tile
  menu (open / remove), Back closes overlays.
- One developer channel slot, like the device: sideloading replaces it and
  launches it. The package persists in IndexedDB and the registry in
  localStorage (`storage/devChannelStore.ts`), so a reload behaves like a
  reboot.

Messages are typed in `packages/protocol` (`ShellToWorkerMessage`,
`WorkerToShellMessage`).

## Oracle (`packages/oracle`)

Node client for the reference device:

| Module             | Responsibility                                                                  |
| ------------------ | ------------------------------------------------------------------------------- |
| `config.ts`        | Reads `.env`, validates `ROKU_HOST`.                                            |
| `digest.ts`        | HTTP Digest auth (RFC 7616) for the developer web installer.                    |
| `packager.ts`      | Zips a probe directory (explicit directory entries, no symlinks, size cap).     |
| `devInstaller.ts`  | `/plugin_install` (install/remove) and `/plugin_inspect` (screenshot).          |
| `ecp.ts`           | ECP on port 8060: keypress, launch, `query/device-info`, `query/app-ui`.        |
| `deviceConsole.ts` | Telnet console on port 8085.                                                    |
| `probe.ts`         | Probe spec (`probe.json`), markers, crash detection.                            |
| `recorder.ts`      | Full record cycle: package, install, capture, write `golden/`.                  |
| `profile.ts`       | Device profile with identifiers redacted.                                       |
| `diff.ts`          | Text, XML and pixel diffs between traces.                                       |
| `retry.ts`         | Retries connection-phase failures only (device stalls while Home reloads).      |
| `errors.ts`        | Human-readable network errors with hints (e.g. macOS Local Network permission). |

Workflow: [fidelity-workflow.md](fidelity-workflow.md).

## Security notes

- Zips are untrusted input: path normalization blocks `..`, absolute paths and
  drive letters; entry count and uncompressed size are capped (zip bombs).
- Channel code runs only inside the worker / VM; it has no access to the page.
- Channel-controlled text reaches the page only through React text nodes and
  Canvas2D `fillText` (no HTML injection); Poster HTTP images are fetched by
  the worker without credentials.
- Device credentials live only in the git-ignored `.env`; profiles and goldens
  are redacted before being written.
