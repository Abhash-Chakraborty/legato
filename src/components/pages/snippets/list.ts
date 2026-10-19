import type { Snippet } from "@/bindings";

export type Sort = "used" | "recent" | "newest" | "az";
export const SORTS: Sort[] = ["used", "recent", "newest", "az"];

/** Matches the trigger or the saved text. */
export const matchesQuery = (snippet: Snippet, query: string) => {
  const q = query.trim().toLowerCase();
  return (
    !q ||
    snippet.trigger.toLowerCase().includes(q) ||
    snippet.text.toLowerCase().includes(q)
  );
};

export const sortSnippets = (snippets: Snippet[], sort: Sort): Snippet[] =>
  [...snippets].sort((a, b) => {
    switch (sort) {
      case "recent":
        return (
          (b.last_used_at ?? 0) - (a.last_used_at ?? 0) ||
          b.added_at - a.added_at
        );
      case "newest":
        return b.added_at - a.added_at || b.id - a.id;
      case "az":
        return a.trigger.localeCompare(b.trigger, undefined, {
          sensitivity: "base",
        });
      default:
        return b.uses - a.uses || a.trigger.localeCompare(b.trigger);
    }
  });

/** A piece of a snippet's text as its row shows it: plain text, a variable
 *  such as {{date:long}}, or a line break. */
export type Piece =
  | { kind: "text"; text: string }
  | { kind: "variable"; name: string }
  | { kind: "break" };

const VARIABLE = /\{\{\s*([a-z]+(?::[a-z]+)?)\s*\}\}/g;

export const pieces = (text: string): Piece[] => {
  const out: Piece[] = [];
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  lines.forEach((line, i) => {
    if (i > 0) out.push({ kind: "break" });
    let last = 0;
    for (const m of line.matchAll(VARIABLE)) {
      const at = m.index ?? 0;
      if (at > last) out.push({ kind: "text", text: line.slice(last, at) });
      out.push({ kind: "variable", name: m[1] });
      last = at + m[0].length;
    }
    if (last < line.length) out.push({ kind: "text", text: line.slice(last) });
  });
  // Blank lines would show as two marks in a row; one says enough.
  return out.filter(
    (p, i) => !(p.kind === "break" && out[i - 1]?.kind === "break"),
  );
};

/** The trigger as the backend stores it, for duplicate checks while typing. */
export const normalizeTrigger = (trigger: string) =>
  trigger
    .split(/\s+/)
    .map((w) => w.replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, ""))
    .filter(Boolean)
    .map((w) => w.toLowerCase())
    .join(" ");
