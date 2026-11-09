import { fileURLToPath, URL } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { rokuDevicePlugin } from "../../packages/server/src/vitePlugin";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss(), rokuDevicePlugin()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  worker: { format: "es" },
  // Dev tooling only: never expose the emulator on the LAN by default.
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
  },
  preview: { host: "127.0.0.1" },
});
