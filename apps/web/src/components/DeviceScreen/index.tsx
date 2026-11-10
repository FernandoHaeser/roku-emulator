import type { ReactNode } from "react";
import type { NodeHighlight } from "@/components/NodeInspector";
import { styles } from "./styles";

interface DeviceScreenProps {
  /** Receives the element the emulator mounts its canvas into. */
  hostRef: (element: HTMLDivElement | null) => void;
  width: number;
  height: number;
  /** Shell UI drawn over the channel canvas (the home screen). */
  overlay?: ReactNode;
  /** Node selected in the inspector, outlined over the channel. */
  highlight?: NodeHighlight | null;
}

/** TV frame that keeps a 16:9 aspect ratio at any window size. */
export function DeviceScreen({ hostRef, width, height, overlay, highlight }: DeviceScreenProps) {
  return (
    <section className={styles.frame} aria-label="Tela do dispositivo">
      <div className={styles.screen} style={{ aspectRatio: `${width} / ${height}` }}>
        <div ref={hostRef} className={styles.canvasHost} />
        {overlay}
        {highlight && highlight.bounds.length === 4 && (
          <div
            className={styles.highlight}
            aria-hidden="true"
            style={{
              left: `${(highlight.bounds[0]! / highlight.width) * 100}%`,
              top: `${(highlight.bounds[1]! / highlight.height) * 100}%`,
              width: `${(highlight.bounds[2]! / highlight.width) * 100}%`,
              height: `${(highlight.bounds[3]! / highlight.height) * 100}%`,
            }}
          />
        )}
      </div>
    </section>
  );
}
