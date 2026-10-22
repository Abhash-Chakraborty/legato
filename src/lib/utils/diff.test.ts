import assert from "node:assert/strict";
import { changeCount, wordDiff } from "./diff";

const join = (parts: { text: string }[]) => parts.map((p) => p.text).join("");

// A replaced word reads removed, then added; untouched words stay one run.
const parts = wordDiff(
  "so um move it to doc ploy today",
  "So move it to Dokploy today",
);
assert.deepEqual(parts, [
  { kind: "removed", text: "so um " },
  { kind: "added", text: "So " },
  { kind: "same", text: "move it to " },
  { kind: "removed", text: "doc ploy " },
  { kind: "added", text: "Dokploy " },
  { kind: "same", text: "today" },
]);
assert.deepEqual(changeCount(parts), { added: 2, removed: 4 });

// The parts rebuild the new text.
assert.equal(
  join(parts.filter((p) => p.kind !== "removed")),
  "So move it to Dokploy today",
);

// Identical text is one run; empty sides are all added or all removed.
assert.deepEqual(wordDiff("same text", "same text"), [
  { kind: "same", text: "same text" },
]);
assert.deepEqual(wordDiff("", "new"), [{ kind: "added", text: "new" }]);
assert.deepEqual(wordDiff("old", ""), [{ kind: "removed", text: "old" }]);

// A moved line break isn't a change.
assert.deepEqual(
  wordDiff("one two", "one\ntwo").map((p) => p.kind),
  ["same"],
);

// Punctuation added after a word is the only change shown.
assert.deepEqual(wordDiff("on Windows 11", "on Windows 11."), [
  { kind: "same", text: "on Windows 11" },
  { kind: "added", text: "." },
]);
assert.deepEqual(changeCount(wordDiff("before Friday", "before Friday.")), {
  added: 0,
  removed: 0,
});

console.log("diff: ok");
