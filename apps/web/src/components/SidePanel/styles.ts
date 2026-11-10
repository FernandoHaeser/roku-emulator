export const styles = {
  panel:
    "flex h-[60vh] flex-col overflow-hidden rounded-lg border border-neutral-800 bg-neutral-900 lg:h-auto",
  tabList: "flex border-b border-neutral-800 px-2",
  tab: {
    base: "border-b-2 px-3 py-2 text-xs font-semibold uppercase tracking-wider focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500",
    active: "border-violet-500 text-neutral-100",
    idle: "border-transparent text-neutral-500 hover:text-neutral-300",
  },
  // `hidden` must win over the flex display.
  tabPanel: "flex min-h-0 flex-1 flex-col [&[hidden]]:hidden",
} as const;
