import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, Copy, Play, Trash2 } from "@/icons";
import { commands, type NoteShot } from "@/bindings";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { clock } from "./notes";

let folder: Promise<string> | null = null;

/** The folder notes keep their pictures in, asked for once. */
export const useNotesFolder = (): string | null => {
  const [dir, setDir] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    folder ??= commands.getNotesFolder().catch(() => {
      folder = null;
      return "";
    });
    folder.then((d) => {
      if (live && d) setDir(d);
    });
    return () => {
      live = false;
    };
  }, []);
  return dir;
};

/** A screenshot's address for an <img>. */
export const shotSrc = (dir: string, shot: NoteShot) =>
  convertFileSrc(`${dir}\\${shot.file}`, "asset");

/** The screenshots of `shots` taken from `from` up to `to` milliseconds. */
export const shotsBetween = (shots: NoteShot[], from: number, to: number) =>
  shots.filter((s) => s.at_ms >= from && s.at_ms < to);

/** A screenshot as a small picture that opens it full size. */
export const ShotThumb: React.FC<{
  shot: NoteShot;
  dir: string;
  onOpen: () => void;
  className?: string;
}> = ({ shot, dir, onOpen, className = "" }) => {
  const { t } = useTranslation();
  const label = t("notes.shots.open", { time: clock(shot.at_ms) });
  return (
    <button
      type="button"
      onClick={onOpen}
      aria-label={label}
      title={label}
      className={`block overflow-hidden rounded-[var(--r-md)] border border-line bg-surface-2 transition-[border-color,box-shadow] duration-[var(--dur-1)] hover:border-line-strong hover:shadow-[var(--shadow-1)] hover:duration-0 ${className}`}
    >
      <img
        src={shotSrc(dir, shot)}
        alt={t("notes.shots.alt", { time: clock(shot.at_ms) })}
        width={shot.width}
        height={shot.height}
        loading="lazy"
        decoding="async"
        draggable={false}
        className="block h-auto w-full"
      />
    </button>
  );
};

/** A screenshot full size, with the ones before and after it, Copy and
 *  Delete. `onPlay` plays the recording from the moment it was taken. */
export const ShotViewer: React.FC<{
  shots: NoteShot[];
  /** The screenshot showing, by id; null closes the viewer. */
  open: number | null;
  setOpen: (id: number | null) => void;
  dir: string;
  onPlay?: (ms: number) => void;
}> = ({ shots, open, setOpen, dir, onPlay }) => {
  const { t } = useTranslation();
  const index = shots.findIndex((s) => s.id === open);
  const shot = index >= 0 ? shots[index] : null;

  useEffect(() => {
    if (!shot) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowLeft" && index > 0) setOpen(shots[index - 1].id);
      if (e.key === "ArrowRight" && index < shots.length - 1)
        setOpen(shots[index + 1].id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shot, index, shots, setOpen]);

  if (!shot) return null;

  const copy = async () => {
    const result = await commands
      .copyNoteScreenshot(shot.id)
      .catch(() => ({ status: "error" as const, error: "" }));
    if (result.status === "ok") toast(t("notes.shots.copied"));
    else toast.error(t("notes.shots.copyFailed"));
  };

  const remove = async () => {
    const result = await commands
      .deleteNoteScreenshot(shot.id)
      .catch(() => null);
    if (!result || result.status === "error" || !result.data) {
      toast.error(t("notes.shots.deleteFailed"));
      return;
    }
    const gone = result.data;
    // The next one takes its place, or the viewer closes.
    setOpen(shots[index + 1]?.id ?? shots[index - 1]?.id ?? null);
    toast(t("notes.shots.deleted"), {
      action: {
        label: t("notes.toast.undo"),
        onClick: () => commands.restoreNoteScreenshot(gone),
      },
    });
  };

  return (
    <Dialog
      open
      onOpenChange={(on) => !on && setOpen(null)}
      title={t("notes.shots.title", { time: clock(shot.at_ms) })}
      description={
        shots.length > 1
          ? t("notes.shots.position", {
              n: index + 1,
              count: shots.length,
            })
          : undefined
      }
      closeLabel={t("notes.shots.close")}
      className="!w-[min(1100px,calc(100vw-48px))] !max-w-none"
      contentFades={false}
      footer={
        <>
          {shots.length > 1 && (
            <div className="me-auto flex gap-1.5">
              <Button
                variant="secondary"
                size="sm"
                disabled={index === 0}
                onClick={() => setOpen(shots[index - 1].id)}
                aria-label={t("notes.shots.previous")}
                title={t("notes.shots.previous")}
              >
                <ChevronLeft className="h-4 w-4" aria-hidden="true" />
              </Button>
              <Button
                variant="secondary"
                size="sm"
                disabled={index === shots.length - 1}
                onClick={() => setOpen(shots[index + 1].id)}
                aria-label={t("notes.shots.next")}
                title={t("notes.shots.next")}
              >
                <ChevronRight className="h-4 w-4" aria-hidden="true" />
              </Button>
            </div>
          )}
          <Button variant="ghost" size="sm" onClick={remove}>
            <Trash2 className="h-4 w-4" aria-hidden="true" />
            {t("notes.shots.delete")}
          </Button>
          {onPlay && (
            <Button
              variant="secondary"
              size="sm"
              onClick={() => {
                onPlay(shot.at_ms);
                setOpen(null);
              }}
            >
              <Play className="h-4 w-4" aria-hidden="true" />
              {t("notes.shots.play")}
            </Button>
          )}
          <Button size="sm" onClick={copy}>
            <Copy className="h-4 w-4" aria-hidden="true" />
            {t("notes.shots.copy")}
          </Button>
        </>
      }
    >
      <img
        src={shotSrc(dir, shot)}
        alt={t("notes.shots.alt", { time: clock(shot.at_ms) })}
        width={shot.width}
        height={shot.height}
        draggable={false}
        className="mx-auto block max-h-[calc(100vh-260px)] w-auto max-w-full rounded-[var(--r-sm)] border border-line object-contain"
      />
    </Dialog>
  );
};
