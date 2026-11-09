import type { RemoteKey } from "@roku-emu/protocol";

/**
 * Focus movement on the home screen tile grid (row-major). Like a TV grid,
 * focus stops at the edges instead of wrapping; moving down into a shorter
 * last row lands on its last tile.
 */
export function moveFocus(index: number, key: RemoteKey, count: number, columns: number): number {
  if (count <= 0) return 0;
  const column = index % columns;
  switch (key) {
    case "Left":
      return column > 0 ? index - 1 : index;
    case "Right":
      return column < columns - 1 && index + 1 < count ? index + 1 : index;
    case "Up":
      return index - columns >= 0 ? index - columns : index;
    case "Down": {
      const lastRowStart = Math.floor((count - 1) / columns) * columns;
      if (index >= lastRowStart) return index;
      return Math.min(index + columns, count - 1);
    }
    default:
      return index;
  }
}
