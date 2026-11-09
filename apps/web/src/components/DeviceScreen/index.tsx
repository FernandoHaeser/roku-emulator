import type { ReactNode } from "react";
import { styles } from "./styles";

interface DeviceScreenProps {
  /** Receives the element the emulator mounts its canvas into. */
  hostRef: (element: HTMLDivElement | null) => void;
  width: number;
  height: number;
  /** Shell UI drawn over the channel canvas (the home screen). */
  overlay?: ReactNode;
}

/** TV frame that keeps a 16:9 aspect ratio at any window size. */
export function DeviceScreen({ hostRef, width, height, overlay }: DeviceScreenProps) {
  return (
    <section className={styles.frame} aria-label="Tela do dispositivo">
      <div className={styles.screen} style={{ aspectRatio: `${width} / ${height}` }}>
        <div ref={hostRef} className={styles.canvasHost} />
        {overlay}
      </div>
    </section>
  );
}
