import React from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Pin, PinOff, Trash2, type LucideIcon } from "lucide-react";
import type { DictionaryEntry } from "@/bindings";
import { AppIcon } from "../home/AppIcon";
import { isLearned, relativeTime, shortDate } from "./list";

const RowButton: React.FC<{
  label: string;
  icon: LucideIcon;
  onClick: () => void;
  pressed?: boolean;
  danger?: boolean;
}> = ({ label, icon: Icon, onClick, pressed, danger }) => (
  <button
    type="button"
    onClick={(e) => {
      e.stopPropagation();
      onClick();
    }}
    aria-label={label}
    title={label}
    aria-pressed={pressed}
    className={`grid h-8 w-8 place-items-center rounded-[var(--r-sm)] transition-colors duration-[var(--dur-1)] hover:duration-0 active:scale-[0.97] ${
      danger
        ? "text-text-2 hover:bg-danger-soft hover:text-danger"
        : pressed
          ? "text-teal-700 hover:bg-surface-3"
          : "text-text-2 hover:bg-surface-3 hover:text-text"
    }`}
  >
    <Icon className="h-4 w-4 stroke-[1.75]" aria-hidden="true" />
  </button>
);

interface WordRowProps {
  entry: DictionaryEntry;
  focused: boolean;
  now: number;
  onFocusRow: () => void;
  onEdit: () => void;
  onPin: () => void;
  onDelete: () => void;
}

export const WordRow: React.FC<WordRowProps> = ({
  entry,
  focused,
  now,
  onFocusRow,
  onEdit,
  onPin,
  onDelete,
}) => {
  const { t, i18n } = useTranslation();
  const lang = i18n.language;

  const origin = isLearned(entry)
    ? t(`dictionary.row.origin.${entry.source}`, {
        when: relativeTime(entry.added_at, now, lang),
      })
    : t(`dictionary.row.origin.${entry.source}`, {
        when: shortDate(entry.added_at, lang),
      });

  return (
    <li
      data-row={entry.id}
      tabIndex={focused ? 0 : -1}
      aria-label={entry.text}
      onFocus={(e) => e.target === e.currentTarget && onFocusRow()}
      onClick={onEdit}
      className="ring-inside group relative flex cursor-pointer items-center gap-4 border-b border-line px-4 py-3 transition-colors duration-[var(--dur-1)] last:border-b-0 hover:bg-surface-2 hover:duration-0 focus-visible:bg-surface-2"
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span data-word-text className="truncate text-[15px] font-medium">
            {entry.text}
          </span>
          {entry.case_sensitive && (
            <span
              className="rounded-[var(--r-xs)] border border-line px-1 text-[11px] font-medium leading-4 text-text-3"
              title={t("dictionary.row.caseSensitive")}
              aria-label={t("dictionary.row.caseSensitive")}
            >
              {t("dictionary.row.caseBadge")}
            </span>
          )}
          {entry.pinned && (
            <Pin
              className="h-3.5 w-3.5 flex-none text-teal-700"
              aria-label={t("dictionary.row.pinned")}
            />
          )}
          {isLearned(entry) && (
            <span className="inline-flex items-center gap-1 text-xs text-teal-700">
              <span
                aria-hidden="true"
                className="h-1.5 w-1.5 rounded-full bg-teal-600"
              />
              {t("dictionary.row.learned")}
            </span>
          )}
        </div>
        <div className="mt-0.5 truncate text-[13px] text-text-3">
          {entry.heard.length > 0 ? (
            <>
              {t("dictionary.row.heardAs")}{" "}
              {entry.heard.map((h, i) => (
                <React.Fragment key={h}>
                  {i > 0 && ", "}
                  <span data-word-heard className="text-text-2">
                    {t("dictionary.row.quoted", { text: h })}
                  </span>
                </React.Fragment>
              ))}
            </>
          ) : (
            t("dictionary.row.noMisses")
          )}
          <span aria-hidden="true" className="text-text-4">
            {" · "}
          </span>
          {origin}
        </div>
      </div>

      <div className="flex-none text-end text-[13px] group-hover:invisible group-focus-within:invisible">
        {entry.hits > 0 ? (
          <>
            <div className="tabular-nums text-text-2">
              {t("dictionary.row.fixedTimes", { count: entry.hits })}
            </div>
            {entry.last_fix_at && (
              <div className="mt-0.5 inline-flex items-center gap-1 text-xs text-text-3">
                {relativeTime(entry.last_fix_at, now, lang)}
                {entry.last_app && (
                  <>
                    <span aria-hidden="true">·</span>
                    <AppIcon path={entry.last_app_path} />
                    {entry.last_app}
                  </>
                )}
              </div>
            )}
          </>
        ) : (
          <div className="text-xs text-text-3">
            {t("dictionary.row.notNeeded")}
          </div>
        )}
      </div>

      <div className="invisible absolute end-3 top-1/2 flex -translate-y-1/2 items-center gap-0.5 group-hover:visible group-focus-within:visible">
        <RowButton
          label={
            entry.pinned
              ? t("dictionary.row.unpin", { word: entry.text })
              : t("dictionary.row.pin", { word: entry.text })
          }
          icon={entry.pinned ? PinOff : Pin}
          pressed={entry.pinned}
          onClick={onPin}
        />
        <RowButton
          label={t("dictionary.row.edit", { word: entry.text })}
          icon={Pencil}
          onClick={onEdit}
        />
        <RowButton
          label={t("dictionary.row.delete", { word: entry.text })}
          icon={Trash2}
          danger
          onClick={onDelete}
        />
      </div>
    </li>
  );
};
