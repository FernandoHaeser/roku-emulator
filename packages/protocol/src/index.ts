/**
 * Typed message contract between the web shell (main thread) and the emulator
 * runtime (Web Worker). Every message is a discriminated union on `type`, so both
 * sides get exhaustive checking when handling messages.
 */
import type { RemoteKey } from "./keys";

export * from "./keys";

export type ShellToWorkerMessage =
  | {
      type: "init";
      canvas: OffscreenCanvas;
      /** Logical UI resolution from the channel manifest (e.g. 1920x1080 for FHD). */
      width: number;
      height: number;
    }
  | { type: "key"; key: RemoteKey; pressed: boolean };

export type ConsoleStream = "stdout" | "system";

export type WorkerToShellMessage =
  | { type: "ready"; renderer: string }
  | { type: "console"; stream: ConsoleStream; text: string }
  | { type: "fatal"; message: string };
