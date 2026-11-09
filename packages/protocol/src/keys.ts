/**
 * Roku remote keys, named exactly as ECP / onKeyEvent use them.
 * Kept free of DOM types so Node tooling (oracle, server) can import it.
 */
export const REMOTE_KEYS = [
  "Home",
  "Back",
  "Up",
  "Down",
  "Left",
  "Right",
  "OK",
  "Replay",
  "Rev",
  "Play",
  "Fwd",
  "Options",
  "InstantReplay",
] as const;

export type RemoteKey = (typeof REMOTE_KEYS)[number];

export function isRemoteKey(value: string): value is RemoteKey {
  return (REMOTE_KEYS as readonly string[]).includes(value);
}
