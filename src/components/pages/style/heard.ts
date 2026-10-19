/** A run of the heard text, marked when cleanup dropped or replaced it. */
export interface HeardPart {
  text: string;
  removed: boolean;
}

// Past this many words a side, alignment is skipped and nothing is marked.
const MAX_WORDS = 1500;

/** Words compare without case or punctuation, so capitals and commas Style
 *  added don't count as changes. */
const core = (word: string) =>
  word.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "") || word;

/** The heard text in its own spacing, with the words that didn't make it
 *  into the pasted text marked (longest common subsequence). */
export const heardParts = (heard: string, pasted: string): HeardPart[] => {
  const tokens = heard.match(/\S+\s*/g) ?? [];
  const a = tokens.map((t) => core(t.trim()));
  const b = (pasted.match(/\S+/g) ?? []).map(core);
  if (a.length > MAX_WORDS || b.length > MAX_WORDS)
    return [{ text: heard, removed: false }];

  const w = b.length + 1;
  const dp = new Uint16Array((a.length + 1) * w);
  for (let i = a.length - 1; i >= 0; i--)
    for (let j = b.length - 1; j >= 0; j--)
      dp[i * w + j] =
        a[i] === b[j]
          ? dp[(i + 1) * w + j + 1] + 1
          : Math.max(dp[(i + 1) * w + j], dp[i * w + j + 1]);

  const kept = new Array<boolean>(a.length).fill(false);
  for (let i = 0, j = 0; i < a.length && j < b.length; ) {
    if (a[i] === b[j]) {
      kept[i] = true;
      i++;
      j++;
    } else if (dp[(i + 1) * w + j] >= dp[i * w + j + 1]) i++;
    else j++;
  }

  const parts: HeardPart[] = [];
  tokens.forEach((text, i) => {
    const removed = !kept[i];
    const last = parts[parts.length - 1];
    if (last && last.removed === removed) last.text += text;
    else parts.push({ text, removed });
  });
  return parts;
};
