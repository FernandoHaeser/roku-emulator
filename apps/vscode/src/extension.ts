import { HotReload } from "./hotReload";
import { digestFetch } from "../../../packages/oracle/src/digest";
import { EmulatorDebugSession, packageWorkspace } from "./debugAdapter";
import * as vscode from "vscode";
import { readFile } from "node:fs/promises";
import { randomBytes } from "node:crypto";
import {
  startDeviceServer,
  type RunningDeviceServer,
} from "../../../packages/server/src/deviceServer";
import { startNodeRuntime } from "../../../packages/server/src/nodeRuntime";
import { DEFAULT_CONFIG } from "../../../packages/server/src/config";
import { startWebHost } from "../../../packages/server/src/webHost";
import {
  launchConfiguration,
  localRuntimeUrl,
  updateLaunchConfiguration,
  type EmulatorPorts,
} from "./configuration";
import { isRemoteKey, type RemoteKey } from "../../../packages/protocol/src/keys";

type RunningWeb = Awaited<ReturnType<typeof startWebHost>>;
let shutdown: (() => Promise<void>) | undefined;
export async function activate(context: vscode.ExtensionContext) {
  const output = vscode.window.createOutputChannel("Roku Emulator");
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 25);
  status.text = "$(device-desktop) Roku Emulator";
  status.command = "rokuEmulator.open";
  status.tooltip = "Abrir emulador Roku";
  status.show();
  let runtime: ReturnType<typeof startNodeRuntime> | undefined;
  let device: RunningDeviceServer | undefined;
  let web: RunningWeb | undefined;
  let panel: vscode.WebviewPanel | undefined;
  let starting: Promise<string> | undefined;
  let url: string | undefined;
  let ready = false;
  let isolated: boolean | undefined;
  let serverRuntime = false;
  const settings = () => vscode.workspace.getConfiguration("rokuEmulator");
  const ports = (): EmulatorPorts => ({
    ecpPort: settings().get("ecpPort", 8060),
    consolePort: settings().get("consolePort", 8085),
    debugPort: settings().get("debugPort", 8081),
  });
  let watching: vscode.Disposable | undefined;
  const stopWatching = () => {
    watching?.dispose();
    watching = undefined;
  };
  const stop = async () => {
    stopWatching();
    panel?.dispose();
    panel = undefined;
    ready = false;
    isolated = undefined;
    const ownedWeb = web,
      ownedDevice = device;
    web = undefined;
    device = undefined;
    url = undefined;
    await runtime?.close();
    runtime = undefined;
    await ownedWeb?.close();
    await ownedDevice?.close();
    status.text = "$(device-desktop) Roku Emulator";
    output.appendLine("Emulador parado.");
  };
  const start = async (): Promise<string> => {
    if (url) return url;
    if (starting) return starting;
    starting = (async () => {
      if (settings().get<string>("runtime", "bundled") === "external") {
        url = localRuntimeUrl(settings().get("externalUrl", "")).href;
        return url;
      }
      const profile = JSON.parse(
        await readFile(context.asAbsolutePath("runtime/profile.json"), "utf8"),
      ) as Record<string, string>;
      const configuredPorts = ports();
      const running = await startDeviceServer(
        {
          ...DEFAULT_CONFIG,
          ...configuredPorts,
          installerPort: configuredPorts.ecpPort,
          sceneConsolePort: 0,
        },
        profile,
      );
      try {
        const host = await startWebHost(context.asAbsolutePath("runtime/web"), running);
        device = running;
        web = host;
        runtime = startNodeRuntime(
          running.bridge,
          context.asAbsolutePath("runtime/worker.mjs"),
          vscode.Uri.joinPath(context.storageUri ?? context.globalStorageUri, "registry.json")
            .fsPath,
        );
        url = `${host.url}&host=server`;
        status.text = "$(debug-start) Roku Emulator";
        output.appendLine(`Emulador iniciado em ${url}`);
        return url;
      } catch (error) {
        await running.close();
        throw error;
      }
    })();
    try {
      return await starting;
    } finally {
      starting = undefined;
    }
  };
  const open = async () => {
    const address = await start();
    if (panel) {
      panel.reveal();
      return;
    }
    panel = vscode.window.createWebviewPanel(
      "rokuEmulator",
      "Roku Emulator",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "media")],
      },
    );
    const uri = await vscode.env.asExternalUri(vscode.Uri.parse(address));
    const origin = new URL(uri.toString()).origin;
    const nonce = randomBytes(16).toString("hex");
    const css = panel.webview.asWebviewUri(
      vscode.Uri.joinPath(context.extensionUri, "media/panel.css"),
    );
    const js = panel.webview.asWebviewUri(
      vscode.Uri.joinPath(context.extensionUri, "media/panel.js"),
    );
    const attribute = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
    panel.webview.html = `<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; frame-src ${attribute(origin)}; style-src ${panel.webview.cspSource}; script-src 'nonce-${nonce}';"><link rel="stylesheet" href="${css}"></head><body><nav aria-label="Controle remoto">${[
      ["Home", "Início"],
      ["Back", "Voltar"],
      ["Up", "Cima"],
      ["Left", "Esquerda"],
      ["OK", "OK"],
      ["Right", "Direita"],
      ["Down", "Baixo"],
      ["Play", "Play/Pause"],
    ]
      .map(([key, label]) => `<button data-key="${key}">${label}</button>`)
      .join(
        "",
      )}</nav><iframe title="Emulador Roku" src="${attribute(uri.toString())}" allow="cross-origin-isolated; autoplay; encrypted-media" sandbox="allow-scripts allow-same-origin allow-downloads"></iframe><script nonce="${nonce}" src="${js}"></script></body></html>`;
    panel.webview.onDidReceiveMessage(
      async (message: unknown) => {
        if (!message || typeof message !== "object") return;
        const data = message as {
          type?: unknown;
          key?: unknown;
          isolated?: unknown;
          serverRuntime?: unknown;
        };
        if (data.type === "ready") {
          if (process.env.ROKU_EXTENSION_TEST)
            console.log("Embedded ready message", JSON.stringify(data));
          ready = true;
          isolated = data.isolated === true;
          serverRuntime = data.serverRuntime === true;
          if (!isolated && !serverRuntime)
            output.appendLine(
              "O webview não oferece isolamento de origem. Use Abrir no navegador para rede síncrona e callbacks pausáveis.",
            );
        }
        if (data.type === "remote" && typeof data.key === "string" && isRemoteKey(data.key)) {
          const key = data.key as RemoteKey;
          if (device) {
            device.bridge.key(key, true);
            device.bridge.key(key, false);
          } else {
            await fetch(
              `http://127.0.0.1:${ports().ecpPort}/keypress/${key === "OK" ? "Select" : key === "Options" ? "Info" : key}`,
              { method: "POST" },
            );
          }
        }
      },
      undefined,
      context.subscriptions,
    );
    panel.onDidDispose(() => {
      panel = undefined;
      ready = false;
    });
  };
  const configure = async () => {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders?.length) throw new Error("Abra a pasta do canal Roku primeiro.");
    const folder =
      folders.length === 1
        ? folders[0]
        : (
            await vscode.window.showQuickPick(
              folders.map((folder) => ({ label: folder.name, folder })),
            )
          )?.folder;
    if (!folder) return;
    try {
      await vscode.workspace.fs.stat(vscode.Uri.joinPath(folder.uri, "manifest"));
    } catch {
      throw new Error("A pasta selecionada não contém o manifest de um canal Roku.");
    }
    const directory = vscode.Uri.joinPath(folder.uri, ".vscode");
    const file = vscode.Uri.joinPath(directory, "launch.json");
    let source = '{\n  "version": "0.2.0",\n  "configurations": []\n}\n';
    try {
      source = Buffer.from(await vscode.workspace.fs.readFile(file)).toString();
    } catch (error) {
      if (!(error instanceof vscode.FileSystemError) || error.code !== "FileNotFound") throw error;
    }
    const updated = updateLaunchConfiguration(
      source,
      launchConfiguration("${workspaceFolder}", ports()),
    );
    await vscode.workspace.fs.createDirectory(directory);
    await vscode.workspace.fs.writeFile(file, Buffer.from(updated));
    await vscode.window.showTextDocument(file);
    output.appendLine(`Configuração adicionada: ${file.fsPath}`);
  };
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory("roku-emulator", {
      createDebugAdapterDescriptor: () =>
        new vscode.DebugAdapterInlineImplementation(
          new EmulatorDebugSession(async () => ({
            url: await start(),
            ...ports(),
            password:
              settings().get<string>("runtime", "bundled") === "external"
                ? ((await context.secrets.get("externalPassword")) ?? "rokudev")
                : "rokudev",
          })),
        ),
    }),
  );
  const toggleWatch = async () => {
    if (watching) {
      stopWatching();
      output.appendLine("Hot reload desativado.");
      return;
    }
    if (vscode.debug.activeDebugSession?.type === "roku-emulator")
      throw new Error("Encerre a sessão de depuração antes de ativar o hot reload.");
    const folders = vscode.workspace.workspaceFolders;
    if (!folders?.length) throw new Error("Abra a pasta do canal Roku primeiro.");
    const folder =
      folders.length === 1
        ? folders[0]
        : (
            await vscode.window.showQuickPick(
              folders.map((folder) => ({ label: folder.name, folder })),
            )
          )?.folder;
    if (!folder) return;
    await vscode.workspace.fs.stat(vscode.Uri.joinPath(folder.uri, "manifest"));
    await open();
    if (vscode.debug.activeDebugSession?.type === "roku-emulator")
      throw new Error("Encerre a sessão de depuração antes de ativar o hot reload.");
    const reload = new HotReload(
      async (signal) => {
        const zip = await packageWorkspace(folder.uri.fsPath);
        if (signal.aborted) return;
        const password =
          settings().get<string>("runtime", "bundled") === "external"
            ? ((await context.secrets.get("externalPassword")) ?? "rokudev")
            : "rokudev";
        const installed = await digestFetch(
          `http://127.0.0.1:${ports().ecpPort}/plugin_install`,
          () => {
            const body = new FormData();
            body.set("mysubmit", "Install");
            body.set("archive", new Blob([zip]), "channel.zip");
            return { method: "POST", body, signal };
          },
          { user: "rokudev", password },
        );
        if (!installed.ok || !(await installed.text()).includes("Install Success."))
          throw new Error("Falha na instalação do canal.");
        output.appendLine("Hot reload: canal atualizado; registro preservado.");
      },
      (error) => output.appendLine(`Hot reload: ${String(error)}`),
    );
    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(folder, "**/*"),
    );
    const changed = (uri: vscode.Uri) => {
      const parts = vscode.workspace.asRelativePath(uri, false).split(/[\\/]/);
      if (
        parts.some(
          (p) =>
            [
              ".git",
              ".vscode",
              "node_modules",
              "out",
              "dist",
              "coverage",
              "golden",
              ".DS_Store",
            ].includes(p) ||
            p.startsWith(".env") ||
            p.endsWith(".vsix"),
        )
      )
        return;
      reload.notify();
    };
    watching = vscode.Disposable.from(
      reload,
      watcher,
      watcher.onDidChange(changed),
      watcher.onDidCreate(changed),
      watcher.onDidDelete(changed),
    );
    reload.notify();
    output.appendLine(
      `Hot reload ativado para ${folder.name}. Execute o comando novamente para desativar.`,
    );
  };
  context.subscriptions.push(
    vscode.debug.onDidStartDebugSession((session) => {
      if (session.type === "roku-emulator" && watching) {
        stopWatching();
        output.appendLine("Hot reload desativado para iniciar a depuração.");
      }
    }),
  );
  const commands: Record<string, () => unknown> = {
    open,
    start,
    stop,
    restart: async () => {
      await stop();
      await open();
    },
    configure,
    watch: toggleWatch,
    browser: async () => vscode.env.openExternal(vscode.Uri.parse(await start())),
    password: async () =>
      settings().get<string>("runtime", "bundled") === "external"
        ? ((await context.secrets.get("externalPassword")) ?? "rokudev")
        : "rokudev",
    selectRuntime: async () => {
      const value = await vscode.window.showQuickPick([
        { label: "Runtime incluído", value: "bundled" },
        { label: "Servidor local existente", value: "external" },
      ]);
      if (!value) return;
      if (value.value === "external") {
        const input = await vscode.window.showInputBox({
          title: "URL do emulador local",
          value: settings().get("externalUrl", ""),
        });
        if (!input) return;
        const target = localRuntimeUrl(input);
        const password = await vscode.window.showInputBox({
          title: "Senha do instalador local",
          password: true,
          prompt: "Deixe em branco para usar rokudev",
        });
        if (password === undefined) return;
        await context.secrets.store("externalPassword", password || "rokudev");
        await settings().update("externalUrl", target.href, vscode.ConfigurationTarget.Global);
      }
      await stop();
      await settings().update("runtime", value.value, vscode.ConfigurationTarget.Global);
    },
    _status: () => ({
      running: !!url,
      watching: !!watching,
      url,
      ready,
      isolated,
      serverRuntime,
      ports: ports(),
    }),
  };
  for (const [name, handler] of Object.entries(commands))
    context.subscriptions.push(
      vscode.commands.registerCommand(`rokuEmulator.${name}`, async () => {
        try {
          return await handler();
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          output.appendLine(message);
          if (name === "_status" || process.env.ROKU_EXTENSION_TEST) throw error;
          void vscode.window.showErrorMessage(`Roku Emulator: ${message}`);
          return undefined;
        }
      }),
    );
  context.subscriptions.push(output, status, {
    dispose: () => {
      void stop();
    },
  });
  shutdown = stop;
  return {
    start,
    stop,
    open,
    status: () => ({ running: !!url, url, ready, isolated, serverRuntime }),
  };
}
export async function deactivate() {
  await shutdown?.();
  shutdown = undefined;
}
