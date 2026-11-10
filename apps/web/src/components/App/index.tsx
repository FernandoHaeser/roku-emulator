import { useRef, useState } from "react";
import { ChannelLoader } from "@/components/ChannelLoader";
import { ConsolePanel } from "@/components/ConsolePanel";
import { DeviceScreen } from "@/components/DeviceScreen";
import { HomeScreen } from "@/components/HomeScreen";
import { NodeInspector, type NodeHighlight } from "@/components/NodeInspector";
import { RegistryPanel } from "@/components/RegistryPanel";
import { NetworkPanel } from "@/components/NetworkPanel";
import { Remote } from "@/components/Remote";
import { SidePanel } from "@/components/SidePanel";
import { SURFACE } from "@/config/surface";
import { useEmulator } from "@/hooks/useEmulator";
import { styles } from "./styles";

const STATUS_LABEL = { starting: "Iniciando", ready: "Pronto", failed: "Falhou" } as const;

export function App() {
  const emulator = useEmulator(SURFACE.width, SURFACE.height);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const { status, renderer, channel } = emulator;
  const [highlight, setHighlight] = useState<NodeHighlight | null>(null);

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
        <div className={styles.screenColumn}>
          <DeviceScreen
            hostRef={emulator.setScreenHost}
            width={SURFACE.width}
            height={SURFACE.height}
            highlight={emulator.view === "channel" ? highlight : null}
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
          <Remote onKey={emulator.pressRemote} />
        </div>
        <SidePanel
          tabs={[
            {
              id: "console",
              label: "Console",
              render: () => <ConsolePanel lines={emulator.lines} onClear={emulator.clearConsole} />,
            },
            {
              id: "registry",
              label: "Registro",
              render: (active) => (
                <RegistryPanel
                  active={active && emulator.view === "channel"}
                  inspect={emulator.inspectRegistry}
                  update={emulator.setRegistryEntry}
                />
              ),
            },
            {
              id: "network",
              label: "Rede",
              render: () => (
                <NetworkPanel entries={emulator.network} onClear={emulator.clearNetwork} />
              ),
            },
            {
              id: "nodes",
              label: "Nodes",
              render: (active) => (
                <NodeInspector
                  inspect={emulator.inspectScene}
                  active={active && emulator.view === "channel"}
                  onHighlight={setHighlight}
                />
              ),
            },
          ]}
        />
      </main>
    </div>
  );
}
