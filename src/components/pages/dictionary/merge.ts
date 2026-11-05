import { toast } from "sonner";
import type { TFunction } from "i18next";
import { commands, type DictionaryEntry } from "@/bindings";

/** Which of two words stays the main spelling by default: the one with
 *  more words ("Abhash Chakraborty" over "Abhash"), then the one used more. */
export const mainOf = (
  a: DictionaryEntry,
  b: DictionaryEntry,
): [DictionaryEntry, DictionaryEntry] => {
  const size = (e: DictionaryEntry) => e.text.split(/\s+/).length;
  if (size(a) !== size(b)) return size(a) > size(b) ? [a, b] : [b, a];
  return b.hits > a.hits ? [b, a] : [a, b];
};

/** Joins `other` into `keep`, with an Undo toast that puts both back. */
export const mergeWords = async (
  t: TFunction,
  keep: DictionaryEntry,
  other: DictionaryEntry,
): Promise<boolean> => {
  const result = await commands.mergeDictionaryEntries(keep.id, [other.id]);
  if (result.status === "error") {
    toast.error(t("dictionary.merge.failed"), { description: result.error });
    return false;
  }
  const before = result.data.before;
  toast(t("dictionary.merge.done", { other: other.text, word: keep.text }), {
    action: {
      label: t("dictionary.toast.undo"),
      onClick: () => {
        commands.restoreDictionaryEntries(before).catch(() => {});
      },
    },
  });
  return true;
};
