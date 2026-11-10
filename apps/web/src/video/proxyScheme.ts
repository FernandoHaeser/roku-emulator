/**
 * Shaka networking through the shell's HTTP proxy (`/__bridge/http`, the
 * same route roUrlTransfer uses). Stream servers rarely send CORS headers and
 * the device does not need them, so manifests, segments and licenses go
 * through the loopback-only proxy instead of the browser's fetch.
 */
import { BRIDGE_HEADER, BRIDGE_PATH } from "@roku-emu/protocol/bridge";
import shaka from "shaka-player/dist/shaka-player.compiled.js";

interface ProxyResponse {
  status: number;
  headers?: Record<string, string>;
  body?: string;
  failure?: string;
}

function toBase64(data: ArrayBuffer | ArrayBufferView | null | undefined): string {
  if (!data) return "";
  const bytes =
    data instanceof ArrayBuffer
      ? new Uint8Array(data)
      : new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000)
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(text);
}

function fromBase64(text: string): ArrayBuffer {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

/** Shaka scheme plugin for http/https. */
export const proxyScheme: shaka.extern.SchemePlugin = (uri, request, requestType) => {
  const controller = new AbortController();
  const started = performance.now();
  const promise = fetch(`${BRIDGE_PATH}/http`, {
    method: "POST",
    headers: { "Content-Type": "application/json", [BRIDGE_HEADER]: "1" },
    signal: controller.signal,
    body: JSON.stringify({
      url: uri,
      method: request.method,
      headers: request.headers,
      body: toBase64(request.body),
      timeout: request.retryParameters.timeout || 30000,
    }),
  })
    .then(async (response) => {
      if (!response.ok) throw new Error(`HTTP proxy: ${response.status}`);
      return (await response.json()) as ProxyResponse;
    })
    .then((result) => {
      const data = fromBase64(result.body ?? "");
      const headers = Object.fromEntries(
        Object.entries(result.headers ?? {}).map(([name, value]) => [name.toLowerCase(), value]),
      );
      if (result.status < 200 || result.status >= 300) {
        throw new shaka.util.Error(
          shaka.util.Error.Severity.CRITICAL,
          shaka.util.Error.Category.NETWORK,
          result.status > 0
            ? shaka.util.Error.Code.BAD_HTTP_STATUS
            : shaka.util.Error.Code.HTTP_ERROR,
          uri,
          result.status,
          result.failure ?? "",
          headers,
          requestType,
        );
      }
      return {
        uri,
        originalUri: uri,
        data,
        headers,
        status: result.status,
        timeMs: performance.now() - started,
      };
    });
  return new shaka.util.AbortableOperation(promise, () => {
    controller.abort();
    return Promise.resolve();
  });
};
