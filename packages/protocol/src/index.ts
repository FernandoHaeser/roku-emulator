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
      /** Surface size in pixels; display lists are scaled from the design resolution. */
      width: number;
      height: number;
    }
  | { type: "key"; key: RemoteKey; pressed: boolean }
  /** Reads a package's manifest (title, version, icon) without launching it. */
  | { type: "inspectChannel"; requestId: number; zip: ArrayBuffer }
  /**
   * Launches a sideload package (.zip), transferred to the worker.
   * `registry` is the channel's persisted registry JSON, if any.
   */
  | {
      type: "launchChannel";
      channelId: string;
      name: string;
      zip: ArrayBuffer;
      registry: string | null;
    }
  /** Exits the running channel (remote Home key). */
  | { type: "stopChannel" };

export type ConsoleStream = "stdout" | "system";

/** Lifecycle of the running channel, mirrored from the engine's status codes. */
export type ChannelStatus =
  "idle" | "running" | "sleeping" | "finished" | "crashed" | "stopped" | "error";

/** Manifest data shown on the home screen. */
export interface ChannelInfo {
  title: string;
  version: string;
  /** Focus icon bytes (`mm_icon_focus_*`), when the package has one. */
  icon: ArrayBuffer | null;
}

export type WorkerToShellMessage =
  | { type: "ready"; renderer: string }
  | { type: "console"; stream: ConsoleStream; text: string }
  | { type: "channelStatus"; status: ChannelStatus; name: string; channelId: string }
  | { type: "channelInfo"; requestId: number; info: ChannelInfo }
  | { type: "channelInfoError"; requestId: number; message: string }
  /** The running channel changed its registry; the shell persists it. */
  | { type: "registry"; channelId: string; json: string }
  | { type: "fatal"; message: string };
