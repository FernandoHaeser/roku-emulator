# Real-channel compatibility check

A local development ZIP is executed unchanged. Its sources, configuration,
credentials and assets are not stored in this repository.

The channel now compiles, completes the account-service callback, loads its
provider list and shows the provider selection screen. Login, catalog navigation
and playback still require the user's provider and account; they have not been
validated with the private channel.

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
