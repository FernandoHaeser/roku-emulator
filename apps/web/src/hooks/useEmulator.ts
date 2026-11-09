/**
 * React hook owning the runtime worker: home screen vs. running channel,
 * sideloading, remote key routing, registry persistence and console lines.
 */
import type {
  ChannelInfo,
  ChannelStatus,
  ConsoleStream,
  RemoteKey,
  ShellToWorkerMessage,
  WorkerToShellMessage,
} from "@roku-emu/protocol";
import { useCallback, useEffect, useRef, useState } from "react";
import { mapKeyboardCode } from "@/keyboard";
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

/** Statuses after which the device leaves the channel and shows the home screen. */
const EXIT_STATUSES: ReadonlySet<ChannelStatus> = new Set([
  "finished",
  "crashed",
  "stopped",
  "error",
  "idle",
]);

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
  const workerRef = useRef<Worker | null>(null);
  const viewRef = useRef<ScreenView>("home");
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

  const send = useCallback((message: ShellToWorkerMessage, transfer: Transferable[] = []) => {
    workerRef.current?.postMessage(message, transfer);
  }, []);

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
    screenHost.appendChild(canvas);

    const worker = new Worker(new URL("../worker/runtime.worker.ts", import.meta.url), {
      type: "module",
    });
    const inspections = pendingInspections.current;

    worker.onmessage = (event: MessageEvent<WorkerToShellMessage>) => {
      const message = event.data;
      switch (message.type) {
        case "ready":
          setStatus("ready");
          setRenderer(message.renderer);
          break;
        case "console":
          append(message.stream, message.text);
          break;
        case "channelStatus":
          setChannel({ name: message.name, status: message.status });
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
    const offscreen = canvas.transferControlToOffscreen();
    worker.postMessage(
      { type: "init", canvas: offscreen, width, height } satisfies ShellToWorkerMessage,
      [offscreen],
    );

    let homeSequence = 0;
    /** Keys whose press reached the channel; only their releases are forwarded. */
    const forwardedKeys = new Set<RemoteKey>();
    const onKey = (pressed: boolean) => (event: KeyboardEvent) => {
      // Ignore auto-repeat: the runtime generates repeats itself with device timing.
      if (event.repeat) return;
      // Typing in a form control (console filter, file picker) is not remote input.
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
        return;
      }
      const key = mapKeyboardCode(event.code);
      if (!key) return;
      event.preventDefault();
      if (viewRef.current === "home") {
        forwardedKeys.clear();
        if (pressed) setHomeKey({ key, sequence: homeSequence++ });
        return;
      }
      if (key === "Home") {
        // Home is handled by the OS, never delivered to the channel.
        if (pressed) {
          worker.postMessage({ type: "stopChannel" } satisfies ShellToWorkerMessage);
          changeView("home");
        }
        return;
      }
      if (pressed) forwardedKeys.add(key);
      else if (!forwardedKeys.delete(key)) return;
      worker.postMessage({ type: "key", key, pressed } satisfies ShellToWorkerMessage);
    };
    const onKeyDown = onKey(true);
    const onKeyUp = onKey(false);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);

    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      worker.terminate();
      workerRef.current = null;
      for (const pending of inspections.values()) pending.reject(new Error("worker stopped"));
      inspections.clear();
      canvas.remove();
    };
  }, [screenHost, width, height, append, changeView]);

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
    (stored: StoredChannel) => {
      // Transfer a copy: the stored package must stay usable for relaunches.
      const zip = stored.zip.slice(0);
      send(
        {
          type: "launchChannel",
          channelId: DEV_CHANNEL_ID,
          name: stored.fileName,
          zip,
          registry: loadRegistry(DEV_CHANNEL_ID),
        },
        [zip],
      );
      changeView("channel");
    },
    [send, changeView],
  );

  /** Sideloads a package like the device's dev installer: replaces the dev channel and launches it. */
  const sideload = useCallback(
    async (file: File) => {
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
      launch(stored);
    },
    [inspect, launch, append],
  );

  const launchDevChannel = useCallback(() => {
    if (devChannel) launch(devChannel);
  }, [devChannel, launch]);

  const removeDevChannel = useCallback(() => {
    setDevChannel(null);
    void deleteDevChannel();
  }, []);

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
    sideload,
    launchDevChannel,
    removeDevChannel,
  };
}
