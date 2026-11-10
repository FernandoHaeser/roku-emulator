# Real-channel compatibility check

A local development ZIP is executed unchanged. Its sources, configuration,
credentials and assets are not stored in this repository.

The channel now compiles, completes the account-service callback, loads its
provider list and shows the provider selection screen. The app's login form now
opens, accepts test text through its standard keyboard dialogs, returns it to the
username and password fields, and masks the password. On 2026-11-10, login with
the user-provided test account succeeded on the second result for `demo`
(`demo.cdn.tv.br`): the authentication callback completed and the channel grid
loaded without reported runtime errors. Credentials, tokens and private traces
are not stored in this repository. Catalog overlap was corrected with native LayoutGroup and MarkupGrid geometry;
playback has not been validated by this login check.

On 2026-11-10, the same remote-key sequence typed exactly `demo` in the
emulator and the physical Roku. Both displayed the same six matching providers.
Moving down out of the MiniKeyboard, then right and OK, checked the first
provider and transferred focus to “Próximo” in both runtimes. The check stopped
there, before login. The physical run reported no compile or runtime errors.
Screenshots still show differences in Poster aspect ratio, button artwork and
the list highlight; this validates the interaction, not full visual parity.

Follow-up fixes preserve Poster aspect ratio for `scaleToFit`, clip
`scaleToZoom`, autobox intrinsic values for `GetInterface`, and avoid borrowing
a node's own reference during field equality and conversion. The latter caused
the WebAssembly panic when the login screen repeatedly focused the same node.
Regression tests cover self-references, repeated focus and Poster layout.

The new `widgets/standard-keyboard-dialog` device trace covers typing, text
binding, confirmation and closure. Remote text entry is implemented; voice
dictation and complete standard-dialog visual/theme parity remain pending.

Compatibility fixes:

- Newline-separated array and associative-array members.
- Optional field access (`?.`), including the debugger evaluator.
- Unboxing SceneGraph strings and booleans for typed parameters/returns.
- `BusySpinner.poster`, URI forwarding, clock-driven rotation, controls, direction
  and interval. Its Poster exists before custom `init()`, loads asynchronously,
  and follows the device's replacement/URI behavior.
- `signalBeacon` accepts the supported launch/dialog/EPG/resume milestones and
  prints a console marker. Performance duration accounting is not implemented.
- `RadioButtonList` inherits `LabelList`; selection state, `checkOnSelect`, key
  propagation, icon spacing and wrap-divider layout follow device probes.
- Keyboard bounds are available during initialization; boundary keys propagate
  and focus feedback disappears on blur.
- MiniKeyboard uses its own six-column layout, 25-character default limit and
  clear/space/backspace row. New HD and FHD device probes match its behavioral
  trace; visual parity is still being refined.

Roku account services are unavailable locally. `ChannelStore` returns an explicit
failure result for credential, catalog, purchase and attestation requests, and
cancellation for user-data requests. Repeated commands notify again. This does not
supply Roku cloud credentials or grant purchases. `Roku_Analytics:AnalyticsNode`
is a clearly logged local no-op mock; it transmits no telemetry. Configurable
service mocks remain part of F9.

New device traces: `widgets/busy-spinner`, `widgets/selection-controls` and its HD
variant. The selection screenshots use explicit difference budgets of 1.25% FHD
and 1.5% HD for the substitute font and keyboard/icon artwork. Existing visual
budgets were not changed. Behavioral traces must match exactly.

References: [collection literals and optional operators](https://developer.roku.com/dev/docs/expressions-variables-types),
[BusySpinner](https://developer.roku.com/dev/docs/busyspinner),
[RadioButtonList](https://developer.roku.com/dev/docs/radiobuttonlist), and
[ChannelStore](https://developer.roku.com/dev/docs/channelstore).

The MiniKeyboard screenshots currently differ by 0.263% (FHD) and 0.241% (HD)
on macOS (0.402% / 0.396% on the Linux CI runner before the latest changes).
Key labels are bold like the captures, the clear/space/delete icons and the
focus box (1 px black border, rounded corners, 2 px gradient bands) use
geometry measured on the device screenshots. The remainder is glyph shape:
the key grid alone accounts for about 1,900 differing pixels (about 55 per
glyph, the substitute font against the Roku system font), more than a whole
0.1% budget (921 pixels at 1280x720). Both probes therefore use a 0.5%
budget, like the other keyboard probes whose budgets cover the substitute
font, with margin for the Linux rasterizer. Any non-glyph regression still
fails it.
Behavioral traces still pass in both resolutions. The standard keyboard dialog's
final-state screenshot passes its current budget; its open-dialog artwork still
differs from the device and requires broader visual coverage.

Physical keyboard input is an emulator convenience alongside remote navigation.
The active TextEditBox or keyboard accepts Unicode text, spaces and punctuation,
Backspace/Delete, and Home/End. Left/right move the insertion point in a standalone
field; with the virtual keyboard open they continue navigating its keys. Text
uses the same field observers and length limit as virtual entry, and password
fields remain masked. Escape and the on-screen remote still provide Back and
Options when their former letter/backspace shortcuts are used for typing.

The first result for `demo` (`CDNTV - SEDE-POA`) supplies the bare hostname
`demo`. Authentication fails before an HTTP response on both runtimes: the
physical Roku reports DNS error -6. The emulator now maps resolver failures
(ENOTFOUND/EAI_AGAIN, including wrapped fetch causes) to -6, while connection
refusal remains -7. A transport test verifies both cases. The endpoint is not
rewritten to another provider.

The InfoPane used to show that failure is now implemented from the
`widgets/info-pane` probe: subtype `InfoLabel`, five children, stacked text,
bullets and footer blocks, and the requested height acting as a minimum. Its
screenshot differs by 0.137% (budget 0.5%, as for other text-heavy probes);
the remainder is glyph rasterization from the substitute font.

Device rules recorded while doing so, each with its own probe:

- A Label's default `font` is a Font node (uri `""`, size 34 FHD / 22 HD)
  measured like SmallSystemFont, with a 36 px FHD line. Code such as
  `label.font.size = 40` used to fail because the emulator stored a string.
- A Font node with a system uri ignores `size`, whatever the assignment
  order; a channel Font node without a uri draws nothing (0x0)
  (`scenegraph/label-font-sizes{,-hd}`).
- Poster `loadDisplayMode` only applies when set before `uri`; later changes
  are ignored (`scenegraph/poster-display-mode`).
- Decimal integer literals with 10 or more digits are Double, and FormatJson
  writes integral Float/Double values in full (`runtime/json-numbers`).
- `isSubtype()` excludes the node's own type, except where a native widget
  reports a public type other than its own (InfoPane → `InfoLabel`).

The physical device became reachable again on 2026-11-10. The new
`scenegraph/field-alias` probe validates initialization before `init()`, explicit
initial values, parent/child writes, nested aliases, callback identity and order,
message-port snapshots, repeated vector values, removal and clone isolation.
Native aliases do not redirect built-in fields. Cloning a custom alias field
that is absent from the native class returns `invalid` with a diagnostic;
cloning an ordinary target child does not retain its parent's alias bindings.
This coverage does not establish complete SceneGraph compatibility;
`ComponentLibrary` and broader invalid-alias cases remain pending.

The default Scene background is now based on a separate device measurement
(`scenegraph/scene-background`): `backgroundColor` initially reads `0x0B001BFF`,
while an opaque Geneva theme image is visible above it. Assigning only the color
does not remove that image. Explicitly assigning `backgroundUri = ""`, even
though the field already reads empty, clears it and reveals the color. The
renderer reproduces the measured solid theme color; custom background images
and other Roku themes still require coverage.

Opening a catalog item exposed the missing `Video.setHttpAgent()` method.
`scenegraph/http-agent` now covers native agent access, interface discovery,
ancestor inheritance, per-node overrides and rejected assignments. Video
metadata/preflight and adaptive web playback use the selected agent's headers;
`ContentNode.HttpHeaders` replaces its custom headers, as observed on the Roku.
The browser HLS tests require the expected headers on both manifests and media
segments and reject stale agent headers after metadata replacement. The same
owned HLS fixture was checked on the physical device in both modes.

This does not complete media/network compatibility: forwarding node-agent
settings in the Poster loader, custom headers for native progressive browser
playback, complete certificate/client identity handling and broader separate
DRM-agent coverage remain open.

After the agent fix, the unchanged ZIP also opened the first catalog stream
from the authenticated second `demo` provider. The browser's actual media
player reported `readyState = 4`, advancing past 18 seconds with `paused = false`,
no media error and no channel runtime error. This is a single-stream playback
check; captions, alternate audio, trick play, other providers and protected
streams are not validated by it.
