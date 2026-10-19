import React, { useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search } from "lucide-react";
import type { TransformRun } from "@/bindings";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import { AppIcon } from "../home/AppIcon";
import { dayLabel, localDay } from "../home/format";
import {
  filterCounts,
  formatRunTime,
  inFilter,
  RUN_FILTERS,
  runMatches,
  type RunFilter,
} from "./list";
import { KeyCaps } from "./TransformCard";

const RESULT_DOT: Record<TransformRun["result"], string> = {
  kept: "bg-ok",
  copied: "bg-ok",
  undone: "bg-text-4",
  failed: "bg-danger",
};

const groupRuns = (runs: TransformRun[]) => {
  const groups: { day: string; runs: TransformRun[] }[] = [];
  for (const run of runs) {
    const day = localDay(run.at);
    const last = groups[groups.length - 1];
    if (last && last.day === day) last.runs.push(run);
    else groups.push({ day, runs: [run] });
  }
  return groups;
};

/** What each transform did lately: kept, undone or failed, and where. */
export const RecentRuns: React.FC<{
  runs: TransformRun[];
  viewKeys: string[];
}> = ({ runs, viewKeys }) => {
  const { t, i18n } = useTranslation();
  const [filter, setFilter] = useState<RunFilter>("all");
  const [query, setQuery] = useState("");
  const listRef = useRef<HTMLDivElement>(null);

  const counts = useMemo(() => filterCounts(runs), [runs]);
  const shown = useMemo(
    () => runs.filter((r) => inFilter(r, filter) && runMatches(r, query)),
    [runs, filter, query],
  );
  const groups = useMemo(() => groupRuns(shown), [shown]);
  const now = new Date();

  useTextHighlight(
    listRef,
    "runs-search",
    query.trim() ? [query.trim()] : [],
    "[data-run-text]",
  );

  const day = (key: string) => {
    const label = dayLabel(key, now, i18n.language);
    return label.kind === "date" ? label.text : t(`home.day.${label.kind}`);
  };

  const meta = (run: TransformRun) => {
    const parts: string[] = [];
    if (run.source !== "selection")
      parts.push(t(`transforms.runs.source.${run.source}`));
    if (run.result === "failed")
      parts.push(t("transforms.runs.unchanged", { count: run.in_words }));
    else {
      parts.push(
        t("transforms.runs.words", {
          from: run.in_words,
          count: run.out_words,
        }),
      );
      parts.push(formatRunTime(run.ms));
    }
    return parts;
  };

  return (
    <section className="mb-7">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-[17px] font-semibold tracking-[-0.01em]">
          {t("transforms.runs.title")}
        </h2>
        {viewKeys.length > 0 && (
          <span className="inline-flex items-center gap-1 rounded-[var(--r-sm)] bg-surface-2 py-1 ps-1.5 pe-2.5 text-[13px] text-text-2">
            <KeyCaps keys={viewKeys} />
            <span className="ms-1">{t("transforms.runs.viewChip")}</span>
          </span>
        )}
      </div>

      {runs.length === 0 ? (
        <div className="grid place-items-center gap-1.5 rounded-[var(--r-lg)] border border-line px-6 py-12 text-center">
          <h3 className="text-[15px] font-medium">
            {t("transforms.runs.empty.title")}
          </h3>
          <p className="max-w-[440px] text-[13px] text-text-3">
            {t("transforms.runs.empty.body")}
          </p>
        </div>
      ) : (
        <>
          <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
            <div className="relative w-[260px]">
              <Search
                className="pointer-events-none absolute start-[10px] top-1/2 h-[14px] w-[14px] -translate-y-1/2 text-text-3"
                aria-hidden="true"
              />
              <input
                type="search"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape" && query) {
                    e.preventDefault();
                    e.stopPropagation();
                    setQuery("");
                  }
                }}
                placeholder={t("transforms.runs.search")}
                aria-label={t("transforms.runs.search")}
                className="input compact w-full ps-[30px] [&::-webkit-search-cancel-button]:hidden"
              />
            </div>
            <div
              role="tablist"
              aria-label={t("transforms.runs.filter")}
              className="flex rounded-[var(--r-md)] bg-surface-2 p-0.5"
            >
              {RUN_FILTERS.map((f) => (
                <button
                  key={f}
                  type="button"
                  role="tab"
                  aria-selected={filter === f}
                  onClick={() => setFilter(f)}
                  className={`inline-flex items-center gap-1.5 rounded-[6px] px-2.5 py-1 text-[13px] transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                    filter === f
                      ? "bg-surface font-medium text-text shadow-[var(--shadow-1)]"
                      : "text-text-2 hover:text-text"
                  }`}
                >
                  {t(`transforms.runs.filters.${f}`)}
                  <span className="tabular-nums text-xs text-text-3">
                    {counts[f]}
                  </span>
                </button>
              ))}
            </div>
          </div>

          {shown.length === 0 ? (
            <div className="grid place-items-center gap-1.5 px-6 py-12 text-center">
              <h3 className="text-[15px] font-medium">
                {query.trim()
                  ? t("transforms.runs.noResults.titleSearch", {
                      query: query.trim(),
                    })
                  : t("transforms.runs.noResults.title")}
              </h3>
              <button
                type="button"
                onClick={() => {
                  setQuery("");
                  setFilter("all");
                }}
                className="mt-1 text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
              >
                {t("transforms.runs.noResults.clear")}
              </button>
            </div>
          ) : (
            <div ref={listRef}>
              {groups.map((group) => (
                <div key={group.day} className="mb-4">
                  <h3 className="mb-1.5 ps-0.5 text-[13px] text-text-3">
                    {day(group.day)}
                  </h3>
                  <ul className="divide-y divide-line overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface">
                    {group.runs.map((run) => (
                      <li key={run.id} className="flex gap-4 px-4 py-3">
                        <span className="w-[68px] flex-none pt-px text-[13px] tabular-nums text-text-3">
                          {new Date(run.at * 1000).toLocaleTimeString(
                            i18n.language,
                            { hour: "numeric", minute: "2-digit" },
                          )}
                        </span>
                        <div className="min-w-0 flex-1">
                          <p
                            className="text-sm font-medium text-text"
                            data-run-text
                          >
                            {run.name}
                          </p>
                          <p className="mt-0.5 flex min-w-0 flex-wrap items-center gap-x-2 text-[13px] text-text-3">
                            {run.app_name && (
                              <span className="inline-flex min-w-0 items-center gap-1.5">
                                <AppIcon
                                  path={run.app_path}
                                  className="h-3.5 w-3.5"
                                />
                                <span className="truncate" data-run-text>
                                  {run.app_name}
                                </span>
                              </span>
                            )}
                            {meta(run).map((part, i) => (
                              <React.Fragment key={i}>
                                {(i > 0 || run.app_name) && (
                                  <span aria-hidden="true">·</span>
                                )}
                                <span className="tabular-nums">{part}</span>
                              </React.Fragment>
                            ))}
                          </p>
                          {run.error && (
                            <p
                              className="mt-1 text-[13px] text-danger"
                              data-run-text
                            >
                              {t("transforms.runs.error", {
                                error: run.error,
                              })}
                            </p>
                          )}
                        </div>
                        <span className="inline-flex flex-none items-center gap-1.5 self-start pt-px text-[13px] text-text-2">
                          <span
                            aria-hidden="true"
                            className={`h-2 w-2 rounded-full ${RESULT_DOT[run.result]}`}
                          />
                          {t(`transforms.runs.result.${run.result}`)}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </section>
  );
};
