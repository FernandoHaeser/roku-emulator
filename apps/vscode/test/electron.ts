import * as assert from "node:assert/strict";
import * as vscode from "vscode";

interface RuntimeStatus {
  running: boolean;
  url?: string;
  ready: boolean;
  isolated?: boolean;
  serverRuntime?: boolean;
  ports: { ecpPort: number; consolePort: number; debugPort: number };
}
async function waitFor<T>(
  read: () => PromiseLike<T>,
  accept: (v: T) => boolean,
  timeout = 30_000,
): Promise<T> {
  const deadline = Date.now() + timeout;
  let last: T;
  do {
    last = await read();
    if (accept(last)) return last;
    await new Promise((resolve) => setTimeout(resolve, 100));
  } while (Date.now() < deadline);
  throw new Error(`Condition timed out: ${JSON.stringify(last)}`);
}
export async function run() {
  const extension = vscode.extensions.getExtension("FernandoHaeser.roku-emulator");
  assert.ok(extension);
  await extension.activate();
  await vscode.commands.executeCommand("rokuEmulator.configure");
  const folder = vscode.workspace.workspaceFolders![0]!;
  const launch = vscode.Uri.joinPath(folder.uri, ".vscode/launch.json");
  const text = Buffer.from(await vscode.workspace.fs.readFile(launch)).toString();
  assert.ok(text.includes("Physical Roku"));
  assert.ok(text.includes("// preserve"));
  await vscode.commands.executeCommand("rokuEmulator.open");
  const status = await waitFor(
    () => vscode.commands.executeCommand<RuntimeStatus>("rokuEmulator._status"),
    (value) => value.ready,
  );
  console.log("Runtime status", JSON.stringify(status));
  assert.equal(status.running, true);
  assert.equal(
    status.serverRuntime,
    true,
    "Embedded runtime must use the Node Worker host without browser flags",
  );
  assert.ok(
    (
      await (await fetch(`http://127.0.0.1:${status.ports.ecpPort}/query/device-info`)).text()
    ).includes("developer-enabled"),
  );
  const brightscript = vscode.extensions.getExtension("RokuCommunity.brightscript");
  assert.ok(brightscript, "Real BrightScript extension is installed in isolated test profile");
  await brightscript.activate();
  let stopped = false;
  let started = false;
  const tracker = vscode.debug.registerDebugAdapterTrackerFactory("roku-emulator", {
    createDebugAdapterTracker: () => ({
      onDidSendMessage(message: { event?: string; type?: string }) {
        if (message.type === "event" && message.event === "stopped") stopped = true;
      },
    }),
  });
  const listener = vscode.debug.onDidStartDebugSession(() => {
    started = true;
  });
  try {
    const ports = status.ports;
    const launched = await vscode.debug.startDebugging(folder, {
      name: "Integration",
      type: "roku-emulator",
      request: "launch",
      host: "127.0.0.1",
      password: "rokudev",
      rootDir: folder.uri.fsPath,
      packagePort: ports.ecpPort,
      remotePort: ports.ecpPort,
      brightScriptConsolePort: ports.consolePort,
      controlPort: ports.debugPort,
      enableDebugProtocol: true,
      stopOnEntry: true,
      enableDebuggerAutoRecovery: false,
    });
    assert.equal(launched, true);
    await waitFor(
      async () => started && stopped,
      (value) => value,
      45_000,
    );
    const session = vscode.debug.activeDebugSession;
    assert.ok(session);
    const threads = (await session.customRequest("threads")) as { threads: { id: number }[] };
    assert.ok(threads.threads.length);
    const stack = (await session.customRequest("stackTrace", {
      threadId: threads.threads[0]!.id,
    })) as { stackFrames: { id: number }[] };
    assert.ok(stack.stackFrames.length);
    const threadId = threads.threads[0]!.id;
    const breakpoint = (await session.customRequest("setBreakpoints", {
      source: { path: vscode.Uri.joinPath(folder.uri, "source/main.brs").fsPath },
      breakpoints: [{ line: 3 }],
    })) as { breakpoints: { verified: boolean }[] };
    assert.equal(breakpoint.breakpoints[0]?.verified, true);
    stopped = false;
    await session.customRequest("continue", { threadId });
    await waitFor(
      async () => stopped,
      (value) => value,
    );
    const pausedStack = (await session.customRequest("stackTrace", { threadId })) as {
      stackFrames: { id: number; line: number }[];
    };
    const frameId = pausedStack.stackFrames[0]!.id;
    assert.equal(pausedStack.stackFrames[0]!.line, 3);
    const watch = (await session.customRequest("evaluate", {
      frameId,
      expression: "answer + 8",
      context: "watch",
    })) as { result: string };
    assert.equal(watch.result, "50");
    const scopes = (await session.customRequest("scopes", { frameId })) as {
      scopes: { variablesReference: number }[];
    };
    const variablesReference = scopes.scopes[0]!.variablesReference;
    await session.customRequest("setVariable", { variablesReference, name: "answer", value: "99" });
    const changed = (await session.customRequest("evaluate", {
      frameId,
      expression: "answer",
    })) as { result: string };
    assert.equal(changed.result, "99");
    const container = (await session.customRequest("evaluate", {
      frameId,
      expression: "[answer, {nested: [7]}]",
      context: "watch",
    })) as { variablesReference: number };
    const children = (await session.customRequest("variables", {
      variablesReference: container.variablesReference,
    })) as { variables: { value: string; variablesReference: number }[] };
    assert.equal(children.variables[0]!.value, "99");
    const nested = (await session.customRequest("variables", {
      variablesReference: children.variables[1]!.variablesReference,
    })) as { variables: { variablesReference: number }[] };
    assert.ok(nested.variables[0]!.variablesReference > 0);
    stopped = false;
    await session.customRequest("next", { threadId });
    await waitFor(
      async () => stopped,
      (value) => value,
    );
    await session.customRequest("continue", { threadId });
    await vscode.debug.stopDebugging(session);
  } finally {
    tracker.dispose();
    listener.dispose();
    await vscode.commands.executeCommand("rokuEmulator.stop");
  }
  assert.equal(
    (await vscode.commands.executeCommand<RuntimeStatus>("rokuEmulator._status")).running,
    false,
  );
  console.log(
    "VS Code extension: embedded isolated runtime, project config and real BrightScript debug session passed.",
  );
}
