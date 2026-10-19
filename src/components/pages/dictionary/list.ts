import type { DictionaryEntry } from "@/bindings";

export type Tab = "all" | "learned" | "mine";
export const TABS: Tab[] = ["all", "learned", "mine"];

export type Sort = "pinned" | "hits" | "recent" | "newest" | "az";
export const SORTS: Sort[] = ["pinned", "hits", "recent", "newest", "az"];

/** Learned words came from Fix a word; the rest the user added. */
export const tabOf = (entry: DictionaryEntry): Exclude<Tab, "all"> =>
  entry.source === "fixed" ? "learned" : "mine";

export const inTab = (entry: DictionaryEntry, tab: Tab) =>
  tab === "all" || tabOf(entry) === tab;

export const matchesQuery = (entry: DictionaryEntry, query: string) => {
  const q = query.trim().toLowerCase();
  return (
    !q ||
    entry.text.toLowerCase().includes(q) ||
    entry.heard.some((h) => h.toLowerCase().includes(q))
  );
};

const lastUsed = (e: DictionaryEntry) => e.last_fix_at ?? 0;

export const sortEntries = (
  entries: DictionaryEntry[],
  sort: Sort,
): DictionaryEntry[] =>
  [...entries].sort((a, b) => {
    switch (sort) {
      case "hits":
        return b.hits - a.hits || a.text.localeCompare(b.text);
      case "recent":
        return lastUsed(b) - lastUsed(a) || b.added_at - a.added_at;
      case "newest":
        return b.added_at - a.added_at || b.id - a.id;
      case "az":
        return a.text.localeCompare(b.text, undefined, {
          sensitivity: "base",
        });
      default:
        return (
          Number(b.pinned) - Number(a.pinned) ||
          b.hits - a.hits ||
          a.text.localeCompare(b.text)
        );
    }
  });

/** "4 min ago" within a week, then a short date. */
export const relativeTime = (
  timestamp: number,
  now: number,
  language: string,
): string => {
  const seconds = Math.round(timestamp - now);
  const rtf = new Intl.RelativeTimeFormat(language, {
    numeric: "auto",
    style: "short",
  });
  const abs = Math.abs(seconds);
  if (abs < 60) return rtf.format(0, "second");
  if (abs < 3600) return rtf.format(Math.round(seconds / 60), "minute");
  if (abs < 86_400) return rtf.format(Math.round(seconds / 3600), "hour");
  if (abs < 7 * 86_400) return rtf.format(Math.round(seconds / 86_400), "day");
  return shortDate(timestamp, language);
};

export const shortDate = (timestamp: number, language: string) =>
  new Date(timestamp * 1000).toLocaleDateString(language, {
    month: "short",
    day: "numeric",
  });

export type Matching = "strict" | "normal" | "loose";
export const MATCHING: Matching[] = ["strict", "normal", "loose"];

/** Dictionary › Matching sets the fuzzy matcher's threshold. */
export const MATCHING_THRESHOLD: Record<Matching, number> = {
  strict: 0.1,
  normal: 0.18,
  loose: 0.28,
};

export const matchingOf = (threshold: number): Matching =>
  threshold <= 0.14 ? "strict" : threshold <= 0.23 ? "normal" : "loose";
