import assert from "node:assert/strict";
import type { HistoryEntry } from "@/bindings";
import {
  dayLabel,
  formatSpeech,
  groupByDay,
  localDay,
  searchTerms,
} from "./format";

const at = (y: number, m: number, d: number, h = 12) =>
  new Date(y, m - 1, d, h).getTime() / 1000;
const entry = (id: number, timestamp: number) =>
  ({ id, timestamp }) as unknown as HistoryEntry;

assert.equal(formatSpeech(7900), "7.9 s");
assert.equal(formatSpeech(174_000), "2 min 54 s");
assert.equal(formatSpeech(120_000), "2 min");
assert.equal(formatSpeech(3_900_000), "1 h 5 min");

assert.equal(localDay(at(2026, 10, 9)), "2026-10-09");

const groups = groupByDay([
  entry(3, at(2026, 10, 19, 17)),
  entry(2, at(2026, 10, 19, 9)),
  entry(1, at(2026, 10, 18, 23)),
]);
assert.deepEqual(
  groups.map((g) => [g.day, g.entries.map((e) => e.id)]),
  [
    ["2026-10-19", [3, 2]],
    ["2026-10-18", [1]],
  ],
);

const now = new Date(2026, 9, 19, 15);
assert.equal(dayLabel("2026-10-19", now, "en").kind, "today");
assert.equal(dayLabel("2026-10-18", now, "en").kind, "yesterday");
assert.equal(dayLabel("2026-10-16", now, "en").text, "Friday, October 16");
assert.equal(
  dayLabel("2025-10-16", now, "en").text,
  "Thursday, October 16, 2025",
);

assert.deepEqual(searchTerms('  dok  "net '), ["dok", "net"]);

console.log("format tests passed");
