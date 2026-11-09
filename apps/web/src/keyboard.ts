import type { RemoteKey } from "@roku-emu/protocol";

/**
 * Host keyboard -> Roku remote mapping. Keyed by KeyboardEvent.code so the
 * mapping is layout independent (works the same on ABNT2, US, etc.).
 */
const KEYBOARD_TO_REMOTE: Readonly<Record<string, RemoteKey>> = {
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Enter: "OK",
  NumpadEnter: "OK",
  Backspace: "Back",
  Escape: "Back",
  Home: "Home",
  KeyH: "Home",
  Space: "Play",
  Comma: "Rev",
  Period: "Fwd",
  KeyO: "Options",
  KeyR: "InstantReplay",
};

export function mapKeyboardCode(code: string): RemoteKey | null {
  return KEYBOARD_TO_REMOTE[code] ?? null;
}
