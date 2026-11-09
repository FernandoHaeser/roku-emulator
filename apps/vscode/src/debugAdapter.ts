/** VS Code DAP facade over the same RokuCommunity socket client, with configurable ports. */
import {
  DebugSession,
  InitializedEvent,
  StoppedEvent,
  TerminatedEvent,
  OutputEvent,
  Thread,
  StackFrame,
  Source,
  Scope,
  Breakpoint,
} from "@vscode/debugadapter";
import type { DebugProtocol } from "@vscode/debugprotocol";
import { DebugProtocolClient } from "roku-debug/dist/debugProtocol/client/DebugProtocolClient.js";
import { logger } from "roku-debug/dist/logging.js";
import { digestFetch } from "../../../packages/oracle/src/digest";
import { BRIDGE_HEADER, BRIDGE_PATH } from "../../../packages/protocol/src/bridge";
import { lstat, readdir, readFile } from "node:fs/promises";
import { relative, join, basename, resolve, sep } from "node:path";
import { zipSync, type Zippable } from "fflate";

interface Variable {
  name: string;
  type: string;
  value: unknown;
  container?: boolean;
  count?: number;
  children?: Variable[];
}
interface Frame {
  thread: number;
  frame: number;
}
interface Variables extends Frame {
  path: string[];
  expression?: string;
}
export interface DebugTarget {
  url: string;
  ecpPort: number;
  consolePort: number;
  debugPort: number;
  password: string;
}
interface LaunchArgs extends DebugProtocol.LaunchRequestArguments {
  rootDir?: string;
  stopOnEntry?: boolean;
  files?: string[];
}
export async function packageWorkspace(root: string): Promise<Uint8Array<ArrayBuffer>> {
  const entries: Zippable = {};
  let total = 0;
  const ignored = new Set([
    ".git",
    ".vscode",
    "node_modules",
    "out",
    "dist",
    "coverage",
    "golden",
    ".DS_Store",
  ]);
  const walk = async (directory: string) => {
    for (const name of (await readdir(directory)).sort()) {
      if (ignored.has(name) || name.startsWith(".env") || name.endsWith(".vsix")) continue;
      const file = join(directory, name);
      const stat = await lstat(file);
      if (stat.isSymbolicLink()) throw new Error("Channel packages cannot contain symlinks");
      const path = relative(root, file).split(sep).join("/");
      if (stat.isDirectory()) {
        entries[path + "/"] = new Uint8Array();
        await walk(file);
      } else {
        total += stat.size;
        if (total > 256 * 1024 * 1024) throw new Error("Channel exceeds 256 MiB");
        entries[path] = new Uint8Array(await readFile(file));
      }
    }
  };
  await walk(root);
  if (!entries.manifest) throw new Error("No manifest at channel root");
  return Uint8Array.from(zipSync(entries, { level: 6, mtime: new Date("2020-01-01T00:00:00Z") }));
}
export class EmulatorDebugSession extends DebugSession {
  private client: DebugProtocolClient | undefined;
  private target: DebugTarget | undefined;
  private root = "";
  private configured = false;
  private entry = true;
  private terminated = false;
  private frames = new Map<number, Frame>();
  private variables = new Map<number, Variables>();
  private nextHandle = 1;
  private sourceBreakpoints = new Map<string, number[]>();
  constructor(private readonly targetProvider: () => Promise<DebugTarget>) {
    super();
    if (process.env.ROKU_EXTENSION_TEST) console.log("Emulator DAP constructed");
    this.setDebuggerLinesStartAt1(true);
    this.setDebuggerColumnsStartAt1(true);
    logger.logLevel = "error";
  }
  protected override initializeRequest(response: DebugProtocol.InitializeResponse): void {
    if (process.env.ROKU_EXTENSION_TEST) console.log("Emulator DAP initialize");
    response.body = {
      supportsConfigurationDoneRequest: true,
      supportsEvaluateForHovers: true,
      supportsSetVariable: true,
      supportsTerminateRequest: true,
      supportsConditionalBreakpoints: false,
      supportsHitConditionalBreakpoints: true,
    };
    this.sendResponse(response);
  }
  private async rpc(request: Record<string, unknown>): Promise<Variable> {
    const response = await fetch(new URL(`${BRIDGE_PATH}/debug`, this.target!.url), {
      method: "POST",
      headers: { "Content-Type": "application/json", [BRIDGE_HEADER]: "1" },
      body: JSON.stringify(request),
    });
    const body = (await response.json()) as { result?: Variable; error?: string };
    if (!response.ok || body.error || !body.result)
      throw new Error(body.error || "Debugger evaluation failed");
    return body.result;
  }
  protected override async launchRequest(
    response: DebugProtocol.LaunchResponse,
    args: LaunchArgs,
  ): Promise<void> {
    try {
      if (process.env.ROKU_EXTENSION_TEST) console.log("Emulator DAP launch");
      if (!args.rootDir) throw new Error("rootDir is required");
      this.root = resolve(args.rootDir);
      this.entry = args.stopOnEntry === true;
      this.target = await this.targetProvider();
      this.client = new DebugProtocolClient({
        device: { host: "127.0.0.1" },
        controlPort: this.target.debugPort,
        shutdownTimeout: 100,
        exitChannelTimeout: 100,
      });
      this.client.on("suspend", () => {
        this.frames.clear();
        this.variables.clear();
        if (this.configured)
          this.sendEvent(new StoppedEvent("breakpoint", this.client!.primaryThread + 1));
      });
      this.client.on("io-output", (text) => this.sendEvent(new OutputEvent(text, "stdout")));
      this.client.on("app-exit", () => this.finish());
      await this.client.connect();
      const suspended = Promise.race([
        this.client.once("suspend"),
        new Promise<never>((_resolve, reject) => {
          const timer = setTimeout(
            () => reject(new Error("Channel did not pause within 30 seconds")),
            30_000,
          );
          timer.unref();
        }),
      ]);
      const zip = await packageWorkspace(this.root);
      const form = () => {
        const body = new FormData();
        body.set("mysubmit", "Install");
        body.set("remotedebug", "1");
        body.set("archive", new Blob([zip]), "channel.zip");
        return { method: "POST", body };
      };
      const installed = await digestFetch(
        `http://127.0.0.1:${this.target.ecpPort}/plugin_install`,
        form,
        { user: "rokudev", password: this.target.password },
      );
      if (!installed.ok || !(await installed.text()).includes("Install Success."))
        throw new Error("Package installation failed");
      await suspended;
      this.sendEvent(new InitializedEvent());
      this.sendResponse(response);
    } catch (error) {
      if (process.env.ROKU_EXTENSION_TEST)
        console.error(
          "Emulator DAP launch failed",
          error instanceof Error ? error.stack : String(error),
        );
      this.sendErrorResponse(response, 1, String(error));
      await this.client?.destroy(true);
      this.finish();
    }
  }
  protected override async configurationDoneRequest(
    response: DebugProtocol.ConfigurationDoneResponse,
  ): Promise<void> {
    this.configured = true;
    this.sendResponse(response);
    if (this.entry)
      this.sendEvent(new StoppedEvent("entry", (this.client?.primaryThread ?? 0) + 1));
    else await this.client?.continue();
  }
  protected override async setBreakPointsRequest(
    response: DebugProtocol.SetBreakpointsResponse,
    args: DebugProtocol.SetBreakpointsArguments,
  ): Promise<void> {
    try {
      if (!this.client || !args.source.path) throw new Error("No running debug target");
      const file = args.source.path;
      const old = this.sourceBreakpoints.get(file);
      if (old?.length) await this.client.removeBreakpoints(old);
      const path = `pkg:/${relative(this.root, file).split(sep).join("/")}`;
      const added = await this.client.addBreakpoints(
        (args.breakpoints ?? []).map((bp) => ({
          filePath: path,
          lineNumber: bp.line,
          ...(bp.hitCondition ? { hitCount: Math.max(0, Number(bp.hitCondition) - 1) } : {}),
        })),
      );
      this.sourceBreakpoints.set(
        file,
        added.data.breakpoints.map((bp) => bp.id).filter((id) => id > 0),
      );
      response.body = {
        breakpoints: added.data.breakpoints.map((bp, i) => {
          const breakpoint = new Breakpoint(bp.id > 0, args.breakpoints?.[i]?.line);
          breakpoint.setId(bp.id);
          return breakpoint;
        }),
      };
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 2, String(error));
    }
  }
  protected override async threadsRequest(response: DebugProtocol.ThreadsResponse): Promise<void> {
    try {
      const result = await this.client!.threads();
      // The protocol only lists threads while stopped; VS Code also asks while
      // running (e.g. to enable Pause), so expose the primary thread then.
      if (!result) {
        response.body = { threads: [new Thread((this.client?.primaryThread ?? 0) + 1, "Main")] };
        this.sendResponse(response);
        return;
      }
      response.body = {
        threads: result.data.threads
          .map((thread, i) => ({ thread, i }))
          .filter(({ thread }) => !thread.isDetached)
          .map(({ thread, i }) => new Thread(i + 1, thread.functionName || `Thread ${i}`)),
      };
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 3, String(error));
    }
  }
  protected override async stackTraceRequest(
    response: DebugProtocol.StackTraceResponse,
    args: DebugProtocol.StackTraceArguments,
  ): Promise<void> {
    try {
      const thread = args.threadId - 1;
      const result = await this.client!.getStackTrace(thread);
      if (!result) {
        response.body = { stackFrames: [], totalFrames: 0 };
        this.sendResponse(response);
        return;
      }
      const entries = result.data.entries;
      const stackFrames = entries.map((frame, i) => {
        const id = this.nextHandle++;
        this.frames.set(id, { thread, frame: entries.length - 1 - i });
        return new StackFrame(
          id,
          frame.functionName,
          new Source(
            basename(frame.filePath),
            join(this.root, frame.filePath.replace(/^pkg:\//, "")),
          ),
          Math.max(1, frame.lineNumber),
          1,
        );
      });
      response.body = { stackFrames, totalFrames: stackFrames.length };
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 4, String(error));
    }
  }
  protected override scopesRequest(
    response: DebugProtocol.ScopesResponse,
    args: DebugProtocol.ScopesArguments,
  ): void {
    const frame = this.frames.get(args.frameId);
    if (!frame) {
      this.sendErrorResponse(response, 5, "Invalid frame");
      return;
    }
    const id = this.nextHandle++;
    this.variables.set(id, { ...frame, path: [] });
    response.body = { scopes: [new Scope("Locais", id, false)] };
    this.sendResponse(response);
  }
  private childExpression(parent: string, key: string, type: string): string {
    const indexed = ["roArray", "roList", "roByteArray", "roXMLList"].includes(type);
    return `(${parent})[${indexed && /^\d+$/.test(key) ? key : '"' + key.replace(/"/g, '""') + '"'}]`;
  }
  private display(value: unknown): string {
    return typeof value === "string"
      ? JSON.stringify(value)
      : value === null
        ? "invalid"
        : String(value);
  }
  protected override async variablesRequest(
    response: DebugProtocol.VariablesResponse,
    args: DebugProtocol.VariablesArguments,
  ): Promise<void> {
    try {
      const reference = this.variables.get(args.variablesReference);
      if (!reference) throw new Error("Invalid variable reference");
      if (reference.expression) {
        const stack = await this.client!.getStackTrace(reference.thread);
        const value = await this.rpc({
          op: "evaluate",
          thread: reference.thread,
          frame: stack.data.entries.length - 1 - reference.frame,
          code: `return ${reference.expression}`,
        });
        response.body = {
          variables: (value.children ?? []).map((child) => {
            let id = 0;
            if (child.container) {
              id = this.nextHandle++;
              this.variables.set(id, {
                ...reference,
                expression: this.childExpression(reference.expression!, child.name, value.type),
              });
            }
            return {
              name: child.name,
              type: child.type,
              value: child.container
                ? `${child.type} (${child.count ?? 0})`
                : this.display(child.value),
              variablesReference: id,
            };
          }),
        };
        this.sendResponse(response);
        return;
      }
      const result = await this.client!.getVariables(
        reference.path,
        reference.frame,
        reference.thread,
      );
      const values = reference.path.length
        ? (result.data.variables[0]?.children ?? [])
        : result.data.variables;
      response.body = {
        variables: values.map((value) => {
          let id = 0;
          if (value.isContainer) {
            id = this.nextHandle++;
            this.variables.set(id, { ...reference, path: [...reference.path, value.name ?? ""] });
          }
          return {
            name: value.name ?? "",
            type: value.type,
            value: value.isContainer
              ? `${value.type} (${value.childCount ?? value.children?.length ?? 0})`
              : this.display(value.value),
            variablesReference: id,
          };
        }),
      };
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 6, String(error));
    }
  }
  protected override async evaluateRequest(
    response: DebugProtocol.EvaluateResponse,
    args: DebugProtocol.EvaluateArguments,
  ): Promise<void> {
    try {
      const frame = (args.frameId !== undefined ? this.frames.get(args.frameId) : undefined) ?? {
        thread: this.client!.primaryThread,
        frame: 0,
      };
      const stack = await this.client!.getStackTrace(frame.thread);
      const value = await this.rpc({
        op: "evaluate",
        thread: frame.thread,
        frame: stack.data.entries.length - 1 - frame.frame,
        code: `return ${args.expression}`,
      });
      let id = 0;
      if (value.container) {
        id = this.nextHandle++;
        this.variables.set(id, { ...frame, path: [], expression: args.expression });
      }
      response.body = {
        result: value.container ? `${value.type} (${value.count ?? 0})` : this.display(value.value),
        type: value.type,
        variablesReference: id,
      };
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 7, String(error));
    }
  }
  protected override async setVariableRequest(
    response: DebugProtocol.SetVariableResponse,
    args: DebugProtocol.SetVariableArguments,
  ): Promise<void> {
    try {
      const reference = this.variables.get(args.variablesReference);
      if (!reference) throw new Error("Invalid variable reference");
      const path = [...reference.path, args.name];
      const expression = path
        .map((key, i) =>
          i === 0 ? key : `[${/^\d+$/.test(key) ? key : '"' + key.replace(/"/g, '""') + '"'}]`,
        )
        .join("");
      const stack = await this.client!.getStackTrace(reference.thread);
      const frame = stack.data.entries.length - 1 - reference.frame;
      await this.rpc({
        op: "evaluate",
        thread: reference.thread,
        frame,
        code: `${expression} = ${args.value}`,
      });
      const value = await this.rpc({
        op: "evaluate",
        thread: reference.thread,
        frame,
        code: `return ${expression}`,
      });
      response.body = { value: this.display(value.value), type: value.type, variablesReference: 0 };
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 8, String(error));
    }
  }
  protected override async continueRequest(
    response: DebugProtocol.ContinueResponse,
  ): Promise<void> {
    try {
      await this.client!.continue();
      response.body = { allThreadsContinued: true };
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 9, String(error));
    }
  }
  protected override async nextRequest(
    response: DebugProtocol.NextResponse,
    args: DebugProtocol.NextArguments,
  ): Promise<void> {
    try {
      await this.client!.stepOver(args.threadId - 1);
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 10, String(error));
    }
  }
  protected override async stepInRequest(
    response: DebugProtocol.StepInResponse,
    args: DebugProtocol.StepInArguments,
  ): Promise<void> {
    try {
      await this.client!.stepIn(args.threadId - 1);
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 11, String(error));
    }
  }
  protected override async stepOutRequest(
    response: DebugProtocol.StepOutResponse,
    args: DebugProtocol.StepOutArguments,
  ): Promise<void> {
    try {
      await this.client!.stepOut(args.threadId - 1);
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 12, String(error));
    }
  }
  protected override async pauseRequest(response: DebugProtocol.PauseResponse): Promise<void> {
    try {
      await this.client!.pause(true);
      this.sendResponse(response);
    } catch (error) {
      this.sendErrorResponse(response, 13, String(error));
    }
  }
  protected override async disconnectRequest(
    response: DebugProtocol.DisconnectResponse,
  ): Promise<void> {
    try {
      await this.client?.exitChannel();
    } finally {
      await this.client?.destroy(true);
      this.sendResponse(response);
      this.finish();
    }
  }
  protected override async terminateRequest(
    response: DebugProtocol.TerminateResponse,
  ): Promise<void> {
    try {
      await this.client?.exitChannel();
    } finally {
      await this.client?.destroy(true);
      this.sendResponse(response);
      this.finish();
    }
  }
  private finish() {
    if (!this.terminated) {
      this.terminated = true;
      this.sendEvent(new TerminatedEvent());
    }
  }
}
