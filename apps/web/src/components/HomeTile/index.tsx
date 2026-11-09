import { useEffect, useState } from "react";
import { Icon, type IconName } from "@/components/Icon";
import { styles } from "./styles";

interface HomeTileProps {
  label: string;
  focused: boolean;
  /** Shown when there is no image (or it failed to decode). */
  icon: IconName;
  /** Channel focus icon (mm_icon_focus_*) bytes. */
  image?: ArrayBuffer | null | undefined;
  badge?: string | undefined;
  onSelect: () => void;
}

/** One 16:9 tile of the home screen grid. Activated by OK or a click. */
export function HomeTile({ label, focused, icon, image, badge, onSelect }: HomeTileProps) {
  const imageUrl = useObjectUrl(image ?? null);
  const [imageFailed, setImageFailed] = useState(false);
  const showImage = imageUrl !== null && !imageFailed;

  return (
    <button
      type="button"
      className={`${styles.tile.base} ${focused ? styles.tile.focused : ""}`}
      aria-current={focused ? "true" : undefined}
      // Remote navigation drives focus; keep mouse clicks from moving DOM focus.
      tabIndex={-1}
      onClick={onSelect}
    >
      {showImage ? (
        <img src={imageUrl} alt="" className={styles.image} onError={() => setImageFailed(true)} />
      ) : (
        <Icon name={icon} size="tile" />
      )}
      {!showImage && <span className={styles.label}>{label}</span>}
      {showImage && <span className="sr-only">{label}</span>}
      {badge && <span className={styles.badge}>{badge}</span>}
    </button>
  );
}

/** Object URL for image bytes, revoked when the bytes change or on unmount. */
function useObjectUrl(bytes: ArrayBuffer | null): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!bytes) {
      setUrl(null);
      return;
    }
    const objectUrl = URL.createObjectURL(new Blob([bytes]));
    setUrl(objectUrl);
    return () => URL.revokeObjectURL(objectUrl);
  }, [bytes]);
  return url;
}
