import assert from "node:assert/strict";
import type { Snippet } from "@/bindings";
import { matchesQuery, normalizeTrigger, pieces, sortSnippets } from "./list";

const snippet = (more: Partial<Snippet>): Snippet => ({
  id: 1,
  trigger: "sign off",
  text: "Best regards,\nAbhash",
  alone: true,
  uses: 0,
  added_at: 0,
  last_used_at: null,
  last_app: null,
  last_app_path: null,
  ...more,
});

// Lines and variables split into pieces; a blank line shows one break.
assert.deepEqual(pieces("Today is {{date:long}}.\n\nBye"), [
  { kind: "text", text: "Today is " },
  { kind: "variable", name: "date:long" },
  { kind: "text", text: "." },
  { kind: "break" },
  { kind: "text", text: "Bye" },
]);

// Search looks at the trigger and the text.
const s = snippet({});
assert.equal(matchesQuery(s, "SIGN"), true);
assert.equal(matchesQuery(s, "abhash"), true);
assert.equal(matchesQuery(s, "linkedin"), false);

// Most used first, then by trigger.
const list = [
  snippet({ id: 1, trigger: "b", uses: 2 }),
  snippet({ id: 2, trigger: "a", uses: 2 }),
  snippet({ id: 3, trigger: "c", uses: 9 }),
];
assert.deepEqual(
  sortSnippets(list, "used").map((x) => x.id),
  [3, 2, 1],
);
assert.deepEqual(
  sortSnippets(list, "az").map((x) => x.id),
  [2, 1, 3],
);

// Triggers compare the way the backend normalizes them.
assert.equal(normalizeTrigger("  My  LinkedIn! "), "my linkedin");
assert.equal(normalizeTrigger("today's date."), "today's date");

console.log("snippets list: ok");
