import { defineConfig, devices } from "@playwright/test";

// A test owns its server and device transports. Never share the developer's
// browser/VS Code runtime: every bridge client otherwise executes each command.
for (const [name, fallback] of Object.entries({
  ECP: "18060",
  INSTALLER: "18060",
  CONSOLE: "18085",
  DEBUG: "18081",
  SCENE_CONSOLE: "18080",
}))
  process.env[`ROKU_EMU_${name}_PORT`] = process.env[`ROKU_EMU_TEST_${name}_PORT`] ?? fallback;
process.env.ROKU_EMU_HOST = "127.0.0.1";
process.env.ROKU_EMU_DEV_PASSWORD = "rokudev";
const webPort = Number(process.env.ROKU_EMU_TEST_WEB_PORT ?? "15173");
const baseURL = `http://127.0.0.1:${webPort}`;

export default defineConfig({
  testDir: "e2e",
  // All pages share one device installer and ECP state.
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 15_000 },
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `pnpm exec vite --port ${webPort} --strictPort`,
    url: baseURL,
    reuseExistingServer: false,
  },
});
