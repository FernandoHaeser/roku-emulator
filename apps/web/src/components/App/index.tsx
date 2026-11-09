import { ConsolePanel } from "@/components/ConsolePanel";
import { DeviceScreen } from "@/components/DeviceScreen";
import { useEmulator } from "@/hooks/useEmulator";
import { styles } from "./styles";

/** FHD logical resolution; will come from the channel manifest (ui_resolutions). */
const SCREEN_WIDTH = 1920;
const SCREEN_HEIGHT = 1080;

const STATUS_LABEL = { starting: "Iniciando", ready: "Pronto", failed: "Falhou" } as const;

export function App() {
  const { setScreenHost, status, renderer, lines, clearConsole } = useEmulator(
    SCREEN_WIDTH,
    SCREEN_HEIGHT,
  );

  return (
    <div className={styles.root}>
      <header className={styles.header}>
        <h1 className={styles.title}>Roku Emulator</h1>
        <span
          className={`${styles.status.base} ${styles.status[status]}`}
          title={renderer || undefined}
        >
          {STATUS_LABEL[status]}
        </span>
      </header>
      <main className={styles.main}>
        <DeviceScreen hostRef={setScreenHost} width={SCREEN_WIDTH} height={SCREEN_HEIGHT} />
        <ConsolePanel lines={lines} onClear={clearConsole} />
      </main>
    </div>
  );
}
