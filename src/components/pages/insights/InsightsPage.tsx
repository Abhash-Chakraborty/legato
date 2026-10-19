import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  events,
  type Insights,
  type InsightsRange,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { formatKeyCombination } from "../../../lib/utils/keyboard";
import { Segmented } from "../../ui/Segmented";
import { parseDay } from "./insights";
import { UsageTab } from "./UsageTab";
import { HabitsTab } from "./HabitsTab";
import { AccuracyTab } from "./AccuracyTab";

type Tab = "usage" | "habits" | "accuracy";
const TABS: Tab[] = ["usage", "habits", "accuracy"];
const RANGES: InsightsRange[] = ["week", "month", "all"];
const RANGE_KEY = "legato.insights.range";

const readRange = (): InsightsRange => {
  try {
    const saved = localStorage.getItem(RANGE_KEY);
    return RANGES.includes(saved as InsightsRange)
      ? (saved as InsightsRange)
      : "month";
  } catch {
    return "month";
  }
};

/** How you use Legato, from the daily totals kept on this PC. */
export const InsightsPage: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { settings } = useSettings();
  const [range, setRange] = useState<InsightsRange>(readRange);
  const [tab, setTab] = useState<Tab>("usage");
  const [data, setData] = useState<Insights | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const request = useRef(0);

  const load = useCallback(async (r: InsightsRange) => {
    const token = ++request.current;
    const result = await commands.getInsights(r).catch((e) => ({
      status: "error" as const,
      error: String(e),
    }));
    if (token !== request.current) return;
    if (result.status === "ok") {
      setData(result.data);
      setFailed(null);
    } else {
      setFailed(result.error);
    }
  }, []);

  useEffect(() => {
    load(range);
    try {
      localStorage.setItem(RANGE_KEY, range);
    } catch {
      // The range just isn't remembered.
    }
  }, [range, load]);

  // A new dictation updates the totals.
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unlisten = events.historyUpdatePayload.listen(() => {
      clearTimeout(timer);
      timer = setTimeout(() => load(range), 400);
    });
    return () => {
      clearTimeout(timer);
      unlisten.then((u) => u());
    };
  }, [range, load]);

  const onTabKey = (e: React.KeyboardEvent) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = TABS[(TABS.indexOf(tab) + step + TABS.length) % TABS.length];
    setTab(next);
    requestAnimationFrame(() =>
      document.getElementById(`insights-tab-${next}`)?.focus(),
    );
  };

  const all = data?.days ?? [];
  const dictations = all.reduce((a, d) => a + d.dictations, 0);
  const first = all[0]?.day;
  const shortcut = formatKeyCombination(
    settings?.bindings?.transcribe?.current_binding ?? "",
  );

  return (
    <div className="max-w-[1180px]">
      <div className="mb-6 flex min-h-10 flex-wrap items-center justify-between gap-4">
        <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
          {t("insights.title")}
        </h1>
        {dictations > 0 && (
          <Segmented
            label={t("insights.range.label")}
            value={range}
            onChange={setRange}
            options={RANGES.map((r) => ({
              value: r,
              label: t(`insights.range.${r}`),
            }))}
          />
        )}
      </div>
      <p className="-mt-3.5 mb-5 max-w-[680px] text-sm text-text-2">
        {first
          ? t("insights.subtitle", {
              count: dictations,
              formatted: dictations.toLocaleString(i18n.language),
              date: parseDay(first).toLocaleDateString(i18n.language, {
                month: "long",
                day: "numeric",
                year: "numeric",
              }),
            })
          : t("insights.subtitleEmpty")}
      </p>

      {failed ? (
        <p role="alert" className="text-sm text-danger">
          {t("insights.failed", { error: failed })}
        </p>
      ) : !data ? null : dictations === 0 ? (
        <div className="grid place-items-center gap-1.5 rounded-[var(--r-lg)] border border-line bg-surface px-6 py-16 text-center">
          <h2 className="text-[15px] font-medium">
            {t("insights.empty.title")}
          </h2>
          <p className="max-w-[420px] text-[13px] text-text-3">
            {shortcut
              ? t("insights.empty.body", { shortcut })
              : t("insights.empty.bodyNoShortcut")}
          </p>
        </div>
      ) : (
        <>
          <div
            role="tablist"
            aria-label={t("insights.title")}
            onKeyDown={onTabKey}
            className="mb-5 flex gap-5 overflow-x-auto border-b border-line"
          >
            {TABS.map((id) => (
              <button
                key={id}
                id={`insights-tab-${id}`}
                type="button"
                role="tab"
                aria-selected={tab === id}
                aria-controls="insights-panel"
                tabIndex={tab === id ? 0 : -1}
                onClick={() => setTab(id)}
                className={`-mb-px flex-none border-b-2 pb-2.5 text-sm whitespace-nowrap transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                  tab === id
                    ? "border-teal-600 font-medium text-text"
                    : "border-transparent text-text-2 hover:text-text"
                }`}
              >
                {t(`insights.tabs.${id}`)}
              </button>
            ))}
          </div>
          <div
            id="insights-panel"
            role="tabpanel"
            aria-labelledby={`insights-tab-${tab}`}
            className="pb-6"
          >
            {tab === "usage" ? (
              <UsageTab
                data={data}
                range={range}
                onShowHabits={() => setTab("habits")}
              />
            ) : tab === "habits" ? (
              <HabitsTab data={data} range={range} />
            ) : (
              <AccuracyTab data={data} range={range} />
            )}
          </div>
        </>
      )}
    </div>
  );
};
