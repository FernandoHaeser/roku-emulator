import { styles } from "./styles";

interface DeviceScreenProps {
  /** Receives the element the emulator mounts its canvas into. */
  hostRef: (element: HTMLDivElement | null) => void;
  width: number;
  height: number;
}

/** TV frame that keeps the channel's aspect ratio at any window size. */
export function DeviceScreen({ hostRef, width, height }: DeviceScreenProps) {
  return (
    <section className={styles.frame} aria-label="Tela do dispositivo">
      <div
        ref={hostRef}
        className={styles.screen}
        style={{ aspectRatio: `${width} / ${height}` }}
      />
    </section>
  );
}
