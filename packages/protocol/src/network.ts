/** Bounded network-tool metadata; never retain payloads, auth headers or query values. */
export interface NetworkEntry {
  id: number;
  method: string;
  url: string;
  startedAt: number;
  status: number | null;
  durationMs: number | null;
  responseBytes: number;
}
export function networkStart(id: number, request: string, now: number): NetworkEntry {
  let method = "GET",
    display = "Invalid request";
  try {
    const parsed = JSON.parse(request) as { url?: string; method?: string };
    const url = new URL(parsed.url ?? "");
    url.username = "";
    url.password = "";
    url.hash = "";
    const keys = new Set<string>();
    url.searchParams.forEach((_, key) => keys.add(key));
    url.search = "";
    for (const key of keys) url.searchParams.append(key, "…");
    display = url.toString();
    if (typeof parsed.method === "string" && /^[a-z]+$/i.test(parsed.method))
      method = parsed.method.toUpperCase();
  } catch {
    /* malformed requests still get a diagnostic row */
  }
  return {
    id,
    method,
    url: display,
    startedAt: now,
    status: null,
    durationMs: null,
    responseBytes: 0,
  };
}
export function networkFinish(entry: NetworkEntry, response: string, now: number): NetworkEntry {
  let status = -7,
    responseBytes = 0;
  try {
    const result = JSON.parse(response) as { status?: unknown; body?: unknown };
    if (typeof result.status === "number" && Number.isFinite(result.status)) status = result.status;
    if (typeof result.body === "string")
      responseBytes = Math.max(
        0,
        Math.floor((result.body.length * 3) / 4) -
          (result.body.endsWith("==") ? 2 : result.body.endsWith("=") ? 1 : 0),
      );
  } catch {
    /* the transport failed without a valid response */
  }
  return { ...entry, status, responseBytes, durationMs: Math.max(0, now - entry.startedAt) };
}
