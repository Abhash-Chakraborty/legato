import type {
  Note,
  NoteLine,
  NoteOutline,
  NoteSpeaker,
  Speaker,
} from "@/bindings";

/** Which notes the list shows. */
export type Filter = "all" | "meeting" | "memo";
export const FILTERS: Filter[] = ["all", "meeting", "memo"];

/** "45 s", "38 min" or "1 h 57 min". */
export const durationLabel = (ms: number): string => {
  const secs = Math.round(Math.max(0, ms) / 1000);
  if (secs < 60) return `${secs} s`;
  const min = Math.round(secs / 60);
  if (min < 60) return `${min} min`;
  const h = Math.floor(min / 60);
  const rest = min % 60;
  return rest ? `${h} h ${rest} min` : `${h} h`;
};

/** Notes worth merging `note` with, as parts of one meeting: the others
 *  that began within 12 hours of it and aren't being recorded, nearest
 *  first. The first is the nearest one before it, when there is one, since
 *  a cut-off meeting goes on in the note after. */
export const mergeCandidates = (notes: Note[], note: Note, max = 5): Note[] => {
  const near = notes
    .filter(
      (n) =>
        n.id !== note.id &&
        n.status !== "recording" &&
        n.status !== "transcribing" &&
        Math.abs(n.started_at - note.started_at) <= 12 * 3600,
    )
    .sort(
      (a, b) =>
        Math.abs(a.started_at - note.started_at) -
        Math.abs(b.started_at - note.started_at),
    );
  const before = near.find((n) => n.started_at <= note.started_at);
  const ordered = before ? [before, ...near.filter((n) => n !== before)] : near;
  return ordered.slice(0, max);
};

/** "0:04", "23:30" or "1:02:05", as transcripts show times. */
export const clock = (ms: number): string => {
  const secs = Math.floor(Math.max(0, ms) / 1000);
  const h = Math.floor(secs / 3600);
  const m = Math.floor(secs / 60) % 60;
  const s = String(secs % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
};

/** "00:12:04", the recording clock. */
export const longClock = (ms: number): string => {
  const secs = Math.floor(Math.max(0, ms) / 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(Math.floor(secs / 3600))}:${pad(Math.floor(secs / 60) % 60)}:${pad(secs % 60)}`;
};

export const inFilter = (note: Note, filter: Filter, tag: string | null) =>
  (filter === "all" || note.kind === filter) &&
  (tag === null ||
    note.tags.some((t) => t.toLowerCase() === tag.toLowerCase()));

/** The line playing at `ms`: the last one that started by then. */
export const lineAt = (lines: NoteLine[], ms: number): number => {
  let found = -1;
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].at_ms <= ms) found = i;
    else break;
  }
  return found;
};

/** A word of a transcript line: where it sits in the text, and when it
 *  was said in ms from the start of the recording. */
export interface LineWord {
  text: string;
  from: number;
  to: number;
  start: number;
  end: number;
}

/** `start..end` shared among words by their length, as the backend
 *  spreads a line's words when the model gave no times. */
const spread = (
  words: string[],
  start: number,
  end: number,
): [number, number][] => {
  const span = Math.max(end, start) - start;
  const weights = words.map((w) => Math.max(1, [...w].length));
  const total = Math.max(
    1,
    weights.reduce((a, b) => a + b, 0),
  );
  let done = 0;
  return weights.map((w) => {
    const from = start + Math.floor((span * done) / total);
    done += w;
    return [from, start + Math.floor((span * done) / total)];
  });
};

/** A line's words with their times: the speech model's when it gave them,
 *  else spread across the line. */
export const lineWords = (line: NoteLine): LineWord[] => {
  const found = [...line.text.matchAll(/\S+/g)].map((m) => ({
    text: m[0],
    from: m.index ?? 0,
    to: (m.index ?? 0) + m[0].length,
  }));
  const times =
    line.word_ms && line.word_ms.length === found.length
      ? line.word_ms
      : spread(
          found.map((w) => w.text),
          line.at_ms,
          line.end_ms,
        );
  return found.map((w, i) => ({ ...w, start: times[i][0], end: times[i][1] }));
};

/** The word being said at `ms`: the last one that started, until the next
 *  starts or a moment after it ends; -1 before the first or after the
 *  last. */
export const wordAt = (words: LineWord[], ms: number): number => {
  let found = -1;
  for (let i = 0; i < words.length; i++) {
    if (words[i].start <= ms) found = i;
    else break;
  }
  if (found < 0) return -1;
  const next = words[found + 1];
  const until = next ? next.start : words[found].end + 300;
  return ms < Math.max(until, words[found].end) ? found : -1;
};

/** The word at a character offset in the line's text, or the next one
 *  when the offset falls between words. */
export const wordAtOffset = (words: LineWord[], offset: number): number => {
  const i = words.findIndex((w) => offset < w.to);
  return i < 0 ? words.length - 1 : i;
};

export type AnswerPart =
  | { kind: "text"; text: string }
  | { kind: "time"; label: string; ms: number };

/** An Ask answer with its "[12:04]" citations as times to jump to. */
export const answerParts = (answer: string): AnswerPart[] => {
  const parts: AnswerPart[] = [];
  const re = /\[(\d{1,2}(?::\d{2}){1,2})\]/g;
  let last = 0;
  for (const match of answer.matchAll(re)) {
    const at = match.index ?? 0;
    if (at > last) parts.push({ kind: "text", text: answer.slice(last, at) });
    const nums = match[1].split(":").map(Number);
    const secs = nums.reduce((acc, n) => acc * 60 + n, 0);
    parts.push({ kind: "time", label: match[1], ms: secs * 1000 });
    last = at + match[0].length;
  }
  if (last < answer.length)
    parts.push({ kind: "text", text: answer.slice(last) });
  return parts;
};

export type NotesAnswerPart =
  | { kind: "text"; text: string }
  | { kind: "note"; id: number };

/** An Ask across notes answer with its "[note 12]" citations as notes to
 *  open; "[note 3, note 7]" cites both. */
export const notesAnswerParts = (answer: string): NotesAnswerPart[] => {
  const parts: NotesAnswerPart[] = [];
  const re = /\[(note \d+(?:\s*,\s*note \d+)*)\]/gi;
  let last = 0;
  for (const match of answer.matchAll(re)) {
    const at = match.index ?? 0;
    if (at > last) parts.push({ kind: "text", text: answer.slice(last, at) });
    for (const id of match[1].match(/\d+/g) ?? [])
      parts.push({ kind: "note", id: Number(id) });
    last = at + match[0].length;
  }
  if (last < answer.length)
    parts.push({ kind: "text", text: answer.slice(last) });
  return parts;
};

/** The transcript as text, for Copy transcript. */
/** What transcripts call each side, and a voice the speaker model found. */
export interface SpeakerNames {
  you: string;
  them: string;
  /** "Speaker 2". */
  voice: (n: number) => string;
}

/** Who said a line: the name given to its voice, "Speaker 2", or You or
 *  Them before voices were told apart. */
/** The speaker number a name for the whole other side goes under, before
 *  (or without) the speaker model telling its voices apart (as in Rust). */
export const THEM_SIDE = -1;

export const speakerLabel = (
  line: NoteLine,
  speakers: NoteSpeaker[] | undefined,
  names: SpeakerNames,
): string => {
  const n = line.speaker;
  if (n == null)
    return line.who === "you"
      ? names.you
      : speakers?.find((s) => s.speaker === THEM_SIDE)?.name || names.them;
  return speakers?.find((s) => s.speaker === n)?.name || names.voice(n);
};

/** How many voice colours the theme has (`--voice-1` to `--voice-8`). */
export const VOICE_COLOURS = 8;

/** The colour a line's speaker is named in: one per voice the speaker
 *  model found, teal for you, plain for the other side before that. */
export const voiceColour = (line: NoteLine): string =>
  line.speaker != null
    ? `var(--voice-${((((line.speaker - 1) % VOICE_COLOURS) + VOICE_COLOURS) % VOICE_COLOURS) + 1})`
    : line.who === "you"
      ? "var(--teal-700)"
      : "var(--text-2)";

/** The speaker a line's name belongs to, so renaming it renames all their
 *  lines: its voice, else the other side as a whole. You aren't renamed. */
export const renamable = (line: NoteLine): number | null =>
  line.speaker ?? (line.who === "them" ? THEM_SIDE : null);

/** The voices found in a transcript, in order. */
export const voicesIn = (lines: NoteLine[]): number[] =>
  [
    ...new Set(lines.flatMap((l) => (l.speaker == null ? [] : [l.speaker]))),
  ].sort((a, b) => a - b);

/** Who a line can be given to: a side, or one of the voices on it. */
export interface Voice {
  who: Speaker;
  speaker: number | null;
}

/** Said by's choices once voices were told apart: each voice, plus You or
 *  Them for a side with no voices of its own. Empty before that. */
export const voiceChoices = (lines: NoteLine[]): Voice[] => {
  const found = new Map<number, Speaker>();
  for (const l of lines)
    if (l.speaker != null && !found.has(l.speaker)) found.set(l.speaker, l.who);
  if (!found.size) return [];
  const voices: Voice[] = [...found]
    .sort((a, b) => a[0] - b[0])
    .map(([speaker, who]) => ({ who, speaker }));
  const bare = (who: Speaker): Voice[] =>
    voices.some((v) => v.who === who) ? [] : [{ who, speaker: null }];
  return [...bare("you"), ...voices, ...bare("them")];
};

export const transcriptText = (
  lines: NoteLine[],
  names: SpeakerNames,
  speakers?: NoteSpeaker[],
): string =>
  lines
    .map(
      (l) => `${clock(l.at_ms)} ${speakerLabel(l, speakers, names)}: ${l.text}`,
    )
    .join("\n");

/** Whether a line contains every word of the query, ignoring case. */
const isWordChar = (c: string | undefined) => !!c && /[\p{L}\p{N}]/u.test(c);

/** `text` with each whole-word `from` (any case) written as `to`, or null
 *  when it has none; the same rule the backend fixes lines with. */
export const replaceWords = (
  text: string,
  from: string,
  to: string,
): string | null => {
  const needle = from.trim().toLowerCase();
  if (!needle) return null;
  const lower = text.toLowerCase();
  if (lower.length !== text.length) return null;
  let out = "";
  let last = 0;
  let found = false;
  for (let at = lower.indexOf(needle); at !== -1; ) {
    const end = at + needle.length;
    if (!isWordChar(text[at - 1]) && !isWordChar(text[end])) {
      out += text.slice(last, at) + to;
      last = end;
      found = true;
    }
    at = lower.indexOf(needle, end);
  }
  return found ? out + text.slice(last) : null;
};

/** How many lines writing `heard` as `word` would change. */
export const linesToFix = (
  lines: NoteLine[],
  heard: string,
  word: string,
): number =>
  word.trim()
    ? lines.filter((l) => {
        const fixed = replaceWords(l.text, heard, word.trim());
        return fixed !== null && fixed !== l.text;
      }).length
    : 0;

export const lineMatches = (line: NoteLine, query: string): boolean => {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const text = line.text.toLowerCase();
  return words.length > 0 && words.every((w) => text.includes(w));
};

/** The summary as text, for Copy summary. */
export const summaryText = (
  note: Note,
  heads: { decisions: string; actions: string },
): string => {
  const parts = [note.summary.trim()];
  for (const topic of note.outline?.topics ?? [])
    parts.push(`${topic.heading}\n${topic.text}`);
  if (note.decisions.length)
    parts.push(
      `${heads.decisions}\n${note.decisions.map((d) => `- ${d}`).join("\n")}`,
    );
  if (note.actions.length)
    parts.push(
      `${heads.actions}\n${note.actions
        .map((a) => `- [${a.done ? "x" : " "}] ${a.text} (${a.owner})`)
        .join("\n")}`,
    );
  return parts.filter(Boolean).join("\n\n");
};

const sameDay = (a: Date, b: Date) =>
  a.getFullYear() === b.getFullYear() &&
  a.getMonth() === b.getMonth() &&
  a.getDate() === b.getDate();

/** "Today · 1:54 PM", "Thursday · 8:41 PM" within the week, else
 *  "Oct 3 · 3:30 PM". */
export const whenLabel = (
  startedAt: number,
  now: Date,
  locale: string,
): string => {
  const at = new Date(startedAt * 1000);
  const time = at.toLocaleTimeString(locale, {
    hour: "numeric",
    minute: "2-digit",
  });
  const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const days = Math.ceil((midnight.getTime() - at.getTime()) / 86_400_000);
  let day: string;
  if (sameDay(at, now) || days <= 1) {
    const words = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
    day = words.format(sameDay(at, now) ? 0 : -1, "day");
    day = day.charAt(0).toLocaleUpperCase(locale) + day.slice(1);
  } else if (days < 7) {
    day = at.toLocaleDateString(locale, { weekday: "long" });
  } else {
    day = at.toLocaleDateString(locale, {
      month: "short",
      day: "numeric",
      ...(at.getFullYear() === now.getFullYear() ? {} : { year: "numeric" }),
    });
  }
  return `${day} · ${time}`;
};

/** A line around the first whole-word `heard`, for showing a fix: the
 *  words before (cut to about `room` characters), the match and the words
 *  after. Null when the line doesn't have it. */
export const fixContext = (
  text: string,
  heard: string,
  room = 48,
): { before: string; match: string; after: string } | null => {
  const escaped = heard.trim().replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  if (!escaped) return null;
  const found = new RegExp(
    `(?<![\\p{L}\\p{N}])${escaped}(?![\\p{L}\\p{N}])`,
    "iu",
  ).exec(text);
  if (!found) return null;
  const cut = (part: string, fromEnd: boolean) => {
    if (part.length <= room) return part;
    if (fromEnd) {
      const tail = part.slice(part.length - room);
      return `…${tail.slice(tail.indexOf(" ") + 1)}`;
    }
    const head = part.slice(0, room);
    return `${head.slice(0, head.lastIndexOf(" "))}…`;
  };
  return {
    before: cut(text.slice(0, found.index), true),
    match: found[0],
    after: cut(text.slice(found.index + found[0].length), false),
  };
};

export type TaskShow = "mine" | "everyone" | "done";

/** One note's next steps for the Tasks view. */
export interface TaskGroup {
  note: Note;
  actions: Note["actions"];
}

/** Next steps from every note, newest note first: yours or everyone's
 *  still open, or the ones done; `query` matches the step, its owner or the
 *  note's title. */
export const taskGroups = (
  notes: Note[],
  show: TaskShow,
  query: string,
): TaskGroup[] => {
  const q = query.trim().toLowerCase();
  return [...notes]
    .sort((a, b) => b.started_at - a.started_at)
    .map((note) => ({
      note,
      actions: note.actions.filter(
        (a) =>
          (show === "done"
            ? a.done
            : !a.done && (show === "everyone" || isYou(a.owner))) &&
          (!q ||
            a.text.toLowerCase().includes(q) ||
            a.owner.toLowerCase().includes(q) ||
            note.title.toLowerCase().includes(q)),
      ),
    }))
    .filter((g) => g.actions.length > 0);
};

/** An action item's owner is the person who recorded. */
export const isYou = (owner: string) => owner.trim().toLowerCase() === "you";

/** The speeds the recording plays at, in the order the speed button steps
 *  through them. */
export const SPEEDS = [1, 1.25, 1.5, 2];

/** The speed `step` places faster (or slower, below zero) than `rate`, kept
 *  within SPEEDS; with `wrap`, past the fastest comes 1× again. */
export const nextSpeed = (rate: number, step: number, wrap = false): number => {
  const at = SPEEDS.indexOf(rate);
  const i = (at < 0 ? 0 : at) + step;
  if (wrap)
    return SPEEDS[((i % SPEEDS.length) + SPEEDS.length) % SPEEDS.length];
  return SPEEDS[Math.min(SPEEDS.length - 1, Math.max(0, i))];
};

/** A moment the seek bar marks: where a topic or detail of the summary
 *  starts. */
export interface PlayerMark {
  ms: number;
  label: string;
}

/** The summary's timed topics and details, in time order, inside the
 *  recording. Two within a second of each other show as one, the topic
 *  first, since a detail usually starts where its topic does. */
export const playerMarks = (
  outline: NoteOutline | null | undefined,
  total: number,
): PlayerMark[] => {
  const all = [
    ...(outline?.topics ?? []).map((t) => ({ ms: t.at_ms, label: t.heading })),
    ...(outline?.details ?? []).map((d) => ({ ms: d.at_ms, label: d.heading })),
  ].filter(
    (m): m is PlayerMark =>
      m.ms != null && m.ms >= 0 && m.ms <= total && !!m.label.trim(),
  );
  const marks: PlayerMark[] = [];
  for (const m of all.sort((a, b) => a.ms - b.ms))
    if (!marks.some((k) => Math.abs(k.ms - m.ms) < 1000)) marks.push(m);
  return marks;
};

/** A stretch of the recording one speaker holds, for the band under the
 *  seek bar. */
export interface SpeakerBand {
  from: number;
  to: number;
  colour: string;
}

/** Who speaks where: each line's span in its speaker's colour, joined with
 *  the next when the same speaker carries on within three seconds. Empty
 *  when only one colour would show, since one band says nothing. */
export const speakerBands = (lines: NoteLine[]): SpeakerBand[] => {
  const bands: SpeakerBand[] = [];
  for (const line of lines) {
    const colour = voiceColour(line);
    const to = Math.max(line.end_ms, line.at_ms);
    const last = bands[bands.length - 1];
    if (last && last.colour === colour && line.at_ms - last.to <= 3000)
      last.to = Math.max(last.to, to);
    else bands.push({ from: line.at_ms, to, colour });
  }
  return new Set(bands.map((b) => b.colour)).size > 1 ? bands : [];
};

/** Where the next line starts (`dir` 1) or the one playing, or the one
 *  before when the playing line began under two seconds ago (`dir` -1), as
 *  a music player's previous does. None past the last line. */
export const lineStep = (
  lines: NoteLine[],
  ms: number,
  dir: 1 | -1,
): number | null => {
  const i = lineAt(lines, ms);
  if (dir === 1) return lines[i + 1]?.at_ms ?? null;
  if (i < 0) return 0;
  if (ms - lines[i].at_ms > 2000) return lines[i].at_ms;
  return lines[i - 1]?.at_ms ?? 0;
};
