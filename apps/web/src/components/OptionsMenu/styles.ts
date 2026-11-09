export const styles = {
  backdrop: "absolute inset-0 z-20 flex justify-end bg-black/50",
  panel:
    "flex h-full w-[30cqw] flex-col gap-[0.6cqw] bg-slate-900/95 px-[2cqw] py-[3cqw] shadow-[-1cqw_0_3cqw_rgba(0,0,0,0.5)]",
  title: "mb-[1cqw] truncate text-[1.6cqw] font-semibold text-white",
  item: {
    base: "flex items-center gap-[0.8cqw] rounded-[0.5cqw] px-[1cqw] py-[0.8cqw] text-left text-[1.3cqw] text-slate-300",
    focused: "bg-white text-slate-900",
  },
} as const;
