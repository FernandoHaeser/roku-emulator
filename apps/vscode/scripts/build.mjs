import { build } from "esbuild";
import { cp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repo = resolve(root, "../..");
await mkdir(resolve(root, "dist"), { recursive: true });
await build({
  entryPoints: [resolve(root, "src/extension.ts")],
  outfile: resolve(root, "dist/extension.cjs"),
  bundle: true,
  plugins: [
    {
      name: "standalone-roku-codecs",
      setup(build) {
        build.onResolve({ filter: /^\.\.\/(?:\.\.\/)*util$/ }, (args) =>
          args.importer.includes("roku-debug") &&
          /(?:[\\/]ProtocolUtil\.js|[\\/]events[\\/]responses[\\/]VariablesResponse\.js)$/.test(
            args.importer,
          )
            ? { path: "nullish", namespace: "roku-codecs" }
            : undefined,
        );
        build.onLoad({ filter: /.*/, namespace: "roku-codecs" }, () => ({
          contents: "exports.util = { isNullish: value => value === null || value === undefined };",
          loader: "js",
        }));
      },
    },
  ],
  platform: "node",
  format: "cjs",
  target: "node20",
  mainFields: ["module", "main"],
  external: ["vscode", "fsevents"],
  legalComments: "linked",
  logOverride: { "empty-import-meta": "silent" },
});
await build({
  entryPoints: [resolve(root, "test/electron.ts")],
  outfile: resolve(root, "dist/electron-test.cjs"),
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node20",
  mainFields: ["module", "main"],
  external: ["vscode", "fsevents"],
});
await rm(resolve(root, "runtime"), { recursive: true, force: true });
await mkdir(resolve(root, "runtime"), { recursive: true });
await build({
  entryPoints: [resolve(repo, "packages/server/src/nodeWorkerBootstrap.ts")],
  outfile: resolve(root, "runtime/worker.mjs"),
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node20",
});
await build({
  entryPoints: [resolve(repo, "apps/web/src/worker/runtime.worker.ts")],
  outfile: resolve(root, "runtime/runtime.worker.mjs"),
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node20",
  loader: { ".wasm": "file" },
  alias: { "@": resolve(repo, "apps/web/src") },
  plugins: [
    {
      name: "wasm-url",
      setup(build) {
        build.onResolve({ filter: /\.wasm\?url$/ }, (args) => ({
          path: resolve(
            args.resolveDir,
            args.path.replace(/^@\//, resolve(repo, "apps/web/src") + "/").replace(/\?url$/, ""),
          ),
        }));
      },
    },
  ],
});
await cp(resolve(repo, "apps/web/dist"), resolve(root, "runtime/web"), { recursive: true });
await cp(resolve(repo, "profiles/H302X-16.0.4.json"), resolve(root, "runtime/profile.json"));
for (const file of ["LICENSE", "NOTICE"]) await cp(resolve(repo, file), resolve(root, file));
const packageJson = JSON.parse(await readFile(resolve(root, "package.json"), "utf8"));
await writeFile(
  resolve(root, "runtime/version.json"),
  JSON.stringify({ version: packageJson.version, engine: "brs-wasm" }) + "\n",
);
