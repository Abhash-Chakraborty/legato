import type { AppCategory, ModelInfo, PracticeMode } from "@/bindings";
import { wordDiff } from "@/lib/utils/diff";

/** Setup's steps in order. Each is kept in `setup_step`, so setup left
 *  part-way comes back to the same step. */
export const STEPS = [
  "mic",
  "model",
  "message",
  "email",
  "list",
  "words",
  "done",
] as const;
export type Step = (typeof STEPS)[number];

export const PRACTICE = ["message", "email", "list"] as const;
export type Scene = (typeof PRACTICE)[number];

/** The groups the top bar names; Practice covers the three scenes. */
export const GROUPS = ["mic", "model", "practice", "words"] as const;
export type Group = (typeof GROUPS)[number];

export const isStep = (s: string | null | undefined): s is Step =>
  !!s && (STEPS as readonly string[]).includes(s);

export const isScene = (s: string): s is Scene =>
  (PRACTICE as readonly string[]).includes(s);

export const groupOf = (step: Step): Group | null =>
  isScene(step) ? "practice" : step === "done" ? null : step;

/** Where a dictation goes on each step: a scene, or only the hint. */
export const practiceMode = (step: Step): PracticeMode =>
  step === "message"
    ? "chat"
    : step === "email"
      ? "email"
      : step === "list"
        ? "document"
        : "hint";

/** The kind of app each scene is styled as. */
export const SCENE_CATEGORY: Record<Scene, AppCategory> = {
  message: "personal",
  email: "email",
  list: "documents",
};

/** Names in each script a speech model tends to get wrong, which the last
 *  step offers to add to the Dictionary. */
export const SCENE_TERMS: Record<Scene, string[]> = {
  message: [],
  email: ["ALV", "Coolify"],
  list: [],
};

/** A word compared without case or the punctuation around it. */
const wordKey = (w: string) =>
  w.toLowerCase().replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, "");

/** Matches between two word lists (longest common subsequence), as pairs
 *  of indexes. */
function align(a: string[], b: string[]): [number, number][] {
  const ka = a.map(wordKey);
  const kb = b.map(wordKey);
  const lcs: Uint16Array[] = Array.from(
    { length: a.length + 1 },
    () => new Uint16Array(b.length + 1),
  );
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i][j] =
        ka[i] && ka[i] === kb[j]
          ? lcs[i + 1][j + 1] + 1
          : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const pairs: [number, number][] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (ka[i] && ka[i] === kb[j]) {
      pairs.push([i, j]);
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) i++;
    else j++;
  }
  return pairs;
}

const words = (text: string) => text.split(/\s+/).filter(Boolean);

/** A run of what the speech model wrote, and whether the rules took it out. */
export interface HeardRun {
  text: string;
  removed: boolean;
}

/** What the speech model wrote, in runs, with the words that didn't reach
 *  the final text marked removed. Case and punctuation don't count. */
export function struckRuns(heard: string, text: string): HeardRun[] {
  const a = words(heard);
  const kept = new Set(align(a, words(text)).map(([i]) => i));
  const runs: HeardRun[] = [];
  a.forEach((w, i) => {
    // A lone mark ("-") has nothing to keep or remove.
    const removed = !kept.has(i) && wordKey(w) !== "";
    const last = runs[runs.length - 1];
    if (last && last.removed === removed) last.text += ` ${w}`;
    else runs.push({ text: w, removed });
  });
  return runs;
}

/** The runs a self-correction took out: "twelve, actually" from "twelve,
 *  actually one". */
export function droppedRuns(uncorrected: string, corrected: string): string[] {
  if (uncorrected === corrected) return [];
  return wordDiff(uncorrected, corrected)
    .filter((p) => p.kind === "removed")
    .map((p) => p.text.trim())
    .filter((t) => wordKey(t) !== "");
}

const BULLET = /^\s*[-•]\s+(.*)$/;
const NUMBERED = /^\s*\d+[.)]\s+(.*)$/;

/** A paragraph's lines, or a list's items. */
export type Block =
  | { kind: "p"; lines: string[] }
  | { kind: "ul" | "ol"; items: string[] };

/** Text as the scenes lay it out: paragraphs split on blank lines, and runs
 *  of "- " or "1. " lines as lists. */
export function blocks(text: string): Block[] {
  const out: Block[] = [];
  for (const para of text.split(/\n{2,}/)) {
    let current: Block | null = null;
    for (const line of para.split("\n")) {
      const b = line.match(BULLET);
      const n = b ? null : line.match(NUMBERED);
      const kind = b ? "ul" : n ? "ol" : "p";
      const content = (b ?? n)?.[1] ?? line;
      if (!content.trim()) continue;
      if (!current || current.kind !== kind) {
        current =
          kind === "p"
            ? { kind: "p", lines: [] }
            : { kind: kind as "ul" | "ol", items: [] };
        out.push(current);
      }
      if (current.kind === "p") current.lines.push(content);
      else current.items.push(content);
    }
  }
  return out;
}

/** The list a dictation became, if any. */
export function listOf(
  text: string,
): { items: number; numbered: boolean } | null {
  const list = blocks(text).find(
    (b): b is Extract<Block, { kind: "ul" | "ol" }> => b.kind !== "p",
  );
  if (!list) return null;
  return { items: list.items.length, numbered: list.kind === "ol" };
}

/** Words in the text, without list markers. */
export const countWords = (text: string) =>
  words(text.replace(/^\s*(?:[-•]|\d+[.)])\s+/gm, "")).length;

const hasWord = (text: string, term: string) =>
  words(text).some((w) => wordKey(w) === term.toLowerCase());

/** How the speech model wrote a term from the script: the term itself when
 *  it came out right, the words that took its place when it didn't, or null
 *  when there's nothing to tell (it wasn't said). */
export function termAsHeard(
  term: string,
  script: string,
  text: string,
): string | null {
  if (hasWord(text, term)) return term;
  const s = words(script);
  const t = words(text);
  const at = s.findIndex((w) => wordKey(w) === term.toLowerCase());
  if (at < 0) return null;
  const pairs = align(s, t);
  const before = [...pairs].reverse().find(([i]) => i < at);
  const after = pairs.find(([i]) => i > at);
  const from = before ? before[1] + 1 : 0;
  const to = after ? after[1] : t.length;
  if (to <= from || to - from > 4) return null;
  const heard = t
    .slice(from, to)
    .join(" ")
    .replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, "");
  return heard || null;
}

/** The first language tag that isn't English, as its base code ("de"). */
export const otherLanguage = (tags: string[]): string | null =>
  tags.map((t) => t.split("-")[0].toLowerCase()).find((l) => l !== "en") ??
  null;

/** The two models setup offers: the first recommended English model and the
 *  first recommended multilingual one, the latter first when a Windows input
 *  language isn't English. Covering that language beats rank. */
export function pickModels(
  models: ModelInfo[],
  inputLanguages: string[],
  usable: (m: ModelInfo) => boolean = () => true,
): {
  first: ModelInfo | null;
  second: ModelInfo | null;
  because: string | null;
} {
  const pool = models.filter(usable);
  const recommended = pool.filter((m) => m.is_recommended);
  const english = (list: ModelInfo[]) =>
    list.find(
      (m) =>
        m.supported_languages.length === 1 && m.supported_languages[0] === "en",
    );
  const other = otherLanguage(inputLanguages);
  const multi = (list: ModelInfo[]) => {
    const many = list.filter((m) => m.supported_languages.length > 1);
    return other
      ? (many.find((m) => m.supported_languages.includes(other)) ?? null)
      : (many[0] ?? null);
  };
  const en = english(recommended) ?? english(pool) ?? null;
  const ml = multi(recommended) ?? multi(pool) ?? null;
  if (other && ml) return { first: ml, second: en, because: other };
  return { first: en ?? ml, second: en ? ml : null, because: null };
}
