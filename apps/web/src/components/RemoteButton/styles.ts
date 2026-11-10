export const styles = {
  base: "flex select-none items-center justify-center text-neutral-200 transition-colors hover:bg-neutral-700 active:bg-violet-700 active:text-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 touch-none",
  variant: {
    round: "h-9 w-9 rounded-full bg-neutral-800",
    pad: "h-9 w-9 rounded-md bg-neutral-800",
    ok: "h-9 w-9 rounded-full bg-violet-800 text-white",
  },
  text: "text-xs font-semibold",
} as const;
