import { createHash } from "node:crypto";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { connect } from "node:net";
import { once } from "node:events";
import { expect, test } from "@playwright/test";
import { packChannel } from "../../../packages/oracle/src/packager";

const md5 = (text: string) => createHash("md5").update(text).digest("hex");

test("installer deploy reaches WASM and prints to the telnet console", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
  const base = "http://127.0.0.1:8060";
  const challenge = await fetch(`${base}/plugin_install`);
  const nonce = /nonce="([^"]+)"/.exec(challenge.headers.get("www-authenticate")!)![1];
  const uri = "/plugin_install";
  const digest = md5(
    `${md5("rokudev:rokudev:rokudev")}:${nonce}:00000001:e2e:auth:${md5(`POST:${uri}`)}`,
  );
  const socket = connect(8085, "127.0.0.1");
  let output = "";
  socket.on("data", (chunk) => {
    output += String(chunk);
  });
  await once(socket, "connect");
  try {
    const form = new FormData();
    form.set("mysubmit", "Install");
    const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
    form.set(
      "archive",
      new Blob([new Uint8Array(packChannel(join(root, "fidelity/language/hello")))]),
      "hello.zip",
    );
    const installed = await fetch(`${base}${uri}`, {
      method: "POST",
      headers: {
        Authorization: `Digest username="rokudev", realm="rokudev", nonce="${nonce}", uri="${uri}", qop=auth, nc=00000001, cnonce="e2e", response="${digest}"`,
      },
      body: form,
    });
    expect(await installed.text()).toContain("Install Success.");
    await expect(page.getByText("Finalizado", { exact: true })).toBeVisible();
    await expect.poll(() => output).toContain("@@PROBE_END\r\n");
    expect(output).toContain("AppCompileComplete");
    expect(output).toContain("AppExitComplete");
  } finally {
    socket.destroy();
  }
});

test("RokuCommunity adapter deploys, inspects a STOP, steps and continues in WASM", async ({
  page,
}) => {
  const { createRokuAdapter, rokuDeploy } =
    await import("../../../packages/server/test/rokuAdapter");
  const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const scratch = await mkdtemp(join(tmpdir(), "roku-emu-debug-"));
  const adapter = createRokuAdapter(8085, 8060);
  try {
    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await adapter.connect();
    await adapter.activate();
    const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
    const zip = join(scratch, "debugger.zip");
    await writeFile(zip, packChannel(join(root, "apps/web/e2e/fixtures/debugger")));
    await rokuDeploy.sideload({
      device: { host: "127.0.0.1" },
      password: "rokudev",
      packagePort: 8060,
      ecpPort: 8060,
      zip,
      failOnCompileError: false,
    });
    await expect(page.getByText("STOP", { exact: true })).toBeVisible();
    await expect(page.getByLabel("Tela inicial")).toBeHidden();
    await expect.poll(() => adapter.isAtDebuggerPrompt).toBe(true);
    const threads = await adapter.getThreads();
    expect(threads[0]).toMatchObject({ threadId: 0, lineNumber: 3, isSelected: true });
    const frames = await adapter.getStackTrace();
    expect(frames[0]).toMatchObject({ functionIdentifier: "main", lineNumber: 3 });
    expect(await adapter.getScopeVariables()).toContain("answer");
    const answer = await adapter.getVariable("answer");
    expect(answer.value).toBe("21");
    expect((await adapter.getVariable("answer * 2")).value).toBe("42");
    expect(
      await adapter.requestPipeline.executeCommand("print answer", { waitForPrompt: true }),
    ).toContain("21");
    await adapter.stepInto();
    await expect
      .poll(async () => {
        if (!adapter.isAtDebuggerPrompt) return null;
        return (await adapter.getStackTrace())[0]?.lineNumber;
      })
      .toBe(4);
    await adapter.stepInto();
    await expect
      .poll(async () => {
        if (!adapter.isAtDebuggerPrompt) return null;
        return (await adapter.getStackTrace())[0]?.functionIdentifier;
      })
      .toBe("twice");
    await adapter.stepOut();
    await expect
      .poll(async () => {
        if (!adapter.isAtDebuggerPrompt) return null;
        return (await adapter.getStackTrace())[0]?.functionIdentifier;
      })
      .toBe("main");
    await adapter.continue();
    await expect(page.getByText("Finalizado", { exact: true })).toBeVisible();
    await expect(page.locator('[role="log"]')).toContainText("answer= 42");
  } finally {
    await adapter.destroy();
    await rm(scratch, { recursive: true, force: true });
  }
});

test("socket debugger negotiates, installs breakpoints and reads/sets live variables", async ({
  page,
}) => {
  const { DebugProtocolClient, rokuDeploy } =
    await import("../../../packages/server/test/rokuAdapter");
  const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const scratch = await mkdtemp(join(tmpdir(), "roku-protocol-"));
  const client = new DebugProtocolClient({
    device: { host: "127.0.0.1" },
    controlPort: 8081,
    shutdownTimeout: 100,
    exitChannelTimeout: 100,
  });
  try {
    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await client.connect();
    expect(client.protocolVersion).toBe("3.0.0");
    const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
    const zip = join(scratch, "protocol.zip");
    await writeFile(zip, packChannel(join(root, "apps/web/e2e/fixtures/protocol")));
    const entry = client.once("suspend");
    await rokuDeploy.sideload({
      device: { host: "127.0.0.1" },
      password: "rokudev",
      packagePort: 8060,
      ecpPort: 8060,
      zip,
      remoteDebug: true,
      failOnCompileError: false,
    });
    await entry;
    const added = await client.addBreakpoints([
      { filePath: "pkg:/source/main.brs", lineNumber: 3 },
    ]);
    expect(added.data.breakpoints[0]?.id).toBeGreaterThan(0);
    const breakpoint = client.once("suspend");
    await client.continue();
    await breakpoint;
    const stack = await client.getStackTrace(0);
    expect(stack.data.entries[0]?.lineNumber).toBe(3);
    const variables = await client.getVariables(["answer"], 0, 0);
    expect(variables.data.variables[0]?.value).toBe(21);
    const changed = await client.executeCommand("answer = 25", 0, 0);
    expect(changed.data.executeSuccess).toBe(true);
    expect((await client.getVariables(["answer"], 0, 0)).data.variables[0]?.value).toBe(25);
    await client.removeBreakpoints(added.data.breakpoints.map((bp) => bp.id));
    const stepped = client.once("suspend");
    await client.stepIn(0);
    await stepped;
    expect((await client.getStackTrace(0)).data.entries[0]?.functionName.toLowerCase()).toBe(
      "twice",
    );
    await client.continue();
    await expect(page.getByText("Finalizado", { exact: true })).toBeVisible();
    await expect(page.locator('[role="log"]')).toContainText("answer= 50");
  } finally {
    await client.destroy(true);
    await rm(scratch, { recursive: true, force: true });
  }
});

test("STOP inside SceneGraph init preserves the native callback and resumes", async ({ page }) => {
  const { createRokuAdapter } = await import("../../../packages/server/test/rokuAdapter");
  const adapter = createRokuAdapter(8085, 8060);
  try {
    await page.goto("/");
    await expect(page.getByText("Pronto", { exact: true })).toBeVisible();
    await adapter.connect();
    await adapter.activate();
    const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
    await page.locator('input[type="file"]').setInputFiles({
      name: "callback.zip",
      mimeType: "application/zip",
      buffer: Buffer.from(packChannel(join(root, "apps/web/e2e/fixtures/debug-callback"))),
    });
    await expect(page.getByText("STOP", { exact: true })).toBeVisible();
    await expect.poll(() => adapter.isAtDebuggerPrompt).toBe(true);
    expect((await adapter.getThreads())[0]?.threadId).toBe(1);
    expect((await adapter.getVariable("localValue")).value).toBe("7");
    await adapter.continue();
    await expect(page.locator('[role="log"]')).toContainText("callback completed");
    await expect(page.locator('[role="log"]')).toContainText("init resumed= 7");
    await page.keyboard.press("h");
    await expect(page.getByLabel("Tela inicial")).toBeVisible();
  } finally {
    await adapter.destroy();
  }
});
