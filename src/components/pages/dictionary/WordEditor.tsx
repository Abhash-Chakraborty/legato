import React, { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { Trash2 } from "lucide-react";
import { toast } from "sonner";
import {
  commands,
  type DictionaryEntry,
  type EntryInput,
  type WordKind,
} from "@/bindings";
import { Button } from "../../ui/Button";
import { ChipField } from "../../ui/ChipField";
import { Segmented } from "../../ui/Segmented";
import { Sheet } from "../../ui/Sheet";
import { TestSentence } from "./TestSentence";

const WORD_KINDS: (WordKind | "word")[] = [
  "word",
  "person",
  "project",
  "organisation",
  "term",
];

interface WordEditorProps {
  /** The word to edit, or null to add one. */
  entry: DictionaryEntry | null;
  /** What a new word starts with, such as one from Needs review. */
  initial?: {
    text: string;
    heard: string[];
    also?: string[];
    kind?: WordKind | null;
  };
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
  const textRef = useRef<HTMLInputElement>(null);
  const [text, setText] = useState(entry?.text ?? initial?.text ?? "");
  const [heard, setHeard] = useState<string[]>(
    entry?.heard ?? initial?.heard ?? [],
  );
  const [heardInput, setHeardInput] = useState("");
  const [also, setAlso] = useState<string[]>(
    entry?.also ?? initial?.also ?? [],
  );
  const [alsoInput, setAlsoInput] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(
    entry?.case_sensitive ?? false,
  );
  const [pinned, setPinned] = useState(entry?.pinned ?? false);
  const [kind, setKind] = useState<WordKind | "word">(
    entry?.kind ?? initial?.kind ?? "word",
  );
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

  const allHeard = heardInput.trim() ? [...heard, heardInput.trim()] : heard;
  const allAlso = alsoInput.trim() ? [...also, alsoInput.trim()] : also;
  const same = (a: string, b: string) =>
    a.trim().toLowerCase() === b.trim().toLowerCase();
  const otherWord = (form: string) =>
    entries.find((e) => e.id !== entry?.id && same(e.text, form));
  // Forms the save would skip, said here rather than dropped quietly.
  const heardWarnings = allHeard.flatMap((h) =>
    otherWord(h)
      ? [t("dictionary.editor.warn.heardIsWord", { form: h })]
      : allAlso.some((a) => same(a, h))
        ? [t("dictionary.editor.warn.heardIsAlso", { form: h })]
        : [],
  );
  const alsoWarnings = allAlso.flatMap((a) => {
    const other = otherWord(a);
    if (other) return [t("dictionary.editor.warn.alsoIsWord", { form: a })];
    const owner = entries.find(
      (e) => e.id !== entry?.id && (e.also ?? []).some((f) => same(f, a)),
    );
    return owner
      ? [t("dictionary.editor.warn.alsoMoves", { form: a, word: owner.text })]
      : [];
  });

  const draft: EntryInput = {
    text: trimmed,
    heard: allHeard,
    also: allAlso,
    pinned,
    case_sensitive: caseSensitive,
    kind: kind === "word" ? null : kind,
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
          <p className="mb-1.5 text-[13px] font-medium">
            {t("dictionary.editor.kind")}
          </p>
          <Segmented
            label={t("dictionary.editor.kind")}
            value={kind}
            onChange={setKind}
            options={WORD_KINDS.map((k) => ({
              value: k,
              label: t(`dictionary.kind.${k}`),
            }))}
          />
          <p className="mt-1.5 text-[13px] text-text-3">
            {t("dictionary.editor.kindHelp")}
          </p>
        </div>

        <ChipField
          label={t("dictionary.editor.heard")}
          help={t("dictionary.editor.heardHelp")}
          chips={heard}
          setChips={setHeard}
          input={heardInput}
          setInput={setHeardInput}
          placeholder={t("dictionary.editor.heardPlaceholder")}
          morePlaceholder={t("dictionary.editor.heardMore")}
          removeLabel={(h) => t("dictionary.editor.removeHeard", { heard: h })}
          warnings={heardWarnings}
        />

        <ChipField
          label={t("dictionary.editor.also")}
          help={t("dictionary.editor.alsoHelp")}
          chips={also}
          setChips={setAlso}
          input={alsoInput}
          setInput={setAlsoInput}
          placeholder={t("dictionary.editor.alsoPlaceholder")}
          morePlaceholder={t("dictionary.editor.heardMore")}
          removeLabel={(a) => t("dictionary.editor.removeAlso", { form: a })}
          warnings={alsoWarnings}
        />

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
