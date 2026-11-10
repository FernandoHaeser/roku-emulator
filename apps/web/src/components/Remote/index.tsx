import type { RemoteKey } from "@roku-emu/protocol";
import { RemoteButton } from "@/components/RemoteButton";
import { styles } from "./styles";

interface RemoteProps {
  onKey: (key: RemoteKey, pressed: boolean) => void;
}

/** Clickable remote: the same keys as the keyboard mapping (src/keyboard.ts). */
export function Remote({ onKey }: RemoteProps) {
  return (
    <section className={styles.remote} aria-label="Controle remoto">
      <div className={styles.group}>
        <RemoteButton remoteKey="Back" label="Voltar" icon="back" onKey={onKey} />
        <RemoteButton remoteKey="Home" label="Início" icon="home" onKey={onKey} />
      </div>
      <div className={styles.pad}>
        <span />
        <RemoteButton
          remoteKey="Up"
          label="Para cima"
          icon="chevronUp"
          variant="pad"
          onKey={onKey}
        />
        <span />
        <RemoteButton
          remoteKey="Left"
          label="Para a esquerda"
          icon="chevronLeft"
          variant="pad"
          onKey={onKey}
        />
        <RemoteButton remoteKey="OK" label="OK" text="OK" variant="ok" onKey={onKey} />
        <RemoteButton
          remoteKey="Right"
          label="Para a direita"
          icon="chevronRight"
          variant="pad"
          onKey={onKey}
        />
        <span />
        <RemoteButton
          remoteKey="Down"
          label="Para baixo"
          icon="chevronDown"
          variant="pad"
          onKey={onKey}
        />
        <span />
      </div>
      <div className={styles.group}>
        <RemoteButton
          remoteKey="InstantReplay"
          label="Replay instantâneo"
          icon="replay"
          onKey={onKey}
        />
        <RemoteButton remoteKey="Options" label="Opções" icon="options" onKey={onKey} />
      </div>
      <div className={styles.group}>
        <RemoteButton remoteKey="Rev" label="Retroceder" icon="rewind" onKey={onKey} />
        <RemoteButton remoteKey="Play" label="Reproduzir/Pausar" icon="playPause" onKey={onKey} />
        <RemoteButton remoteKey="Fwd" label="Avançar" icon="fastForward" onKey={onKey} />
      </div>
    </section>
  );
}
