/** Roku socket debugger, protocol 3.0 profile. Wire codecs come from the real client. */
import { createServer, type Socket } from "node:net";
import { ErrorCode, StopReason, StepType } from "roku-debug/dist/debugProtocol/Constants.js";
import { HandshakeV3Response } from "roku-debug/dist/debugProtocol/events/responses/HandshakeV3Response.js";
import { IOPortOpenedUpdate } from "roku-debug/dist/debugProtocol/events/updates/IOPortOpenedUpdate.js";
import { GenericV3Response } from "roku-debug/dist/debugProtocol/events/responses/GenericV3Response.js";
import { ThreadsResponse } from "roku-debug/dist/debugProtocol/events/responses/ThreadsResponse.js";
import { StackTraceV3Response } from "roku-debug/dist/debugProtocol/events/responses/StackTraceV3Response.js";
import {
  VariablesResponse,
  VariableType,
  type Variable,
} from "roku-debug/dist/debugProtocol/events/responses/VariablesResponse.js";
import { ExecuteV3Response } from "roku-debug/dist/debugProtocol/events/responses/ExecuteV3Response.js";
import { ListBreakpointsResponse } from "roku-debug/dist/debugProtocol/events/responses/ListBreakpointsResponse.js";
import { AllThreadsStoppedUpdate } from "roku-debug/dist/debugProtocol/events/updates/AllThreadsStoppedUpdate.js";
import { StackTraceRequest } from "roku-debug/dist/debugProtocol/events/requests/StackTraceRequest.js";
import { VariablesRequest } from "roku-debug/dist/debugProtocol/events/requests/VariablesRequest.js";
import { StepRequest } from "roku-debug/dist/debugProtocol/events/requests/StepRequest.js";
import { ExecuteRequest } from "roku-debug/dist/debugProtocol/events/requests/ExecuteRequest.js";
import { AddBreakpointsRequest } from "roku-debug/dist/debugProtocol/events/requests/AddBreakpointsRequest.js";
import { RemoveBreakpointsRequest } from "roku-debug/dist/debugProtocol/events/requests/RemoveBreakpointsRequest.js";
import type { DeviceBridge } from "./bridge";

interface Frame {
  file: string;
  function: string;
  line: number;
  source?: string;
}
interface Thread {
  id: number;
  primary: boolean;
  active: boolean;
  location: Frame;
}
interface DebugVariable {
  name: string;
  type: string;
  value: unknown;
  container: boolean;
  count: number;
  children: DebugVariable[];
}
const TYPES: Record<string, VariableType> = {
  roAssociativeArray: VariableType.AssociativeArray,
  roArray: VariableType.Array,
  roList: VariableType.List,
  roSGNode: VariableType.Object,
  roByteArray: VariableType.Array,
  roXMLList: VariableType.Array,
};
function variable(value: DebugVariable): Variable {
  const type =
    TYPES[value.type] ??
    (Object.values(VariableType).includes(value.type as VariableType)
      ? (value.type as VariableType)
      : VariableType.Object);
  const result: Variable = {
    name: value.name,
    refCount: 0,
    isConst: false,
    type,
    value: value.value as Variable["value"],
    isContainer: value.container,
    childCount: value.count,
    keyType:
      type === VariableType.Array || type === VariableType.List
        ? VariableType.Integer
        : VariableType.String,
  };
  if (value.children.length) result.children = value.children.map(variable);
  return result;
}

export class DebugProtocolServer {
  private readonly sockets = new Set<Socket>();
  private readonly server = createServer((socket) => this.connect(socket));
  public ioPort = 8085;
  private nextBreakpoint = 1;
  private readonly breakpoints = new Map<
    number,
    { id: number; file: string; line: number; ignore: number }
  >();
  constructor(private readonly bridge: DeviceBridge) {}

  private connect(socket: Socket): void {
    this.sockets.add(socket);
    socket.on("error", () => socket.destroy());
    let buffer = Buffer.alloc(0);
    let handshake = false;
    let queue = Promise.resolve();
    let paused = false;
    let running = false;
    const disconnect = this.bridge.connectState((state) => {
      if (handshake && running && state.runningTitle === null) {
        socket.end();
      }
      running = state.runningTitle !== null;
      if (handshake && state.paused && !paused) {
        queue = queue
          .then(async () => {
            const threads = await this.bridge.rpc<Thread[]>({ op: "threads" });
            const index = threads.find((t) => t.primary)?.id ?? 0;
            if (!socket.destroyed)
              socket.write(
                AllThreadsStoppedUpdate.fromJson({
                  threadIndex: index,
                  stopReason: StopReason.Break,
                  stopReasonDetail: "Emulator paused",
                }).toBuffer(),
              );
          })
          .catch(() => undefined);
      }
      paused = state.paused === true;
    });
    socket.on("close", () => {
      disconnect();
      this.sockets.delete(socket);
    });
    socket.on("data", (chunk: Buffer) => {
      buffer = Buffer.concat([buffer, chunk]);
      if (!handshake) {
        if (buffer.length < 8) return;
        if (buffer.subarray(0, 8).toString() !== "bsdebug\0") {
          socket.destroy();
          return;
        }
        buffer = buffer.subarray(8);
        socket.write(
          HandshakeV3Response.fromJson({
            magic: "bsdebug",
            protocolVersion: "3.0.0",
            revisionTimestamp: new Date("2026-11-09T00:00:00Z"),
          }).toBuffer(),
        );
        handshake = true;
        socket.write(IOPortOpenedUpdate.fromJson({ port: this.ioPort }).toBuffer());
      }
      while (buffer.length >= 4) {
        const size = buffer.readUInt32LE(0);
        if (size < 12 || size > 1024 * 1024) {
          socket.destroy();
          return;
        }
        if (buffer.length < size) return;
        const packet = Buffer.from(buffer.subarray(0, size));
        buffer = buffer.subarray(size);
        queue = queue.then(async () => {
          const id = packet.readUInt32LE(4);
          try {
            const response = await this.handle(packet);
            if (!socket.destroyed) socket.write(response);
          } catch {
            if (!socket.destroyed)
              socket.write(
                GenericV3Response.fromJson({
                  requestId: id,
                  errorCode: ErrorCode.INVALID_ARGS,
                }).toBuffer(),
              );
          }
        });
      }
    });
  }

  private async handle(packet: Buffer): Promise<Buffer> {
    const requestId = packet.readUInt32LE(4);
    const command = packet.readUInt32LE(8);
    const ok = () => GenericV3Response.fromJson({ requestId, errorCode: ErrorCode.OK }).toBuffer();
    switch (command) {
      case 1:
        this.bridge.debuggerCommand("break");
        return ok();
      case 2:
        this.bridge.debuggerCommand("c");
        return ok();
      case 3: {
        const threads = await this.bridge.rpc<Thread[]>({ op: "threads" });
        return ThreadsResponse.fromJson({
          requestId,
          threads: threads.map((t) => ({
            isPrimary: t.primary,
            isDetached: !t.active,
            stopReason: StopReason.Break,
            stopReasonDetail: "",
            lineNumber: t.location.line,
            functionName: t.location.function,
            filePath: t.location.file,
            codeSnippet: t.location.source ?? "",
          })),
        }).toBuffer();
      }
      case 4: {
        const request = StackTraceRequest.fromBuffer(packet);
        if (!request.success) throw new Error("Invalid request");
        const stack = await this.bridge.rpc<Frame[]>({
          op: "stack",
          thread: request.data.threadIndex,
        });
        return StackTraceV3Response.fromJson({
          requestId,
          entries: stack.map((f) => ({
            lineNumber: f.line,
            functionName: f.function,
            filePath: f.file,
          })),
        }).toBuffer();
      }
      case 5: {
        const request = VariablesRequest.fromBuffer(packet);
        if (!request.success) throw new Error("Invalid request");
        const stack = await this.bridge.rpc<Frame[]>({
          op: "stack",
          thread: request.data.threadIndex,
        });
        const variables = await this.bridge.rpc<DebugVariable[]>({
          op: "variables",
          thread: request.data.threadIndex,
          frame: stack.length - 1 - request.data.stackFrameIndex,
          path: request.data.variablePathEntries.map((v) => v.name),
          children: request.data.getChildKeys,
        });
        return VariablesResponse.fromJson({
          requestId,
          variables: variables.map(variable),
        }).toBuffer();
      }
      case 6: {
        const request = StepRequest.fromBuffer(packet);
        if (!request.success) throw new Error("Invalid request");
        this.bridge.debuggerCommand(`thread ${request.data.threadIndex}`);
        this.bridge.debuggerCommand(
          request.data.stepType === StepType.Line
            ? "step"
            : request.data.stepType === StepType.Over
              ? "over"
              : "out",
        );
        return ok();
      }
      case 7: {
        const request = AddBreakpointsRequest.fromBuffer(packet);
        if (!request.success) throw new Error("Invalid request");
        const result = [];
        for (const entry of request.data.breakpoints) {
          const bp = {
            id: this.nextBreakpoint++,
            file: entry.filePath,
            line: entry.lineNumber,
            ignore: entry.ignoreCount,
          };
          try {
            await this.bridge.rpc({ op: "breakpoints", add: [bp] });
            this.breakpoints.set(bp.id, bp);
            result.push({ id: bp.id, errorCode: 0, ignoreCount: bp.ignore });
          } catch {
            result.push({ id: 0, errorCode: ErrorCode.INVALID_ARGS, ignoreCount: 0 });
          }
        }
        return ListBreakpointsResponse.fromJson({ requestId, breakpoints: result }).toBuffer();
      }
      case 8:
        return ListBreakpointsResponse.fromJson({
          requestId,
          breakpoints: [...this.breakpoints.values()].map((b) => ({
            id: b.id,
            errorCode: 0,
            ignoreCount: b.ignore,
          })),
        }).toBuffer();
      case 9: {
        const request = RemoveBreakpointsRequest.fromBuffer(packet);
        if (!request.success) throw new Error("Invalid request");
        const ids = request.data.breakpointIds;
        await this.bridge.rpc({ op: "breakpoints", remove: ids });
        const result = ids.map((id) => ({
          id,
          errorCode: this.breakpoints.delete(id) ? 0 : ErrorCode.INVALID_ARGS,
          ignoreCount: 0,
        }));
        return ListBreakpointsResponse.fromJson({ requestId, breakpoints: result }).toBuffer();
      }
      case 10: {
        const request = ExecuteRequest.fromBuffer(packet);
        if (!request.success) throw new Error("Invalid request");
        const stack = await this.bridge.rpc<Frame[]>({
          op: "stack",
          thread: request.data.threadIndex,
        });
        try {
          await this.bridge.rpc({
            op: "evaluate",
            thread: request.data.threadIndex,
            frame: stack.length - 1 - request.data.stackFrameIndex,
            code: request.data.sourceCode,
          });
          return ExecuteV3Response.fromJson({
            requestId,
            executeSuccess: true,
            runtimeStopCode: 0,
            compileErrors: [],
            runtimeErrors: [],
            otherErrors: [],
          }).toBuffer();
        } catch (error) {
          return ExecuteV3Response.fromJson({
            requestId,
            executeSuccess: false,
            runtimeStopCode: 0,
            compileErrors: [],
            runtimeErrors: [String(error)],
            otherErrors: [],
          }).toBuffer();
        }
      }
      case 122:
        this.bridge.key("Home", true);
        return ok();
      default:
        return GenericV3Response.fromJson({
          requestId,
          errorCode: ErrorCode.UNDEFINED_COMMAND,
        }).toBuffer();
    }
  }

  listen(port: number, host: string): Promise<number> {
    return new Promise((resolve, reject) => {
      this.server.once("error", reject);
      this.server.listen(port, host, () => {
        this.server.off("error", reject);
        const address = this.server.address();
        resolve(typeof address === "object" && address ? address.port : port);
      });
    });
  }
  async close(): Promise<void> {
    for (const socket of this.sockets) socket.destroy();
    await new Promise<void>((resolve) => this.server.close(() => resolve()));
  }
}
