import type { AppliedFix, DictionaryEntry } from "@/bindings";

/** A word in a transcript: letters and digits, with inner apostrophes,
 *  dots, dashes and underscores ("don't", "Node.js", "co-op"). */
const WORD = /[\p{L}\p{N}](?:[\p{L}\p{N}'’._-]*[\p{L}\p{N}])?/gu;

export interface Token {
  /** Start and end offsets in the text. */
  s: number;
  e: number;
}

export const tokenize = (text: string): Token[] =>
  Array.from(text.matchAll(WORD), (m) => ({
    s: m.index ?? 0,
    e: (m.index ?? 0) + m[0].length,
  }));

const isWordChar = (c: string | undefined) => !!c && /[\p{L}\p{N}]/u.test(c);

/** Whole-word, any-case places where `word` appears in `text`. */
export const findWord = (text: string, word: string): Token[] => {
  const out: Token[] = [];
  const needle = word.toLowerCase();
  if (!needle) return out;
  const hay = text.toLowerCase();
  for (let i = hay.indexOf(needle); i !== -1; i = hay.indexOf(needle, i + 1)) {
    const e = i + needle.length;
    if (!isWordChar(text[i - 1]) && !isWordChar(text[e])) out.push({ s: i, e });
  }
  return out;
};

export interface Segment {
  text: string;
  fix?: AppliedFix;
}

/** Splits text so each word a fix put there can be marked. Later fixes win
 *  where two would overlap. */
export const markFixes = (text: string, fixes: AppliedFix[]): Segment[] => {
  const spans: (Token & { fix: AppliedFix })[] = [];
  for (const fix of [...fixes].reverse()) {
    for (const hit of findWord(text, fix.to)) {
      if (text.slice(hit.s, hit.e) !== fix.to) continue;
      if (spans.some((x) => hit.s < x.e && x.s < hit.e)) continue;
      spans.push({ ...hit, fix });
    }
  }
  spans.sort((a, b) => a.s - b.s);
  const out: Segment[] = [];
  let at = 0;
  for (const span of spans) {
    if (span.s > at) out.push({ text: text.slice(at, span.s) });
    out.push({ text: text.slice(span.s, span.e), fix: span.fix });
    at = span.e;
  }
  if (at < text.length || out.length === 0) out.push({ text: text.slice(at) });
  return out;
};

const norm = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");

const distance = (a: string, b: string) => {
  let prev = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    const cur = [i];
    for (let j = 1; j <= b.length; j++)
      cur[j] = Math.min(
        prev[j] + 1,
        cur[j - 1] + 1,
        prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1),
      );
    prev = cur;
  }
  return prev[b.length];
};

export interface Guess {
  text: string;
  /** Why it was suggested: a known heard-as form, or the closest word. */
  why: "heard" | "close" | null;
}

/** The spelling Fix a word suggests for a misheard span. */
export const guessSpelling = (
  span: string,
  entries: DictionaryEntry[],
): Guess => {
  const s = span.trim().toLowerCase();
  const heard = entries.find((e) => e.heard.some((h) => h.toLowerCase() === s));
  if (heard) return { text: heard.text, why: "heard" };
  const a = norm(span);
  let best: DictionaryEntry | null = null;
  let bestScore = 1;
  for (const e of entries) {
    const b = norm(e.text);
    if (!a || !b) continue;
    const score = distance(a, b) / Math.max(a.length, b.length);
    if (score < bestScore) {
      bestScore = score;
      best = e;
    }
  }
  if (best && bestScore <= 0.34 && best.text !== span.trim())
    return { text: best.text, why: "close" };
  return { text: span.trim(), why: null };
};
