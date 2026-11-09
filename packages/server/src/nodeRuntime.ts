/** Shared WASM runtime hosted by Node so VS Code webviews need no browser flags. */
import { readFileSync } from "node:fs";
import { mkdir, writeFile, rename } from "node:fs/promises";
import { dirname } from "node:path";
import { Worker } from "node:worker_threads";
import { DEBUG_MAILBOX_BYTES, writeDebugCommand } from "../../../apps/web/src/worker/debugMailbox";
import type { ShellToWorkerMessage, WorkerToShellMessage } from "@roku-emu/protocol";
import { httpRequest } from "./httpProxy";
import type { DeviceBridge } from "./bridge";

export function startNodeRuntime(bridge: DeviceBridge, workerPath: string, registryFile?: string) {
  const worker = new Worker(workerPath);
  const mailbox = new SharedArrayBuffer(DEBUG_MAILBOX_BYTES);
  const registries = new Map<string, string>();
  let registryWrites = Promise.resolve();
  if (registryFile)
    try {
      for (const [key, value] of Object.entries(
        JSON.parse(readFileSync(registryFile, "utf8")) as Record<string, string>,
      ))
        if (typeof value === "string") registries.set(key, value);
    } catch {
      /* first launch has no registry file */
    }
  let paused = false,
    waiting = false;
  const commands: string[] = [];
  const controllers = new Map<number, AbortController>();
  const flush = () => {
    if (waiting && commands.length) {
      waiting = false;
      writeDebugCommand(mailbox, commands.shift()!);
    }
  };
  const send = (message: ShellToWorkerMessage) => {
    if (
      paused &&
      (message.type === "debuggerCommand" ||
        message.type === "stopChannel" ||
        message.type === "launchChannel" ||
        message.type === "inspectChannel")
    ) {
      commands.push(message.type === "debuggerCommand" ? message.command : "exit");
      flush();
      if (message.type === "debuggerCommand") return;
    }
    worker.postMessage(message);
  };
  const launch = (args: Record<string, string> = {}) => {
    const installed = bridge.package;
    if (!installed) return;
    send({
      type: "launchChannel",
      channelId: "dev",
      name: installed.fileName,
      zip: Uint8Array.from(installed.zip).buffer,
      registry: registries.get("dev") ?? null,
      stopOnEntry: installed.debug,
      launchArgs: args,
    });
  };
  bridge.runtimeReceiver = (input, zip) => {
    if (!input || typeof input !== "object") throw new Error("Invalid runtime message");
    const message = input as ShellToWorkerMessage;
    if (message.type === "launchChannel" || message.type === "inspectChannel") {
      if (!zip) throw new Error("Missing channel package");
      message.zip = Uint8Array.from(zip).buffer;
      if (message.type === "launchChannel")
        bridge.install(message.name, zip, message.stopOnEntry ?? false, false);
      if (message.type === "launchChannel" && !message.registry)
        message.registry = registries.get(message.channelId) ?? registries.get("dev") ?? null;
    }
    send(message);
  };
  const disconnect = bridge.connectShell((event) => {
    switch (event.type) {
      case "install":
        launch();
        break;
      case "delete":
        send({ type: "stopChannel" });
        break;
      case "launch":
        launch(event.args);
        break;
      case "debuggerCommand":
        send({ type: "debuggerCommand", command: event.command });
        break;
      case "input":
        send({ type: "input", args: event.args });
        break;
      case "key":
        if (event.key === "Home") {
          if (event.pressed) send({ type: "stopChannel" });
        } else send({ type: "key", key: event.key, pressed: event.pressed });
        break;
    }
  });
  worker.on("message", (message: WorkerToShellMessage) => {
    switch (message.type) {
      case "debuggerWaiting":
        paused = true;
        waiting = true;
        flush();
        return;
      case "consoleRaw":
        bridge.writeConsole(message.text);
        return;
      case "channelStatus":
        paused = message.status === "stopped";
        bridge.setShellState({
          runningTitle: ["finished", "crashed", "error", "idle"].includes(message.status)
            ? null
            : message.name,
          paused: message.status === "stopped",
        });
        break;
      case "registry":
        registries.set(message.channelId, message.json);
        if (registryFile)
          registryWrites = registryWrites.then(async () => {
            await mkdir(dirname(registryFile), { recursive: true });
            await writeFile(registryFile + ".tmp", JSON.stringify(Object.fromEntries(registries)), {
              mode: 0o600,
            });
            await rename(registryFile + ".tmp", registryFile);
          });
        break;
      case "httpRequest": {
        const controller = new AbortController();
        controllers.set(message.id, controller);
        void httpRequest(JSON.parse(message.request), controller.signal)
          .then((result) => {
            controllers.delete(message.id);
            const response = JSON.stringify(result);
            if (message.buffer) writeDebugCommand(message.buffer, response);
            else worker.postMessage({ type: "httpComplete", id: message.id, response });
          })
          .catch((error) => {
            const response = JSON.stringify({ status: -7, failure: String(error) });
            if (message.buffer) writeDebugCommand(message.buffer, response);
            else worker.postMessage({ type: "httpComplete", id: message.id, response });
          });
        return;
      }
      case "httpCancel":
        controllers.get(message.id)?.abort();
        controllers.delete(message.id);
        return;
    }
    bridge.runtimeMessage(message);
  });
  worker.on("error", (error) => bridge.runtimeMessage({ type: "fatal", message: error.message }));
  worker.postMessage({
    type: "init",
    width: 1920,
    height: 1080,
    headless: true,
    debugMailbox: mailbox,
  } satisfies ShellToWorkerMessage);
  return {
    close: async () => {
      disconnect();
      bridge.runtimeReceiver = undefined;
      for (const c of controllers.values()) c.abort();
      await worker.terminate();
      await registryWrites;
    },
  };
}
