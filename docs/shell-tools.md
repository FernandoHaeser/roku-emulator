# Shell tools (F8)

The web shell and embedded VS Code runtime share the clickable remote, console,
live node inspector and development panels.

- **Rede** records the last 200 runtime HTTP requests (including Poster loads and
  video metadata/license preflight), with method, sanitized URL, status, response
  size and duration. Headers, payloads, credentials, fragments and query values
  are not retained. Shaka's adaptive media segment requests and browser-native
  progressive video transfers are not included in this first network view.
- **Registro** polls only while visible. It reads, creates, edits and deletes
  individual keys of the active development channel. Values are masked until
  explicitly revealed. Changes persist through reload/redeploy in both hosts.
- **Roku Emulator: Ativar/desativar hot reload** watches a channel workspace in
  VS Code. After edits settle, it packages and reinstalls the channel, keeping
  registry data. Deployments are serialized; ignored package paths, including
  `.git`, `.vscode`, `.env*`, dependencies and build outputs, do not trigger it.
  Stopping the emulator cancels the watcher and pending work. Starting a debug
  session disables hot reload so a source save cannot replace a paused program.

The hot reload command requires a channel `manifest` at the workspace root. It
uses the existing local installer and reports package/deploy errors in the Roku
Emulator output channel. It does not publish the channel or the extension.

Browser tests own a separate Vite server and separate device transport ports.
They never reuse the interactive development server: two browser workers on one
bridge would both execute debugger/installer commands. Defaults are web 15173,
ECP/installer 18060, console 18085, debugger 18081 and SceneGraph console 18080.
Override them with `ROKU_EMU_TEST_*_PORT` when needed. The VS Code integration test
uses its own workspace ports as well.

No Marketplace publication or release tags are part of this delivery.
