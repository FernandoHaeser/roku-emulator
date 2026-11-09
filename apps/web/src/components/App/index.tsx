import { useRef } from "react";
import { ChannelLoader } from "@/components/ChannelLoader";
import { ConsolePanel } from "@/components/ConsolePanel";
import { DeviceScreen } from "@/components/DeviceScreen";
import { HomeScreen } from "@/components/HomeScreen";
import { useEmulator } from "@/hooks/useEmulator";
import { styles } from "./styles";

/** Output surface (FHD). Channels render at their design resolution, scaled to it. */
const SCREEN_WIDTH = 1920;
const SCREEN_HEIGHT = 1080;

const STATUS_LABEL = { starting: "Iniciando", ready: "Pronto", failed: "Falhou" } as const;

export function App() {
  const emulator = useEmulator(SCREEN_WIDTH, SCREEN_HEIGHT);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const { status, renderer, channel } = emulator;

  return (
    <div className={styles.root}>
      <header className={styles.header}>
        <h1 className={styles.title}>Roku Emulator</h1>
        <div className={styles.actions}>
          <ChannelLoader
            channelName={channel.name}
            status={channel.status}
            disabled={status !== "ready"}
            onLoad={(file) => void emulator.sideload(file)}
            inputRef={fileInputRef}
          />
          <span
            className={`${styles.status.base} ${styles.status[status]}`}
            title={renderer || undefined}
          >
            {STATUS_LABEL[status]}
          </span>
        </div>
      </header>
      <main className={styles.main}>
        <DeviceScreen
          hostRef={emulator.setScreenHost}
          width={SCREEN_WIDTH}
          height={SCREEN_HEIGHT}
          overlay={
            emulator.view === "home" && (
              <HomeScreen
                devChannel={emulator.devChannel}
                homeKey={emulator.homeKey}
                onLaunch={emulator.launchDevChannel}
                onInstall={() => fileInputRef.current?.click()}
                onRemove={emulator.removeDevChannel}
              />
            )
          }
        />
        <ConsolePanel lines={emulator.lines} onClear={emulator.clearConsole} />
      </main>
    </div>
  );
}
