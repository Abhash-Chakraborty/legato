import type { InsightsApp, InsightsDay, InsightsRange } from "@/bindings";

/** Days each range covers, ending today; All has no start. */
export const RANGE_DAYS: Record<InsightsRange, number | null> = {
  week: 7,
  month: 30,
  all: null,
};

/** People type about this fast; Speed compares against it. */
export const TYPING_WPM = 40;

/** Local date as YYYY-MM-DD. */
export const isoDay = (d: Date): string =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
    d.getDate(),
  ).padStart(2, "0")}`;

export const parseDay = (day: string): Date => {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
};

export const addDays = (d: Date, n: number): Date =>
  new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);

export interface Totals {
  dictations: number;
  words: number;
  speech_ms: number;
  fixes: number;
  transformed: number;
  snippets: number;
}

export const sum = (days: InsightsDay[]): Totals =>
  days.reduce(
    (t, d) => ({
      dictations: t.dictations + d.dictations,
      words: t.words + d.words,
      speech_ms: t.speech_ms + d.speech_ms,
      fixes: t.fixes + d.fixes,
      transformed: t.transformed + d.transformed,
      snippets: t.snippets + d.snippets,
    }),
    {
      dictations: 0,
      words: 0,
      speech_ms: 0,
      fixes: 0,
      transformed: 0,
      snippets: 0,
    },
  );

/** The first day of a range ending today, or null for All. */
export const rangeStart = (
  range: InsightsRange,
  today: Date,
): string | null => {
  const span = RANGE_DAYS[range];
  return span === null ? null : isoDay(addDays(today, 1 - span));
};

/** The days in the range, and for 7 and 30 days the same span before it. */
export const split = (
  days: InsightsDay[],
  range: InsightsRange,
  today: Date,
): { current: InsightsDay[]; previous: InsightsDay[] | null } => {
  const span = RANGE_DAYS[range];
  if (span === null) return { current: days, previous: null };
  const start = isoDay(addDays(today, 1 - span));
  const before = isoDay(addDays(today, 1 - 2 * span));
  return {
    current: days.filter((d) => d.day >= start),
    previous: days.filter((d) => d.day >= before && d.day < start),
  };
};

/** Words a minute of speech, once there are ten seconds of it. */
export const wpm = (words: number, speechMs: number): number | null =>
  speechMs >= 10_000 ? Math.round(words / (speechMs / 60_000)) : null;

/** Change against before, in percent; null when there was nothing before. */
export const change = (now: number, before: number): number | null =>
  before > 0 ? Math.round(((now - before) / before) * 100) : null;

/** Days in a row with a dictation, ending today, or yesterday when today
 *  has none yet. */
export const streak = (days: InsightsDay[], today: Date): number => {
  const active = new Set(
    days.filter((d) => d.dictations > 0).map((d) => d.day),
  );
  let cursor = active.has(isoDay(today)) ? today : addDays(today, -1);
  let count = 0;
  while (active.has(isoDay(cursor))) {
    count += 1;
    cursor = addDays(cursor, -1);
  }
  return count;
};

export interface HeatCell {
  day: string;
  words: number;
  dictations: number;
  /** After today, drawn empty. */
  future: boolean;
}

/** The activity map: whole weeks, Sunday first, ending with this week. */
export const heatWeeks = (
  days: InsightsDay[],
  today: Date,
  weeks = 18,
): HeatCell[][] => {
  const byDay = new Map(days.map((d) => [d.day, d]));
  const first = addDays(today, -today.getDay() - 7 * (weeks - 1));
  const todayIso = isoDay(today);
  return Array.from({ length: weeks }, (_, w) =>
    Array.from({ length: 7 }, (_, d) => {
      const day = isoDay(addDays(first, w * 7 + d));
      const row = byDay.get(day);
      return {
        day,
        words: row?.words ?? 0,
        dictations: row?.dictations ?? 0,
        future: day > todayIso,
      };
    }),
  );
};

/** Shade for a day's words: 0 for none, then 1 to 4. */
export const heatLevel = (words: number): 0 | 1 | 2 | 3 | 4 =>
  words <= 0 ? 0 : words < 250 ? 1 : words < 500 ? 2 : words < 750 ? 3 : 4;

/** Words a minute in each of the last `weeks` weeks, oldest first; null for
 *  a week with too little speech to say. */
export const weeklySpeed = (
  days: InsightsDay[],
  today: Date,
  weeks = 12,
): (number | null)[] =>
  Array.from({ length: weeks }, (_, i) => {
    const end = isoDay(addDays(today, -7 * (weeks - 1 - i)));
    const start = isoDay(addDays(today, -7 * (weeks - 1 - i) - 6));
    const t = sum(days.filter((d) => d.day >= start && d.day <= end));
    return wpm(t.words, t.speech_ms);
  });

/** Whether speed went up, down or held over the weeks that have one: the
 *  last three weeks against the first three. */
export const speedTrend = (
  speeds: (number | null)[],
): { kind: "up" | "down" | "steady"; percent: number } | null => {
  const known = speeds.filter((s): s is number => s !== null);
  if (known.length < 4) return null;
  const n = Math.min(3, Math.floor(known.length / 2));
  const avg = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;
  const percent = change(avg(known.slice(-n)), avg(known.slice(0, n))) ?? 0;
  if (Math.abs(percent) < 5) return { kind: "steady", percent: 0 };
  return { kind: percent > 0 ? "up" : "down", percent: Math.abs(percent) };
};

/** Words per bar for the Words card: a day each for 7 and 30 days, a week
 *  each for All, at most a year of weeks. */
export const wordBars = (
  days: InsightsDay[],
  range: InsightsRange,
  today: Date,
): { start: string; words: number }[] => {
  const byDay = new Map(days.map((d) => [d.day, d.words]));
  const span = RANGE_DAYS[range];
  if (span !== null) {
    return Array.from({ length: span }, (_, i) => {
      const day = isoDay(addDays(today, i + 1 - span));
      return { start: day, words: byDay.get(day) ?? 0 };
    });
  }
  const firstDay = days[0]?.day;
  if (!firstDay) return [];
  const weeksSince = Math.floor(
    (today.getTime() - parseDay(firstDay).getTime()) / (7 * 86_400_000),
  );
  const count = Math.min(52, Math.max(1, weeksSince + 1));
  return Array.from({ length: count }, (_, i) => {
    const end = addDays(today, -7 * (count - 1 - i));
    const start = addDays(end, -6);
    const s = isoDay(start);
    const e = isoDay(end);
    return {
      start: s,
      words: days
        .filter((d) => d.day >= s && d.day <= e)
        .reduce((a, d) => a + d.words, 0),
    };
  });
};

/** The apps with the most words, and the rest folded into one row. */
export const topApps = (
  apps: InsightsApp[],
  shown = 6,
): { top: InsightsApp[]; rest: { count: number; words: number } | null } => {
  // Folding a single app into "Other" would hide it for nothing.
  if (apps.length <= shown + 1) return { top: apps, rest: null };
  const rest = apps.slice(shown);
  return {
    top: apps.slice(0, shown),
    rest: { count: rest.length, words: rest.reduce((a, x) => a + x.words, 0) },
  };
};

/** Words by kind of app, most first. */
export const byCategory = (
  apps: InsightsApp[],
): { category: string; words: number }[] => {
  const totals = new Map<string, number>();
  for (const a of apps) {
    const c = a.category ?? "other";
    totals.set(c, (totals.get(c) ?? 0) + a.words);
  }
  return [...totals]
    .map(([category, words]) => ({ category, words }))
    .sort((a, b) => b.words - a.words);
};

/** The busiest hour of the day, or null without dictations. */
export const busiestHour = (
  hours: number[],
): { hour: number; dictations: number } | null => {
  let best = -1;
  hours.forEach((n, h) => {
    if (n > 0 && (best < 0 || n > hours[best])) best = h;
  });
  return best < 0 ? null : { hour: best, dictations: hours[best] };
};
