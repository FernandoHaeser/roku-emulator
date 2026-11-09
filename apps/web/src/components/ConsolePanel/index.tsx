import { useEffect, useRef } from "react";
import type { ConsoleLine } from "@/hooks/useEmulator";
import { styles } from "./styles";

interface ConsolePanelProps {
  lines: ConsoleLine[];
  onClear: () => void;
}

/** BrightScript console output (the emulator's equivalent of telnet port 8085). */
export function ConsolePanel({ lines, onClear }: ConsolePanelProps) {
  const outputRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const output = outputRef.current;
    if (output) output.scrollTop = output.scrollHeight;
  }, [lines]);

  return (
    <section className={styles.panel} aria-label="Console">
      <div className={styles.toolbar}>
        <h2 className={styles.heading}>Console</h2>
        <button type="button" className={styles.clearButton} onClick={onClear}>
          Limpar
        </button>
      </div>
      <div ref={outputRef} className={styles.output} role="log" aria-live="polite">
        {lines.length === 0 ? (
          <p className={styles.empty}>Sem saída.</p>
        ) : (
          lines.map((line) => (
            <div key={line.id} className={styles.line[line.stream]}>
              {line.text}
            </div>
          ))
        )}
      </div>
    </section>
  );
}
