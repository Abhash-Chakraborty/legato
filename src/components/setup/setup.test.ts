import assert from "node:assert/strict";
import type { ModelInfo } from "@/bindings";
import {
  blocks,
  countWords,
  droppedRuns,
  groupOf,
  listOf,
  otherLanguage,
  pickModels,
  practiceMode,
  struckRuns,
  termAsHeard,
} from "./setup";

// Steps map to groups and practice modes.
{
  assert.equal(groupOf("email"), "practice");
  assert.equal(groupOf("done"), null);
  assert.equal(practiceMode("message"), "chat");
  assert.equal(practiceMode("list"), "document");
  assert.equal(practiceMode("words"), "hint");
}

// What the model wrote, with what the rules took out struck.
{
  const heard =
    "Sounds good. Um, let's do twelve, actually one. I'll book a table.";
  const text = "Sounds good, let's do one. I'll book a table.";
  assert.deepEqual(
    struckRuns(heard, text),
    [
      { text: "Sounds good.", removed: false },
      { text: "Um, ", removed: true },
      { text: "let's do", removed: false },
      { text: "twelve, actually", removed: true },
      { text: "one. I'll book a table.", removed: false },
    ].map((r) => ({ ...r, text: r.text.trim() })),
  );
  // Nothing removed: one run.
  assert.deepEqual(struckRuns("Coffee beans", "coffee beans."), [
    { text: "Coffee beans", removed: false },
  ]);
}

// A self-correction's dropped words.
{
  assert.deepEqual(
    droppedRuns("Friday at three, actually four?", "Friday at four?"),
    ["three, actually"],
  );
  assert.deepEqual(droppedRuns("same", "same"), []);
}

// Paragraphs and lists.
{
  const email = "Hi Maya,\n\nThanks for the feedback.\n\nBest regards,\nSam";
  assert.deepEqual(blocks(email), [
    { kind: "p", lines: ["Hi Maya,"] },
    { kind: "p", lines: ["Thanks for the feedback."] },
    { kind: "p", lines: ["Best regards,", "Sam"] },
  ]);
  const list = "For the weekend I need:\n- Oat milk\n- Eggs\n- Coffee beans";
  assert.deepEqual(blocks(list), [
    { kind: "p", lines: ["For the weekend I need:"] },
    { kind: "ul", items: ["Oat milk", "Eggs", "Coffee beans"] },
  ]);
  assert.deepEqual(listOf(list), { items: 3, numbered: false });
  assert.deepEqual(listOf("1. One\n2. Two"), { items: 2, numbered: true });
  assert.equal(listOf(email), null);
  assert.equal(countWords(list), 10);
}

// How a script's term came out.
{
  const script = "and the mockups are now in Figma. Can we go";
  assert.equal(
    termAsHeard("Figma", script, "and the mockups are now in Figma. Can we go"),
    "Figma",
  );
  assert.equal(
    termAsHeard(
      "Figma",
      script,
      "and the mockups are now in fig ma. Can we go",
    ),
    "fig ma",
  );
  assert.equal(
    termAsHeard(
      "KPI",
      "feedback on the KPI deck",
      "feedback on the k p i deck",
    ),
    "k p i",
  );
  // Left out entirely: nothing to tell.
  assert.equal(
    termAsHeard("KPI", "feedback on the KPI deck", "feedback on the deck"),
    null,
  );
  assert.equal(termAsHeard("Granola", script, "anything"), null);
}

// Which models setup offers.
{
  const model = (id: string, langs: string[], recommended = true): ModelInfo =>
    ({
      id,
      name: id,
      supported_languages: langs,
      is_recommended: recommended,
    }) as ModelInfo;
  const models = [
    model("parakeet", ["en"]),
    model("nemotron", ["en", "fr", "es"]),
    model("whisper", ["en", "de", "fr"]),
    model("turbo", ["en", "de"], false),
  ];
  assert.equal(otherLanguage(["en-US", "de-DE"]), "de");
  assert.equal(otherLanguage(["en-GB"]), null);
  const en = pickModels(models, ["en-US"]);
  assert.equal(en.first?.id, "parakeet");
  assert.equal(en.second?.id, "nemotron");
  assert.equal(en.because, null);
  // German comes first, from the first recommended model that has it.
  const de = pickModels(models, ["en-US", "de-DE"]);
  assert.equal(de.first?.id, "whisper");
  assert.equal(de.second?.id, "parakeet");
  assert.equal(de.because, "de");
  // Unknown languages (not Windows): the English pick.
  assert.equal(pickModels(models, []).first?.id, "parakeet");
}

console.log("setup tests passed");
