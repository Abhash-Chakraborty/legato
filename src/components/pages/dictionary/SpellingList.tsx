import React, { useCallback, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { toast } from "sonner";
import { commands, type Spelling } from "@/bindings";
import { Button } from "../../ui/Button";
import { AppIcon } from "../home/AppIcon";
import { relativeTime } from "./list";

interface SpellingListProps {
  spellings: Spelling[];
  now: number;
  /** Opens the word editor with the term and its forms; without it the
   *  rows offer only Add and Ignore. */
  onEdit?: (spelling: Spelling) => void;
  /** The list's frame; a bordered box by default. */
  className?: string;
}

/** The forms other than the term itself. */
export const otherForms = (s: Spelling) =>
  s.forms.filter((f) => f.toLowerCase() !== s.word.toLowerCase());

/** Terms the model wrote more than one way, to add in one go or ignore.
 *  Shared by Needs review in the Dictionary and Habits in Insights. */
export const SpellingList: React.FC<SpellingListProps> = ({
  spellings,
  now,
  onEdit,
  className = "overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface",
}) => {
  const { t, i18n } = useTranslation();
  const [focused, setFocused] = useState<string | null>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const words = spellings.map((s) => s.word);
  const active =
    focused !== null && words.includes(focused) ? focused : words[0];

  const focusRow = useCallback((word: string | undefined) => {
    if (word === undefined) return;
    setFocused(word);
    requestAnimationFrame(() =>
      listRef.current
        ?.querySelector<HTMLElement>(`[data-row="${CSS.escape(word)}"]`)
        ?.focus(),
    );
  }, []);

  const add = async (s: Spelling) => {
    const result = await commands.addSpelling(s.word, s.forms);
    if (result.status === "error") {
      toast.error(t("dictionary.review.toast.addFailed"), {
        description: result.error,
      });
      return;
    }
    const id = result.data.id;
    toast(t("dictionary.review.toast.added", { word: s.word }), {
      action: {
        label: t("dictionary.toast.undo"),
        onClick: () => {
          commands.deleteDictionaryEntries([id]).catch(() => {});
        },
      },
    });
  };

  const ignore = async (s: Spelling) => {
    const result = await commands.addNeverLearnWord(s.word);
    if (result.status === "error") return;
    toast(t("dictionary.spelled.toast.ignored", { word: s.word }), {
      action: {
        label: t("dictionary.toast.undo"),
        onClick: () => {
          commands.removeNeverLearnWord(s.word).catch(() => {});
        },
      },
    });
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLUListElement>) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-row]");
    if (!row || e.target !== row) return;
    const word = row.dataset.row ?? "";
    const s = spellings.find((x) => x.word === word);
    const i = words.indexOf(word);
    if (!s) return;
    const key = e.key.toLowerCase();
    if (key === "arrowdown" || key === "j") {
      e.preventDefault();
      focusRow(words[Math.min(i + 1, words.length - 1)]);
    } else if (key === "arrowup" || key === "k") {
      e.preventDefault();
      focusRow(words[Math.max(i - 1, 0)]);
    } else if (key === "enter" || key === "a") {
      e.preventDefault();
      focusRow(words[i + 1] ?? words[i - 1]);
      add(s);
    } else if (key === "e" && onEdit) {
      e.preventDefault();
      onEdit(s);
    } else if (key === "delete") {
      e.preventDefault();
      focusRow(words[i + 1] ?? words[i - 1]);
      ignore(s);
    }
  };

  return (
    <ul
      ref={listRef}
      aria-label={t("dictionary.spelled.title")}
      onKeyDown={onKeyDown}
      className={className}
    >
      {spellings.map((s) => {
        const others = otherForms(s);
        return (
          <li
            key={s.word}
            data-row={s.word}
            tabIndex={s.word === active ? 0 : -1}
            aria-label={t("dictionary.spelled.rowLabel", {
              word: s.word,
              forms: others.join(", "),
            })}
            onFocus={(e) => e.target === e.currentTarget && setFocused(s.word)}
            className="ring-inside flex items-center gap-4 border-b border-line px-4 py-3 last:border-b-0 focus-visible:bg-surface-2"
          >
            <div className="min-w-0 flex-1">
              <div className="flex min-w-0 items-baseline gap-1.5 text-[15px]">
                <span className="flex-none font-medium">{s.word}</span>
                <span className="min-w-0 truncate text-[13px] text-text-3">
                  {t("dictionary.spelled.writtenAs", {
                    forms: others
                      .map((f) => t("dictionary.row.quoted", { text: f }))
                      .join(", "),
                  })}
                </span>
              </div>
              <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[13px] text-text-3">
                <span className="flex-none tabular-nums">
                  {t("dictionary.spelled.times", { count: s.count })}
                </span>
                <span aria-hidden="true" className="text-text-4">
                  ·
                </span>
                <span className="flex-none tabular-nums">
                  {t("dictionary.spelled.last", {
                    when: relativeTime(s.last_at, now, i18n.language),
                  })}
                </span>
                {s.last_app && (
                  <>
                    <span aria-hidden="true" className="text-text-4">
                      ·
                    </span>
                    <AppIcon path={s.last_app_path} className="h-3.5 w-3.5" />
                    <span className="min-w-0 truncate">{s.last_app}</span>
                  </>
                )}
              </div>
            </div>
            <div className="flex flex-none items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                tabIndex={-1}
                onClick={() => ignore(s)}
              >
                {t("dictionary.spelled.ignore")}
              </Button>
              {onEdit && (
                <Button
                  variant="ghost"
                  size="sm"
                  tabIndex={-1}
                  onClick={() => onEdit(s)}
                >
                  {t("dictionary.review.edit")}
                </Button>
              )}
              <Button
                variant="primary-soft"
                size="sm"
                tabIndex={-1}
                onClick={() => add(s)}
              >
                <Check className="h-3.5 w-3.5" aria-hidden="true" />
                {t("dictionary.review.add")}
              </Button>
            </div>
          </li>
        );
      })}
    </ul>
  );
};
