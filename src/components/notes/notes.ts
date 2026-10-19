import type { Note, NoteLine } from "@/bindings";

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

/** The transcript as text, for Copy transcript. */
export const transcriptText = (
  lines: NoteLine[],
  names: { you: string; them: string },
): string =>
  lines
    .map(
      (l) =>
        `${clock(l.at_ms)} ${l.who === "you" ? names.you : names.them}: ${l.text}`,
    )
    .join("\n");

/** Whether a line contains every word of the query, ignoring case. */
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
