/**
 * Just enough zip reading to vet an uploaded package before handing it to the
 * engine (which does the real, bounded extraction): entry names from the
 * central directory.
 */

const EOCD_SIGNATURE = 0x06054b50;
const CENTRAL_SIGNATURE = 0x02014b50;
/** EOCD record (22 bytes) plus the longest possible comment. */
const EOCD_SEARCH = 22 + 0xffff;

/** Entry names, or null when the buffer is not a readable zip. */
export function zipEntryNames(zip: Buffer): string[] | null {
  const searchStart = Math.max(0, zip.length - EOCD_SEARCH);
  let eocd = -1;
  for (let offset = zip.length - 22; offset >= searchStart; offset -= 1) {
    if (zip.readUInt32LE(offset) === EOCD_SIGNATURE) {
      eocd = offset;
      break;
    }
  }
  if (eocd === -1) return null;
  const count = zip.readUInt16LE(eocd + 10);
  let offset = zip.readUInt32LE(eocd + 16);
  const names: string[] = [];
  for (let index = 0; index < count; index += 1) {
    if (offset + 46 > zip.length || zip.readUInt32LE(offset) !== CENTRAL_SIGNATURE) return null;
    const nameLength = zip.readUInt16LE(offset + 28);
    const extraLength = zip.readUInt16LE(offset + 30);
    const commentLength = zip.readUInt16LE(offset + 32);
    const nameEnd = offset + 46 + nameLength;
    if (nameEnd > zip.length) return null;
    names.push(zip.subarray(offset + 46, nameEnd).toString("utf8"));
    offset = nameEnd + extraLength + commentLength;
  }
  return names;
}
