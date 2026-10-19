import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import type { ScratchNote } from "@/bindings";
import { relativeTime } from "../pages/dictionary/list";
import { wordCount } from "./notes";
import type { SaveState } from "./useNoteDraft";

/** Words in the note, and whether the last edit is saved. */
export const NoteStatus: React.FC<{
  note: ScratchNote;
  text: string;
  state: SaveState;
  onRetry: () => void;
}> = ({ note, text, state, onRetry }) => {
  const { t, i18n } = useTranslation();
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  // Keeps "edited 2 min ago" current.
  useEffect(() => {
    const id = window.setInterval(
      () => setNow(Math.floor(Date.now() / 1000)),
      30_000,
    );
    return () => window.clearInterval(id);
  }, []);
  const [justSaved, setJustSaved] = useState(false);
  const [last, setLast] = useState(state);
  if (state !== last) {
    setLast(state);
    if (last === "saving" && state === "saved") setJustSaved(true);
  }
  useEffect(() => {
    if (!justSaved) return;
    const id = window.setTimeout(() => setJustSaved(false), 1600);
    return () => window.clearTimeout(id);
  }, [justSaved]);

  return (
    <span
      aria-live="polite"
      className="inline-flex min-w-0 items-center gap-[7px] whitespace-nowrap text-xs tabular-nums text-text-3"
    >
      <span>{t("scratchpad.words", { count: wordCount(text) })}</span>
      <span className="h-[3px] w-[3px] flex-none rounded-full bg-text-4" />
      {state === "failed" ? (
        <span className="text-danger">
          {t("scratchpad.status.failed")}{" "}
          <button
            type="button"
            onClick={onRetry}
            className="font-medium underline underline-offset-2"
          >
            {t("scratchpad.status.retry")}
          </button>
        </span>
      ) : state === "saving" ? (
        <span>{t("scratchpad.status.saving")}</span>
      ) : justSaved ? (
        <span className="inline-flex items-center gap-1 text-teal-700">
          <Check
            className="h-[13px] w-[13px] stroke-[2.5]"
            aria-hidden="true"
          />
          {t("scratchpad.status.saved")}
        </span>
      ) : (
        <span>
          {t("scratchpad.status.edited", {
            when: relativeTime(note.updated_at, now, i18n.language),
          })}
        </span>
      )}
    </span>
  );
};
