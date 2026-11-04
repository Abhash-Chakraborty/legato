import React, { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Note } from "@/bindings";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { durationLabel, whenLabel } from "../../notes/notes";

/** Picks the note to merge `note` with: the ones recorded near it, the
 *  nearest one before it chosen. Enter merges, Esc keeps both. */
export const MergeDialog: React.FC<{
  note: Note;
  candidates: Note[];
  onMerge: (other: Note) => void;
  onCancel: () => void;
}> = ({ note, candidates, onMerge, onCancel }) => {
  const { t, i18n } = useTranslation();
  const [picked, setPicked] = useState<number | undefined>(candidates[0]?.id);
  const confirm = useRef<HTMLButtonElement>(null);
  const other = candidates.find((n) => n.id === picked);
  const now = new Date();
  return (
    <Dialog
      open
      onOpenChange={(next) => !next && onCancel()}
      title={t("notes.merge.title")}
      description={t("notes.merge.description")}
      closeLabel={t("common.cancel")}
      initialFocusRef={confirm as React.RefObject<HTMLElement>}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel}>
            {t("common.cancel")}
          </Button>
          <Button
            ref={confirm}
            disabled={!other}
            onClick={() => other && onMerge(other)}
          >
            {t("notes.merge.confirm")}
          </Button>
        </div>
      }
    >
      <fieldset className="flex flex-col gap-1.5">
        <legend className="sr-only">{t("notes.merge.with")}</legend>
        {candidates.map((n) => (
          <label
            key={n.id}
            className={`flex cursor-pointer items-start gap-2.5 rounded-[var(--r-md)] border px-3 py-2 text-[13px] ${
              n.id === picked
                ? "border-teal-600 bg-surface-2"
                : "border-line hover:bg-surface-2"
            }`}
          >
            <input
              type="radio"
              name="merge-with"
              className="mt-0.5 h-4 w-4 flex-none accent-[var(--teal-600)]"
              checked={n.id === picked}
              onChange={() => setPicked(n.id)}
            />
            <span className="min-w-0">
              <span className="block truncate">{n.title}</span>
              <span className="block text-[12px] text-text-3">
                {t(
                  n.started_at <= note.started_at
                    ? "notes.merge.before"
                    : "notes.merge.after",
                  {
                    when: whenLabel(n.started_at, now, i18n.language),
                    length: durationLabel(n.duration_ms),
                  },
                )}
              </span>
            </span>
          </label>
        ))}
      </fieldset>
    </Dialog>
  );
};
