/** Dimensions without decoding pixels; also works in the headless Node host. */
export function imageSize(bytes: Uint8Array): [number, number] | null {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.length >= 24 && [137, 80, 78, 71, 13, 10, 26, 10].every((b, i) => bytes[i] === b)) {
    const width = view.getUint32(16),
      height = view.getUint32(20);
    return width && height ? [width, height] : null;
  }
  if (bytes[0] !== 255 || bytes[1] !== 216) return null;
  let at = 2;
  while (at + 4 <= bytes.length) {
    if (bytes[at++] !== 255) return null;
    while (bytes[at] === 255) at++;
    const marker = bytes[at++];
    if (marker === undefined || marker === 217 || marker === 218) return null;
    if (marker === 1 || (marker >= 208 && marker <= 215)) continue;
    if (at + 2 > bytes.length) return null;
    const length = view.getUint16(at);
    if (length < 2 || at + length > bytes.length) return null;
    if (marker >= 192 && marker <= 207 && ![196, 200, 204].includes(marker)) {
      if (length < 7) return null;
      return [view.getUint16(at + 5), view.getUint16(at + 3)];
    }
    at += length;
  }
  return null;
}
