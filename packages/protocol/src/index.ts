/**
 * Typed message contract between the web shell (main thread) and the emulator
 * runtime (Web Worker). Every message is a discriminated union on `type`, so both
 * sides get exhaustive checking when handling messages.
 */
import type { RemoteKey } from "./keys";

export * from "./bridge";
export * from "./keys";

/** Command for the shell's video player (crates/brs-core/src/sg/video.rs). */
/** `content.drmParams` of a protected stream. */
export interface VideoDrm {
  /** Roku key system name ("widevine", "playready"). */
  keySystem: string;
  licenseServerURL: string;
}

export type VideoCommand =
  | { id: number; op: "load"; url: string; format: string; drm?: VideoDrm }
  | { id: number; op: "pause" | "resume" | "stop" }
  | { id: number; op: "seek"; position: number }
  /** Channel exited: destroy every player. */
  | { op: "reset" };

/** Player event reported back to the engine. */
export type VideoEvent =
  | { id: number; type: "buffering" | "playing" | "paused" | "ended" }
  | { id: number; type: "duration" | "position"; value: number }
  /** `category: "drm"` for license failures (HTTP status of the license request). */
  | { id: number; type: "error"; status: number; message: string; category?: "drm" }
  /** No CDM for the stream's key system: the engine continues on its own clock. */
  | { id: number; type: "unsupported" };

/** Video plane of a player, in the channel's design resolution. */
export interface VideoRect {
  id: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

export type ShellToWorkerMessage =
  | {
      type: "init";
      canvas?: OffscreenCanvas | undefined;
      headless?: boolean;
      debugMailbox?: SharedArrayBuffer | undefined;
      /** Surface size in pixels; display lists are scaled from the design resolution. */
      width: number;
      height: number;
    }
  | { type: "key"; key: RemoteKey; pressed: boolean }
  | { type: "debuggerCommand"; command: string }
  | { type: "httpComplete"; id: number; response: string }
  | { type: "input"; args: Record<string, string> }
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
      stopOnEntry?: boolean;
      launchArgs?: Record<string, string> | undefined;
    }
  /** Exits the running channel (remote Home key). */
  | { type: "stopChannel" }
  | { type: "videoEvent"; event: VideoEvent };

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
  /** Exact engine output for device console clients, including partial prompts. */
  | { type: "consoleRaw"; text: string }
  | { type: "displayList"; json: string | null }
  | { type: "debuggerWaiting" }
  | { type: "httpRequest"; id: number; request: string; buffer?: SharedArrayBuffer | undefined }
  | { type: "httpCancel"; id: number }
  | { type: "channelStatus"; status: ChannelStatus; name: string; channelId: string }
  | { type: "channelInfo"; requestId: number; info: ChannelInfo }
  | { type: "channelInfoError"; requestId: number; message: string }
  /** The running channel changed its registry; the shell persists it. */
  | { type: "registry"; channelId: string; json: string }
  | { type: "videoCommand"; command: VideoCommand }
  /** Video planes of the current frame (design resolution `width` x `height`). */
  | { type: "videoLayout"; width: number; height: number; rects: VideoRect[] }
  | { type: "fatal"; message: string };
