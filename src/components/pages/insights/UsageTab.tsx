import React, { useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowDownRight, ArrowUpRight } from "lucide-react";
import type { Insights, InsightsRange } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useNavStore } from "../../../stores/navStore";
import { Segmented } from "../../ui/Segmented";
import { AppIcon } from "../home/AppIcon";
import { formatSpeech } from "../home/format";
import {
  TYPING_WPM,
  byCategory,
  change,
  heatLevel,
  heatWeeks,
  isoDay,
  parseDay,
  rangeStart,
  speedTrend,
  split,
  streak,
  sum,
  topApps,
  weeklySpeed,
  wordBars,
  wpm,
  type HeatCell,
} from "./insights";
import { Card, Figure, LinkRow, ShareBar } from "./parts";

interface TabProps {
  data: Insights;
  range: InsightsRange;
}

/** Whether a range starts before fillers and snippets were counted. */
export const startsBeforeCounting = (
  data: Insights,
  range: InsightsRange,
): boolean => {
  const since = data.counted_since;
  if (!since) return false;
  const start = rangeStart(range, new Date()) ?? data.days[0]?.day ?? since;
  return start < since;
};

const WordsCard: React.FC<TabProps> = ({ data, range }) => {
  const { t, i18n } = useTranslation();
  const today = new Date();
  const { current, previous } = split(data.days, range, today);
  const now = sum(current);
  const allWords = data.days.reduce((a, d) => a + d.words, 0);
  const delta = previous ? change(now.words, sum(previous).words) : null;
  const bars = wordBars(data.days, range, today);
  const max = Math.max(1, ...bars.map((b) => b.words));
  const fmt = (n: number) => n.toLocaleString(i18n.language);
  const barLabel = (start: string) =>
    parseDay(start).toLocaleDateString(i18n.language, {
      month: "short",
      day: "numeric",
    });

  return (
    <Card title={t("insights.words.title")}>
      <Figure value={now.words} locale={i18n.language} />
      <p className="mt-2 text-[13px] text-text-2">
        {t("insights.words.speech", { time: formatSpeech(now.speech_ms) })}
      </p>
      {delta !== null && (
        <p className="mt-1 inline-flex items-center gap-1 text-[13px] text-text-2">
          {delta >= 0 ? (
            <ArrowUpRight className="h-3.5 w-3.5" aria-hidden="true" />
          ) : (
            <ArrowDownRight className="h-3.5 w-3.5" aria-hidden="true" />
          )}
          {t(`insights.words.vs.${range}`, {
            change: `${delta >= 0 ? "+" : "−"}${Math.abs(delta)}%`,
          })}
        </p>
      )}
      <div
        aria-hidden="true"
        className="mt-auto flex h-12 items-end gap-[3px] pt-4"
      >
        {bars.map((b) => (
          <span
            key={b.start}
            title={t(
              range === "all"
                ? "insights.words.weekBar"
                : "insights.words.dayBar",
              { day: barLabel(b.start), count: b.words, words: fmt(b.words) },
            )}
            className="min-w-0 flex-1 rounded-t-[2px] bg-teal-300"
            style={{
              height: `${b.words ? Math.max(6, (b.words / max) * 100) : 3}%`,
              opacity: b.words ? 1 : 0.35,
            }}
          />
        ))}
      </div>
      <p className="mt-3 border-t border-line pt-3 text-[13px] text-text-2">
        {t("insights.words.allTime")}{" "}
        <span className="font-medium tabular-nums text-text">
          {fmt(allWords)}
        </span>
      </p>
    </Card>
  );
};

const SpeedCard: React.FC<TabProps> = ({ data, range }) => {
  const { t, i18n } = useTranslation();
  const today = new Date();
  const now = sum(split(data.days, range, today).current);
  const speed = wpm(now.words, now.speech_ms);
  const speeds = weeklySpeed(data.days, today);
  const trend = speedTrend(speeds);
  const known = speeds.filter((s): s is number => s !== null);
  const top = Math.max(TYPING_WPM, ...known) * 1.15;
  const W = 240;
  const H = 52;
  const x = (i: number) => (i / (speeds.length - 1)) * W;
  const y = (v: number) => H - (v / top) * H;
  const points = speeds
    .map((s, i) =>
      s === null ? null : `${x(i).toFixed(1)},${y(s).toFixed(1)}`,
    )
    .filter(Boolean)
    .join(" ");

  return (
    <Card title={t("insights.speed.title")}>
      {speed === null ? (
        <>
          <div className="text-[28px] font-semibold leading-none text-text-3">
            —
          </div>
          <p className="mt-2 text-[13px] text-text-2">
            {t("insights.speed.notYet")}
          </p>
        </>
      ) : (
        <>
          <Figure
            value={speed}
            locale={i18n.language}
            unit={t("insights.speed.unit")}
          />
          <p className="mt-2 text-[13px] text-text-2">
            {t("insights.speed.typing", {
              typing: TYPING_WPM,
              times: (speed / TYPING_WPM).toLocaleString(i18n.language, {
                maximumFractionDigits: 1,
              }),
            })}
          </p>
        </>
      )}
      {known.length >= 2 && (
        <div className="relative mt-auto pt-5">
          <svg
            viewBox={`0 0 ${W} ${H}`}
            preserveAspectRatio="none"
            className="h-[52px] w-full overflow-visible"
            role="img"
            aria-label={t("insights.speed.chart", {
              weeks: speeds.length,
            })}
          >
            <line
              x1={0}
              x2={W}
              y1={y(TYPING_WPM)}
              y2={y(TYPING_WPM)}
              stroke="var(--text-4)"
              strokeDasharray="3 3"
              vectorEffect="non-scaling-stroke"
            />
            <polyline
              points={points}
              fill="none"
              stroke="var(--teal-600)"
              strokeWidth={2}
              strokeLinejoin="round"
              strokeLinecap="round"
              vectorEffect="non-scaling-stroke"
            />
          </svg>
          <div
            aria-hidden="true"
            className="mt-1 flex justify-between text-[11px] text-text-3"
          >
            <span className="inline-flex items-center gap-1.5">
              <span className="h-0.5 w-3 rounded bg-teal-600" />
              {t("insights.speed.you")}
            </span>
            <span className="inline-flex items-center gap-1.5">
              <span className="w-3 border-t border-dashed border-text-4" />
              {t("insights.speed.typingLegend")}
            </span>
          </div>
        </div>
      )}
      {trend && (
        <p className="mt-3 border-t border-line pt-3 text-[13px] text-text-2">
          {t(`insights.speed.trend.${trend.kind}`, {
            percent: trend.percent,
            weeks: speeds.length,
          })}
        </p>
      )}
    </Card>
  );
};

const FixesCard: React.FC<TabProps & { onShowHabits: () => void }> = ({
  data,
  range,
  onShowHabits,
}) => {
  const { t, i18n } = useTranslation();
  const go = useNavStore((s) => s.go);
  const now = sum(split(data.days, range, new Date()).current);
  const fillers = data.fillers.reduce((a, f) => a + f.count, 0);
  const total = fillers + now.fixes + now.snippets;
  const fmt = (n: number) => n.toLocaleString(i18n.language);
  const since = data.counted_since;

  return (
    <Card title={t("insights.fixes.title")}>
      <Figure value={total} locale={i18n.language} />
      <div className="mt-3 flex flex-col">
        <LinkRow
          label={t("insights.fixes.fillers")}
          value={fmt(fillers)}
          onClick={onShowHabits}
        />
        <LinkRow
          label={t("insights.fixes.dictionary")}
          value={fmt(now.fixes)}
          onClick={() => go("dictionary")}
        />
        <LinkRow
          label={t("insights.fixes.snippets")}
          value={fmt(now.snippets)}
          onClick={() => go("snippets")}
        />
      </div>
      <div className="mt-auto flex flex-col border-t border-line pt-2">
        <LinkRow
          label={t("insights.fixes.transforms")}
          value={fmt(data.transforms)}
          onClick={() => go("transforms")}
        />
      </div>
      {since && startsBeforeCounting(data, range) && (
        <p className="mt-2 text-xs text-text-3">
          {t("insights.countedSince", {
            date: parseDay(since).toLocaleDateString(i18n.language, {
              month: "long",
              day: "numeric",
            }),
          })}
        </p>
      )}
    </Card>
  );
};

const WhereCard: React.FC<TabProps> = ({ data }) => {
  const { t, i18n } = useTranslation();
  const { settings } = useSettings();
  const goTo = useNavStore((s) => s.goTo);
  const go = useNavStore((s) => s.go);
  const [view, setView] = useState<"apps" | "categories">("apps");
  const total = data.apps.reduce((a, x) => a + x.words, 0);
  const fmt = (n: number) => n.toLocaleString(i18n.language);
  const pct = (n: number) =>
    `${Math.round((n / Math.max(total, 1)) * 100).toLocaleString(i18n.language)}%`;
  const remembers = settings?.remember_app ?? true;
  const { top, rest } = topApps(data.apps);
  const [showRest, setShowRest] = useState(false);
  const rows = showRest ? data.apps : top;

  const row = (
    key: string,
    icon: React.ReactNode,
    label: string,
    words: number,
    onClick?: () => void,
  ) => {
    const content = (
      <>
        <span className="flex min-w-0 flex-[1.2] items-center gap-2.5">
          {icon}
          <span className="truncate">{label}</span>
        </span>
        <span className="hidden flex-1 min-[520px]:block">
          <ShareBar share={words / Math.max(total, 1)} />
        </span>
        <span className="w-[92px] flex-none text-end tabular-nums text-text-2">
          {t("insights.where.words", { count: words, words: fmt(words) })}
        </span>
        <span className="w-10 flex-none text-end tabular-nums font-medium">
          {pct(words)}
        </span>
      </>
    );
    return onClick ? (
      <li key={key}>
        <button
          type="button"
          onClick={onClick}
          title={t("insights.where.open", { app: label })}
          className="-mx-2 flex h-9 w-[calc(100%+16px)] items-center gap-3 rounded-[var(--r-sm)] px-2 text-start text-sm transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0"
        >
          {content}
        </button>
      </li>
    ) : (
      <li key={key} className="flex h-9 items-center gap-3 text-sm">
        {content}
      </li>
    );
  };

  return (
    <Card
      title={t("insights.where.title")}
      aside={
        remembers && data.apps.length > 0 ? (
          <Segmented
            label={t("insights.where.title")}
            value={view}
            onChange={setView}
            options={[
              { value: "apps", label: t("insights.where.apps") },
              { value: "categories", label: t("insights.where.categories") },
            ]}
          />
        ) : null
      }
    >
      {!remembers ? (
        <div className="flex flex-1 flex-col items-start justify-center gap-2 py-6">
          <p className="text-[13px] text-text-2">{t("insights.where.off")}</p>
          <button
            type="button"
            onClick={() => go("settings", "privacy")}
            className="text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
          >
            {t("insights.where.openPrivacy")}
          </button>
        </div>
      ) : data.apps.length === 0 ? (
        <p className="py-6 text-[13px] text-text-2">
          {t("insights.where.none")}
        </p>
      ) : view === "apps" ? (
        <ul className="flex flex-col">
          {rows.map((a) =>
            row(
              a.name,
              <AppIcon path={a.path} className="h-[18px] w-[18px] flex-none" />,
              a.name,
              a.words,
              () => goTo("home", `app:${a.name}`),
            ),
          )}
          {rest && !showRest && (
            <li>
              <button
                type="button"
                onClick={() => setShowRest(true)}
                className="-mx-2 flex h-9 w-[calc(100%+16px)] items-center gap-3 rounded-[var(--r-sm)] px-2 text-start text-sm text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0"
              >
                <span className="flex min-w-0 flex-[1.2] items-center gap-2.5">
                  <span className="h-[18px] w-[18px] flex-none" />
                  <span className="truncate">
                    {t("insights.where.other", { count: rest.count })}
                  </span>
                </span>
                <span className="hidden flex-1 min-[520px]:block">
                  <ShareBar share={rest.words / Math.max(total, 1)} />
                </span>
                <span className="w-[92px] flex-none text-end tabular-nums">
                  {t("insights.where.words", {
                    count: rest.words,
                    words: fmt(rest.words),
                  })}
                </span>
                <span className="w-10 flex-none text-end tabular-nums font-medium text-text">
                  {pct(rest.words)}
                </span>
              </button>
            </li>
          )}
        </ul>
      ) : (
        <ul className="flex flex-col">
          {byCategory(data.apps).map((c) =>
            row(
              c.category,
              <span
                aria-hidden="true"
                className="h-2 w-2 flex-none rounded-full bg-teal-600"
              />,
              t(`style.categories.${c.category}`, {
                defaultValue: t("style.categories.other"),
              }),
              c.words,
            ),
          )}
        </ul>
      )}
    </Card>
  );
};

const LEVEL_CLASS = [
  "bg-surface-3",
  "bg-teal-100",
  "bg-teal-300",
  "bg-teal-500",
  "bg-teal-700",
];

const ActivityCard: React.FC<TabProps> = ({ data }) => {
  const { t, i18n } = useTranslation();
  const goTo = useNavStore((s) => s.goTo);
  const today = useMemo(() => new Date(), []);
  const weeks = useMemo(() => heatWeeks(data.days, today), [data.days, today]);
  const todayIso = isoDay(today);
  const [selected, setSelected] = useState(todayIso);
  const gridRef = useRef<HTMLDivElement>(null);
  const days = streak(data.days, today);
  const cells = weeks.flat();
  const chosen =
    cells.find((c) => c.day === selected) ?? cells[cells.length - 1];
  const fmt = (n: number) => n.toLocaleString(i18n.language);
  const dayText = (day: string) =>
    day === todayIso
      ? t("home.day.today")
      : parseDay(day).toLocaleDateString(i18n.language, {
          weekday: "short",
          month: "short",
          day: "numeric",
        });
  const cellLabel = (c: HeatCell) =>
    t("insights.activity.cell", {
      day: dayText(c.day),
      count: c.words,
      words: fmt(c.words),
      dictations: c.dictations,
    });

  const months = weeks.map((week, i) => {
    const month = parseDay(week[0].day).getMonth();
    const before = i > 0 ? parseDay(weeks[i - 1][0].day).getMonth() : -1;
    return month !== before && i < weeks.length - 1
      ? parseDay(week[0].day).toLocaleDateString(i18n.language, {
          month: "short",
        })
      : "";
  });

  const move = (day: string) => {
    setSelected(day);
    requestAnimationFrame(() =>
      gridRef.current
        ?.querySelector<HTMLElement>(`[data-day="${day}"]`)
        ?.focus(),
    );
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const i = cells.findIndex((c) => c.day === chosen.day);
    const step =
      {
        ArrowLeft: -7,
        ArrowRight: 7,
        ArrowUp: -1,
        ArrowDown: 1,
        Home: -i,
        End: cells.findIndex((c) => c.day === todayIso) - i,
      }[e.key] ?? 0;
    if (!step) return;
    e.preventDefault();
    const next = cells[Math.min(Math.max(i + step, 0), cells.length - 1)];
    if (next && !next.future) move(next.day);
  };

  return (
    <Card
      title={t("insights.activity.title")}
      aside={
        days >= 2 ? (
          <span className="text-[13px] text-text-2">
            {t("insights.activity.streak", { count: days })}
          </span>
        ) : null
      }
    >
      <div className="overflow-x-auto">
        <div className="grid min-w-[300px] max-w-[520px] grid-cols-[auto_1fr] gap-x-2">
          <span />
          <div
            aria-hidden="true"
            className="grid grid-flow-col gap-[3px] pb-1 text-[11px] text-text-3"
            style={{
              gridTemplateColumns: `repeat(${weeks.length}, minmax(12px, 1fr))`,
            }}
          >
            {months.map((m, i) => (
              <span key={i} className="whitespace-nowrap">
                {m}
              </span>
            ))}
          </div>
          <div
            aria-hidden="true"
            className="grid grid-rows-7 gap-[3px] text-[11px] text-text-3"
          >
            {[0, 1, 2, 3, 4, 5, 6].map((d) => (
              <span key={d} className="flex items-center">
                {d % 2 === 1
                  ? new Date(2026, 9, 11 + d).toLocaleDateString(
                      i18n.language,
                      { weekday: "short" },
                    )
                  : ""}
              </span>
            ))}
          </div>
          <div
            ref={gridRef}
            role="group"
            aria-label={t("insights.activity.title")}
            onKeyDown={onKeyDown}
            className="grid grid-flow-col grid-rows-7 gap-[3px]"
            style={{
              gridTemplateColumns: `repeat(${weeks.length}, minmax(12px, 1fr))`,
            }}
          >
            {cells.map((c) =>
              c.future ? (
                <span
                  key={c.day}
                  aria-hidden="true"
                  className="aspect-square"
                />
              ) : (
                <button
                  key={c.day}
                  type="button"
                  data-day={c.day}
                  tabIndex={c.day === chosen.day ? 0 : -1}
                  aria-label={cellLabel(c)}
                  aria-pressed={c.day === chosen.day}
                  title={cellLabel(c)}
                  onClick={() => setSelected(c.day)}
                  onDoubleClick={() => goTo("home", `day:${c.day}`)}
                  className={`aspect-square w-full rounded-[3px] ${LEVEL_CLASS[heatLevel(c.words)]} ${
                    c.day === chosen.day
                      ? "outline outline-2 outline-offset-1 outline-text-2"
                      : ""
                  }`}
                />
              ),
            )}
          </div>
        </div>
      </div>
      <div
        aria-hidden="true"
        className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-text-3"
      >
        {["0", "1–249", "250–499", "500–749", "750+"].map((label, i) => (
          <span key={label} className="inline-flex items-center gap-1.5">
            <span className={`h-2.5 w-2.5 rounded-[2px] ${LEVEL_CLASS[i]}`} />
            {label}
          </span>
        ))}
        <span>{t("insights.activity.legend")}</span>
      </div>
      <div className="mt-auto flex flex-wrap items-center justify-between gap-2 border-t border-line pt-3 text-[13px]">
        <span className="text-text-2" aria-live="polite">
          <span className="font-medium text-text">{dayText(chosen.day)}</span>{" "}
          {chosen.dictations
            ? t("insights.activity.summary", {
                count: chosen.dictations,
                words: fmt(chosen.words),
              })
            : t("insights.activity.nothing")}
        </span>
        {chosen.dictations > 0 && (
          <button
            type="button"
            onClick={() => goTo("home", `day:${chosen.day}`)}
            className="text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
          >
            {t("insights.activity.open")}
          </button>
        )}
      </div>
    </Card>
  );
};

/** Words, speed and fixes, where the words go, and the days you dictated. */
export const UsageTab: React.FC<TabProps & { onShowHabits: () => void }> = ({
  data,
  range,
  onShowHabits,
}) => (
  <div className="flex flex-col gap-4">
    <div className="grid grid-cols-1 gap-4 min-[960px]:grid-cols-3">
      <WordsCard data={data} range={range} />
      <SpeedCard data={data} range={range} />
      <FixesCard data={data} range={range} onShowHabits={onShowHabits} />
    </div>
    <div className="grid grid-cols-1 gap-4 min-[1100px]:grid-cols-[minmax(0,1.15fr)_minmax(0,1fr)]">
      <WhereCard data={data} range={range} />
      <ActivityCard data={data} range={range} />
    </div>
  </div>
);
