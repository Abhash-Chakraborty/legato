import assert from "node:assert/strict";
import type { DictionaryEntry } from "@/bindings";
import { findWord, guessSpelling, markFixes, tokenize } from "./words";

const words = (text: string) => tokenize(text).map((t) => text.slice(t.s, t.e));
assert.deepEqual(words("Ask carol, then restart Node.js — don't wait."), [
  "Ask",
  "carol",
  "then",
  "restart",
  "Node.js",
  "don't",
  "wait",
]);
assert.deepEqual(words("你好 world"), ["你好", "world"]);

assert.deepEqual(findWord("Carol and carols, CAROL.", "carol"), [
  { s: 0, e: 5 },
  { s: 18, e: 23 },
]);
assert.deepEqual(findWord("abc", ""), []);

const fix = (from: string, to: string) => ({ from, to });
assert.deepEqual(
  markFixes("restart Dokploy on Netbird", [fix("doc ploy", "Dokploy")]),
  [
    { text: "restart " },
    { text: "Dokploy", fix: fix("doc ploy", "Dokploy") },
    { text: " on Netbird" },
  ],
);
// A word typed in another case wasn't the fix.
assert.deepEqual(markFixes("dokploy", [fix("doc ploy", "Dokploy")]), [
  { text: "dokploy" },
]);
assert.deepEqual(markFixes("", []), [{ text: "" }]);

const entry = (text: string, heard: string[] = []) =>
  ({ text, heard }) as unknown as DictionaryEntry;
const dict = [
  entry("Coolify", ["cool if I"]),
  entry("Dokploy"),
  entry("Karol"),
];
assert.deepEqual(guessSpelling("cool if i", dict), {
  text: "Coolify",
  why: "heard",
});
assert.deepEqual(guessSpelling("dokploi", dict), {
  text: "Dokploy",
  why: "close",
});
assert.deepEqual(guessSpelling("weather", dict), {
  text: "weather",
  why: null,
});

console.log("words: ok");
