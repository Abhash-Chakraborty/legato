import React from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Trash2, type LucideIcon } from "lucide-react";
import type { Snippet } from "@/bindings";
import { AppIcon } from "../home/AppIcon";
import { relativeTime } from "../dictionary/list";
import { SnippetText } from "./SnippetText";

const RowButton: React.FC<{
  label: string;
  icon: LucideIcon;
  onClick: () => void;
  danger?: boolean;
}> = ({ label, icon: Icon, onClick, danger }) => (
  <button
    type="button"
    onClick={(e) => {
      e.stopPropagation();
      onClick();
    }}
    aria-label={label}
    title={label}
    className={`grid h-8 w-8 place-items-center rounded-[var(--r-sm)] transition-colors duration-[var(--dur-1)] hover:duration-0 active:scale-[0.97] ${
      danger
        ? "text-text-2 hover:bg-danger-soft hover:text-danger"
        : "text-text-2 hover:bg-surface-3 hover:text-text"
    }`}
  >
    <Icon className="h-4 w-4 stroke-[1.75]" aria-hidden="true" />
  </button>
);

interface SnippetRowProps {
  snippet: Snippet;
  focused: boolean;
  selected: boolean;
  /** Some row is selected, so every checkbox shows. */
  selecting: boolean;
  now: number;
  onFocusRow: () => void;
  onSelect: (selected: boolean) => void;
  onEdit: () => void;
  onDelete: () => void;
}

export const SnippetRow: React.FC<SnippetRowProps> = ({
  snippet,
  focused,
  selected,
  selecting,
  now,
  onFocusRow,
  onSelect,
  onEdit,
  onDelete,
}) => {
  const { t, i18n } = useTranslation();

  return (
    <li
      data-row={snippet.id}
      tabIndex={focused ? 0 : -1}
      aria-label={snippet.trigger}
      aria-selected={selected}
      onFocus={(e) => e.target === e.currentTarget && onFocusRow()}
      onClick={onEdit}
      className={`group relative flex cursor-pointer items-center gap-3 border-b border-line py-3 ps-3 pe-4 transition-colors duration-[var(--dur-1)] last:border-b-0 hover:duration-0 focus-visible:outline-offset-[-2px] ${
        selected
          ? "bg-teal-50 hover:bg-teal-50"
          : "hover:bg-surface-2 focus-visible:bg-surface-2"
      }`}
    >
      <label
        onClick={(e) => e.stopPropagation()}
        className={`grid h-8 w-8 flex-none cursor-pointer place-items-center ${
          selecting || selected
            ? ""
            : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
        }`}
      >
        <input
          type="checkbox"
          className="check"
          checked={selected}
          onChange={(e) => onSelect(e.target.checked)}
          aria-label={t("snippets.row.select", { trigger: snippet.trigger })}
        />
      </label>
      <div className="w-[220px] min-w-0 flex-none">
        <div data-snippet-trigger className="truncate text-[15px] font-medium">
          {snippet.trigger}
        </div>
        {snippet.alone && (
          <div className="mt-0.5 truncate text-[13px] text-text-3">
            {t("snippets.row.alone")}
          </div>
        )}
      </div>
      <div className="flex min-w-0 flex-1 text-sm text-text-2">
        <SnippetText text={snippet.text} />
      </div>

      <div className="w-[170px] flex-none text-end text-[13px] group-focus-within:invisible group-hover:invisible">
        {snippet.uses > 0 ? (
          <>
            <div className="tabular-nums text-text-2">
              {t("snippets.row.used", { count: snippet.uses })}
            </div>
            {snippet.last_used_at && (
              <div className="mt-0.5 inline-flex max-w-full items-center gap-1 text-xs text-text-3">
                <span className="truncate">
                  {snippet.last_app
                    ? t("snippets.row.lastIn", {
                        when: relativeTime(
                          snippet.last_used_at,
                          now,
                          i18n.language,
                        ),
                      })
                    : relativeTime(snippet.last_used_at, now, i18n.language)}
                </span>
                {snippet.last_app && (
                  <>
                    <AppIcon path={snippet.last_app_path} />
                    <span className="truncate">{snippet.last_app}</span>
                  </>
                )}
              </div>
            )}
          </>
        ) : (
          <div className="text-xs text-text-3">{t("snippets.row.notUsed")}</div>
        )}
      </div>

      <div className="invisible absolute end-3 top-1/2 flex -translate-y-1/2 items-center gap-0.5 group-focus-within:visible group-hover:visible">
        <RowButton
          label={t("snippets.row.edit", { trigger: snippet.trigger })}
          icon={Pencil}
          onClick={onEdit}
        />
        <RowButton
          label={t("snippets.row.delete", { trigger: snippet.trigger })}
          icon={Trash2}
          danger
          onClick={onDelete}
        />
      </div>
    </li>
  );
};
