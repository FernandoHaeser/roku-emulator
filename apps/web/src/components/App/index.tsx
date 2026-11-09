import { useRef } from "react";
import { ChannelLoader } from "@/components/ChannelLoader";
import { ConsolePanel } from "@/components/ConsolePanel";
import { DeviceScreen } from "@/components/DeviceScreen";
import { HomeScreen } from "@/components/HomeScreen";
import { SURFACE } from "@/config/surface";
import { useEmulator } from "@/hooks/useEmulator";
import { styles } from "./styles";

const STATUS_LABEL = { starting: "Iniciando", ready: "Pronto", failed: "Falhou" } as const;

export function App() {
  const emulator = useEmulator(SURFACE.width, SURFACE.height);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const { status, renderer, channel } = emulator;

  return (
    <div className={styles.root}>
      <header className={styles.header}>
        <h1 className={styles.title}>
          <img src="/favicon.svg" alt="" className={styles.logo} />
          Roku Emulator
        </h1>
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
          width={SURFACE.width}
          height={SURFACE.height}
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
