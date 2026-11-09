import { applyEdits, modify, parse, type ParseError } from "jsonc-parser";
export interface EmulatorPorts {
  ecpPort: number;
  consolePort: number;
  debugPort: number;
}
export function launchConfiguration(root: string, ports: EmulatorPorts) {
  return {
    name: "Roku Emulator",
    type: "roku-emulator",
    request: "launch",
    host: "127.0.0.1",
    password: "${command:rokuEmulator.password}",
    rootDir: root,
    packagePort: ports.ecpPort,
    remotePort: ports.ecpPort,
    brightScriptConsolePort: ports.consolePort,
    controlPort: ports.debugPort,
    enableDebugProtocol: true,
  };
}
export function updateLaunchConfiguration(
  source: string,
  configuration: Record<string, unknown>,
): string {
  const errors: ParseError[] = [];
  const parsed = parse(source || "{}", errors, { allowTrailingComma: true }) as {
    version?: unknown;
    configurations?: unknown;
  } | null;
  if (errors.length || !parsed || typeof parsed !== "object" || Array.isArray(parsed))
    throw new Error("launch.json contém JSON inválido; corrija-o antes de configurar o emulador.");
  if (parsed.configurations !== undefined && !Array.isArray(parsed.configurations))
    throw new Error("launch.json: configurations precisa ser uma lista.");
  const configurations = Array.isArray(parsed.configurations) ? parsed.configurations : [];
  const index = configurations.findIndex(
    (c: unknown) =>
      !!c && typeof c === "object" && (c as { name?: unknown }).name === configuration.name,
  );
  const options = { formattingOptions: { insertSpaces: true, tabSize: 2, eol: "\n" } };
  let result = source || "{}";
  if (parsed.version === undefined)
    result = applyEdits(result, modify(result, ["version"], "0.2.0", options));
  if (!Array.isArray(parsed.configurations))
    result = applyEdits(result, modify(result, ["configurations"], [], options));
  return applyEdits(
    result,
    modify(result, ["configurations", index < 0 ? -1 : index], configuration, options),
  );
}
export function localRuntimeUrl(input: string): URL {
  const url = new URL(input);
  if (
    url.protocol !== "http:" ||
    !["127.0.0.1", "localhost", "[::1]"].includes(url.hostname) ||
    url.username ||
    url.password
  )
    throw new Error("Use a URL HTTP de um emulador local (localhost).");
  url.searchParams.set("device", "1");
  return url;
}
