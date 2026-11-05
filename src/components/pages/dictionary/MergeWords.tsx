import React, { useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { DictionaryEntry } from "@/bindings";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { samePerson } from "../../notes/notes";
import { mainOf, mergeWords } from "./merge";

/** Picks a word to merge `entry` with, and which spelling stays the main
 *  one; the other is kept as a way it's also written. Words that look
 *  like the same name come first. */
export const MergeWords: React.FC<{
  entry: DictionaryEntry;
  entries: DictionaryEntry[];
  onClose: () => void;
}> = ({ entry, entries, onClose }) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const others = useMemo(() => {
    const related = (e: DictionaryEntry) =>
      samePerson(e.text, entry.text) ||
      e.text.toLowerCase().includes(entry.text.toLowerCase()) ||
      entry.text.toLowerCase().includes(e.text.toLowerCase());
    return entries
      .filter((e) => e.id !== entry.id)
      .sort(
        (a, b) =>
          Number(related(b)) - Number(related(a)) ||
          a.text.localeCompare(b.text),
      );
  }, [entries, entry]);
  const shown = others.filter((e) =>
    e.text.toLowerCase().includes(query.trim().toLowerCase()),
  );
  const [picked, setPicked] = useState<number | undefined>(others[0]?.id);
  const other = others.find((e) => e.id === picked);
  const [mainId, setMainId] = useState<number | null>(null);
  const [keep, merged] = other
    ? mainId === other.id
      ? [other, entry]
      : mainId === entry.id
        ? [entry, other]
        : mainOf(entry, other)
    : [entry, undefined];
  const search = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);

  const confirm = async () => {
    if (!merged || busy) return;
    setBusy(true);
    const ok = await mergeWords(t, keep, merged);
    setBusy(false);
    if (ok) onClose();
  };

  return (
    <Dialog
      open
      onOpenChange={(next) => !next && onClose()}
      title={t("dictionary.merge.title", { word: entry.text })}
      description={t("dictionary.merge.description")}
      closeLabel={t("dictionary.editor.cancel")}
      initialFocusRef={search as React.RefObject<HTMLElement>}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            {t("dictionary.editor.cancel")}
          </Button>
          <Button disabled={!merged || busy} onClick={confirm}>
            {t("dictionary.merge.confirm")}
          </Button>
        </div>
      }
    >
      <input
        ref={search}
        className="input mb-2 w-full"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder={t("dictionary.merge.search")}
        aria-label={t("dictionary.merge.search")}
        autoComplete="off"
        spellCheck={false}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            if (shown[0]) setPicked(shown[0].id);
          }
        }}
      />
      <fieldset className="flex max-h-[220px] flex-col gap-1 overflow-y-auto">
        <legend className="sr-only">{t("dictionary.merge.with")}</legend>
        {shown.length === 0 && (
          <p className="px-1 py-3 text-[13px] text-text-3">
            {t("dictionary.merge.none")}
          </p>
        )}
        {shown.map((e) => (
          <label
            key={e.id}
            className={`flex cursor-pointer items-center gap-2.5 rounded-[var(--r-md)] border px-3 py-1.5 text-[13px] ${
              e.id === picked
                ? "border-teal-600 bg-surface-2"
                : "border-line hover:bg-surface-2"
            }`}
          >
            <input
              type="radio"
              name="merge-word"
              className="h-4 w-4 flex-none accent-[var(--teal-600)]"
              checked={e.id === picked}
              onChange={() => {
                setPicked(e.id);
                setMainId(null);
              }}
            />
            <span className="truncate">{e.text}</span>
          </label>
        ))}
      </fieldset>
      {other && merged && (
        <div className="mt-4">
          <p className="mb-1.5 text-[13px] font-medium">
            {t("dictionary.merge.main")}
          </p>
          <div className="flex flex-wrap gap-1.5">
            {[entry, other].map((e) => (
              <label
                key={e.id}
                className={`inline-flex cursor-pointer items-center gap-2 rounded-[var(--r-md)] border px-2.5 py-1 text-[13px] ${
                  e.id === keep.id
                    ? "border-teal-600 bg-surface-2"
                    : "border-line hover:bg-surface-2"
                }`}
              >
                <input
                  type="radio"
                  name="merge-main"
                  className="h-3.5 w-3.5 accent-[var(--teal-600)]"
                  checked={e.id === keep.id}
                  onChange={() => setMainId(e.id)}
                />
                {e.text}
              </label>
            ))}
          </div>
          <p className="mt-2 text-[13px] text-text-3">
            {t("dictionary.merge.result", {
              word: keep.text,
              other: merged.text,
            })}
          </p>
        </div>
      )}
    </Dialog>
  );
};
