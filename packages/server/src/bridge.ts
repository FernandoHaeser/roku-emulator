/**
 * Hub between the device-facing servers (installer, ECP, console) and the web
 * shell that actually runs the channel.
 */
import { randomBytes } from "node:crypto";
import type { BridgeEvent, ShellState } from "@roku-emu/protocol/bridge";
import type { RemoteKey } from "@roku-emu/protocol/keys";

export interface InstalledPackage {
  id: string;
  fileName: string;
  zip: Buffer;
  debug: boolean;
}

type Listener = (event: BridgeEvent) => void;

export class DeviceBridge {
  private readonly shells = new Set<Listener>();
  private readonly pendingRpc = new Map<
    string,
    {
      resolve(value: unknown): void;
      reject(error: Error): void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();
  private rpcTail = "";
  private readonly stateSinks = new Set<(state: ShellState) => void>();
  private readonly consoleSinks = new Set<(text: string) => void>();
  public runtimeReceiver: ((message: unknown, zip?: Buffer) => void) | undefined;
  private readonly runtimeReplay = new Map<string, unknown>();
  private runtimeConsole: unknown[] = [];
  private installed: InstalledPackage | null = null;
  /** An install that no shell has received yet (delivered on connect). */
  private pendingInstall = false;
  private debugInstall = false;
  private state: ShellState = { runningTitle: null };

  get shellCount(): number {
    return this.shells.size;
  }

  get package(): InstalledPackage | null {
    return this.installed;
  }

  get shellState(): ShellState {
    return this.state;
  }

  /** Registers a shell; returns the unsubscribe function. */
  connectShell(listener: Listener): () => void {
    this.shells.add(listener);
    if (this.runtimeReceiver) {
      for (const message of this.runtimeReplay.values()) listener({ type: "runtime", message });
      for (const message of this.runtimeConsole) listener({ type: "runtime", message });
    }
    if (this.pendingInstall && this.installed) {
      this.pendingInstall = false;
      listener({
        type: "install",
        id: this.installed.id,
        fileName: this.installed.fileName,
        debug: this.debugInstall,
      });
    }
    return () => this.shells.delete(listener);
  }

  /** Registers a console output sink (telnet client); returns the unsubscribe function. */
  connectConsole(sink: (text: string) => void): () => void {
    this.consoleSinks.add(sink);
    return () => this.consoleSinks.delete(sink);
  }

  private broadcast(event: BridgeEvent): boolean {
    for (const listener of this.shells) listener(event);
    return this.shells.size > 0;
  }

  runtimeMessage(message: { type: string; [key: string]: unknown }): void {
    if (["ready", "channelStatus", "displayList", "fatal"].includes(message.type))
      this.runtimeReplay.set(message.type, message);
    if (message.type === "console") {
      this.runtimeConsole.push(message);
      if (this.runtimeConsole.length > 5000) this.runtimeConsole = this.runtimeConsole.slice(-5000);
    }
    this.broadcast({ type: "runtime", message });
  }

  /** Dev installer upload: replaces the dev channel and launches it. */
  install(fileName: string, zip: Buffer, debug = false, launch = true): InstalledPackage {
    this.installed = { id: randomBytes(8).toString("hex"), fileName, zip, debug };
    this.debugInstall = debug;
    const delivered = launch
      ? this.broadcast({ type: "install", id: this.installed.id, fileName, debug })
      : true;
    if (!delivered) {
      this.pendingInstall = true;
      this.writeConsole(
        "[roku-emu] Package received. Open the emulator page (make dev, http://127.0.0.1:5173) to run it.\n",
      );
    }
    return this.installed;
  }

  deletePackage(): void {
    this.installed = null;
    this.pendingInstall = false;
    this.broadcast({ type: "delete" });
  }

  /** ECP `launch/dev`. Returns false when there is nothing to launch. */
  launchDev(args: Record<string, string> = {}): boolean {
    if (!this.installed) return false;
    this.broadcast({ type: "launch", args });
    return true;
  }

  input(args: Record<string, string>): void {
    this.broadcast({ type: "input", args });
  }

  key(key: RemoteKey, pressed: boolean): void {
    this.broadcast({ type: "key", key, pressed });
  }

  debuggerCommand(command: string): void {
    if (!this.broadcast({ type: "debuggerCommand", command })) {
      this.writeConsole("[roku-emu] No browser runtime connected.\n");
    }
  }

  async rpc<T>(request: Record<string, unknown>): Promise<T> {
    if (!this.shellCount) throw new Error("No browser runtime connected");
    const id = randomBytes(16).toString("hex");
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingRpc.delete(id);
        reject(new Error("Debugger request timed out"));
      }, 10_000);
      this.pendingRpc.set(id, { resolve: (value) => resolve(value as T), reject, timer });
      timer.unref();
      if (request.op === "screenshot") this.broadcast({ type: "screenshot", id });
      else this.debuggerCommand(`__emu ${JSON.stringify({ ...request, id })}`);
    });
  }

  resolveRpc(response: { id: string; result?: unknown; error?: string }): void {
    const pending = this.pendingRpc.get(response.id);
    if (!pending) return;
    clearTimeout(pending.timer);
    this.pendingRpc.delete(response.id);
    if (response.error) pending.reject(new Error(response.error));
    else pending.resolve(response.result);
  }

  connectState(listener: (state: ShellState) => void): () => void {
    this.stateSinks.add(listener);
    return () => this.stateSinks.delete(listener);
  }

  writeConsole(text: string): void {
    const marker = "@@ROKU_EMU_RPC ";
    text = this.rpcTail + text;
    this.rpcTail = "";
    let start = text.indexOf(marker);
    while (start >= 0) {
      if (start) for (const sink of this.consoleSinks) sink(text.slice(0, start));
      const end = text.indexOf("\n", start);
      if (end < 0) {
        this.rpcTail = text.slice(start);
        return;
      }
      const line = text.slice(start + marker.length, end);
      try {
        const response = JSON.parse(line) as { id: string; result?: unknown; error?: string };
        const pending = this.pendingRpc.get(response.id);
        if (pending) {
          clearTimeout(pending.timer);
          this.pendingRpc.delete(response.id);
          if (response.error) pending.reject(new Error(response.error));
          else pending.resolve(response.result);
        }
      } catch {
        /* Malformed runtime response is not a debugger result. */
      }
      text = text.slice(end + 1);
      start = text.indexOf(marker);
    }
    for (let size = Math.min(marker.length - 1, text.length); size > 0; size--) {
      if (marker.startsWith(text.slice(-size))) {
        this.rpcTail = text.slice(-size);
        text = text.slice(0, -size);
        break;
      }
    }
    for (const sink of this.consoleSinks) sink(text);
  }

  setShellState(state: ShellState): void {
    this.state = state;
    for (const sink of this.stateSinks) sink(state);
  }
}
