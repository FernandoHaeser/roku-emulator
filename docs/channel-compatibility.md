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
are not stored in this repository. Catalog layout still shows overlaps;
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

The MiniKeyboard screenshots differ by 0.357% (FHD) and 0.382% (HD). Their
behavioral traces pass in both resolutions. Like the selection-control probes,
they document a substitute-artwork budget (0.5%): the clear/space/delete icons
are locally drawn glyphs instead of Roku system artwork, and key labels use
the substitute font. Icon and glyph parity remain open. The standard keyboard dialog's final-state
screenshot passes its current budget; its open-dialog artwork still differs
from the device and requires broader visual coverage.

Physical keyboard input is an emulator convenience alongside remote navigation.
The active TextEditBox or keyboard accepts Unicode text, spaces and punctuation,
Backspace/Delete, and Home/End. Left/right move the insertion point in a standalone
field; with the virtual keyboard open they continue navigating its keys. Text
uses the same field observers and length limit as virtual entry, and password
fields remain masked. Escape and the on-screen remote still provide Back and
Options when their former letter/backspace shortcuts are used for typing.

The first result for `demo` (`CDNTV - SEDE-POA`) supplies the bare hostname
`demo`. Authentication fails before an HTTP response on both runtimes: the
physical Roku reports DNS error -6; the emulator currently reports transport
error -7. The transport error difference remains open; the endpoint is not
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
