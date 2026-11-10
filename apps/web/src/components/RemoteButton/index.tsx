import { useRef, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import type { RemoteKey } from "@roku-emu/protocol";
import { Icon, type IconName } from "@/components/Icon";
import { styles } from "./styles";

interface RemoteButtonProps {
  remoteKey: RemoteKey;
  label: string;
  icon?: IconName;
  /** Short text shown instead of an icon (e.g. "OK"). */
  text?: string;
  variant?: keyof typeof styles.variant;
  onKey: (key: RemoteKey, pressed: boolean) => void;
}

/**
 * One remote button. Pointer down/up map to key press/release, so holding a
 * button repeats like a held remote key (the runtime generates repeats).
 * Keyboard activation (Enter/Space on the focused button) sends a press and
 * a release.
 */
export function RemoteButton({
  remoteKey,
  label,
  icon,
  text,
  variant = "round",
  onKey,
}: RemoteButtonProps) {
  const pressed = useRef(false);

  const press = (event: PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return;
    // Keep keyboard focus where it was: arrows keep driving the channel.
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    pressed.current = true;
    onKey(remoteKey, true);
  };
  const release = () => {
    if (!pressed.current) return;
    pressed.current = false;
    onKey(remoteKey, false);
  };
  const activate = (event: MouseEvent<HTMLButtonElement>) => {
    // detail 0: activated from the keyboard, not by a pointer.
    if (event.detail !== 0) return;
    onKey(remoteKey, true);
    onKey(remoteKey, false);
  };
  const stopRepeat = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.repeat) event.preventDefault();
  };

  return (
    <button
      type="button"
      className={`${styles.base} ${styles.variant[variant]}`}
      aria-label={label}
      title={label}
      onPointerDown={press}
      onPointerUp={release}
      onPointerCancel={release}
      onLostPointerCapture={release}
      onClick={activate}
      onKeyDown={stopRepeat}
    >
      {icon ? <Icon name={icon} size="md" /> : <span className={styles.text}>{text}</span>}
    </button>
  );
}
