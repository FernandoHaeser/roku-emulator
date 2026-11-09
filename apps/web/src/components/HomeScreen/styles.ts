export const styles = {
  /** `@container` makes cqw units relative to the TV screen width. */
  root: "@container absolute inset-0 flex flex-col overflow-hidden bg-[radial-gradient(ellipse_at_top_left,#1e3a5f_0%,#0f172a_55%,#020617_100%)] text-white select-none",
  header: "flex items-center justify-between px-[4cqw] pt-[2.5cqw]",
  brand: "flex items-center gap-[0.8cqw] text-[1.5cqw] font-semibold tracking-wide text-sky-200",
  clock: "text-right",
  time: "text-[2.2cqw] font-light leading-none",
  date: "mt-[0.3cqw] text-[1cqw] capitalize text-slate-400",
  body: "flex flex-1 flex-col px-[4cqw] pt-[3cqw]",
  sectionTitle: "mb-[1.5cqw] text-[1.4cqw] font-medium text-slate-300",
  grid: "grid grid-cols-4 gap-[1.6cqw]",
  caption: "mt-[2.5cqw] min-h-[4cqw]",
  captionTitle: "text-[1.8cqw] font-semibold",
  captionDetail: "mt-[0.3cqw] text-[1.1cqw] text-slate-400",
  footer: "flex gap-[2.5cqw] px-[4cqw] pb-[2cqw] text-[1cqw] text-slate-400",
  footerKey:
    "mr-[0.4cqw] rounded-[0.3cqw] bg-slate-700 px-[0.5cqw] py-[0.1cqw] font-semibold text-slate-100",
} as const;
