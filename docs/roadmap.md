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
   publisher created; finish the Entra ID (OIDC) publishing setup in
   [marketplace-publishing.md](marketplace-publishing.md) and decide
   when to publish.
4. F4 renderer: in progress. Done: 720p surface like the device, screenshot
   diff suite (`apps/web/e2e/visual.spec.ts`, budgets in each `probe.json`),
   device edge snapping for rectangles/posters, calibrated text metrics
   (`scripts/gen-font-metrics.mjs`), bundled substitute font, Poster bitmap
   loading and natural size, blendColor, rotation, Animation/interpolators,
   remote Poster load results in native and WASM hosts, 9-patch and package
   fonts (XML `role="font"`, shaping and shared line layout). Still open:
   system-font metrics remain approximate (including a 3 px difference in
   `label-package-font`'s system-font reference); substitute glyphs differ
   visually from Roku's proprietary font. Open measurements: one HD column of
   `render-rects-hd` is 1 px narrower than the floor rule; creating render
   nodes from the main thread fails on the device but not in the emulator.
5. F5 widgets: in progress on `feature/f5-widgets`. Seven HD/FHD device probes cover
   LabelList navigation/wrap, RowList per-row column memory, MarkupGrid custom
   item bindings, ButtonGroup/modal Dialog events and Keyboard text/cursor
   behavior. Browser tests compare event traces and device screenshots. Widget
   animation timing, sections, specialized variants and full theme geometry
   still require more work; this is not complete widget compatibility.
6. F6 video: in progress on `feature/f6-video`. Done: Video node with the
   device's 146 field defaults and state sequence (`video/lifecycle`: MP4
   from HTTP and `pkg:/`, HLS, HTTP 404 error fields), a simulated player for
   CLI/VS Code and real playback in the web shell (`<video>` under the canvas,
   Shaka Player + mux.js for HLS/DASH over the local proxy). Widevine DRM
   (`video/drm`): `drmParams` license server, device error fields for a
   missing or failing license server, EME playback through Shaka where a CDM
   exists. Next: trick play, captions and audio tracks.
7. F8 shell tools: in progress on `feature/f8-shell`. Done: clickable remote
   (pointer press/release, same routing as the keyboard, Home handled by the
   OS) and the node inspector (live tree from the engine's `scene`
   inspection, fields, filter, highlight over the screen). Next: network
   panel, registry panel and hot reload.
8. After the agreed
   implementation sequence, freeze feature work and review GitHub tasks #5–#16
   (platform integrity and CI/CD). Native desktop/F9 mocks remain later roadmap
   items. Do not start release/publication automation ahead of that review.

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
