import assert from "node:assert/strict";
import { heardParts } from "./heard";

// Fillers and replaced words are marked; capitals and commas don't count.
assert.deepEqual(
  heardParts(
    "hey so um I don't want twelve changes uh",
    "Hey so, I don't want 12 changes.",
  ),
  [
    { text: "hey so ", removed: false },
    { text: "um ", removed: true },
    { text: "I don't want ", removed: false },
    { text: "twelve ", removed: true },
    { text: "changes ", removed: false },
    { text: "uh", removed: true },
  ],
);

// Nothing changed, nothing marked.
assert.deepEqual(heardParts("all good", "All good."), [
  { text: "all good", removed: false },
]);

console.log("style heard: ok");
