import { useEffect, useMemo, useRef, useState } from "react";
import { AboutPanel } from "@/components/AboutPanel";
import { HomeTile } from "@/components/HomeTile";
import { Icon, type IconName } from "@/components/Icon";
import { OptionsMenu, type OptionsMenuItem } from "@/components/OptionsMenu";
import { moveFocus } from "@/home/gridNavigation";
import type { HomeKeyEvent } from "@/hooks/useEmulator";
import type { StoredChannel } from "@/storage/devChannelStore";
import { styles } from "./styles";

const GRID_COLUMNS = 4;

type TileId = "dev" | "install" | "about";

interface Tile {
  id: TileId;
  label: string;
  detail: string;
  icon: IconName;
}

const CHANNEL_OPTIONS: readonly OptionsMenuItem[] = [
  { id: "open", label: "Abrir canal", icon: "play" },
  { id: "remove", label: "Remover canal", icon: "trash" },
  { id: "cancel", label: "Cancelar", icon: "close" },
];

interface HomeScreenProps {
  devChannel: StoredChannel | null;
  /** Latest remote key while the home screen is active. */
  homeKey: HomeKeyEvent | null;
  onLaunch: () => void;
  onInstall: () => void;
  onRemove: () => void;
}

/**
 * Emulator home screen: an original TV launcher (not the Roku OS UI) with the
 * device's interaction model: a channel grid driven by the remote, OK opens,
 * Options (*) opens the tile menu, Back closes overlays, Home returns here.
 */
export function HomeScreen({
  devChannel,
  homeKey,
  onLaunch,
  onInstall,
  onRemove,
}: HomeScreenProps) {
  const tiles = useMemo<Tile[]>(() => {
    const list: Tile[] = [];
    if (devChannel) {
      list.push({
        id: "dev",
        label: devChannel.title,
        detail: `Canal de desenvolvimento · versão ${devChannel.version}`,
        icon: "tv",
      });
    }
    list.push(
      {
        id: "install",
        label: "Instalar canal",
        detail: "Carregar um pacote .zip (sideload)",
        icon: "plus",
      },
      { id: "about", label: "Sobre o dispositivo", detail: "Perfil emulado", icon: "info" },
    );
    return list;
  }, [devChannel]);

  const [focusIndex, setFocusIndex] = useState(0);
  const [menuIndex, setMenuIndex] = useState<number | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const now = useClock();

  const focused = Math.min(focusIndex, tiles.length - 1);
  const focusedTile = tiles[focused];

  const activate = (tile: Tile | undefined) => {
    if (tile?.id === "dev") onLaunch();
    else if (tile?.id === "install") onInstall();
    else if (tile?.id === "about") setAboutOpen(true);
  };

  const selectOption = (item: OptionsMenuItem) => {
    setMenuIndex(null);
    if (item.id === "open") onLaunch();
    else if (item.id === "remove") {
      onRemove();
      setFocusIndex(0);
    }
  };

  // Keeps the latest handlers for the key effect without re-running it on every render.
  const handlers = useRef({ activate, selectOption });
  handlers.current = { activate, selectOption };

  // The screen unmounts while a channel runs, but the last key event stays in
  // the emulator state. That event belongs to the previous mount: handling it
  // again on remount would relaunch the channel (an OK that opened it).
  const keyAtMount = useRef(homeKey);

  useEffect(() => {
    if (!homeKey || homeKey === keyAtMount.current) return;
    const { key } = homeKey;
    if (aboutOpen) {
      if (key === "Back" || key === "OK" || key === "Home") setAboutOpen(false);
      return;
    }
    if (menuIndex !== null) {
      if (key === "Up") setMenuIndex(Math.max(0, menuIndex - 1));
      else if (key === "Down") setMenuIndex(Math.min(CHANNEL_OPTIONS.length - 1, menuIndex + 1));
      else if (key === "OK") handlers.current.selectOption(CHANNEL_OPTIONS[menuIndex]!);
      else if (key === "Back" || key === "Options" || key === "Home") setMenuIndex(null);
      return;
    }
    if (key === "OK") handlers.current.activate(focusedTile);
    else if (key === "Options" && focusedTile?.id === "dev") setMenuIndex(0);
    else if (key === "Home") setFocusIndex(0);
    else setFocusIndex(moveFocus(focused, key, tiles.length, GRID_COLUMNS));
    // Only a new key event should trigger navigation.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [homeKey]);

  return (
    <div className={styles.root} aria-label="Tela inicial">
      <header className={styles.header}>
        <span className={styles.brand}>
          <Icon name="tv" size="screen" />
          Emulador
        </span>
        <div className={styles.clock}>
          <p className={styles.time}>
            {now.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}
          </p>
          <p className={styles.date}>
            {now.toLocaleDateString("pt-BR", { weekday: "long", day: "numeric", month: "long" })}
          </p>
        </div>
      </header>

      <main className={styles.body}>
        <p className={styles.sectionTitle}>Seus canais</p>
        <div className={styles.grid}>
          {tiles.map((tile, index) => (
            <HomeTile
              key={tile.id}
              label={tile.label}
              icon={tile.icon}
              image={tile.id === "dev" ? (devChannel?.icon ?? null) : null}
              badge={tile.id === "dev" ? "DEV" : undefined}
              focused={index === focused}
              onSelect={() => {
                setFocusIndex(index);
                activate(tile);
              }}
            />
          ))}
        </div>
        {focusedTile && (
          <div className={styles.caption}>
            <p className={styles.captionTitle}>{focusedTile.label}</p>
            <p className={styles.captionDetail}>{focusedTile.detail}</p>
          </div>
        )}
      </main>

      <footer className={styles.footer}>
        <span>
          <kbd className={styles.footerKey}>OK</kbd>abrir
        </span>
        <span>
          <kbd className={styles.footerKey}>*</kbd>opções (tecla O)
        </span>
        <span>
          <kbd className={styles.footerKey}>Home</kbd>voltar ao início (tecla H)
        </span>
      </footer>

      {menuIndex !== null && devChannel && (
        <OptionsMenu
          title={devChannel.title}
          items={CHANNEL_OPTIONS}
          focusedIndex={menuIndex}
          onSelect={selectOption}
        />
      )}
      {aboutOpen && <AboutPanel onClose={() => setAboutOpen(false)} />}
    </div>
  );
}

/** Current time, refreshed every 15 s (minute precision is enough). */
function useClock(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(timer);
  }, []);
  return now;
}
