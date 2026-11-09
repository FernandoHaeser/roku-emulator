import { styles } from "./styles";

/**
 * Emulated device profile. Mirrors DeviceProfile::default() in
 * crates/brs-core/src/environment.rs (what roDeviceInfo reports to channels).
 */
const DEVICE_PROFILE: ReadonlyArray<readonly [string, string]> = [
  ["Modelo", "H000X (emulado)"],
  ["Resolução da interface", "1280 x 720 (HD)"],
  ["Modo de vídeo", "720p"],
  ["Idioma / região", "pt_BR"],
  ["Fuso horário", "UTC-03:00"],
  ["Renderizador", "Canvas2D (display list)"],
];

interface AboutPanelProps {
  onClose: () => void;
}

/** "Sobre o dispositivo" dialog of the home screen. Closed with Back/OK or a click. */
export function AboutPanel({ onClose }: AboutPanelProps) {
  return (
    <div className={styles.backdrop} onClick={onClose}>
      <div className={styles.panel} role="dialog" aria-label="Sobre o dispositivo">
        <p className={styles.title}>Sobre o dispositivo</p>
        <dl className={styles.list}>
          {DEVICE_PROFILE.map(([term, value]) => (
            <div key={term} className={styles.row}>
              <dt className={styles.term}>{term}</dt>
              <dd className={styles.value}>{value}</dd>
            </div>
          ))}
        </dl>
        <p className={styles.hint}>Voltar ou OK para fechar</p>
      </div>
    </div>
  );
}
