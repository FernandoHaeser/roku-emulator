export const styles = {
  panel:
    "flex h-[60vh] flex-col overflow-hidden rounded-lg border border-neutral-800 bg-neutral-900 lg:h-auto",
  toolbar: "flex items-center justify-between border-b border-neutral-800 px-3 py-2",
  heading: "text-xs font-semibold uppercase tracking-wider text-neutral-400",
  clearButton:
    "rounded px-2 py-1 text-xs text-neutral-300 hover:bg-neutral-800 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500",
  output: "flex-1 overflow-y-auto p-3 font-mono text-xs leading-5",
  // min-h keeps blank lines visible (the debugger dump relies on them).
  line: {
    stdout: "min-h-5 whitespace-pre-wrap text-neutral-100",
    system: "min-h-5 whitespace-pre-wrap text-neutral-500",
    error: "min-h-5 whitespace-pre-wrap text-red-400",
  },
  empty: "text-neutral-600",
} as const;
