/// <reference lib="webworker" />
import type { ShellToWorkerMessage, WorkerToShellMessage } from "@roku-emu/protocol";

/**
 * Emulator runtime entry point. Runs inside a dedicated Web Worker so channel
 * code never touches the DOM. For now it only owns the WebGL2 surface and
 * echoes input; the BrightScript runtime plugs in here in F1.
 */

declare const self: DedicatedWorkerGlobalScope;

const post = (message: WorkerToShellMessage): void => self.postMessage(message);

let gl: WebGL2RenderingContext | null = null;

function init(canvas: OffscreenCanvas, width: number, height: number): void {
  canvas.width = width;
  canvas.height = height;
  gl = canvas.getContext("webgl2", { alpha: false, antialias: false });
  if (!gl) {
    post({ type: "fatal", message: "WebGL2 is not available in this browser" });
    return;
  }
  gl.viewport(0, 0, width, height);
  gl.clearColor(0, 0, 0, 1);
  gl.clear(gl.COLOR_BUFFER_BIT);
  post({ type: "ready", renderer: `WebGL2 ${width}x${height}` });
  post({ type: "console", stream: "system", text: "Runtime ready. No channel loaded." });
}

self.onmessage = (event: MessageEvent<ShellToWorkerMessage>) => {
  const message = event.data;
  switch (message.type) {
    case "init":
      init(message.canvas, message.width, message.height);
      break;
    case "key":
      post({
        type: "console",
        stream: "system",
        text: `key ${message.key} ${message.pressed ? "down" : "up"}`,
      });
      break;
  }
};
