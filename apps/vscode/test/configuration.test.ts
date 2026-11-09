import { expect, it } from "vitest";
import {
  launchConfiguration,
  localRuntimeUrl,
  updateLaunchConfiguration,
} from "../src/configuration";
it("adds one emulator configuration, preserving comments and other launch entries", () => {
  const source =
    '{\n // Keep my device\n "version":"0.2.0", "configurations":[{"name":"Physical Roku","type":"brightscript"}]\n}';
  const config = launchConfiguration("${workspaceFolder}", {
    ecpPort: 8060,
    consolePort: 8085,
    debugPort: 8081,
  });
  const changed = updateLaunchConfiguration(source, config);
  expect(changed).toContain("// Keep my device");
  expect(changed).toContain("Physical Roku");
  expect(changed).toContain("${command:rokuEmulator.password}");
  expect(updateLaunchConfiguration(changed, config)).toBe(changed);
});
it("rejects invalid configuration and nonlocal or credential-bearing runtime URLs", () => {
  expect(() => updateLaunchConfiguration('{"configurations": 1}', {})).toThrow();
  expect(() => localRuntimeUrl("https://example.com/")).toThrow();
  expect(() => localRuntimeUrl("http://secret@localhost/")).toThrow();
  expect(localRuntimeUrl("http://localhost:5173/").searchParams.get("device")).toBe("1");
});
