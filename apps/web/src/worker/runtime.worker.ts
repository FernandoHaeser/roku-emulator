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
  WorkerToShellMessage,
} from "@roku-emu/protocol";
import engineUrl from "@/wasm/brs.wasm?url";
import { CanvasRenderer } from "./canvasRenderer";
import { BrsEngine, EngineStatus, type EngineStatusCode } from "./engine";
import { LineBuffer } from "./lineBuffer";

declare const self: DedicatedWorkerGlobalScope;

/** Instructions per engine call, and max wall time per slice before yielding. */
const INSTRUCTION_BUDGET = 50_000;
const SLICE_MS = 12;

const post = (message: WorkerToShellMessage, transfer: Transferable[] = []): void =>
  self.postMessage(message, transfer);

let renderer: CanvasRenderer | null = null;
let enginePromise: Promise<BrsEngine> | null = null;
let active: BrsEngine | null = null;
let channelName = "";
let channelId = "";
/** Incremented on every launch/stop so a stale scheduled slice stops itself. */
let generation = 0;
/** Display list revision last drawn (-1 forces a redraw). */
let drawnRevision = -1;
/** Pending wake-up (sleep or yield) of the current run. */
let wakeTimer: ReturnType<typeof setTimeout> | null = null;
const lines = new LineBuffer();

function engine(): Promise<BrsEngine> {
  enginePromise ??= BrsEngine.load(engineUrl);
  return enginePromise;
}

function initSurface(canvas: OffscreenCanvas, width: number, height: number): void {
  canvas.width = width;
  canvas.height = height;
  try {
    renderer = new CanvasRenderer(canvas, (uri) => active?.packageFile(uri) ?? null);
  } catch (error) {
    post({ type: "fatal", message: String(error) });
    return;
  }
  post({ type: "ready", renderer: `Canvas2D ${width}x${height}` });
  post({ type: "console", stream: "system", text: "Runtime pronto. Abra um canal (.zip)." });
}

function emitOutput(instance: BrsEngine, final = false): void {
  const text = instance.takeOutput();
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
  const result = instance.displayList(drawnRevision);
  if (result.kind === "list") {
    drawnRevision = instance.revision();
    renderer?.show(result.json);
  }
  const registry = instance.takeRegistry();
  if (registry !== null) post({ type: "registry", channelId, json: registry });
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

function stopChannel(): void {
  generation += 1;
  if (wakeTimer !== null) clearTimeout(wakeTimer);
  wakeTimer = null;
  if (active) {
    emitOutput(active, true);
    active.unload();
    active = null;
    // "idle": the user left the channel (Home), as opposed to a STOP statement.
    reportStatus("idle");
  }
  renderer?.clear();
  drawnRevision = -1;
}

async function launchChannel(
  id: string,
  name: string,
  zip: ArrayBuffer,
  registry: string | null,
): Promise<void> {
  stopChannel();
  const runGeneration = generation;
  channelName = name;
  channelId = id;
  lines.flush();
  try {
    const instance = await engine();
    if (runGeneration !== generation) return;
    const epochOffset = Date.now() - performance.now();
    const loaded = instance.loadChannel(new Uint8Array(zip), performance.now(), epochOffset);
    if (!loaded) {
      emitOutput(instance, true);
      reportStatus("error");
      return;
    }
    if (registry) instance.loadRegistry(registry);
    active = instance;
    reportStatus("running");
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
      initSurface(message.canvas, message.width, message.height);
      break;
    case "launchChannel":
      void launchChannel(message.channelId, message.name, message.zip, message.registry);
      break;
    case "stopChannel":
      stopChannel();
      break;
    case "inspectChannel":
      void inspectChannel(message.requestId, message.zip);
      break;
    case "key":
      sendKey(message.key, message.pressed);
      break;
  }
};
