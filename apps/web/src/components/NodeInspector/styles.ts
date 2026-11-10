export const styles = {
  inspector: "flex min-h-0 flex-1 flex-col",
  toolbar: "border-b border-neutral-800 p-2",
  filter:
    "w-full rounded bg-neutral-800 px-2 py-1 text-xs text-neutral-100 placeholder:text-neutral-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500",
  error: "px-3 py-2 text-xs text-red-400",
  empty: "px-3 py-2 text-xs text-neutral-600",
  tree: "min-h-0 flex-1 overflow-auto p-2",
  details: "max-h-[45%] min-h-0 overflow-auto border-t border-neutral-800 p-2",
  detailsTitle: "mb-2 flex flex-wrap gap-x-2 font-mono text-[11px] text-neutral-400",
  bounds: "text-neutral-500",
  table: "w-full border-collapse font-mono text-[11px]",
  fieldName: "w-1/3 py-0.5 pr-2 text-left align-top font-normal text-sky-300",
  fieldValue: "break-all py-0.5 text-neutral-200",
} as const;
