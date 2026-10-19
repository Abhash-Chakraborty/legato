import type { TransformRun, TransformUsage } from "@/bindings";

export const RUN_FILTERS = ["all", "kept", "undone", "failed"] as const;
export type RunFilter = (typeof RUN_FILTERS)[number];

/** Copied results count as kept: nothing was undone. */
export const inFilter = (run: TransformRun, filter: RunFilter): boolean => {
  switch (filter) {
    case "all":
      return true;
    case "kept":
      return run.result === "kept" || run.result === "copied";
    default:
      return run.result === filter;
  }
};

export const runMatches = (run: TransformRun, query: string): boolean => {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [run.name, run.app_name ?? "", run.error ?? ""].some((field) =>
    field.toLowerCase().includes(q),
  );
};

export const filterCounts = (runs: TransformRun[]): Record<RunFilter, number> =>
  Object.fromEntries(
    RUN_FILTERS.map((f) => [f, runs.filter((r) => inFilter(r, f)).length]),
  ) as Record<RunFilter, number>;

export const usageOf = (
  usage: TransformUsage[] | undefined,
  id: string,
): TransformUsage | undefined => usage?.find((u) => u.transform_id === id);

/** Seconds with one decimal, as runs are short: "1.2 s". */
export const formatRunTime = (ms: number): string =>
  `${(Math.round(ms / 100) / 10).toFixed(1)} s`;

/** The keys of a binding as separate caps, for display. */
export const keyCaps = (formatted: string): string[] =>
  formatted.split(" + ").filter(Boolean);
