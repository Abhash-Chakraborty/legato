import assert from "node:assert/strict";
import type { TransformRun } from "@/bindings";
import {
  filterCounts,
  formatRunTime,
  inFilter,
  keyCaps,
  runMatches,
  usageOf,
} from "./list";

const run = (over: Partial<TransformRun>): TransformRun => ({
  id: 1,
  at: 0,
  transform_id: "polish",
  name: "Polish",
  source: "selection",
  app_name: "Slack",
  app_path: null,
  in_words: 63,
  out_words: 54,
  ms: 1210,
  result: "kept",
  error: null,
  history_id: null,
  ...over,
});

const runs = [
  run({ id: 1 }),
  run({ id: 2, result: "copied", name: "To German", app_name: "Outlook" }),
  run({ id: 3, result: "undone" }),
  run({
    id: 4,
    result: "failed",
    error: "Ollama isn't running",
    app_name: null,
  }),
];

assert.deepEqual(filterCounts(runs), { all: 4, kept: 2, undone: 1, failed: 1 });
assert.equal(inFilter(runs[1], "kept"), true);
assert.equal(inFilter(runs[1], "undone"), false);

assert.deepEqual(
  runs.filter((r) => runMatches(r, "german")).map((r) => r.id),
  [2],
);
assert.deepEqual(
  runs.filter((r) => runMatches(r, "outlook")).map((r) => r.id),
  [2],
);
assert.deepEqual(
  runs.filter((r) => runMatches(r, "ollama")).map((r) => r.id),
  [4],
);
assert.equal(runs.filter((r) => runMatches(r, "  ")).length, 4);

assert.equal(formatRunTime(1210), "1.2 s");
assert.equal(formatRunTime(940), "0.9 s");
assert.deepEqual(keyCaps("Win + Alt + 1"), ["Win", "Alt", "1"]);
assert.deepEqual(keyCaps(""), []);

const usage = [{ transform_id: "polish", uses: 2, avg_ms: 1000, last_at: 5 }];
assert.equal(usageOf(usage, "polish")?.uses, 2);
assert.equal(usageOf(usage, "bullets"), undefined);

console.log("transform list: ok");
