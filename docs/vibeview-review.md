# VibeView reuse assessment

Reviewed on 2026-11-10: `vibeview` 0.76.0 from npm, and
`@vibeview/roku-bsc-plugin` 0.1.1 / public repository commit
`4aef66984bd09e6580a657d42b95cdf6b00b2fdd`.
Packages were inspected in a temporary directory without executing their CLI,
install scripts, login or cloud sessions. No channel was uploaded.

## Architecture and relevance

VibeView runs Roku channels on physical Roku hardware streamed from its cloud.
Its Roku development loop packages a ZIP, uploads it and installs/relaunches it
in a remote session. This can provide another reference device for conformance;
it does not provide a local BrightScript VM, SceneGraph renderer or RokuOS
implementation to incorporate into this emulator. See the
[official Roku workflow](https://vibeview.io/guides/claude-code-roku/) and
[CLI documentation](https://vibeview.io/docs/cli/).

The published CLI package declares `license: UNLICENSED`. Its repository link
points to the public skills repository rather than an openly licensed emulator
engine. No CLI code was copied. The separate
[Roku BrighterScript plugin](https://github.com/vibeview/roku-bsc-plugin)
contains an MIT license (copyright VibeView 2026); any future copied plugin code
must retain its copyright and permission notice.

## Useful techniques

| Technique                          | Existing implementation                                               | Useful extension                                                                                     |
| ---------------------------------- | --------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Build, install, relaunch on change | VS Code hot reload and deploy transports                              | A shared CLI development loop and machine-readable results; no cloud dependency required             |
| Inspect UI, logs and screenshots   | Scene inspector, console and native oracle captures                   | A common automation API against either the emulator or a reference device                            |
| Stable item identity               | Inspector exposes node fields; probes mostly use remote-key sequences | Resolve list items by `itemContent.id` plus component/node identity; use labels only as fallback     |
| Separate highlight from focus      | Core tracks focus and list item state                                 | Tests should report focused container and highlighted item separately                                |
| Track recycled item content        | List bindings update `itemContent`                                    | Recompute semantic identity when content changes instead of caching the original item title/index    |
| Remote reference sessions          | Local physical Roku oracle                                            | Optional cloud adapter for additional hardware/OS profiles, after account, cost and upload decisions |

These are design opportunities, not newly implemented features or additions to
the agreed delivery sequence. Existing hot reload and device transports should
be reused instead of introducing a second deployment stack.

## What the open plugin actually does

The plugin observes configured highlight and identity fields and adds a hidden
1×1 Label containing focus/id/label markers that the device's ECP app-ui dump
can expose. The analyzer handles component inheritance and checks whether the
configured highlight field is boolean. The injected runtime updates identity
when recycled list content changes. It is build-time instrumentation, not a
SceneGraph implementation. See
[`src/runtime.ts`](https://github.com/vibeview/roku-bsc-plugin/blob/4aef66984bd09e6580a657d42b95cdf6b00b2fdd/src/runtime.ts),
[`src/analyze.ts`](https://github.com/vibeview/roku-bsc-plugin/blob/4aef66984bd09e6580a657d42b95cdf6b00b2fdd/src/analyze.ts)
and the [plugin guide](https://vibeview.io/docs/roku-plugin/).

Our engine already sees the full node tree, so semantic item lookup can be
implemented through inspection without injecting marker nodes into a user's
ZIP. Marker instrumentation would be optional and limited to channels built
for that test purpose. It changes the child tree; even invisible children can
affect bounds and enumeration. Therefore an instrumented build must not replace
unchanged-ZIP conformance tests.

## Decision

Do not add VibeView as a runtime dependency. Keep the local emulator and native
oracle as the implementation baseline. The strongest opportunities are semantic
focus/item targeting and an optional reference-device adapter. Neither fixes
font rasterization, missing SceneGraph APIs, video controls, DRM availability
or other RokuOS compatibility gaps by itself. Actual cloud execution was not
tested; it requires a separate authenticated service session.
