/** ECP key names (`/keypress/<key>`) to remote keys. */
import type { RemoteKey } from "@roku-emu/protocol/keys";

const ECP_TO_REMOTE: Readonly<Record<string, RemoteKey>> = {
  home: "Home",
  back: "Back",
  up: "Up",
  down: "Down",
  left: "Left",
  right: "Right",
  select: "OK",
  // Not an ECP name, but accepted for convenience (our oracle sends Select).
  ok: "OK",
  replay: "Replay",
  instantreplay: "InstantReplay",
  rev: "Rev",
  play: "Play",
  fwd: "Fwd",
  info: "Options",
};

/** Case-insensitive like the device; null for keys the emulator does not model yet. */
export function remoteKeyFromEcp(name: string): RemoteKey | null {
  return ECP_TO_REMOTE[name.toLowerCase()] ?? null;
}
