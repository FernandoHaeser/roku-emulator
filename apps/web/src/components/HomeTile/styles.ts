export const styles = {
  tile: {
    base: "relative flex aspect-video flex-col items-center justify-center gap-[0.8cqw] overflow-hidden rounded-[0.8cqw] bg-slate-800/80 text-slate-200 transition-transform duration-150 ease-out outline-none",
    focused:
      "z-10 scale-[1.06] bg-slate-700 text-white ring-[0.25cqw] ring-white shadow-[0_0_2.5cqw_rgba(125,211,252,0.45)]",
  },
  image: "absolute inset-0 h-full w-full object-cover",
  label: "px-[1cqw] text-center text-[1.3cqw] font-medium leading-tight",
  badge:
    "absolute top-[0.6cqw] left-[0.6cqw] rounded-[0.3cqw] bg-amber-400 px-[0.5cqw] py-[0.1cqw] text-[0.9cqw] font-bold tracking-wider text-slate-900",
} as const;
