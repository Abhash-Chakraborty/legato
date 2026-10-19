import assert from "node:assert/strict";
import type { DictionaryEntry } from "@/bindings";
import {
  inTab,
  matchesQuery,
  matchingOf,
  relativeTime,
  sortEntries,
} from "./list";

const entry = (id: number, text: string, more: Partial<DictionaryEntry> = {}) =>
  ({
    id,
    text,
    heard: [],
    source: "manual",
    pinned: false,
    case_sensitive: false,
    added_at: id,
    last_fix_at: null,
    last_app: null,
    last_app_path: null,
    hits: 0,
    ...more,
  }) as DictionaryEntry;

const karol = entry(1, "Karol", { pinned: true, hits: 3, heard: ["carol"] });
const dokploy = entry(2, "Dokploy", {
  source: "fixed",
  hits: 9,
  last_fix_at: 50,
});
const netbird = entry(3, "netbird", { hits: 9 });
const all = [karol, dokploy, netbird];

const ids = (list: DictionaryEntry[]) => list.map((e) => e.id);
assert.deepEqual(ids(sortEntries(all, "pinned")), [1, 2, 3]);
assert.deepEqual(ids(sortEntries(all, "hits")), [2, 3, 1]);
assert.deepEqual(ids(sortEntries(all, "recent")), [2, 3, 1]);
assert.deepEqual(ids(sortEntries(all, "newest")), [3, 2, 1]);
assert.deepEqual(ids(sortEntries(all, "az")), [2, 1, 3]);

assert.equal(inTab(dokploy, "learned"), true);
assert.equal(inTab(karol, "learned"), false);
assert.equal(inTab(karol, "mine"), true);
assert.equal(matchesQuery(karol, "CAR"), true);
assert.equal(matchesQuery(karol, "dok"), false);

assert.equal(matchingOf(0.1), "strict");
assert.equal(matchingOf(0.18), "normal");
assert.equal(matchingOf(0.28), "loose");

const now = 1_000_000;
assert.equal(relativeTime(now - 240, now, "en"), "4 min. ago");
assert.equal(relativeTime(now - 10, now, "en"), "now");

console.log("dictionary list: ok");
