import assert from "node:assert/strict";
import type { Note, NoteLine } from "@/bindings";
import {
  answerParts,
  clock,
  durationLabel,
  fixContext,
  inFilter,
  lineAt,
  lineMatches,
  lineWords,
  wordAt,
  wordAtOffset,
  linesToFix,
  replaceWords,
  longClock,
  mergeCandidates,
  isYourName,
  peopleGroups,
  samePerson,
  lineStep,
  nextSpeed,
  playerMarks,
  speakerBands,
  renamable,
  speakerLabel,
  voiceColour,
  THEM_SIDE,
  summaryText,
  notesAnswerParts,
  taskGroups,
  voiceChoices,
  voicesIn,
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

const names = {
  you: "You",
  them: "Them",
  voice: (n: number) => `Speaker ${n}`,
};
assert.equal(
  transcriptText(lines, names),
  "0:04 You: Okay, let's start.\n0:11 Them: Netbird routes are up.",
);
// Voices told apart: a name where one was given, a number elsewhere.
const voiced: NoteLine[] = [
  { ...lines[0], speaker: 1 },
  { ...lines[1], speaker: 2 },
  { ...lines[1], id: 9, at_ms: 15_000, speaker: 1 },
];
const named = [{ speaker: 1, name: "Robert" }];
assert.equal(speakerLabel(voiced[1], named, names), "Speaker 2");
assert.equal(
  speakerLabel(voiced[1], [{ speaker: 2, name: "" }], names),
  "Speaker 2",
);
assert.deepEqual(voicesIn([...voiced, lines[0]]), [1, 2]);
// The other side can be named before its voices are told apart.
const side = [{ speaker: THEM_SIDE, name: "Alex" }];
assert.equal(speakerLabel(lines[1], side, names), "Alex");
assert.equal(speakerLabel(lines[0], side, names), "You");
assert.equal(speakerLabel(voiced[1], side, names), "Speaker 2");
assert.equal(renamable(lines[1]), THEM_SIDE);
assert.equal(renamable(lines[0]), null);
assert.equal(renamable(voiced[1]), 2);
// A meeting's voices are on Them, so You stays a choice of its own.
assert.deepEqual(
  voiceChoices([lines[0], voiced[1], { ...voiced[2], speaker: 3 }]),
  [
    { who: "you", speaker: null },
    { who: "them", speaker: 2 },
    { who: "them", speaker: 3 },
  ],
);
assert.deepEqual(voiceChoices(lines), []);
assert.equal(
  transcriptText(voiced, names, named),
  "0:04 Robert: Okay, let's start.\n0:11 Speaker 2: Netbird routes are up.\n0:15 Robert: Netbird routes are up.",
);
assert.ok(lineMatches(lines[1], "netbird UP"));
assert.ok(!lineMatches(lines[1], "netbird down"));
assert.ok(!lineMatches(lines[1], "  "));

// Selected words are fixed only where they stand as whole words.
assert.equal(
  replaceWords("IPES ships, and ipes is late", "ipes", "AIPS"),
  "AIPS ships, and AIPS is late",
);
assert.equal(
  replaceWords("Recipes are not IPES", "ipes", "AIPS"),
  "Recipes are not AIPS",
);
assert.equal(replaceWords("nothing here", "ipes", "AIPS"), null);
assert.equal(replaceWords("anything", "  ", "AIPS"), null);
{
  const said = (text: string) => ({ ...lines[0], text });
  const transcript = [said("IPES is up"), said("AIPS is up"), said("no ipes")];
  assert.equal(linesToFix(transcript, "IPES", "AIPS"), 2);
  // Adding a word spelled right changes no line.
  assert.equal(linesToFix(transcript, "AIPS", "AIPS"), 0);
  assert.equal(linesToFix(transcript, "IPES", " "), 0);
}

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

assert.deepEqual(
  fixContext("we deploy with Cube Cuddle today", "cube cuddle"),
  {
    before: "we deploy with ",
    match: "Cube Cuddle",
    after: " today",
  },
);
assert.equal(fixContext("scrumptious", "scrum"), null);
assert.deepEqual(
  fixContext(
    "one two three four five six seven eight nine ten eleven twelve kubectl and then the rest of it goes on",
    "kubectl",
    20,
  ),
  {
    before: "…ten eleven twelve ",
    match: "kubectl",
    after: " and then the rest…",
  },
);

// Each voice gets its own colour; the ninth comes round to the first.
assert.equal(voiceColour({ ...lines[1], speaker: 1 }), "var(--voice-1)");
assert.equal(voiceColour({ ...lines[1], speaker: 8 }), "var(--voice-8)");
assert.equal(voiceColour({ ...lines[1], speaker: 9 }), "var(--voice-1)");
assert.equal(voiceColour(lines[0]), "var(--teal-700)");
// Tasks: yours still open by default, newest note first; done ones apart.
{
  const task = (id: number, owner: string, text: string, done = false) => ({
    id,
    owner,
    text,
    done,
  });
  const older = note({
    id: 11,
    title: "Pricing sync",
    started_at: 100,
    actions: [task(1, "You", "Send the deck"), task(2, "Priya", "Book a room")],
  });
  const newer = note({
    id: 12,
    title: "Standup",
    started_at: 200,
    actions: [task(3, "you", "Fix the build", true), task(4, "You", "Ship it")],
  });
  const mine = taskGroups([older, newer], "mine", "");
  assert.deepEqual(
    mine.map((g) => [g.note.id, g.actions.map((a) => a.id)]),
    [
      [12, [4]],
      [11, [1]],
    ],
  );
  assert.deepEqual(
    taskGroups([older, newer], "everyone", "")[1].actions.map((a) => a.id),
    [1, 2],
  );
  assert.deepEqual(
    taskGroups([older, newer], "done", "").map((g) => g.note.id),
    [12],
  );
  // Search reads the step, its owner and the note's title.
  assert.equal(taskGroups([older, newer], "everyone", "priya").length, 1);
  assert.equal(taskGroups([older, newer], "mine", "pricing")[0].note.id, 11);
}
// Ask across notes cites notes to open.
assert.deepEqual(
  notesAnswerParts("Send the deck [note 12]. Done [Note 3, note 7]"),
  [
    { kind: "text", text: "Send the deck " },
    { kind: "note", id: 12 },
    { kind: "text", text: ". Done " },
    { kind: "note", id: 3 },
    { kind: "note", id: 7 },
  ],
);
assert.deepEqual(notesAnswerParts("Nothing about that."), [
  { kind: "text", text: "Nothing about that." },
]);
// A line's words follow the model's times, or spread across the line.
{
  const timed: NoteLine = {
    ...line(1_000, "them", "We ship  Friday."),
    word_ms: [
      [1_000, 1_200],
      [1_300, 1_700],
      [2_000, 2_600],
    ],
  };
  const words = lineWords(timed);
  assert.deepEqual(
    words.map((w) => [w.text, w.from, w.to, w.start, w.end]),
    [
      ["We", 0, 2, 1_000, 1_200],
      ["ship", 3, 7, 1_300, 1_700],
      ["Friday.", 9, 16, 2_000, 2_600],
    ],
  );
  assert.equal(wordAt(words, 900), -1);
  assert.equal(wordAt(words, 1_250), 0);
  assert.equal(wordAt(words, 1_800), 1);
  assert.equal(wordAt(words, 2_800), 2);
  assert.equal(wordAt(words, 3_000), -1);
  assert.equal(wordAtOffset(words, 4), 1);
  assert.equal(wordAtOffset(words, 8), 2);
  assert.equal(wordAtOffset(words, 99), 2);
  // Times that don't fit the words (or none, from Whisper) are spread.
  const spreadOut = lineWords({
    ...line(1_000, "you", "ab abcd ab"),
    end_ms: 1_800,
  });
  assert.deepEqual(
    spreadOut.map((w) => [w.start, w.end]),
    [
      [1_000, 1_200],
      [1_200, 1_600],
      [1_600, 1_800],
    ],
  );
  assert.deepEqual(
    lineWords({ ...timed, text: "two words" }).map((w) => w.start),
    [1_000, 1_750],
  );
  assert.deepEqual(lineWords(line(0, "you", "  ")), []);
}
// Merging: the nearest note before comes first, then the rest by distance.
{
  const at = 1_760_700_000;
  const here = note({ id: 5, started_at: at });
  const list = [
    note({ id: 1, started_at: at - 13 * 3600 }),
    note({ id: 2, started_at: at - 3000 }),
    note({ id: 3, started_at: at + 600 }),
    note({ id: 4, started_at: at + 900, status: "recording" }),
    here,
  ];
  assert.deepEqual(
    mergeCandidates(list, here).map((n) => n.id),
    [2, 3],
  );
  assert.deepEqual(
    mergeCandidates(list, note({ id: 2, started_at: at - 3000 })).map(
      (n) => n.id,
    ),
    [5, 3],
  );
}

// The player: speeds, marks from the summary, speaker bands, line steps.
assert.equal(nextSpeed(1, 1), 1.25);
assert.equal(nextSpeed(2, 1), 2);
assert.equal(nextSpeed(1, -1), 1);
assert.equal(nextSpeed(2, 1, true), 1);
assert.equal(nextSpeed(1.1, 1), 1.25);
assert.deepEqual(
  playerMarks(
    {
      topics: [
        { heading: "Builds", text: "", at_ms: 60_000 },
        { heading: "Old", text: "" },
      ],
      details: [
        { heading: "Workers", text: "", at_ms: 60_400 },
        { heading: "Costs", text: "", at_ms: 5_000 },
        { heading: "Late", text: "", at_ms: 900_000 },
      ],
    },
    600_000,
  ),
  [
    { ms: 5_000, label: "Costs" },
    { ms: 60_000, label: "Builds" },
  ],
);
assert.deepEqual(playerMarks(null, 1000), []);
{
  const said = (at_ms: number, end_ms: number, speaker: number): NoteLine => ({
    ...line(at_ms, "them", "x"),
    end_ms,
    speaker,
  });
  const bands = speakerBands([
    said(0, 4_000, 1),
    said(5_000, 9_000, 1),
    said(20_000, 25_000, 1),
    said(25_000, 30_000, 2),
  ]);
  assert.deepEqual(
    bands.map((b) => [b.from, b.to]),
    [
      [0, 9_000],
      [20_000, 25_000],
      [25_000, 30_000],
    ],
  );
  assert.deepEqual(
    speakerBands([said(0, 4_000, 1), said(5_000, 9_000, 1)]),
    [],
  );
}
assert.equal(lineStep(lines, 5_000, 1), 11_000);
assert.equal(lineStep(lines, 60_000, 1), null);
assert.equal(lineStep(lines, 12_000, -1), 4_000);
assert.equal(lineStep(lines, 20_000, -1), 11_000);
assert.equal(lineStep(lines, 1_000, -1), 0);

// People: one person under two spellings, and the three groups.
{
  const me = "Abhash Chakraborty";
  for (const name of [
    "Abhash",
    "abhash chakraborty",
    "A. Chakraborty",
    "a.chakraborty",
    "A.chkraborty",
    "achakraborty",
    "abhash.chakraborty@example.com",
    "Abhash C",
  ])
    assert.ok(isYourName(name, me), name);
  for (const name of ["Abhash Kumar", "Avash", "Priya Shah", "B. Chakraborty"])
    assert.ok(!samePerson(name, me), name);
  assert.ok(!isYourName("Abhash", ""));
  const groups = peopleGroups(
    note({
      attendees: ["a.chakraborty", "Priya Shah", "Sahil Rao", "Mara Lind"],
      seen: ["Abhash Chakraborty", "Priya Shah", "P. Shah"],
      speakers: [{ speaker: 2, name: "Mara" }],
      outline: {
        people: ["Sahil", "Abhash"],
        mentioned: ["Simon", "Robert", "Ivan"],
      },
    }),
    me,
  );
  assert.deepEqual(groups.attended, ["Priya Shah", "Mara Lind", "Sahil Rao"]);
  assert.deepEqual(groups.invited, []);
  assert.deepEqual(groups.mentioned, ["Simon", "Robert", "Ivan"]);
  const old = peopleGroups(
    note({
      attendees: ["Priya Shah"],
      seen: null,
      outline: { people: ["Priya", "Simon"] },
    }),
    "Abhash",
  );
  assert.deepEqual(old, {
    attended: ["Priya Shah"],
    invited: [],
    mentioned: ["Simon"],
  });
  assert.deepEqual(
    peopleGroups(note({ attendees: ["Jo Park"], seen: [] }), "").invited,
    ["Jo Park"],
  );
}
console.log("notes: ok");
