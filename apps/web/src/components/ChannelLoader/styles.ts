export const styles = {
  root: "flex items-center gap-3",
  button:
    "inline-flex cursor-pointer items-center gap-2 rounded-md bg-sky-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-sky-500 focus-within:outline focus-within:outline-2 focus-within:outline-sky-300",
  input: "sr-only",
  channel: "max-w-56 truncate text-xs text-neutral-400",
  status: {
    base: "rounded px-2 py-0.5 text-xs font-medium",
    idle: "hidden",
    running: "bg-sky-900/60 text-sky-300",
    sleeping: "bg-sky-900/60 text-sky-300",
    finished: "bg-neutral-800 text-neutral-300",
    crashed: "bg-red-900/60 text-red-300",
    stopped: "bg-amber-900/60 text-amber-300",
    error: "bg-red-900/60 text-red-300",
  },
} as const;
