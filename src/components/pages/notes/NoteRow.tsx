import React from "react";
import { useTranslation } from "react-i18next";
import { Mic, Trash2, Users } from "lucide-react";
import type { Note, NoteLine } from "@/bindings";
import { AppIcon } from "../home/AppIcon";
import { clock, durationLabel, whenLabel } from "../../notes/notes";
import { matchSnippet } from "../../scratchpad/notes";

interface NoteRowProps {
  note: Note;
  /** Where a search matched in the transcript, when only that matched. */
  match: NoteLine | null;
  query: string;
  now: Date;
  /** The recording in progress. */
  recording: boolean;
  focused: boolean;
  selected: boolean;
  onFocusRow: () => void;
  onOpen: () => void;
  onDelete: () => void;
}

/** What the row says about a note's state. */
const useStatus = (note: Note, recording: boolean) => {
  const { t } = useTranslation();
  if (recording) return { text: t("notes.status.recording"), live: true };
  switch (note.status) {
    case "recording":
    case "transcribing":
      return { text: t("notes.status.transcribing"), live: true };
    case "summarising":
      return { text: t("notes.status.summarising"), live: true };
    default:
      return {
        text: note.summary.trim()
          ? t("notes.status.summary")
          : t("notes.status.transcript"),
        live: false,
      };
  }
};

/** A note in the list: its app or kind, title, when, how long, and whether
 *  a summary is ready. */
export const NoteRow: React.FC<NoteRowProps> = ({
  note,
  match,
  query,
  now,
  recording,
  focused,
  selected,
  onFocusRow,
  onOpen,
  onDelete,
}) => {
  const { t, i18n } = useTranslation();
  const status = useStatus(note, recording);
  const Glyph = note.kind === "memo" ? Mic : Users;

  return (
    <li
      data-row={note.id}
      tabIndex={focused ? 0 : -1}
      aria-label={note.title}
      aria-current={selected || undefined}
      onFocus={(e) => e.target === e.currentTarget && onFocusRow()}
      onClick={onOpen}
      className={`ring-inside group relative flex min-h-[60px] cursor-default items-start gap-3 border-b border-line px-4 py-3 transition-colors duration-[var(--dur-1)] hover:duration-0 ${
        selected
          ? "bg-teal-50"
          : "hover:bg-surface-2 focus-visible:bg-surface-2"
      }`}
    >
      <span className="mt-0.5 grid h-6 w-6 flex-none place-items-center rounded-[var(--r-sm)] bg-surface-2 text-text-2">
        {note.app_path ? (
          <AppIcon path={note.app_path} className="h-[18px] w-[18px]" />
        ) : (
          <Glyph className="h-3.5 w-3.5 stroke-[1.75]" aria-hidden="true" />
        )}
      </span>
      <div className="min-w-0 flex-1">
        <div
          data-note-title
          className="truncate text-sm font-medium leading-[1.35]"
        >
          {note.title}
        </div>
        <div className="mt-[3px] flex min-w-0 items-center gap-1.5 whitespace-nowrap text-xs text-text-3">
          <span className="tabular-nums">
            {whenLabel(note.started_at, now, i18n.language)}
          </span>
          {!recording && note.duration_ms > 0 && (
            <span className="tabular-nums">
              · {durationLabel(note.duration_ms)}
            </span>
          )}
          <span aria-hidden="true" className="text-text-4">
            ·
          </span>
          <span
            className={`inline-flex min-w-0 items-center gap-1 truncate ${
              recording ? "text-danger" : status.live ? "text-teal-700" : ""
            }`}
          >
            {recording && (
              <span
                aria-hidden="true"
                className="h-1.5 w-1.5 flex-none rounded-full bg-danger"
              />
            )}
            {status.text}
          </span>
        </div>
        {match && (
          <div className="mt-1 flex min-w-0 gap-1.5 text-xs text-text-2">
            <span className="flex-none tabular-nums text-text-3">
              {clock(match.at_ms)}
            </span>
            <span data-note-text className="min-w-0 truncate">
              {matchSnippet(match.text, query) ?? match.text}
            </span>
          </div>
        )}
      </div>
      {!recording && (
        <div
          className={`invisible absolute inset-y-0 end-0 flex items-center pe-3 ps-2.5 group-focus-within:visible group-hover:visible ${
            selected ? "bg-teal-50" : "bg-surface-2"
          }`}
        >
          <button
            type="button"
            tabIndex={-1}
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
            aria-label={t("notes.row.delete", { title: note.title })}
            title={t("notes.row.delete", { title: note.title })}
            className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-danger-soft hover:text-danger hover:duration-0 active:scale-[0.97]"
          >
            <Trash2 className="h-4 w-4 stroke-[1.75]" aria-hidden="true" />
          </button>
        </div>
      )}
    </li>
  );
};
