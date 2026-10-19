import React, { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { commands, type DictionaryEntry, type EntryInput } from "@/bindings";
import { Button } from "../../ui/Button";
import { Sheet } from "../../ui/Sheet";
import { TestSentence } from "./TestSentence";

interface WordEditorProps {
  /** The word to edit, or null to add one. */
  entry: DictionaryEntry | null;
  /** What a new word starts with, such as one from Needs review. */
  initial?: { text: string; heard: string[] };
  entries: DictionaryEntry[];
  pinLimit: number;
  maxChars: number;
  onClose: () => void;
  onSaved: (entry: DictionaryEntry, created: boolean) => void;
  onDelete: (entry: DictionaryEntry) => void;
  onEditOther: (entry: DictionaryEntry) => void;
}

/** Add or edit one word: its spelling, the ways the model mishears it, case,
 *  pin, and a test field that runs the real correction on the draft. */
export const WordEditor: React.FC<WordEditorProps> = ({
  entry,
  initial,
  entries,
  pinLimit,
  maxChars,
  onClose,
  onSaved,
  onDelete,
  onEditOther,
}) => {
  const { t } = useTranslation();
  const textId = useId();
  const heardId = useId();
  const textRef = useRef<HTMLInputElement>(null);
  const heardRef = useRef<HTMLInputElement>(null);
  const [text, setText] = useState(entry?.text ?? initial?.text ?? "");
  const [heard, setHeard] = useState<string[]>(
    entry?.heard ?? initial?.heard ?? [],
  );
  const [heardInput, setHeardInput] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(
    entry?.case_sensitive ?? false,
  );
  const [pinned, setPinned] = useState(entry?.pinned ?? false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => setError(null), [text, pinned]);

  const trimmed = text.trim();
  const duplicate = trimmed
    ? entries.find(
        (e) =>
          e.id !== entry?.id && e.text.toLowerCase() === trimmed.toLowerCase(),
      )
    : undefined;
  const pinnedOthers = entries.filter(
    (e) => e.pinned && e.id !== entry?.id,
  ).length;
  const pinFull = pinnedOthers >= pinLimit && !entry?.pinned;

  const addChip = () => {
    const value = heardInput.trim().replace(/,$/, "").trim();
    if (!value) return;
    if (!heard.some((h) => h.toLowerCase() === value.toLowerCase()))
      setHeard([...heard, value]);
    setHeardInput("");
  };

  const draft: EntryInput = {
    text: trimmed,
    heard: heardInput.trim() ? [...heard, heardInput.trim()] : heard,
    pinned,
    case_sensitive: caseSensitive,
  };

  const save = async () => {
    if (saving) return;
    if (!trimmed) {
      setError(t("dictionary.editor.errors.empty"));
      textRef.current?.focus();
      return;
    }
    if (duplicate) {
      textRef.current?.focus();
      return;
    }
    if (pinned && pinFull) {
      setError(t("dictionary.editor.errors.pinLimit", { count: pinLimit }));
      return;
    }
    setSaving(true);
    const result = entry
      ? await commands.updateDictionaryEntry(entry.id, draft)
      : await commands.addDictionaryEntry(draft);
    setSaving(false);
    if (result.status === "error") {
      setError(result.error);
      return;
    }
    onSaved(result.data, !entry);
  };

  const description = entry
    ? t("dictionary.editor.editDescription", { count: entry.hits })
    : t("dictionary.editor.addDescription");

  return (
    <Sheet
      open
      title={
        entry
          ? t("dictionary.editor.editTitle")
          : t("dictionary.editor.addTitle")
      }
      description={description}
      onClose={onClose}
      onSubmit={save}
      closeLabel={t("dictionary.editor.cancel")}
      initialFocusRef={textRef}
      footer={
        <>
          {entry && (
            <Button
              type="button"
              variant="danger-ghost"
              size="md"
              onClick={() => onDelete(entry)}
              className="me-auto"
            >
              <Trash2 className="h-4 w-4" aria-hidden="true" />
              {t("dictionary.editor.delete")}
            </Button>
          )}
          <Button
            type="button"
            variant="ghost"
            onClick={onClose}
            className={entry ? "" : "ms-auto"}
          >
            {t("dictionary.editor.cancel")}
          </Button>
          <Button type="submit" variant="primary" disabled={saving}>
            {t("dictionary.editor.save")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-5">
        <div>
          <label
            htmlFor={textId}
            className="mb-1.5 block text-[13px] font-medium"
          >
            {t("dictionary.editor.word")}
          </label>
          <input
            id={textId}
            ref={textRef}
            className={`input w-full ${duplicate || error ? "invalid" : ""}`}
            value={text}
            maxLength={maxChars}
            onChange={(e) => setText(e.target.value)}
            placeholder={t("dictionary.editor.wordPlaceholder")}
            autoComplete="off"
            spellCheck={false}
            aria-invalid={!!duplicate || !!error}
            aria-describedby={`${textId}-msg`}
          />
          <p
            id={`${textId}-msg`}
            aria-live="polite"
            className={`mt-1.5 min-h-[18px] text-[13px] ${duplicate || error ? "text-danger" : "text-text-3"}`}
          >
            {duplicate ? (
              <>
                {t("dictionary.editor.errors.duplicate")}{" "}
                <button
                  type="button"
                  onClick={() => onEditOther(duplicate)}
                  className="font-medium text-teal-700 underline-offset-2 hover:underline"
                >
                  {t("dictionary.editor.editThatOne")}
                </button>
              </>
            ) : (
              (error ??
              (text.length > maxChars - 10
                ? t("dictionary.editor.charsLeft", {
                    count: maxChars - text.length,
                  })
                : ""))
            )}
          </p>
        </div>

        <div>
          <label
            htmlFor={heardId}
            className="mb-1.5 block text-[13px] font-medium"
          >
            {t("dictionary.editor.heard")}
          </label>
          <div
            className="flex min-h-[38px] cursor-text flex-wrap items-center gap-1.5 rounded-[var(--r-md)] border border-line-strong bg-surface px-2 py-[5px] transition-[border-color,box-shadow] duration-[var(--dur-1)] focus-within:border-teal-600 focus-within:shadow-[0_0_0_3px_var(--teal-50)]"
            onClick={() => heardRef.current?.focus()}
          >
            {heard.map((h, i) => (
              <span
                key={h}
                className="inline-flex h-6 items-center gap-1 rounded-[var(--r-xs)] bg-surface-3 ps-2 pe-1 text-[13px]"
              >
                {h}
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setHeard(heard.filter((_, j) => j !== i));
                    heardRef.current?.focus();
                  }}
                  aria-label={t("dictionary.editor.removeHeard", { heard: h })}
                  title={t("dictionary.editor.removeHeard", { heard: h })}
                  className="grid h-[18px] w-[18px] place-items-center rounded text-text-3 hover:bg-surface-4 hover:text-text"
                >
                  <X className="h-3 w-3" aria-hidden="true" />
                </button>
              </span>
            ))}
            <input
              id={heardId}
              ref={heardRef}
              value={heardInput}
              onChange={(e) => setHeardInput(e.target.value)}
              onKeyDown={(e) => {
                if ((e.key === "Enter" && !e.ctrlKey) || e.key === ",") {
                  e.preventDefault();
                  addChip();
                } else if (
                  e.key === "Backspace" &&
                  !heardInput &&
                  heard.length > 0
                ) {
                  setHeard(heard.slice(0, -1));
                }
              }}
              onBlur={addChip}
              placeholder={
                heard.length
                  ? t("dictionary.editor.heardMore")
                  : t("dictionary.editor.heardPlaceholder")
              }
              className="h-[26px] min-w-[140px] flex-1 border-0 bg-transparent text-sm text-text outline-none placeholder:text-text-4"
              autoComplete="off"
              spellCheck={false}
            />
          </div>
          <p className="mt-1.5 text-[13px] text-text-3">
            {t("dictionary.editor.heardHelp")}
          </p>
        </div>

        <div className="flex flex-col gap-3">
          <label className="flex cursor-pointer items-start gap-2.5 text-sm">
            <input
              type="checkbox"
              className="check mt-px"
              checked={caseSensitive}
              onChange={(e) => setCaseSensitive(e.target.checked)}
            />
            <span>
              {t("dictionary.editor.caseSensitive")}
              <span className="block text-[13px] text-text-3">
                {t("dictionary.editor.caseSensitiveHelp")}
              </span>
            </span>
          </label>
          <label
            className={`flex items-start gap-2.5 text-sm ${pinFull && !pinned ? "cursor-not-allowed" : "cursor-pointer"}`}
          >
            <input
              type="checkbox"
              className="check mt-px"
              checked={pinned}
              disabled={pinFull && !pinned}
              onChange={(e) => setPinned(e.target.checked)}
            />
            <span>
              {t("dictionary.editor.pinned")}
              <span className="block text-[13px] text-text-3">
                {pinFull && !pinned
                  ? t("dictionary.editor.pinFull", { count: pinLimit })
                  : t("dictionary.editor.pinnedHelp", {
                      pinned: pinnedOthers + (pinned ? 1 : 0),
                      limit: pinLimit,
                    })}
              </span>
            </span>
          </label>
        </div>

        <TestSentence
          draft={draft}
          draftId={entry?.id ?? null}
          label={t("dictionary.editor.test")}
          placeholder={t("dictionary.editor.testPlaceholder")}
        />
      </div>
    </Sheet>
  );
};

/** The toast after a save, with Undo for a new word. */
export const toastSaved = (
  t: TFunction,
  entry: DictionaryEntry,
  created: boolean,
) => {
  if (!created) {
    toast.success(t("dictionary.toast.saved", { word: entry.text }));
    return;
  }
  toast.success(t("dictionary.toast.added", { word: entry.text }), {
    action: {
      label: t("dictionary.toast.undo"),
      onClick: () => {
        commands.deleteDictionaryEntries([entry.id]).catch(() => {});
      },
    },
  });
};
