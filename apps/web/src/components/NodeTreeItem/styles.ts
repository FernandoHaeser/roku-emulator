export const styles = {
  row: {
    base: "flex items-center gap-1 rounded py-0.5 pr-2 font-mono text-xs",
    idle: "text-neutral-300 hover:bg-neutral-800",
    selected: "bg-violet-900/60 text-white",
  },
  toggle:
    "flex h-4 w-4 items-center justify-center rounded text-neutral-400 hover:text-neutral-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500",
  spacer: "h-4 w-4",
  label:
    "flex min-w-0 flex-1 items-center gap-1.5 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500",
  type: "text-sky-300",
  id: "truncate text-neutral-400",
  focus: "rounded bg-amber-900/70 px-1 text-[10px] uppercase tracking-wide text-amber-200",
  group: "",
} as const;
