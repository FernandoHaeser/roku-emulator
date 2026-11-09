export const styles = {
  root: "flex min-h-screen flex-col bg-neutral-950 text-neutral-100",
  header: "flex items-center justify-between border-b border-neutral-800 px-4 py-3",
  title: "text-sm font-semibold tracking-wide",
  status: {
    base: "rounded px-2 py-0.5 text-xs font-medium",
    starting: "bg-neutral-800 text-neutral-300",
    ready: "bg-emerald-900/60 text-emerald-300",
    failed: "bg-red-900/60 text-red-300",
  },
  main: "grid flex-1 gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_420px]",
} as const;
