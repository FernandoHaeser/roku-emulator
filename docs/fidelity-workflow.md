# Fidelity workflow

Every emulator behavior is driven by the reference device. The loop is:

1. **Write a probe** that exercises one behavior.
2. **Record** it on the device (`make record PROBE=...`) to produce a golden.
3. **Run conformance** (`make conformance`) and implement until it passes.
4. Commit probe, golden and implementation together.

CI replays the goldens without a device.

## 1. Writing a probe

A probe is a minimal channel:

```
fidelity/<area>/<name>/
  manifest          # title=probe-<area>-<name>, version fields
  source/main.brs   # prints between the markers
  probe.json        # optional
  golden/           # written by the recorder, committed
```

`source/main.brs` prints its observable output between the markers:

```brightscript
sub Main()
    print "@@PROBE_BEGIN"
    print 7 \ 2, -7 mod 3
    print "@@PROBE_END"
end sub
```

Guidelines:

- One area per probe, many cases per area. Keep cases independent so one error
  does not hide the rest; when a case is expected to crash, put it last (a crash
  ends the probe, and the dump is part of the golden).
- Print types with `type(x)` when the type is part of the behavior.
- Never print device identifiers, tokens or anything user specific.

`probe.json` (optional):

```json
{
  "kind": "conformance",
  "steps": [{ "key": "Down", "waitMs": 300 }],
  "startDelayMs": 1000,
  "settleMs": 500,
  "capture": { "appUi": true, "screenshot": true }
}
```

- `kind`: `conformance` (default, diffed) or `bench` (timings only, never
  diffed; used to calibrate device mode).
- `steps`: remote keys sent in order, each followed by `waitMs`. They start
  once `@@PROBE_BEGIN` is printed plus `startDelayMs` (default 1000), so the
  scene is ready to receive them. `OK` is sent to ECP as `Select`. Conformance
  injects the same keys (press + release) into the emulator.
- `startDelayMs`: delay between the BEGIN marker and the first step.
- `settleMs`: extra wait before app-ui/screenshot capture.
- `capture`: also save `query/app-ui` and a screenshot (SceneGraph probes).

## 2. Recording on the device

Requirements: `make setup` done, `.env` filled (see `.env.example`), device
in Developer Mode, _Control by mobile apps_ set to **Permissive**.

```sh
make record PROBE=fidelity/language            # every probe in an area
make record PROBE=fidelity/language/numbers    # a single probe
```

For each probe the recorder packages the directory (zip with explicit directory
entries, which the device's installer requires), sideloads it, captures the
telnet console on port 8085 until the end marker or a crash dump, and writes:

| File                    | Contents                                                                                                                                            |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `golden/console.txt`    | Text between the markers; on a crash, everything after the begin marker including the full debugger dump (the recorder waits 2 s for it to finish). |
| `golden/meta.json`      | Kind, status, profile, timestamp, duration, `packageMd5`.                                                                                           |
| `golden/app-ui.xml`     | `query/app-ui` dump (when `capture.appUi`).                                                                                                         |
| `golden/screenshot.jpg` | Device screenshot (when `capture.screenshot`).                                                                                                      |

Raw logs go to the git-ignored `.oracle/`. Review the golden diff before
committing it: a golden is a specification.

`packageMd5` matters: the crash dump prints the first 8 hex digits of the
package MD5, and conformance reuses it so the emulator's dump matches.

Re-record an area after a Roku OS update and review what changed.

## 3. Conformance

```sh
make conformance        # native engine vs every golden
make e2e                # browser (wasm) vs selected goldens, via Playwright
```

Output per probe: `PASS`, `FAIL` (first differing line, device vs emulator),
`SKIP` (needs a subsystem not built yet, e.g. SceneGraph) or `BENCH` (emulator
and device timings side by side).

A failing probe is never edited or deleted to make CI green. If the emulator
cannot match yet, keep the failure and list it as a known gap in the README and
in [brightscript-semantics.md](brightscript-semantics.md).

## 4. Documenting a finding

When a probe reveals a behavior, add it to
[brightscript-semantics.md](brightscript-semantics.md) with the probe name.
Mark guesses as **provisional** until a probe pins them.
