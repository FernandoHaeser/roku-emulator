/// <reference lib="webworker" />
/**
 * Emulator runtime entry point. Runs inside a dedicated Web Worker so channel
 * code never touches the DOM. Drives the Rust engine in time slices, renders
 * the SceneGraph display list onto the transferred canvas and forwards remote
 * keys.
 */
import type {
  ChannelStatus,
  RemoteKey,
  ShellToWorkerMessage,
  VideoCommand,
  WorkerToShellMessage,
} from "@roku-emu/protocol";
import { SHELL_RPC_PREFIX } from "@roku-emu/protocol";
import engineUrl from "@/wasm/brs.wasm?url";
import { CanvasRenderer } from "./canvasRenderer";
import { parseDisplayList, videoRects } from "./displayList";
import { BrsEngine, EngineStatus, type EngineStatusCode } from "./engine";
import { waitDebugCommand } from "./debugMailbox";
import { imageSize } from "./imageHeaders";
import { LineBuffer } from "./lineBuffer";
import { loadSubstituteFont } from "./substituteFont";

declare const self: DedicatedWorkerGlobalScope;

/** Instructions per engine call, and max wall time per slice before yielding. */
const INSTRUCTION_BUDGET = 50_000;
const SLICE_MS = 12;

const post = (message: WorkerToShellMessage, transfer: Transferable[] = []): void =>
  self.postMessage(message, transfer);

let renderer: CanvasRenderer | null = null;
let headless = false;
let debugMailbox: SharedArrayBuffer | undefined;
let nextHttpId = 1;
const httpRequests = new Map<number, { coreId: number; generation: number; imageUri?: string }>();
let enginePromise: Promise<BrsEngine> | null = null;
let active: BrsEngine | null = null;
let channelName = "";
let channelId = "";
/** Incremented on every launch/stop so a stale scheduled slice stops itself. */
let generation = 0;
/** Display list revision last drawn (-1 forces a redraw). */
let drawnRevision = -1;
/** Last video layout sent to the shell, and blob URLs of package media. */
let videoLayoutKey = "[]";
const videoBlobUrls: string[] = [];
/** Pending wake-up (sleep or yield) of the current run. */
let wakeTimer: ReturnType<typeof setTimeout> | null = null;
const lines = new LineBuffer();

function engine(): Promise<BrsEngine> {
  enginePromise ??= BrsEngine.load(
    engineUrl,
    debugMailbox
      ? {
          wait(output) {
            if (output) {
              post({ type: "consoleRaw", text: output });
              for (const line of [...lines.push(output), ...lines.flush()])
                post({ type: "console", stream: "stdout", text: line });
            }
            reportStatus("stopped");
            return waitDebugCommand(debugMailbox!, () => post({ type: "debuggerWaiting" }));
          },
          resumed() {
            reportStatus("running");
          },
        }
      : undefined,
    {
      request(coreId, request) {
        const id = nextHttpId++;
        if (coreId) {
          httpRequests.set(id, { coreId, generation });
          post({ type: "httpRequest", id, request });
          return null;
        }
        if (typeof SharedArrayBuffer !== "function")
          return JSON.stringify({
            status: -7,
            failure: "Synchronous HTTP requires cross-origin isolation",
          });
        const buffer = new SharedArrayBuffer(12 * 1024 * 1024 + 8);
        return waitDebugCommand(buffer, () => post({ type: "httpRequest", id, request, buffer }));
      },
      cancel(coreId) {
        for (const [id, pending] of httpRequests)
          if (pending.coreId === coreId && pending.generation === generation) {
            httpRequests.delete(id);
            post({ type: "httpCancel", id });
          }
      },
    },
  );
  return enginePromise;
}

async function initSurface(canvas: OffscreenCanvas, width: number, height: number): Promise<void> {
  canvas.width = width;
  canvas.height = height;
  await loadSubstituteFont();
  try {
    renderer = new CanvasRenderer(canvas, (uri) => active?.packageFile(uri) ?? null);
  } catch (error) {
    post({ type: "fatal", message: String(error) });
    return;
  }
  post({ type: "ready", renderer: `Canvas2D ${width}x${height}` });
  post({ type: "console", stream: "system", text: "Runtime pronto. Abra um canal (.zip)." });
}

/** Inspection responses the engine writes on the console. */
const RPC_LINE = /\n?@@ROKU_EMU_RPC ([^\n]*)\n/g;

/**
 * Removes responses to the shell's own inspection requests (node inspector
 * polling) from the console stream and posts them as `inspection` messages.
 * Responses for other clients (bridge/ECP) stay in the stream.
 */
function takeShellResponses(text: string): string {
  if (!text.includes("@@ROKU_EMU_RPC ")) return text;
  return text.replace(RPC_LINE, (line: string, json: string) => {
    let id: unknown = "";
    try {
      id = (JSON.parse(json) as { id?: unknown }).id;
    } catch {
      return line;
    }
    if (typeof id !== "string" || !id.startsWith(SHELL_RPC_PREFIX)) return line;
    post({ type: "inspection", json });
    return "";
  });
}

function emitOutput(instance: BrsEngine, final = false): void {
  const text = takeShellResponses(instance.takeOutput());
  if (text) post({ type: "consoleRaw", text });
  const complete = lines.push(text);
  if (final) complete.push(...lines.flush());
  for (const line of complete) post({ type: "console", stream: "stdout", text: line });
}

function reportStatus(status: ChannelStatus): void {
  post({ type: "channelStatus", status, name: channelName, channelId });
}

function toChannelStatus(code: EngineStatusCode): ChannelStatus {
  switch (code) {
    case EngineStatus.Running:
    case EngineStatus.Waiting:
      return "running";
    case EngineStatus.Sleeping:
      return "sleeping";
    case EngineStatus.Finished:
      return "finished";
    case EngineStatus.Crashed:
      return "crashed";
    case EngineStatus.Stopped:
      return "stopped";
    default:
      return "error";
  }
}

/** Pushes the SceneGraph frame and registry changes produced by the last slice. */
function publishState(instance: BrsEngine): void {
  post({
    type: "textInputState",
    active: instance.textInputMode() !== 0,
    virtualKeyboard: instance.textInputMode() === 2,
  });
  for (const uri of instance.imageRequests()) {
    const id = nextHttpId++;
    httpRequests.set(id, { coreId: 0, generation, imageUri: uri });
    post({
      type: "httpRequest",
      id,
      request: JSON.stringify({ url: uri, method: "GET", timeoutMs: 30000 }),
    });
  }
  for (const command of instance.takeVideoCommands() as VideoCommand[]) {
    post({ type: "videoCommand", command: resolveVideoSource(instance, command) });
  }
  const result = instance.displayList(drawnRevision);
  if (result.kind === "list") {
    drawnRevision = instance.revision();
    if (headless) post({ type: "displayList", json: result.json });
    else renderer?.show(result.json);
    if (!headless) publishVideoLayout(result.json);
  }
  const registry = instance.takeRegistry();
  if (registry !== null) post({ type: "registry", channelId, json: registry });
}

/** Package media becomes a same-origin blob URL the shell's player can open. */
function resolveVideoSource(instance: BrsEngine, command: VideoCommand): VideoCommand {
  if (command.op !== "load" || !command.url.startsWith("pkg:/")) return command;
  const bytes = instance.packageFile(command.url);
  if (!bytes) return command;
  const url = URL.createObjectURL(new Blob([bytes], { type: "video/mp4" }));
  videoBlobUrls.push(url);
  return { ...command, url };
}

/** Tells the shell where video planes are, only when they change. */
function publishVideoLayout(json: string): void {
  const list = json.includes('["v",') ? parseDisplayList(json) : null;
  const rects = list ? videoRects(list) : [];
  const key = JSON.stringify(rects);
  if (key === videoLayoutKey) return;
  videoLayoutKey = key;
  post({ type: "videoLayout", width: list?.w ?? 0, height: list?.h ?? 0, rects });
}

function resetVideo(): void {
  post({ type: "videoCommand", command: { op: "reset" } });
  if (videoLayoutKey !== "[]") post({ type: "videoLayout", width: 0, height: 0, rects: [] });
  videoLayoutKey = "[]";
  for (const url of videoBlobUrls) URL.revokeObjectURL(url);
  videoBlobUrls.length = 0;
}

function schedule(instance: BrsEngine, runGeneration: number, delayMs: number): void {
  if (wakeTimer !== null) clearTimeout(wakeTimer);
  wakeTimer = setTimeout(() => {
    wakeTimer = null;
    runSlice(instance, runGeneration);
  }, delayMs);
}

function runSlice(instance: BrsEngine, runGeneration: number): void {
  if (runGeneration !== generation) return;
  const sliceStart = performance.now();
  let status: EngineStatusCode;
  do {
    status = instance.run(INSTRUCTION_BUDGET, performance.now());
  } while (status === EngineStatus.Running && performance.now() - sliceStart < SLICE_MS);

  const alive =
    status === EngineStatus.Running ||
    status === EngineStatus.Sleeping ||
    status === EngineStatus.Waiting;
  emitOutput(instance, !alive);
  publishState(instance);
  if (status === EngineStatus.Running) {
    schedule(instance, runGeneration, 0);
  } else if (status === EngineStatus.Sleeping) {
    schedule(instance, runGeneration, Math.max(0, instance.sleepUntil() - performance.now()));
  } else if (status === EngineStatus.Stopped) {
    reportStatus("stopped");
  } else if (status === EngineStatus.Waiting) {
    // Nothing runnable until input arrives; `key` restarts the loop.
  } else {
    // The device returns to the home screen when a channel exits or crashes.
    instance.unload();
    active = null;
    renderer?.clear();
    drawnRevision = -1;
    reportStatus(toChannelStatus(status));
  }
}

/**
 * Unloads the active channel. `reportExit` is false when a new launch replaces
 * it: reporting "idle" there would send the shell back to the home screen
 * while the new channel starts.
 */
function stopChannel(reportExit = true): void {
  post({ type: "textInputState", active: false, virtualKeyboard: false });
  for (const id of httpRequests.keys()) post({ type: "httpCancel", id });
  httpRequests.clear();
  generation += 1;
  if (wakeTimer !== null) clearTimeout(wakeTimer);
  wakeTimer = null;
  if (active) {
    active.unload();
    emitOutput(active, true);
    active = null;
    // "idle": the user left the channel (Home), as opposed to a STOP statement.
    if (reportExit) reportStatus("idle");
  }
  renderer?.clear();
  if (headless) post({ type: "displayList", json: null });
  else resetVideo();
  drawnRevision = -1;
}

async function launchChannel(
  id: string,
  name: string,
  zip: ArrayBuffer,
  registry: string | null,
  stopOnEntry = false,
  launchArgs: Record<string, string> = {},
): Promise<void> {
  stopChannel(false);
  const runGeneration = generation;
  channelName = name;
  channelId = id;
  lines.flush();
  try {
    const instance = await engine();
    if (runGeneration !== generation) return;
    const epochOffset = Date.now() - performance.now();
    const loaded = instance.loadChannel(
      new Uint8Array(zip),
      performance.now(),
      epochOffset,
      launchArgs,
    );
    if (!loaded) {
      emitOutput(instance, true);
      reportStatus("error");
      return;
    }
    if (registry) instance.loadRegistry(registry);
    // The shell plays video with a real player; headless hosts keep the
    // engine's simulated playback.
    if (!headless) instance.setVideoExternal(true);
    active = instance;
    reportStatus("running");
    if (stopOnEntry) instance.debuggerCommand("break");
    runSlice(instance, runGeneration);
  } catch (error) {
    post({ type: "fatal", message: `Falha ao carregar o engine: ${String(error)}` });
    reportStatus("error");
  }
}

async function inspectChannel(requestId: number, zip: ArrayBuffer): Promise<void> {
  try {
    const instance = await engine();
    const summary = instance.inspect(new Uint8Array(zip));
    const icon = summary.icon ? summary.icon.buffer : null;
    post(
      {
        type: "channelInfo",
        requestId,
        info: { title: summary.title, version: summary.version, icon },
      },
      icon ? [icon] : [],
    );
  } catch (error) {
    post({
      type: "channelInfoError",
      requestId,
      message: error instanceof Error ? error.message : String(error),
    });
  }
}

function sendKey(key: RemoteKey, pressed: boolean): void {
  const instance = active;
  if (!instance) return;
  instance.key(key, pressed);
  // Wake a channel blocked on input right away (sleeping ones too: keys are
  // delivered by the render thread, which must not wait for the sleep to end).
  schedule(instance, generation, 0);
}

self.onmessage = (event: MessageEvent<ShellToWorkerMessage>) => {
  const message = event.data;
  switch (message.type) {
    case "init":
      debugMailbox = message.debugMailbox;
      headless = message.headless === true;
      if (headless) post({ type: "ready", renderer: "Node Worker / WebAssembly" });
      else if (message.canvas) void initSurface(message.canvas, message.width, message.height);
      break;
    case "launchChannel":
      void launchChannel(
        message.channelId,
        message.name,
        message.zip,
        message.registry,
        message.stopOnEntry,
        message.launchArgs,
      );
      break;
    case "stopChannel":
      stopChannel();
      break;
    case "videoEvent":
      if (active) {
        active.videoEvent(message.event);
        schedule(active, generation, 0);
      }
      break;
    case "inspectChannel":
      void inspectChannel(message.requestId, message.zip);
      break;
    case "input":
      if (active) {
        active.input(message.args);
        schedule(active, generation, 0);
      }
      break;
    case "httpComplete": {
      const request = httpRequests.get(message.id);
      httpRequests.delete(message.id);
      if (active && request && request.generation === generation) {
        if (request.imageUri) {
          let size: [number, number] | null = null;
          try {
            const response = JSON.parse(message.response) as { status: number; body?: string };
            if (response.status >= 200 && response.status < 300) {
              size = imageSize(
                Uint8Array.from(atob(response.body ?? ""), (char) => char.charCodeAt(0)),
              );
            }
          } catch {
            /* A malformed response is a failed image load. */
          }
          active.imageComplete(request.imageUri, ...(size ?? [0, 0]));
        } else active.httpComplete(request.coreId, message.response);
        schedule(active, generation, 0);
      }
      break;
    }
    case "debuggerCommand":
      if (active) {
        const resumed = active.debuggerCommand(message.command);
        emitOutput(active, true);
        const registry = active.takeRegistry();
        if (registry !== null) post({ type: "registry", channelId, json: registry });
        if (resumed) {
          reportStatus("running");
          schedule(active, generation, 0);
        }
      }
      break;
    case "textInput":
      if (active?.textInput(message.text, message.command)) schedule(active, generation, 0);
      break;
    case "key":
      sendKey(message.key, message.pressed);
      break;
  }
};
