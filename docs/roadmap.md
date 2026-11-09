# Delivery roadmap

The implementation status in the [README](../README.md#roadmap) is the source
of truth. The old planning snapshot predates the Rust core, Canvas2D host and
home screen; those are the current architecture.

## Next milestones

1. F7 deploy, console and debugger: done. Binary protocol (8081), telnet,
   expression evaluation, variable expansion/assignment, frame/thread selection
   and suspension inside synchronous SceneGraph callbacks are covered by
   browser and VS Code tests. See [supported operations and limits](vscode-deploy.md).
2. F2 network and remaining runtime objects: done. Local proxy with sync/async
   `roUrlTransfer`, events, headers, cookies, Digest and cancellation;
   `roFileSystem`/`roPath`, `roRegex`, `roEVPDigest`/`roHMAC`/`roEVPCipher`,
   verified on the device by `runtime/extended`.
3. F10 VS Code extension: ready for publication. Embedded runtime (Node
   Worker), webview screen, project configuration, `roku-emulator` debug type,
   Marketplace listing (README, icon, changelog, compatibility), VSIX
   install/upgrade/uninstall and debug-session tests on Linux, Windows and
   macOS in CI, and a tag-driven release workflow. Remaining (owner actions):
   create the Marketplace publisher, add `VSCE_PAT` to the `marketplace`
   environment and decide when to publish.
4. Resume F4/F5 visual fidelity and widgets, then F6 playback/DRM and the
   remaining tools (F8), native desktop and F9 mocks.

No release dates are committed. A phase being implemented does not imply
complete Roku compatibility; the recorded probes define verified behavior.

## F10 — First-party VS Code extension

This is an additional delivery, separate from F7 compatibility with the
existing BrightScript extension. Keep RokuCommunity language and debug tooling
as the integration target; avoid duplicating its language services.

Planned capabilities:

- Install/select a compatible emulator runtime; report missing prerequisites
  and allow a user-supplied local runtime.
- Start, stop and restart the emulator from commands and a status indicator;
  clean up processes owned by the extension.
- Show the emulator screen inside VS Code, with remote controls and console.
- Detect a Roku channel workspace and offer a reviewable configuration change
  for deployment/debugging; preserve existing launch configurations.
- Reuse F7 deployment and debugger transports, with actionable startup errors.
- Build a versioned VSIX, document compatibility, test install/upgrade/uninstall,
  and prepare Marketplace listing, icon, license and release automation.

Dependencies: stable runtime startup/distribution, F7 transport compatibility,
webview integration and cross-platform validation. Prototype with a local VSIX
before Marketplace publication. Publisher identity and public publication are
separate release actions, not implied by adding this roadmap entry.

Acceptance: a fresh supported VS Code installation can install the extension,
open a channel project, start the emulator, configure and deploy the channel,
view its screen and console, and use the debugger features supported by F7.
The repository must contain repeatable validation and clearly document any
platform or feature limits. Nothing has been published yet.
