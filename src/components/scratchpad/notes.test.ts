import assert from "node:assert/strict";
import type { ScratchNote } from "@/bindings";
import {
  insertAt,
  toggleTask,
  matchesQuery,
  matchSnippet,
  plainText,
  previewOf,
  wordCount,
  wrapSelection,
  prefixLines,
} from "./notes";

const note = (title: string, text: string): ScratchNote => ({
  id: 1,
  title,
  auto_title: true,
  text,
  pinned: false,
  tab: null,
  created_at: 0,
  updated_at: 0,
  source: null,
  source_app: null,
  source_app_path: null,
  reason: null,
  versions: 1,
});

// Markdown marks don't show in previews or search.
assert.equal(
  plainText("## Datev review\n- [ ] bring the **open** questions\n"),
  "Datev review bring the open questions",
);
const n = note(
  "Datev review",
  "## Datev review\n\n- move it to Thursday\n- ask Karol",
);
assert.equal(previewOf(n), "move it to Thursday · ask Karol");
// A long first line was cut for the title; the row goes on from the cut.
assert.equal(
  previewOf(
    note(
      "hey Karol, can we move the Datev review to Thursday? i'll…",
      "hey Karol, can we move the Datev review to Thursday? i'll bring the open questions",
    ),
  ),
  "…bring the open questions",
);
assert.equal(previewOf(note("Groceries", "Groceries")), "");
assert.ok(matchesQuery(n, "karol"));
assert.ok(matchesQuery(n, "DATEV"));
assert.ok(!matchesQuery(n, "invoice"));
assert.equal(
  matchSnippet(n.text, "thursday"),
  "Datev review move it to Thursday ask Karol",
);
assert.equal(matchSnippet(n.text, "nothing"), null);

assert.equal(wordCount("hey Karol, can we — move it?"), 6);
assert.equal(wordCount("   "), 0);

// Dictated words get spaces where they'd run into a word, not before
// punctuation.
assert.deepEqual(insertAt("ask Karol", 3, 3, "Ben and"), {
  text: "ask Ben and Karol",
  cursor: 11,
});
assert.deepEqual(insertAt("ask Karol", 9, 9, "today"), {
  text: "ask Karol today",
  cursor: 15,
});
assert.deepEqual(insertAt("call him.", 8, 8, "tomorrow"), {
  text: "call him tomorrow.",
  cursor: 17,
});
assert.deepEqual(insertAt("", 0, 0, " hello "), { text: "hello", cursor: 5 });
// A selection is replaced.
assert.deepEqual(insertAt("move it to Friday", 11, 17, "Thursday"), {
  text: "move it to Thursday",
  cursor: 19,
});

// Task boxes tick from the list item's start.
const list = "Shopping\n\n- [ ] milk\n- [x] eggs\n";
assert.equal(toggleTask(list, 10), "Shopping\n\n- [x] milk\n- [x] eggs\n");
assert.equal(toggleTask(list, 20), "Shopping\n\n- [ ] milk\n- [ ] eggs\n");
assert.equal(toggleTask("no boxes", 0), "no boxes");

// Formatting wraps and unwraps the selection, and marks lines.
assert.deepEqual(wrapSelection("call Karol now", 5, 10, "**"), {
  text: "call **Karol** now",
  from: 7,
  to: 12,
});
assert.deepEqual(wrapSelection("call **Karol** now", 7, 12, "**"), {
  text: "call Karol now",
  from: 5,
  to: 10,
});
assert.equal(
  prefixLines("oat milk\neggs\nspinach", 0, 20, "- ").text,
  "- oat milk\n- eggs\n- spinach",
);
assert.equal(
  prefixLines("- oat milk\n- eggs", 0, 15, "- ").text,
  "oat milk\neggs",
);
assert.equal(
  prefixLines("- oat milk\n- eggs", 0, 15, "1. ").text,
  "1. oat milk\n2. eggs",
);
assert.equal(
  prefixLines("intro\nsend the doc\nend", 8, 8, "- [ ] ").text,
  "intro\n- [ ] send the doc\nend",
);

console.log("notes tests passed");
