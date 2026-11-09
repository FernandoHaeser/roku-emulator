/**
 * Messages between the local device server (packages/server: dev installer,
 * ECP, telnet console) and the web shell that runs the channel. The server
 * pushes `BridgeEvent`s over Server-Sent Events; the shell posts console
 * output and its state back.
 */
import type { RemoteKey } from "./keys";

/** Same-origin routes, mounted on the web shell's dev server. */
export const BRIDGE_PATH = "/__roku-emu";

/** Custom header required on bridge POSTs (cross-origin pages cannot send it without CORS). */
export const BRIDGE_HEADER = "x-roku-emu-bridge";

export type BridgeEvent =
  /** A package was sideloaded through the dev installer; fetch it and launch it. */
  | { type: "install"; id: string; fileName: string; debug?: boolean }
  /** The dev channel was deleted through the dev installer. */
  | { type: "delete" }
  /** ECP `launch/dev`: relaunch the installed dev channel. */
  | { type: "launch"; args?: Record<string, string> }
  /** ECP key event. */
  | { type: "key"; key: RemoteKey; pressed: boolean }
  | { type: "debuggerCommand"; command: string }
  | { type: "screenshot"; id: string }
  | { type: "runtime"; message: unknown }
  | { type: "input"; args: Record<string, string> };

/** What the shell reports so ECP queries (active-app) answer like a device. */
export interface ShellState {
  /** Title of the running dev channel, or null on the home screen. */
  runningTitle: string | null;
  paused?: boolean;
}

export interface ConsolePost {
  /** Console text exactly as printed (including unterminated debugger prompts). */
  text: string;
}
