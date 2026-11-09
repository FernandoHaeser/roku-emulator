/** Minimal multipart/form-data parser for the dev installer upload. */

export interface FormPart {
  filename: string | null;
  data: Buffer;
}

export function boundaryOf(contentType: string | undefined): string | null {
  const match = /boundary=(?:"([^"]+)"|([^;\s]+))/i.exec(contentType ?? "");
  return match ? (match[1] ?? match[2] ?? null) : null;
}

/** Parts by field name. Malformed input yields the parts read so far. */
export function parseMultipart(body: Buffer, boundary: string): Map<string, FormPart> {
  const parts = new Map<string, FormPart>();
  const delimiter = Buffer.from(`--${boundary}`);
  let position = body.indexOf(delimiter);
  while (position !== -1) {
    const headerStart = position + delimiter.length;
    // "--" right after a delimiter closes the body.
    if (body.subarray(headerStart, headerStart + 2).toString() === "--") break;
    const headerEnd = body.indexOf("\r\n\r\n", headerStart);
    if (headerEnd === -1) break;
    const next = body.indexOf(delimiter, headerEnd + 4);
    if (next === -1) break;
    const headers = body.subarray(headerStart, headerEnd).toString("utf8");
    // Part data ends with the CRLF that precedes the next delimiter.
    const data = body.subarray(headerEnd + 4, Math.max(headerEnd + 4, next - 2));
    const name = /\bname="([^"]*)"/i.exec(headers)?.[1];
    if (name !== undefined) {
      const filename = /\bfilename="([^"]*)"/i.exec(headers)?.[1] ?? null;
      parts.set(name, { filename, data: Buffer.from(data) });
    }
    position = next;
  }
  return parts;
}
