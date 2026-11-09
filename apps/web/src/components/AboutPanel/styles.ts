export const styles = {
  backdrop: "absolute inset-0 z-20 flex items-center justify-center bg-black/60",
  panel:
    "w-[50cqw] rounded-[1cqw] bg-slate-900/95 px-[3cqw] py-[2.5cqw] text-slate-200 shadow-[0_1cqw_4cqw_rgba(0,0,0,0.6)]",
  title: "mb-[1.5cqw] text-[1.9cqw] font-semibold text-white",
  row: "contents",
  list: "grid grid-cols-[auto_1fr] gap-x-[2cqw] gap-y-[0.7cqw] text-[1.25cqw]",
  term: "text-slate-400",
  value: "font-medium text-white",
  hint: "mt-[2cqw] text-[1cqw] text-slate-400",
} as const;
