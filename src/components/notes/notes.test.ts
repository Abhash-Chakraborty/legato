import assert from "node:assert/strict";
import type { Note, NoteLine } from "@/bindings";
import {
  answerParts,
  clock,
  durationLabel,
  inFilter,
  lineAt,
  lineMatches,
  longClock,
  summaryText,
  transcriptText,
  whenLabel,
} from "./notes";

const line = (at_ms: number, who: "you" | "them", text: string): NoteLine => ({
  id: at_ms,
  note_id: 1,
  at_ms,
  end_ms: at_ms + 2000,
  who,
  text,
});

const note = (over: Partial<Note> = {}): Note => ({
  id: 1,
  title: "Infra sync",
  kind: "meeting",
  status: "ready",
  app: "Microsoft Teams",
  app_path: null,
  microphone: "Yeti X",
  system_audio: "Microsoft Teams",
  started_at: 1_760_700_000,
  duration_ms: 2_280_000,
  audio_file: "note-1.wav",
  tags: ["AIGHT RE"],
  my_notes: "",
  summary: "Both Dokploy instances move to the ARM box.",
  decisions: ["Consolidate on the ARM box"],
  summary_model: null,
  summary_at: null,
  summary_error: null,
  failed_lines: 0,
  lines: 12,
  actions: [{ id: 1, owner: "Samir", text: "Move Dokploy", done: true }],
  ...over,
});

assert.equal(durationLabel(45_000), "45 s");
assert.equal(durationLabel(2_280_000), "38 min");
assert.equal(durationLabel(7_020_000), "1 h 57 min");
assert.equal(durationLabel(3_600_000), "1 h");

assert.equal(clock(4_000), "0:04");
assert.equal(clock(1_410_000), "23:30");
assert.equal(clock(3_725_000), "1:02:05");
assert.equal(longClock(6_000), "00:00:06");

assert.ok(inFilter(note(), "all", null));
assert.ok(inFilter(note(), "meeting", "aight re"));
assert.ok(!inFilter(note(), "memo", null));
assert.ok(!inFilter(note(), "all", "ALV"));

const lines = [
  line(4_000, "you", "Okay, let's start."),
  line(11_000, "them", "Netbird routes are up."),
];
assert.equal(lineAt(lines, 0), -1);
assert.equal(lineAt(lines, 5_000), 0);
assert.equal(lineAt(lines, 60_000), 1);

assert.deepEqual(
  answerParts("You said you'd ask Robert [1:11], and send the doc [1:43]."),
  [
    { kind: "text", text: "You said you'd ask Robert " },
    { kind: "time", label: "1:11", ms: 71_000 },
    { kind: "text", text: ", and send the doc " },
    { kind: "time", label: "1:43", ms: 103_000 },
    { kind: "text", text: "." },
  ],
);
assert.deepEqual(answerParts("[1:02:05] only"), [
  { kind: "time", label: "1:02:05", ms: 3_725_000 },
  { kind: "text", text: " only" },
]);
assert.deepEqual(answerParts("No times [here]."), [
  { kind: "text", text: "No times [here]." },
]);

assert.equal(
  transcriptText(lines, { you: "You", them: "Them" }),
  "0:04 You: Okay, let's start.\n0:11 Them: Netbird routes are up.",
);
assert.ok(lineMatches(lines[1], "netbird UP"));
assert.ok(!lineMatches(lines[1], "netbird down"));
assert.ok(!lineMatches(lines[1], "  "));

assert.equal(
  summaryText(note(), { decisions: "Decisions", actions: "Action items" }),
  "Both Dokploy instances move to the ARM box.\n\nDecisions\n- Consolidate on the ARM box\n\nAction items\n- [x] Move Dokploy (Samir)",
);

const now = new Date(2026, 9, 17, 17, 5);
const at = (d: Date) => Math.floor(d.getTime() / 1000);
assert.equal(
  whenLabel(at(new Date(2026, 9, 17, 13, 54)), now, "en-US"),
  "Today · 1:54 PM",
);
assert.equal(
  whenLabel(at(new Date(2026, 9, 16, 23, 0)), now, "en-US"),
  "Yesterday · 11:00 PM",
);
assert.equal(
  whenLabel(at(new Date(2026, 9, 15, 20, 41)), now, "en-US"),
  "Thursday · 8:41 PM",
);
assert.equal(
  whenLabel(at(new Date(2026, 9, 3, 15, 30)), now, "en-US"),
  "Oct 3 · 3:30 PM",
);
assert.equal(
  whenLabel(at(new Date(2025, 9, 3, 15, 30)), now, "en-US"),
  "Oct 3, 2025 · 3:30 PM",
);

console.log("notes: ok");
