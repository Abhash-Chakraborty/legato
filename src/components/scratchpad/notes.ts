import type { ScratchNote } from "@/bindings";

/** One line of Markdown as plain text: no heading marks, bullets, task
 *  boxes, emphasis, code ticks or link targets. */
const stripLine = (line: string) =>
  line
    .replace(/^\s*#{1,6}\s+/, "")
    .replace(/^\s*(?:[-*+]|\d+[.)])\s+(?:\[[ xX]\]\s+)?/, "")
    .replace(/^>\s?/, "")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .trim();

/** The note's lines as plain text, blank ones dropped. */
export const plainLines = (text: string) =>
  text.split("\n").map(stripLine).filter(Boolean);

export const plainText = (text: string) => plainLines(text).join(" ");

/** The text under the title in a list row: what follows the title, which
 *  is the first line or the start of it. */
export const previewOf = (note: ScratchNote) => {
  const lines = plainLines(note.text);
  const title = note.title.replace(/…$/, "").toLowerCase();
  if (title && lines[0]?.toLowerCase().startsWith(title)) {
    const rest = lines[0].slice(title.length).trim();
    lines.splice(0, 1, ...(rest ? [`…${rest}`] : []));
  }
  return lines.join(" · ");
};

export const wordCount = (text: string) =>
  text.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w)).length;

export const matchesQuery = (note: ScratchNote, query: string) => {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return (
    note.title.toLowerCase().includes(q) ||
    plainText(note.text).toLowerCase().includes(q)
  );
};

/** Notes edited last come first. */
export const byUpdated = (a: ScratchNote, b: ScratchNote) =>
  b.updated_at - a.updated_at || b.id - a.id;

/** Where a search matched in the text, with some words around it, or null
 *  when it only matched the title. */
export const matchSnippet = (text: string, query: string) => {
  const q = query.trim().toLowerCase();
  const plain = plainText(text);
  const i = q ? plain.toLowerCase().indexOf(q) : -1;
  if (i < 0) return null;
  const start = Math.max(0, i - 26);
  const cut = plain.slice(start, i + q.length + 90);
  return start > 0 ? `…${cut}` : cut;
};

/** Dictated words put into text at the cursor, with a space on either side
 *  where they would otherwise run into a word. Returns the new text and
 *  where the cursor goes. */
export const insertAt = (
  text: string,
  start: number,
  end: number,
  words: string,
) => {
  const before = text.slice(0, start);
  const after = text.slice(end);
  const lead = before && !/\s$/.test(before) ? " " : "";
  const trail = after && !/^[\s.,;:!?)]/.test(after) ? " " : "";
  const piece = lead + words.trim() + trail;
  return {
    text: before + piece + after,
    cursor: before.length + lead.length + words.trim().length,
  };
};
