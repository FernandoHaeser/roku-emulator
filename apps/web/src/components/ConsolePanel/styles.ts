export const styles = {
  panel: "flex min-h-0 flex-1 flex-col",
  toolbar: "flex items-center justify-end border-b border-neutral-800 px-3 py-1",
  // The tab already names the panel; the heading stays for screen readers.
  heading: "sr-only",
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
