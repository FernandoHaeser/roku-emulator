/**
 * Thin wrapper over the Rust engine compiled to WebAssembly (crates/brs-wasm).
 * The module exports a plain C ABI; this class owns the memory copies.
 *
 * Exchange buffers (see crates/brs-wasm/src/lib.rs): small strings go in via
 * the scratch buffer, payloads come out via the blob buffer. Both are only
 * valid until the next call into the module, so every view is copied or
 * decoded immediately.
 */

interface EngineExports {
  memory: WebAssembly.Memory;
  brs_alloc(length: number): number;
  brs_scratch(length: number): number;
  brs_blob_ptr(): number;
  brs_blob_len(): number;
  brs_load_zip(pointer: number, length: number, nowMs: number, epochOffsetMs: number): number;
  brs_unload(): void;
  brs_inspect_zip(pointer: number, length: number): number;
  brs_take_icon(): number;
  brs_run(budget: number, nowMs: number): number;
  brs_sleep_until(): number;
  brs_key(pressed: number): void;
  brs_debugger_command(): number;
  brs_http_complete(id: number): void;
  brs_launch_args(): void;
  brs_input(): void;
  brs_revision(): number;
  brs_display_list(knownRevision: number): number;
  brs_package_file(): number;
  brs_registry_load(): void;
  brs_registry_take(): number;
  brs_take_output(): number;
  brs_output_len(): number;
}

/** Must match the STATUS_* constants in crates/brs-wasm/src/lib.rs. */
export const EngineStatus = {
  NotLoaded: -1,
  Running: 0,
  Sleeping: 1,
  Finished: 2,
  Crashed: 3,
  Stopped: 4,
  /** Blocked on a message port / SceneGraph with nothing scheduled: wake on input. */
  Waiting: 5,
} as const;

export type EngineStatusCode = (typeof EngineStatus)[keyof typeof EngineStatus];

/** Blob producer results (BLOB_UNCHANGED / BLOB_NONE in the Rust side). */
const BLOB_UNCHANGED = -1;

export interface PackageSummary {
  title: string;
  version: string;
  icon: Uint8Array<ArrayBuffer> | null;
}

/** Display list result: unchanged since the known revision, nothing shown, or the JSON. */
export type DisplayListResult =
  { kind: "unchanged" } | { kind: "none" } | { kind: "list"; json: string };

export class BrsEngine {
  private readonly decoder = new TextDecoder();
  private readonly encoder = new TextEncoder();

  private constructor(private readonly exports: EngineExports) {}

  static async load(
    url: string,
    debuggerHost?: { wait(output: string): string; resumed(): void },
    httpHost?: { request(id: number, request: string): string | null; cancel(id: number): void },
  ): Promise<BrsEngine> {
    const response = await fetch(url);
    const { instance } = await WebAssembly.instantiateStreaming(response, {
      host: {
        debug_wait: (pointer: number, length: number, command: number, capacity: number) => {
          if (!debuggerHost) return -1;
          const output = new TextDecoder().decode(
            new Uint8Array(exports.memory.buffer, pointer, length),
          );
          const bytes = new TextEncoder().encode(debuggerHost.wait(output));
          const size = Math.min(bytes.length, capacity);
          new Uint8Array(exports.memory.buffer, command, size).set(bytes.subarray(0, size));
          return size;
        },
        debug_resumed: () => debuggerHost?.resumed(),
        http_request: (
          pointer: number,
          length: number,
          id: number,
          destination: number,
          capacity: number,
        ) => {
          if (!httpHost) return -1;
          const request = new TextDecoder().decode(
            new Uint8Array(exports.memory.buffer, pointer, length),
          );
          const result = httpHost.request(id, request);
          if (result === null) return 0;
          const bytes = new TextEncoder().encode(result);
          if (bytes.length > capacity) return -1;
          new Uint8Array(exports.memory.buffer, destination, bytes.length).set(bytes);
          return bytes.length;
        },
        http_cancel: (id: number) => httpHost?.cancel(id),
      },
    });
    const exports = instance.exports as unknown as EngineExports;
    return new BrsEngine(exports);
  }

  /** Copies `bytes` into a fresh module allocation (ownership passes to Rust). */
  private allocate(bytes: Uint8Array): number {
    const pointer = this.exports.brs_alloc(bytes.byteLength);
    // Re-read the buffer after allocating: memory may have grown.
    new Uint8Array(this.exports.memory.buffer, pointer, bytes.byteLength).set(bytes);
    return pointer;
  }

  private writeScratch(text: string): void {
    const bytes = this.encoder.encode(text);
    const pointer = this.exports.brs_scratch(bytes.byteLength);
    new Uint8Array(this.exports.memory.buffer, pointer, bytes.byteLength).set(bytes);
  }

  private blobBytes(length = this.exports.brs_blob_len()): Uint8Array<ArrayBuffer> {
    const pointer = this.exports.brs_blob_ptr();
    return new Uint8Array(this.exports.memory.buffer, pointer, length).slice();
  }

  private blobText(length?: number): string {
    return this.decoder.decode(this.blobBytes(length));
  }

  /** Returns true on success; errors are reported through `takeOutput`. */
  loadChannel(
    zip: Uint8Array,
    nowMs: number,
    epochOffsetMs: number,
    args: Record<string, string> = {},
  ): boolean {
    this.writeScratch(JSON.stringify(args));
    this.exports.brs_launch_args();
    const pointer = this.allocate(zip);
    return this.exports.brs_load_zip(pointer, zip.byteLength, nowMs, epochOffsetMs) === 0;
  }

  unload(): void {
    this.exports.brs_unload();
  }

  /** Manifest summary without launching; throws with the package error. */
  inspect(zip: Uint8Array): PackageSummary {
    const pointer = this.allocate(zip);
    const failed = this.exports.brs_inspect_zip(pointer, zip.byteLength) !== 0;
    const text = this.blobText();
    if (failed) throw new Error(text);
    const parsed = JSON.parse(text) as { title: string; version: string; icon: boolean };
    const icon = parsed.icon ? this.blobBytes(this.exports.brs_take_icon()) : null;
    return { title: parsed.title, version: parsed.version, icon };
  }

  run(budget: number, nowMs: number): EngineStatusCode {
    return this.exports.brs_run(budget, nowMs) as EngineStatusCode;
  }

  sleepUntil(): number {
    return this.exports.brs_sleep_until();
  }

  input(args: Record<string, string>): void {
    this.writeScratch(JSON.stringify(args));
    this.exports.brs_input();
  }

  httpComplete(id: number, response: string): void {
    this.writeScratch(response);
    this.exports.brs_http_complete(id);
  }

  debuggerCommand(command: string): boolean {
    this.writeScratch(command);
    return this.exports.brs_debugger_command() === 1;
  }

  key(name: string, pressed: boolean): void {
    this.writeScratch(name);
    this.exports.brs_key(pressed ? 1 : 0);
  }

  revision(): number {
    return this.exports.brs_revision();
  }

  displayList(knownRevision: number): DisplayListResult {
    const length = this.exports.brs_display_list(knownRevision);
    if (length === BLOB_UNCHANGED) return { kind: "unchanged" };
    if (length < 0) return { kind: "none" };
    return { kind: "list", json: this.blobText(length) };
  }

  /** Package file by device path ("pkg:/images/a.png"), or null. */
  packageFile(uri: string): Uint8Array<ArrayBuffer> | null {
    this.writeScratch(uri);
    const length = this.exports.brs_package_file();
    return length < 0 ? null : this.blobBytes(length);
  }

  loadRegistry(json: string): void {
    this.writeScratch(json);
    this.exports.brs_registry_load();
  }

  /** Registry JSON when it changed since the last call, else null. */
  takeRegistry(): string | null {
    const length = this.exports.brs_registry_take();
    return length < 0 ? null : this.blobText(length);
  }

  takeOutput(): string {
    const pointer = this.exports.brs_take_output();
    const length = this.exports.brs_output_len();
    if (length === 0) return "";
    return this.decoder.decode(new Uint8Array(this.exports.memory.buffer, pointer, length));
  }
}
