import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { RotateCcw } from "lucide-react";
import { commands, events, type TrashItem, type TrashKind } from "@/bindings";
import { relativeTime } from "../pages/dictionary/list";
import { durationLabel } from "../notes/notes";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { ConfirmDelete } from "./ConfirmDelete";

/** What's in Recently deleted for one kind, kept up to date. */
export const useTrash = (kind: TrashKind): TrashItem[] => {
  const [items, setItems] = useState<TrashItem[]>([]);
  const load = useCallback(async () => {
    const result = await commands.listTrash().catch(() => null);
    if (result?.status === "ok")
      setItems(result.data.filter((i) => i.kind === kind));
  }, [kind]);
  useEffect(() => {
    load();
    const unlisten = events.trashChanged.listen(() => load());
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [load]);
  return items;
};

/** "Recently deleted · 3" under a list; nothing when it's empty. */
export const RecentlyDeletedLink: React.FC<{
  count: number;
  onOpen: () => void;
}> = ({ count, onOpen }) => {
  const { t } = useTranslation();
  if (!count) return null;
  return (
    <button
      type="button"
      onClick={onOpen}
      className="font-medium text-teal-700 underline-offset-2 hover:underline"
    >
      {t("trash.link", { count })}
    </button>
  );
};

/** Deleted notes of one kind, with Restore and Delete for good. Each goes
 *  for good on its own 30 days after it was deleted. */
export const RecentlyDeleted: React.FC<{
  kind: TrashKind;
  items: TrashItem[];
  open: boolean;
  onClose: () => void;
  /** After an item is back, so the page can open it. */
  onRestored?: (item: TrashItem) => void;
}> = ({ kind, items, open, onClose, onRestored }) => {
  const { t, i18n } = useTranslation();
  const [confirm, setConfirm] = useState<TrashItem | "all" | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const now = Date.now() / 1000;
  const untitled =
    kind === "note" ? t("trash.untitledNote") : t("scratchpad.untitled");

  const restore = async (item: TrashItem) => {
    setBusy(item.id);
    const result = await commands.restoreTrash(item.id).catch(() => null);
    setBusy(null);
    if (result?.status === "ok") {
      toast(t("trash.restored", { title: item.title || untitled }));
      onRestored?.(item);
    } else
      toast.error(t("trash.restoreFailed"), {
        description: result?.status === "error" ? result.error : undefined,
      });
  };

  const removeForGood = async () => {
    const target = confirm;
    setConfirm(null);
    if (!target) return;
    const result =
      target === "all"
        ? await commands.emptyTrash(kind).catch(() => null)
        : await commands.deleteTrashForever(target.id).catch(() => null);
    if (result?.status !== "ok") toast.error(t("trash.deleteFailed"));
  };

  return (
    <>
      <Dialog
        open={open && confirm === null}
        onOpenChange={(next) => !next && onClose()}
        title={t("trash.title")}
        description={t("trash.description", { days: 30 })}
        closeLabel={t("common.close")}
        className="w-[min(560px,calc(100vw-32px))]"
        footer={
          <div className="flex items-center justify-between gap-2">
            <Button
              variant="danger-ghost"
              disabled={!items.length}
              onClick={() => setConfirm("all")}
            >
              {t("trash.empty")}
            </Button>
            <Button variant="ghost" onClick={onClose}>
              {t("common.close")}
            </Button>
          </div>
        }
      >
        {items.length === 0 ? (
          <p className="py-8 text-center text-[13px] text-text-3">
            {t("trash.none")}
          </p>
        ) : (
          <ul className="-mx-2 max-h-[52vh] overflow-y-auto">
            {items.map((item) => {
              const left = Math.max(
                1,
                30 - Math.floor((now - item.deleted_at) / 86_400),
              );
              return (
                <li
                  key={item.id}
                  className="group flex items-start gap-3 rounded-[var(--r-md)] px-2 py-2.5 hover:bg-surface-2"
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium text-text">
                      {item.title || untitled}
                    </p>
                    {item.preview && (
                      <p className="mt-0.5 line-clamp-1 text-[13px] text-text-2">
                        {item.preview}
                      </p>
                    )}
                    <p className="mt-0.5 text-xs text-text-3">
                      {[
                        t("trash.deletedAgo", {
                          when: relativeTime(
                            item.deleted_at,
                            now,
                            i18n.language,
                          ),
                        }),
                        item.duration_ms
                          ? durationLabel(item.duration_ms)
                          : null,
                        t("trash.daysLeft", { count: left }),
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </p>
                  </div>
                  <div className="flex flex-none items-center gap-1">
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={busy === item.id}
                      onClick={() => restore(item)}
                    >
                      <RotateCcw className="h-3.5 w-3.5" aria-hidden="true" />
                      {t("trash.restore")}
                    </Button>
                    <Button
                      size="sm"
                      variant="danger-ghost"
                      onClick={() => setConfirm(item)}
                      aria-label={t("trash.deleteNowLabel", {
                        title: item.title || untitled,
                      })}
                    >
                      {t("trash.deleteNow")}
                    </Button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </Dialog>
      <ConfirmDelete
        open={confirm !== null}
        title={
          confirm === "all"
            ? t("trash.confirmEmpty.title", { count: items.length })
            : t("trash.confirmOne.title", {
                title: (confirm && confirm.title) || untitled,
              })
        }
        description={t("trash.confirmOne.description")}
        confirmLabel={t("trash.deleteNow")}
        onConfirm={removeForGood}
        onCancel={() => setConfirm(null)}
      />
    </>
  );
};
