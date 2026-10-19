import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands, type ScratchNote, type ScratchVersion } from "@/bindings";
import { Dialog } from "../ui/Dialog";
import { Button } from "../ui/Button";
import { useScratchStore } from "../../stores/scratchStore";
import { wordCount } from "./notes";

const when = (at: number, language: string) =>
  new Date(at * 1000).toLocaleString(language, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });

/** Earlier texts of a note, newest first, each one restorable. Restoring
 *  keeps the current text as a version too. */
export const VersionHistory: React.FC<{
  note: ScratchNote;
  onClose: () => void;
}> = ({ note, onClose }) => {
  const { t, i18n } = useTranslation();
  const put = useScratchStore((s) => s.put);
  const [versions, setVersions] = useState<ScratchVersion[] | null>(null);
  // null is the current text.
  const [chosen, setChosen] = useState<number | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    commands.getScratchVersions(note.id).then((result) => {
      if (result.status === "ok") setVersions(result.data);
      else setVersions([]);
    });
  }, [note.id]);

  const items: { id: number | null; text: string; at: number }[] = [
    { id: null, text: note.text, at: note.updated_at },
    ...(versions ?? []),
  ];
  const shown = items.find((v) => v.id === chosen) ?? items[0];

  const restore = async () => {
    if (chosen === null) return;
    const result = await commands.restoreScratchVersion(note.id, chosen);
    if (result.status === "error") {
      toast.error(t("scratchpad.versions.failed"), {
        description: result.error,
      });
      return;
    }
    put(result.data);
    toast(
      t("scratchpad.versions.restored", {
        when: when(shown.at, i18n.language),
      }),
    );
    onClose();
  };

  const onListKey = (e: React.KeyboardEvent) => {
    const i = items.findIndex((v) => v.id === chosen);
    const to =
      e.key === "ArrowDown"
        ? Math.min(i + 1, items.length - 1)
        : e.key === "ArrowUp"
          ? Math.max(i - 1, 0)
          : -1;
    if (to < 0) return;
    e.preventDefault();
    setChosen(items[to].id);
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={t("scratchpad.versions.title")}
      description={t("scratchpad.versions.description")}
      closeLabel={t("scratchpad.versions.close")}
      className="max-w-[640px]"
      contentFades={false}
      initialFocusRef={listRef as React.RefObject<HTMLElement>}
      footer={
        <>
          <Button variant="secondary" size="sm" onClick={onClose}>
            {t("scratchpad.versions.cancel")}
          </Button>
          <Button size="sm" disabled={chosen === null} onClick={restore}>
            {t("scratchpad.versions.restore")}
          </Button>
        </>
      }
    >
      <div className="grid min-h-[280px] grid-cols-[minmax(0,180px)_minmax(0,1fr)] gap-4">
        <div
          ref={listRef}
          role="listbox"
          tabIndex={0}
          aria-label={t("scratchpad.versions.list")}
          aria-activedescendant={`version-${chosen ?? "current"}`}
          onKeyDown={onListKey}
          className="flex max-h-[340px] flex-col gap-0.5 overflow-auto rounded-[var(--r-md)] p-0.5 outline-none focus-visible:ring-2 focus-visible:ring-teal-500"
        >
          {items.map((v) => (
            <button
              key={v.id ?? "current"}
              id={`version-${v.id ?? "current"}`}
              type="button"
              role="option"
              tabIndex={-1}
              aria-selected={v.id === chosen}
              onClick={() => setChosen(v.id)}
              className={`flex w-full flex-col items-start gap-px rounded-[var(--r-sm)] px-2.5 py-2 text-start transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                v.id === chosen ? "bg-teal-50" : "hover:bg-surface-2"
              }`}
            >
              <b className="text-[13px] font-medium">
                {v.id === null
                  ? t("scratchpad.versions.current")
                  : when(v.at, i18n.language)}
              </b>
              <span className="text-xs tabular-nums text-text-3">
                {t("scratchpad.words", {
                  count: wordCount(v.text),
                })}
              </span>
            </button>
          ))}
          {versions?.length === 0 && (
            <p className="px-2.5 py-2 text-xs text-text-3">
              {t("scratchpad.versions.none")}
            </p>
          )}
        </div>
        <div className="max-h-[340px] overflow-auto whitespace-pre-wrap break-words rounded-[var(--r-md)] bg-surface-2 px-3.5 py-3 text-sm leading-[1.55]">
          {shown.text || (
            <span className="text-text-4">{t("scratchpad.editor.empty")}</span>
          )}
        </div>
      </div>
    </Dialog>
  );
};
