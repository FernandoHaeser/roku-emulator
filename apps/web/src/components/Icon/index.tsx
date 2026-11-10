import { styles } from "./styles";

/** Path data on a 24x24 grid, drawn with stroke="currentColor". */
const ICON_PATHS = {
  upload: "M12 16V4m0 0-4 4m4-4 4 4M4 16v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2",
  tv: "M4 6h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1Zm4 15h8M12 18v3",
  plus: "M12 5v14M5 12h14",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-5v-5m0-3h.01",
  play: "M7 5v14l11-7L7 5Z",
  trash: "M4 7h16M10 11v6m4-6v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3",
  close: "M6 6l12 12M18 6 6 18",
  chevronUp: "M6 15l6-6 6 6",
  chevronDown: "M6 9l6 6 6-6",
  chevronLeft: "M15 6l-6 6 6 6",
  chevronRight: "M9 6l6 6-6 6",
  back: "M9 14 4 9l5-5M4 9h11a5 5 0 0 1 0 10h-3",
  home: "M3 11l9-8 9 8M5 10v10h5v-6h4v6h5V10",
  replay: "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5",
  options: "M12 4v16M5.1 8l13.8 8M18.9 8 5.1 16",
  rewind: "M11 6 4 12l7 6V6Zm9 0-7 6 7 6V6Z",
  playPause: "M4 5v14l8-7-8-7Zm11 0v14m5-14v14",
  fastForward: "M13 6l7 6-7 6V6ZM4 6l7 6-7 6V6Z",
} as const;

export type IconName = keyof typeof ICON_PATHS;

interface IconProps {
  name: IconName;
  size?: keyof typeof styles.size;
  /** Accessible name; omit for decorative icons next to a text label. */
  label?: string;
}

export function Icon({ name, size = "sm", label }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={styles.size[size]}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <path d={ICON_PATHS[name]} />
    </svg>
  );
}
