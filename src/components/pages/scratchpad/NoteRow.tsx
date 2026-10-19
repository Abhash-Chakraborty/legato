import React from "react";
import { useTranslation } from "react-i18next";
import { Copy, Pin, PinOff, Trash2, type LucideIcon } from "lucide-react";
import type { ScratchNote } from "@/bindings";
import { AppIcon } from "../home/AppIcon";
import { relativeTime } from "../dictionary/list";
import { matchSnippet, previewOf } from "../../scratchpad/notes";

const RowButton: React.FC<{
  label: string;
  icon: LucideIcon;
  onClick: () => void;
  danger?: boolean;
}> = ({ label, icon: Icon, onClick, danger }) => (
  <button
    type="button"
    tabIndex={-1}
    onClick={(e) => {
      e.stopPropagation();
      onClick();
    }}
    aria-label={label}
    title={label}
    className={`grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:duration-0 active:scale-[0.97] ${
      danger
        ? "hover:bg-danger-soft hover:text-danger"
        : "hover:bg-surface-3 hover:text-text"
    }`}
  >
    <Icon className="h-4 w-4 stroke-[1.75]" aria-hidden="true" />
  </button>
);

interface NoteRowProps {
  note: ScratchNote;
  query: string;
  now: number;
  focused: boolean;
  selected: boolean;
  onFocusRow: () => void;
  onOpen: () => void;
  onPin: () => void;
  onCopy: () => void;
  onDelete: () => void;
}

/** A note in the list: title, when it was edited and the start of its text,
 *  or where a search matched. */
export const NoteRow: React.FC<NoteRowProps> = ({
  note,
  query,
  now,
  focused,
  selected,
  onFocusRow,
  onOpen,
  onPin,
  onCopy,
  onDelete,
}) => {
  const { t, i18n } = useTranslation();
  const title = note.title || t("scratchpad.untitled");
  const preview = (query && matchSnippet(note.text, query)) || previewOf(note);

  return (
    <li
      data-row={note.id}
      tabIndex={focused ? 0 : -1}
      aria-label={title}
      aria-current={selected || undefined}
      onFocus={(e) => e.target === e.currentTarget && onFocusRow()}
      onClick={onOpen}
      className={`ring-inside group relative flex min-h-16 cursor-default items-start border-b border-line px-4 py-3 transition-colors duration-[var(--dur-1)] last:border-b-0 hover:duration-0 ${
        selected
          ? "bg-teal-50"
          : "hover:bg-surface-2 focus-visible:bg-surface-2"
      }`}
    >
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-1.5 text-[15px] font-medium leading-[1.35]">
          {note.pinned && (
            <Pin
              className="h-[13px] w-[13px] flex-none text-teal-700"
              aria-label={t("scratchpad.pinned")}
            />
          )}
          <span data-note-title className="truncate">
            {title}
          </span>
        </div>
        <div className="mt-[3px] flex min-w-0 gap-2 whitespace-nowrap text-[13px] text-text-3">
          <span className="flex-none tabular-nums text-text-2">
            {relativeTime(note.updated_at, now, i18n.language)}
          </span>
          {note.text.trim() ? (
            <span data-note-text className="min-w-0 truncate">
              {preview}
            </span>
          ) : (
            <span className="text-text-4">{t("scratchpad.emptyNote")}</span>
          )}
        </div>
        {note.source === "failed_paste" && (
          <div className="mt-1.5 flex items-center gap-1.5 text-xs text-text-3">
            <AppIcon path={note.source_app_path} className="h-4 w-4" />
            <span className="truncate">
              {note.source_app
                ? t("scratchpad.row.failedInto", { app: note.source_app })
                : t("scratchpad.row.failed")}
            </span>
          </div>
        )}
      </div>
      <div
        className={`invisible absolute inset-y-0 end-0 flex items-center gap-0.5 pe-3 ps-2.5 group-focus-within:visible group-hover:visible ${
          selected ? "bg-teal-50" : "bg-surface-2"
        }`}
      >
        <RowButton
          label={
            note.pinned
              ? t("scratchpad.row.unpin", { title })
              : t("scratchpad.row.pin", { title })
          }
          icon={note.pinned ? PinOff : Pin}
          onClick={onPin}
        />
        <RowButton
          label={t("scratchpad.row.copy", { title })}
          icon={Copy}
          onClick={onCopy}
        />
        <RowButton
          label={t("scratchpad.row.delete", { title })}
          icon={Trash2}
          danger
          onClick={onDelete}
        />
      </div>
    </li>
  );
};
