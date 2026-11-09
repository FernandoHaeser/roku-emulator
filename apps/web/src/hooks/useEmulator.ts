import type { ConsoleStream, ShellToWorkerMessage, WorkerToShellMessage } from "@roku-emu/protocol";
import { useCallback, useEffect, useRef, useState } from "react";
import { mapKeyboardCode } from "@/keyboard";

export interface ConsoleLine {
  id: number;
  stream: ConsoleStream | "error";
  text: string;
}

export type EmulatorStatus = "starting" | "ready" | "failed";

/** Keeps the console bounded so long sessions do not grow memory forever. */
const MAX_CONSOLE_LINES = 5_000;

/**
 * Owns the runtime Web Worker: transfers the canvas, forwards remote keys and
 * collects console output. The canvas is created here (not in JSX) because a
 * canvas can be transferred to a worker only once, and React StrictMode mounts
 * effects twice in development.
 */
export function useEmulator(width: number, height: number) {
  const [screenHost, setScreenHost] = useState<HTMLDivElement | null>(null);
  const [status, setStatus] = useState<EmulatorStatus>("starting");
  const [renderer, setRenderer] = useState("");
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const nextLineId = useRef(0);

  const append = useCallback((stream: ConsoleLine["stream"], text: string) => {
    setLines((previous) => {
      const next = [...previous, { id: nextLineId.current++, stream, text }];
      return next.length > MAX_CONSOLE_LINES ? next.slice(-MAX_CONSOLE_LINES) : next;
    });
  }, []);

  useEffect(() => {
    if (!screenHost) return;
    const canvas = document.createElement("canvas");
    canvas.style.width = "100%";
    canvas.style.height = "100%";
    screenHost.appendChild(canvas);

    const worker = new Worker(new URL("../worker/runtime.worker.ts", import.meta.url), {
      type: "module",
    });
    const send = (message: ShellToWorkerMessage, transfer: Transferable[] = []) =>
      worker.postMessage(message, transfer);

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

    const offscreen = canvas.transferControlToOffscreen();
    send({ type: "init", canvas: offscreen, width, height }, [offscreen]);

    const onKey = (pressed: boolean) => (event: KeyboardEvent) => {
      // Ignore auto-repeat: the runtime generates repeats itself with device timing.
      if (event.repeat) return;
      const key = mapKeyboardCode(event.code);
      if (!key) return;
      event.preventDefault();
      send({ type: "key", key, pressed });
    };
    const onKeyDown = onKey(true);
    const onKeyUp = onKey(false);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);

    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      worker.terminate();
      canvas.remove();
    };
  }, [screenHost, width, height, append]);

  const clearConsole = useCallback(() => setLines([]), []);

  return { setScreenHost, status, renderer, lines, clearConsole };
}
