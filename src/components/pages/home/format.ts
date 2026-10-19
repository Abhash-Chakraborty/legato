import type { HistoryEntry } from "@/bindings";

/** Local calendar day of a Unix timestamp (seconds), as YYYY-MM-DD. */
export const localDay = (timestamp: number): string => {
  const d = new Date(timestamp * 1000);
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${month}-${day}`;
};

/** Splits rows into runs of the same local day, keeping their order. */
export const groupByDay = (
  entries: HistoryEntry[],
): { day: string; entries: HistoryEntry[] }[] => {
  const groups: { day: string; entries: HistoryEntry[] }[] = [];
  for (const entry of entries) {
    const day = localDay(entry.timestamp);
    const last = groups[groups.length - 1];
    if (last && last.day === day) last.entries.push(entry);
    else groups.push({ day, entries: [entry] });
  }
  return groups;
};

/** "today", "yesterday", or the date written out. */
export const dayLabel = (
  day: string,
  now: Date,
  locale: string,
): { kind: "today" | "yesterday" | "date"; text: string } => {
  const today = localDay(now.getTime() / 1000);
  const yesterday = localDay(now.getTime() / 1000 - 86400);
  if (day === today) return { kind: "today", text: "" };
  if (day === yesterday) return { kind: "yesterday", text: "" };
  const [y, m, d] = day.split("-").map(Number);
  const date = new Date(y, m - 1, d);
  const sameYear = y === now.getFullYear();
  return {
    kind: "date",
    text: date.toLocaleDateString(locale, {
      weekday: "long",
      month: "long",
      day: "numeric",
      ...(sameYear ? {} : { year: "numeric" }),
    }),
  };
};

/** Seconds with one decimal under a minute ("7.9 s"), then minutes and
 *  seconds ("2 min 54 s"). */
export const formatSpeech = (ms: number): string => {
  if (ms < 60_000) return `${(Math.round(ms / 100) / 10).toFixed(1)} s`;
  const total = Math.round(ms / 1000);
  const min = Math.floor(total / 60);
  const sec = total % 60;
  if (min < 60) return sec ? `${min} min ${sec} s` : `${min} min`;
  const h = Math.floor(min / 60);
  const rest = min % 60;
  return rest ? `${h} h ${rest} min` : `${h} h`;
};

/** The text that was pasted: the transform's output when there was one. */
export const finalText = (entry: HistoryEntry): string =>
  entry.post_processed_text ?? entry.transcription_text;

/** The words of a search, for highlighting what matched. */
export const searchTerms = (query: string): string[] =>
  query
    .trim()
    .split(/\s+/)
    .map((t) => t.replace(/"/g, ""))
    .filter(Boolean);
