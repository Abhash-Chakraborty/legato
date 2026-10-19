/** One run of a word diff: text both sides share, or text only one has. */
export interface DiffPart {
  kind: "same" | "added" | "removed";
  text: string;
}

/** Past this many words a side, the diff gives up on alignment and shows
 *  the whole text as replaced, so a long rewrite never stalls the UI. */
const MAX_WORDS = 1500;

/** Words with the space after each one, so the parts join back to the text. */
const split = (text: string): string[] => text.match(/\S+\s*|\s+/g) ?? [];

const push = (parts: DiffPart[], kind: DiffPart["kind"], text: string) => {
  const last = parts[parts.length - 1];
  if (last && last.kind === kind) last.text += text;
  else parts.push({ kind, text });
};

/** A word diff of `before` and `after` (longest common subsequence). Words
 *  compare without their trailing space, so a moved line break isn't a
 *  change. Removed text comes before the text that replaced it. */
export function wordDiff(before: string, after: string): DiffPart[] {
  const a = split(before);
  const b = split(after);
  const parts: DiffPart[] = [];
  if (a.length > MAX_WORDS || b.length > MAX_WORDS) {
    if (before) push(parts, "removed", before);
    if (after) push(parts, "added", after);
    return parts;
  }
  const key = (w: string) => w.trimEnd();
  // lcs[i][j]: common words between a[i..] and b[j..].
  const lcs: Uint16Array[] = Array.from(
    { length: a.length + 1 },
    () => new Uint16Array(b.length + 1),
  );
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i][j] =
        key(a[i]) === key(b[j])
          ? lcs[i + 1][j + 1] + 1
          : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (key(a[i]) === key(b[j])) {
      push(parts, "same", b[j]);
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      push(parts, "removed", a[i++]);
    } else {
      push(parts, "added", b[j++]);
    }
  }
  while (i < a.length) push(parts, "removed", a[i++]);
  while (j < b.length) push(parts, "added", b[j++]);
  return parts;
}

/** How many words changed, for a one-line summary. */
export function changeCount(parts: DiffPart[]): {
  added: number;
  removed: number;
} {
  const count = (kind: DiffPart["kind"]) =>
    parts
      .filter((p) => p.kind === kind)
      .reduce((n, p) => n + (p.text.match(/\S+/g)?.length ?? 0), 0);
  return { added: count("added"), removed: count("removed") };
}
