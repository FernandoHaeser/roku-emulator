import type { ChannelStatus } from "@roku-emu/protocol";
import type { ChangeEvent, Ref } from "react";
import { Icon } from "@/components/Icon";
import { styles } from "./styles";

const STATUS_LABEL: Record<ChannelStatus, string> = {
  idle: "",
  running: "Executando",
  sleeping: "Executando",
  finished: "Finalizado",
  crashed: "Crash",
  stopped: "STOP",
  error: "Erro ao carregar",
};

interface ChannelLoaderProps {
  channelName: string;
  status: ChannelStatus;
  disabled: boolean;
  onLoad: (file: File) => void;
  /** Lets the home screen's "Instalar canal" tile open the same picker. */
  inputRef?: Ref<HTMLInputElement>;
}

/** Picks a sideload package (.zip), like the device's development installer. */
export function ChannelLoader({
  channelName,
  status,
  disabled,
  onLoad,
  inputRef,
}: ChannelLoaderProps) {
  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (file) onLoad(file);
    // Allow loading the same file again after editing it.
    event.target.value = "";
  };

  return (
    <div className={styles.root}>
      {channelName && <span className={styles.channel}>{channelName}</span>}
      {status !== "idle" && (
        <span className={`${styles.status.base} ${styles.status[status]}`}>
          {STATUS_LABEL[status]}
        </span>
      )}
      <label className={styles.button}>
        <Icon name="upload" />
        Abrir canal (.zip)
        <input
          ref={inputRef}
          type="file"
          accept=".zip,application/zip"
          className={styles.input}
          disabled={disabled}
          onChange={handleChange}
        />
      </label>
    </div>
  );
}
