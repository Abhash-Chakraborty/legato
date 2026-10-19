import React, { useCallback, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, Check, CheckCheck } from "lucide-react";
import { toast } from "sonner";
import { commands, type LearnMode, type PendingWord } from "@/bindings";
import { Button } from "../../ui/Button";
import { AppIcon } from "../home/AppIcon";
import { relativeTime } from "./list";

interface ReviewListProps {
  pending: PendingWord[];
  now: number;
  enabled: boolean;
  mode: LearnMode;
  onEdit: (word: PendingWord) => void;
  onTurnOn: () => void;
}

/** Words corrected right after a paste, waiting to be added, edited first,
 *  or never proposed again. */
export const ReviewList: React.FC<ReviewListProps> = ({
  pending,
  now,
  enabled,
  mode,
  onEdit,
  onTurnOn,
}) => {
  const { t, i18n } = useTranslation();
  const [focusedId, setFocusedId] = useState<number | null>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const ids = pending.map((p) => p.id);
  const activeId =
    focusedId !== null && ids.includes(focusedId) ? focusedId : ids[0];

  const focusRow = useCallback((id: number | undefined) => {
    if (id === undefined) return;
    setFocusedId(id);
    requestAnimationFrame(() =>
      listRef.current
        ?.querySelector<HTMLElement>(`[data-row="${id}"]`)
        ?.focus(),
    );
  }, []);

  const accept = async (words: PendingWord[]) => {
    const result = await commands.acceptPendingWords(words.map((p) => p.id));
    if (result.status === "error") {
      toast.error(t("dictionary.review.toast.addFailed"), {
        description: result.error,
      });
      return;
    }
    const token = result.data;
    toast(
      words.length === 1
        ? t("dictionary.review.toast.added", { word: words[0].word })
        : t("dictionary.review.toast.addedMany", { count: words.length }),
      {
        action: {
          label: t("dictionary.toast.undo"),
          onClick: () => {
            commands.undoAcceptPendingWords(token).catch(() => {});
          },
        },
      },
    );
  };

  const never = async (word: PendingWord) => {
    const removed = await commands.dismissPendingWords([word.id]);
    if (removed.status === "error") return;
    await commands.addNeverLearnWord(word.word);
    toast(t("dictionary.review.toast.never", { word: word.word }), {
      action: {
        label: t("dictionary.toast.undo"),
        onClick: async () => {
          await commands.removeNeverLearnWord(word.word);
          await commands.restorePendingWords(removed.data);
        },
      },
    });
  };

  if (pending.length === 0) {
    return (
      <div className="grid place-items-center gap-1.5 px-6 py-16 text-center">
        <h2 className="text-[15px] font-medium">
          {t("dictionary.review.empty.title")}
        </h2>
        <p className="max-w-[440px] text-[13px] text-text-3">
          {!enabled
            ? t("dictionary.review.empty.off")
            : mode === "auto"
              ? t("dictionary.review.empty.auto")
              : t("dictionary.review.empty.ask")}
        </p>
        {!enabled && (
          <Button className="mt-3" size="sm" onClick={onTurnOn}>
            {t("dictionary.review.empty.turnOn")}
          </Button>
        )}
      </div>
    );
  }

  const onKeyDown = (e: React.KeyboardEvent<HTMLUListElement>) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-row]");
    if (!row || e.target !== row) return;
    const id = Number(row.dataset.row);
    const word = pending.find((p) => p.id === id);
    const i = ids.indexOf(id);
    if (!word) return;
    const key = e.key.toLowerCase();
    if (key === "arrowdown" || key === "j") {
      e.preventDefault();
      focusRow(ids[Math.min(i + 1, ids.length - 1)]);
    } else if (key === "arrowup" || key === "k") {
      e.preventDefault();
      focusRow(ids[Math.max(i - 1, 0)]);
    } else if (key === "enter" || key === "a") {
      e.preventDefault();
      focusRow(ids[i + 1] ?? ids[i - 1]);
      accept([word]);
    } else if (key === "e") {
      e.preventDefault();
      onEdit(word);
    } else if (key === "delete") {
      e.preventDefault();
      focusRow(ids[i + 1] ?? ids[i - 1]);
      never(word);
    }
  };

  return (
    <div>
      <h2 className="text-[15px] font-medium">
        {t("dictionary.review.title")}
      </h2>
      <p className="mb-3 mt-0.5 text-[13px] text-text-3">
        {t("dictionary.review.help")}
      </p>
      <ul
        ref={listRef}
        aria-label={t("dictionary.tabs.review")}
        onKeyDown={onKeyDown}
        className="overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface"
      >
        {pending.map((p) => (
          <li
            key={p.id}
            data-row={p.id}
            tabIndex={p.id === activeId ? 0 : -1}
            aria-label={t("dictionary.review.rowLabel", {
              heard: p.heard,
              word: p.word,
            })}
            onFocus={(e) => e.target === e.currentTarget && setFocusedId(p.id)}
            className="ring-inside flex items-center gap-4 border-b border-line px-4 py-3 last:border-b-0 focus-visible:bg-surface-2"
          >
            <div className="min-w-0 flex-1">
              <div className="flex min-w-0 items-center gap-2 text-[15px]">
                <span className="truncate text-text-3">{p.heard}</span>
                <ArrowRight
                  className="h-3.5 w-3.5 flex-none text-text-4"
                  aria-hidden="true"
                />
                <span className="truncate font-medium">{p.word}</span>
              </div>
              <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[13px] text-text-3">
                {p.app && (
                  <>
                    <AppIcon path={p.app_path} className="h-3.5 w-3.5" />
                    <span className="flex-none">{p.app}</span>
                    <span aria-hidden="true" className="text-text-4">
                      ·
                    </span>
                  </>
                )}
                <span className="flex-none tabular-nums">
                  {relativeTime(p.at, now, i18n.language)}
                </span>
                {p.context && (
                  <>
                    <span aria-hidden="true" className="text-text-4">
                      ·
                    </span>
                    <span className="min-w-0 truncate">
                      {t("dictionary.row.quoted", { text: p.context })}
                    </span>
                  </>
                )}
              </div>
            </div>
            <div className="flex flex-none items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                tabIndex={-1}
                onClick={() => never(p)}
              >
                {t("dictionary.review.never")}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                tabIndex={-1}
                onClick={() => onEdit(p)}
              >
                {t("dictionary.review.edit")}
              </Button>
              <Button
                variant="primary-soft"
                size="sm"
                tabIndex={-1}
                onClick={() => accept([p])}
              >
                <Check className="h-3.5 w-3.5" aria-hidden="true" />
                {t("dictionary.review.add")}
              </Button>
            </div>
          </li>
        ))}
      </ul>
      {pending.length > 1 && (
        <div className="mt-3 flex items-center justify-between gap-3">
          <p className="text-[13px] text-text-3">
            {t("dictionary.review.keys")}
          </p>
          <Button variant="secondary" size="sm" onClick={() => accept(pending)}>
            <CheckCheck className="h-3.5 w-3.5" aria-hidden="true" />
            {t("dictionary.review.addAll", { count: pending.length })}
          </Button>
        </div>
      )}
    </div>
  );
};
