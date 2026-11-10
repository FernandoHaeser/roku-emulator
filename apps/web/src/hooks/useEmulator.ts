import { BRIDGE_HEADER, BRIDGE_PATH, SHELL_RPC_PREFIX, type BridgeEvent } from "@roku-emu/protocol";
/**
 * React hook owning the runtime worker: home screen vs. running channel,
 * sideloading, remote key routing, registry persistence and console lines.
 */
import type {
  ChannelInfo,
  ChannelStatus,
  ConsoleStream,
  RemoteKey,
  SceneInspection,
  ShellToWorkerMessage,
  WorkerToShellMessage,
} from "@roku-emu/protocol";
import { useCallback, useEffect, useRef, useState } from "react";
import { ServerWorker } from "@/worker/serverWorker";
import { CanvasRenderer } from "@/worker/canvasRenderer";
import { DEBUG_MAILBOX_BYTES, writeDebugCommand } from "@/worker/debugMailbox";
import { mapKeyboardCode } from "@/keyboard";
import { VideoHost } from "@/video/videoHost";
import {
  DEV_CHANNEL_ID,
  deleteDevChannel,
  loadDevChannel,
  loadRegistry,
  saveDevChannel,
  saveRegistry,
  type StoredChannel,
} from "@/storage/devChannelStore";

export interface ConsoleLine {
  id: number;
  stream: ConsoleStream | "error";
  text: string;
}

export type EmulatorStatus = "starting" | "ready" | "failed";

/** What the TV shows: the home screen or the running channel. */
export type ScreenView = "home" | "channel";

/** A remote key delivered to the home screen; `sequence` makes repeats distinct. */
export interface HomeKeyEvent {
  key: RemoteKey;
  sequence: number;
}

/** Keeps the console bounded so long sessions do not grow memory forever. */
const MAX_CONSOLE_LINES = 5_000;
const RPC_TIMEOUT_MS = 3_000;

const serverRuntimeEnabled =
  (globalThis as { __ROKU_RUNTIME_HOST__?: string }).__ROKU_RUNTIME_HOST__ === "server" ||
  new URLSearchParams(location.search).get("host") === "server";
const deviceBridgeEnabled =
  serverRuntimeEnabled ||
  import.meta.env.DEV ||
  new URLSearchParams(location.search).get("device") === "1";
let bridgePosts = Promise.resolve();

function postBridge(route: string, body: unknown) {
  if (!deviceBridgeEnabled) return;
  bridgePosts = bridgePosts
    .then(async () => {
      await fetch(`${BRIDGE_PATH}/${route}`, {
        method: "POST",
        headers: { "Content-Type": "application/json", [BRIDGE_HEADER]: "1" },
        body: JSON.stringify(body),
      });
    })
    .catch(() => undefined);
}

/** Statuses after which the device leaves the channel and shows the home screen. */
const EXIT_STATUSES: ReadonlySet<ChannelStatus> = new Set(["finished", "crashed", "error", "idle"]);

/**
 * Owns the runtime Web Worker: transfers the canvas, routes remote keys (home
 * screen or channel) and collects console output. The canvas is created here
 * (not in JSX) because a canvas can be transferred to a worker only once, and
 * React StrictMode mounts effects twice in development.
 */
export function useEmulator(width: number, height: number) {
  const [screenHost, setScreenHost] = useState<HTMLDivElement | null>(null);
  const [status, setStatus] = useState<EmulatorStatus>("starting");
  const [renderer, setRenderer] = useState("");
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const [channel, setChannel] = useState<{ name: string; status: ChannelStatus }>({
    name: "",
    status: "idle",
  });
  const [view, setView] = useState<ScreenView>("home");
  const [devChannel, setDevChannel] = useState<StoredChannel | null>(null);
  const [homeKey, setHomeKey] = useState<HomeKeyEvent | null>(null);
  const nextLineId = useRef(0);
  const nextRequestId = useRef(0);
  const workerRef = useRef<Worker | ServerWorker | null>(null);
  const debugMailboxRef = useRef<{
    buffer: SharedArrayBuffer;
    paused: boolean;
    waiting: boolean;
    queue: string[];
  } | null>(null);
  const flushDebugCommand = useCallback(() => {
    const mailbox = debugMailboxRef.current;
    if (mailbox?.waiting && mailbox.queue.length) {
      mailbox.waiting = false;
      writeDebugCommand(mailbox.buffer, mailbox.queue.shift()!);
    }
  }, []);
  const viewRef = useRef<ScreenView>("home");
  /** Routes a remote key like the device (keyboard and the on-screen remote). */
  const routeKeyRef = useRef<(key: RemoteKey, pressed: boolean) => void>(() => undefined);
  const nextRpcId = useRef(0);
  const pendingRpc = useRef(
    new Map<string, { resolve: (result: unknown) => void; reject: (error: Error) => void }>(),
  );

  /** Engine inspection response `{"id":..,"result":..}` (worker `inspection`). */
  const resolveRpc = useCallback((json: string) => {
    let response: { id?: string; result?: unknown; error?: string };
    try {
      response = JSON.parse(json) as typeof response;
    } catch {
      return;
    }
    const pending = pendingRpc.current.get(response.id ?? "");
    if (!pending) return;
    pendingRpc.current.delete(response.id!);
    if (response.error) pending.reject(new Error(response.error));
    else pending.resolve(response.result);
  }, []);
  const pendingInspections = useRef(
    new Map<number, { resolve: (info: ChannelInfo) => void; reject: (error: Error) => void }>(),
  );

  const changeView = useCallback((next: ScreenView) => {
    viewRef.current = next;
    setView(next);
  }, []);

  const append = useCallback((stream: ConsoleLine["stream"], text: string) => {
    setLines((previous) => {
      const next = [...previous, { id: nextLineId.current++, stream, text }];
      return next.length > MAX_CONSOLE_LINES ? next.slice(-MAX_CONSOLE_LINES) : next;
    });
  }, []);

  const send = useCallback(
    (message: ShellToWorkerMessage, transfer: Transferable[] = []) => {
      const mailbox = debugMailboxRef.current;
      if (
        mailbox?.paused &&
        (message.type === "debuggerCommand" || message.type === "stopChannel")
      ) {
        mailbox.queue.push(message.type === "stopChannel" ? "exit" : message.command);
        flushDebugCommand();
        if (message.type === "debuggerCommand") return;
      }
      workerRef.current?.postMessage(message, transfer);
    },
    [flushDebugCommand],
  );

  useEffect(() => {
    void loadDevChannel().then((stored) => {
      if (stored) setDevChannel(stored);
    });
  }, []);

  useEffect(() => {
    if (!screenHost) return;
    const canvas = document.createElement("canvas");
    canvas.style.width = "100%";
    canvas.style.height = "100%";
    canvas.style.display = "block";
    // Video planes sit under the channel canvas, which clears their areas.
    const videoLayer = document.createElement("div");
    videoLayer.style.position = "absolute";
    videoLayer.style.inset = "0";
    canvas.style.position = "relative";
    screenHost.append(videoLayer, canvas);

    const worker = serverRuntimeEnabled
      ? new ServerWorker()
      : new Worker(new URL("../worker/runtime.worker.ts", import.meta.url), { type: "module" });
    const display = serverRuntimeEnabled
      ? new CanvasRenderer(canvas as unknown as OffscreenCanvas, async (uri) => {
          const response = await fetch(`${BRIDGE_PATH}/file?uri=${encodeURIComponent(uri)}`);
          return response.ok ? new Uint8Array(await response.arrayBuffer()) : null;
        })
      : null;
    const videoHost = new VideoHost(videoLayer, (event) =>
      worker.postMessage({ type: "videoEvent", event } satisfies ShellToWorkerMessage),
    );
    const inspections = pendingInspections.current;
    const httpControllers = new Map<number, AbortController>();
    const mailbox =
      typeof SharedArrayBuffer === "function"
        ? new SharedArrayBuffer(DEBUG_MAILBOX_BYTES)
        : undefined;
    debugMailboxRef.current = mailbox
      ? { buffer: mailbox, paused: false, waiting: false, queue: [] }
      : null;

    worker.onmessage = (event: MessageEvent<WorkerToShellMessage>) => {
      const message = event.data;
      switch (message.type) {
        case "ready":
          setStatus("ready");
          setRenderer(message.renderer);
          if (window.parent !== window)
            window.parent.postMessage(
              {
                type: "roku-emulator-ready",
                isolated: globalThis.crossOriginIsolated,
                serverRuntime: serverRuntimeEnabled,
              },
              "*",
            );
          break;
        case "console":
          if (!message.text.startsWith("@@ROKU_EMU_RPC ")) append(message.stream, message.text);
          break;
        case "inspection":
          resolveRpc(message.json);
          break;
        case "displayList":
          if (message.json === null) display?.clear();
          else display?.show(message.json);
          break;
        case "consoleRaw":
          if (!serverRuntimeEnabled) postBridge("console", { text: message.text });
          break;
        case "debuggerWaiting":
          if (debugMailboxRef.current) {
            debugMailboxRef.current.paused = true;
            debugMailboxRef.current.waiting = true;
            flushDebugCommand();
          }
          break;
        case "channelStatus":
          if (debugMailboxRef.current)
            debugMailboxRef.current.paused = message.status === "stopped";
          setChannel({ name: message.name, status: message.status });
          if (!serverRuntimeEnabled)
            postBridge("state", {
              runningTitle: EXIT_STATUSES.has(message.status) ? null : message.name,
              paused: message.status === "stopped",
            });
          if (EXIT_STATUSES.has(message.status)) changeView("home");
          break;
        case "channelInfo":
          inspections.get(message.requestId)?.resolve(message.info);
          inspections.delete(message.requestId);
          break;
        case "channelInfoError":
          inspections.get(message.requestId)?.reject(new Error(message.message));
          inspections.delete(message.requestId);
          break;
        case "httpRequest": {
          const controller = new AbortController();
          httpControllers.set(message.id, controller);
          void fetch(`${BRIDGE_PATH}/http`, {
            method: "POST",
            headers: { "Content-Type": "application/json", [BRIDGE_HEADER]: "1" },
            body: message.request,
            signal: controller.signal,
          })
            .then(async (response) => {
              if (!response.ok) throw new Error(`HTTP proxy: ${response.status}`);
              return response.text();
            })
            .catch((error: unknown) =>
              JSON.stringify({
                status: controller.signal.aborted ? -28 : -7,
                failure: String(error),
              }),
            )
            .then((response) => {
              httpControllers.delete(message.id);
              if (message.buffer) writeDebugCommand(message.buffer, response);
              else
                worker.postMessage({
                  type: "httpComplete",
                  id: message.id,
                  response,
                } satisfies ShellToWorkerMessage);
            });
          break;
        }
        case "httpCancel":
          httpControllers.get(message.id)?.abort();
          httpControllers.delete(message.id);
          break;
        case "videoCommand":
          videoHost.command(message.command);
          break;
        case "videoLayout":
          videoHost.setLayout(message.width, message.height, message.rects);
          break;
        case "registry":
          saveRegistry(message.channelId, message.json);
          break;
        case "fatal":
          setStatus("failed");
          append("error", message.message);
          break;
      }
    };
    worker.onerror = (event) => {
      setStatus("failed");
      append("error", event.message);
    };

    workerRef.current = worker;
    if (serverRuntimeEnabled) {
      canvas.width = width;
      canvas.height = height;
    }
    const offscreen = serverRuntimeEnabled ? undefined : canvas.transferControlToOffscreen();
    worker.postMessage(
      {
        type: "init",
        canvas: offscreen,
        width,
        height,
        debugMailbox: mailbox,
      } satisfies ShellToWorkerMessage,
      offscreen ? [offscreen] : [],
    );

    let homeSequence = 0;
    /** Keys whose press reached the channel; only their releases are forwarded. */
    const forwardedKeys = new Set<RemoteKey>();
    const routeKey = (key: RemoteKey, pressed: boolean) => {
      if (viewRef.current === "home") {
        forwardedKeys.clear();
        if (pressed) setHomeKey({ key, sequence: homeSequence++ });
        return;
      }
      if (key === "Home") {
        // Home is handled by the OS, never delivered to the channel.
        if (pressed) {
          send({ type: "stopChannel" });
          changeView("home");
        }
        return;
      }
      if (pressed) forwardedKeys.add(key);
      else if (!forwardedKeys.delete(key)) return;
      worker.postMessage({ type: "key", key, pressed } satisfies ShellToWorkerMessage);
    };
    routeKeyRef.current = routeKey;
    const onKey = (pressed: boolean) => (event: KeyboardEvent) => {
      // Ignore auto-repeat: the runtime generates repeats itself with device timing.
      if (event.repeat) return;
      // Typing in a form control (console filter, file picker) is not remote input.
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
        return;
      }
      // Enter/Space on a focused button activate that button, not the remote.
      if (
        event.target instanceof HTMLButtonElement &&
        (event.code === "Enter" || event.code === "NumpadEnter" || event.code === "Space")
      ) {
        return;
      }
      const key = mapKeyboardCode(event.code);
      if (!key) return;
      event.preventDefault();
      routeKey(key, pressed);
    };
    const onKeyDown = onKey(true);
    const onKeyUp = onKey(false);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);

    return () => {
      routeKeyRef.current = () => undefined;
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      for (const controller of httpControllers.values()) controller.abort();
      worker.terminate();
      workerRef.current = null;
      videoHost.reset();
      videoLayer.remove();
      for (const pending of inspections.values()) pending.reject(new Error("worker stopped"));
      inspections.clear();
      canvas.remove();
    };
  }, [screenHost, width, height, append, changeView, send, flushDebugCommand, resolveRpc]);

  const clearConsole = useCallback(() => setLines([]), []);

  const inspect = useCallback(
    (zip: ArrayBuffer) =>
      new Promise<ChannelInfo>((resolve, reject) => {
        const requestId = nextRequestId.current++;
        pendingInspections.current.set(requestId, { resolve, reject });
        send({ type: "inspectChannel", requestId, zip }, [zip]);
      }),
    [send],
  );

  const launch = useCallback(
    (stored: StoredChannel, stopOnEntry = false, launchArgs: Record<string, string> = {}) => {
      // Transfer a copy: the stored package must stay usable for relaunches.
      const zip = stored.zip.slice(0);
      send(
        {
          type: "launchChannel",
          channelId: DEV_CHANNEL_ID,
          name: stored.fileName,
          zip,
          registry: loadRegistry(DEV_CHANNEL_ID),
          stopOnEntry,
          launchArgs,
        },
        [zip],
      );
      changeView("channel");
    },
    [send, changeView],
  );

  /** Sideloads a package like the device's dev installer: replaces the dev channel and launches it. */
  const sideload = useCallback(
    async (file: File, stopOnEntry = false) => {
      const zip = await file.arrayBuffer();
      let info: ChannelInfo;
      try {
        info = await inspect(zip.slice(0));
      } catch (error) {
        append(
          "error",
          `Pacote inválido: ${error instanceof Error ? error.message : String(error)}`,
        );
        setChannel({ name: file.name, status: "error" });
        return;
      }
      const stored: StoredChannel = {
        fileName: file.name,
        title: info.title || file.name.replace(/\.zip$/i, ""),
        version: info.version,
        zip,
        icon: info.icon,
      };
      setDevChannel(stored);
      void saveDevChannel(stored);
      launch(stored, stopOnEntry);
    },
    [inspect, launch, append],
  );

  const launchDevChannel = useCallback(
    (args: Record<string, string> = {}) => {
      if (devChannel) launch(devChannel, false, args);
    },
    [devChannel, launch],
  );

  const removeDevChannel = useCallback(() => {
    setDevChannel(null);
    void deleteDevChannel();
  }, []);

  const bridgeHandlers = useRef({ sideload, launchDevChannel, removeDevChannel });
  bridgeHandlers.current = { sideload, launchDevChannel, removeDevChannel };

  useEffect(() => {
    if (!deviceBridgeEnabled || status !== "ready") return;
    const events = new EventSource(`${BRIDGE_PATH}/events`);
    let active = true;
    let queue = Promise.resolve();
    events.onmessage = (message: MessageEvent<string>) => {
      const event = JSON.parse(message.data) as BridgeEvent;
      queue = queue
        .then(async () => {
          if (!active) return;
          if (serverRuntimeEnabled && !["key", "screenshot"].includes(event.type)) return;
          switch (event.type) {
            case "runtime":
              break;
            case "install": {
              const response = await fetch(`${BRIDGE_PATH}/package/${event.id}`);
              if (!response.ok) throw new Error("Package unavailable");
              const zip = await response.arrayBuffer();
              if (active)
                await bridgeHandlers.current.sideload(
                  new File([zip], event.fileName),
                  event.debug === true,
                );
              break;
            }
            case "delete":
              send({ type: "stopChannel" });
              changeView("home");
              bridgeHandlers.current.removeDevChannel();
              break;
            case "launch":
              bridgeHandlers.current.launchDevChannel(event.args);
              break;
            case "screenshot": {
              try {
                const canvas = screenHost?.querySelector("canvas");
                if (!canvas) throw new Error("No channel surface");
                const copy = document.createElement("canvas");
                copy.width = canvas.width;
                copy.height = canvas.height;
                copy.getContext("2d")!.drawImage(canvas, 0, 0);
                postBridge("response", {
                  id: event.id,
                  result: copy.toDataURL("image/jpeg", 0.92).split(",")[1],
                });
              } catch (error) {
                postBridge("response", { id: event.id, error: String(error) });
              }
              break;
            }
            case "debuggerCommand":
              send({ type: "debuggerCommand", command: event.command });
              break;
            case "key":
              if (event.key === "Home") {
                if (event.pressed) {
                  send({ type: "stopChannel" });
                  changeView("home");
                }
              } else if (viewRef.current === "home") {
                if (event.pressed)
                  setHomeKey((previous) => ({
                    key: event.key,
                    sequence: (previous?.sequence ?? 0) + 1,
                  }));
              } else if (!serverRuntimeEnabled)
                send({ type: "key", key: event.key, pressed: event.pressed });
              break;
          }
        })
        .catch((error: unknown) => append("error", `Device bridge: ${String(error)}`));
    };
    return () => {
      active = false;
      events.close();
      if (!serverRuntimeEnabled) postBridge("state", { runningTitle: null });
    };
  }, [status, send, changeView, append, screenHost]);

  /** On-screen remote: same routing as the keyboard. */
  const pressRemote = useCallback((key: RemoteKey, pressed: boolean) => {
    routeKeyRef.current(key, pressed);
  }, []);

  /**
   * Snapshot of the shown scene through the engine's inspection request (the
   * same one the ECP app-ui query uses). Null when no channel is running.
   */
  const inspectScene = useCallback(async (): Promise<SceneInspection | null> => {
    if (viewRef.current !== "channel" || !workerRef.current) return null;
    const id = `${SHELL_RPC_PREFIX}${nextRpcId.current++}`;
    const result = new Promise<unknown>((resolve, reject) => {
      pendingRpc.current.set(id, { resolve, reject });
      setTimeout(() => {
        if (pendingRpc.current.delete(id)) reject(new Error("scene inspection timed out"));
      }, RPC_TIMEOUT_MS);
    });
    send({ type: "debuggerCommand", command: `__emu ${JSON.stringify({ id, op: "scene" })}` });
    return (await result) as SceneInspection;
  }, [send]);

  return {
    setScreenHost,
    status,
    renderer,
    lines,
    clearConsole,
    channel,
    view,
    devChannel,
    homeKey,
    pressRemote,
    inspectScene,
    sideload,
    launchDevChannel,
    removeDevChannel,
  };
}
