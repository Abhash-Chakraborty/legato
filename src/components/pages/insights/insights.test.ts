import assert from "node:assert/strict";
import type { InsightsApp, InsightsDay } from "@/bindings";
import {
  busiestHour,
  byCategory,
  change,
  heatLevel,
  heatWeeks,
  rangeStart,
  speedTrend,
  split,
  streak,
  sum,
  topApps,
  weeklySpeed,
  wordBars,
  wpm,
} from "./insights";

const day = (
  d: string,
  words: number,
  speech_ms = words * 500,
): InsightsDay => ({
  day: d,
  dictations: words ? 1 : 0,
  words,
  speech_ms,
  fixes: 0,
  transformed: 0,
  snippets: 0,
});

// Saturday, October 17, 2026.
const today = new Date(2026, 9, 17, 17, 5);

// 7 days: the week ending today, and the week before.
{
  const days = [
    day("2026-10-01", 50),
    day("2026-10-10", 100),
    day("2026-10-11", 200),
    day("2026-10-17", 300),
  ];
  const { current, previous } = split(days, "week", today);
  assert.deepEqual(
    current.map((d) => d.day),
    ["2026-10-11", "2026-10-17"],
  );
  assert.deepEqual(
    previous?.map((d) => d.day),
    ["2026-10-10"],
  );
  assert.equal(split(days, "all", today).previous, null);
  assert.equal(sum(current).words, 500);
  assert.equal(rangeStart("week", today), "2026-10-11");
  assert.equal(rangeStart("all", today), null);
}

// Speed and change.
assert.equal(wpm(112, 60_000), 112);
assert.equal(wpm(10, 5_000), null);
assert.equal(change(149, 100), 49);
assert.equal(change(5, 0), null);

// A streak counts back from today, or from yesterday before the first
// dictation of the day.
assert.equal(
  streak(
    [day("2026-10-15", 1), day("2026-10-16", 1), day("2026-10-17", 1)],
    today,
  ),
  3,
);
assert.equal(streak([day("2026-10-15", 1), day("2026-10-16", 1)], today), 2);
assert.equal(streak([day("2026-10-14", 1)], today), 0);

// The activity map ends with this week and starts on a Sunday.
{
  const weeks = heatWeeks([day("2026-10-17", 531)], today);
  assert.equal(weeks.length, 18);
  assert.equal(new Date(2026, 9, 17).getDay(), 6);
  const last = weeks[17];
  assert.equal(last[0].day, "2026-10-11");
  assert.equal(last[6].day, "2026-10-17");
  assert.equal(last[6].words, 531);
  assert.equal(last[6].future, false);
  assert.equal(heatWeeks([], new Date(2026, 9, 14))[17][6].future, true);
}
assert.deepEqual([0, 1, 249, 250, 500, 750].map(heatLevel), [0, 1, 1, 2, 3, 4]);

// Speed by week, and its trend.
{
  const speeds = weeklySpeed(
    [day("2026-10-17", 100, 60_000), day("2026-10-09", 50, 60_000)],
    today,
    3,
  );
  assert.deepEqual(speeds, [null, 50, 100]);
  assert.deepEqual(speedTrend([100, 100, 101, 99]), {
    kind: "steady",
    percent: 0,
  });
  assert.deepEqual(speedTrend([100, 100, 120, 120]), {
    kind: "up",
    percent: 20,
  });
  assert.equal(speedTrend([100, null, 120]), null);
}

// Bars: a day each for 7 days, a week each for All.
{
  const days = [
    day("2026-10-01", 10),
    day("2026-10-16", 20),
    day("2026-10-17", 30),
  ];
  const week = wordBars(days, "week", today);
  assert.equal(week.length, 7);
  assert.deepEqual(
    week.slice(-2).map((b) => b.words),
    [20, 30],
  );
  const all = wordBars(days, "all", today);
  assert.equal(all.length, 3);
  assert.deepEqual(
    all.map((b) => b.words),
    [10, 0, 50],
  );
}

// Apps: six shown, the rest folded, but never a single app folded alone.
{
  const app = (
    name: string,
    words: number,
    category: string | null = "work",
  ): InsightsApp => ({
    name,
    path: null,
    category,
    words,
  });
  const seven = [1, 2, 3, 4, 5, 6, 7].map((n) => app(`A${n}`, 100 - n));
  assert.equal(topApps(seven).rest, null);
  const eight = [...seven, app("A8", 10)];
  const { top, rest } = topApps(eight);
  assert.equal(top.length, 6);
  assert.deepEqual(rest, { count: 2, words: 93 + 10 });
  assert.deepEqual(
    byCategory([
      app("Slack", 10),
      app("Code", 30, "code"),
      app("Teams", 5),
      app("X", 1, null),
    ]),
    [
      { category: "code", words: 30 },
      { category: "work", words: 15 },
      { category: "other", words: 1 },
    ],
  );
}

// The busiest hour.
{
  const hours = Array(24).fill(0);
  hours[16] = 41;
  hours[9] = 12;
  assert.deepEqual(busiestHour(hours), { hour: 16, dictations: 41 });
  assert.equal(busiestHour(Array(24).fill(0)), null);
}

console.log("insights tests passed");
