import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { AppWindow, Copy, History } from "lucide-react";
import { commands, type ScratchNote } from "@/bindings";
import { useScratchStore } from "../../../stores/scratchStore";
import { Button } from "../../ui/Button";
import { Segmented } from "../../ui/Segmented";
import { copyToClipboard } from "../home/clipboard";
import { FailedPasteBanner } from "../../scratchpad/FailedPasteBanner";
import { NotePreview } from "../../scratchpad/NotePreview";
import { NoteStatus } from "../../scratchpad/NoteStatus";
import { VersionHistory } from "../../scratchpad/VersionHistory";
import { useNoteDraft } from "../../scratchpad/useNoteDraft";

type Mode = "write" | "preview";

/** The selected note, editable here as in the window. */
export const NoteCard: React.FC<{
  note: ScratchNote;
  textareaRef: React.RefObject<HTMLTextAreaElement>;
}> = ({ note, textareaRef }) => {
  const { t } = useTranslation();
  const put = useScratchStore((s) => s.put);
  const draft = useNoteDraft(note);
  const [mode, setMode] = useState<Mode>("write");
  const [title, setTitle] = useState(note.auto_title ? "" : note.title);
  const [versions, setVersions] = useState(false);

  useEffect(() => {
    if (document.activeElement?.id !== "note-title")
      setTitle(note.auto_title ? "" : note.title);
  }, [note.id, note.title, note.auto_title]);

  const rename = async () => {
    const next = title.trim();
    if (next === (note.auto_title ? "" : note.title)) return;
    const result = await commands.renameScratchNote(note.id, next);
    if (result.status === "ok") put(result.data);
  };

  const copy = async () => {
    if (draft.text.trim() && (await copyToClipboard(draft.text)))
      toast(t("scratchpad.toast.copied"));
  };

  const openInWindow = async () => {
    await draft.flush();
    const result = await commands.openScratchpad(note.id);
    if (result.status === "error")
      toast.error(t("scratchpad.toast.openFailed"), {
        description: result.error,
      });
  };

  return (
    <section
      aria-label={note.title || t("scratchpad.untitled")}
      className="flex h-[520px] flex-col overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface min-[1000px]:h-auto min-[1000px]:min-h-0 min-[1000px]:flex-1"
    >
      <div className="flex flex-none items-center gap-3 border-b border-line py-2 pe-3 ps-4">
        <input
          id="note-title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={rename}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              textareaRef.current?.focus();
            } else if (e.key === "Escape") {
              setTitle(note.auto_title ? "" : note.title);
            }
          }}
          placeholder={note.title || t("scratchpad.untitled")}
          aria-label={t("scratchpad.card.title")}
          className="h-8 min-w-0 flex-1 rounded-[var(--r-sm)] border border-transparent bg-transparent px-2 -ms-2 text-[15px] font-medium outline-none transition-colors duration-[var(--dur-1)] placeholder:text-text hover:border-line focus:border-teal-600"
        />
        <Segmented
          label={t("scratchpad.editor.mode")}
          value={mode}
          onChange={setMode}
          options={[
            { value: "write", label: t("scratchpad.editor.write") },
            { value: "preview", label: t("scratchpad.editor.preview") },
          ]}
        />
      </div>
      <FailedPasteBanner note={note} text={draft.text} />
      <div className="relative min-h-0 flex-1">
        {mode === "write" ? (
          <textarea
            ref={textareaRef}
            value={draft.text}
            onChange={(e) => draft.setText(e.target.value)}
            aria-label={t("scratchpad.editor.label")}
            placeholder={t("scratchpad.card.placeholder")}
            spellCheck
            className="absolute inset-0 h-full w-full resize-none border-0 bg-transparent px-5 pb-4 pt-3 text-[15px] leading-[1.6] text-text caret-teal-700 placeholder:text-text-4 note-editor"
          />
        ) : (
          <NotePreview
            text={draft.text}
            className="absolute inset-0 px-5 pb-4 pt-3"
          />
        )}
      </div>
      <div className="flex h-[52px] flex-none items-center gap-1.5 border-t border-line pe-2.5 ps-4">
        <NoteStatus
          note={note}
          text={draft.text}
          state={draft.state}
          onRetry={draft.flush}
        />
        <span className="flex-1" />
        <button
          type="button"
          onClick={async () => {
            await draft.flush();
            setVersions(true);
          }}
          disabled={note.versions < 2}
          aria-label={t("scratchpad.actions.versions", {
            count: note.versions,
          })}
          title={
            note.versions < 2
              ? t("scratchpad.card.noVersions")
              : t("scratchpad.actions.versions", { count: note.versions })
          }
          className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 disabled:cursor-not-allowed disabled:opacity-45"
        >
          <History className="h-4 w-4" aria-hidden="true" />
        </button>
        <button
          type="button"
          onClick={copy}
          disabled={!draft.text.trim()}
          aria-label={t("scratchpad.copyAll")}
          title={t("scratchpad.copyAll")}
          className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 disabled:cursor-not-allowed disabled:opacity-45"
        >
          <Copy className="h-4 w-4" aria-hidden="true" />
        </button>
        <Button variant="secondary" size="sm" onClick={openInWindow}>
          <AppWindow className="h-3.5 w-3.5" aria-hidden="true" />
          {t("scratchpad.card.openInWindow")}
        </Button>
      </div>
      {versions && (
        <VersionHistory
          note={
            useScratchStore.getState().notes?.find((n) => n.id === note.id) ??
            note
          }
          onClose={() => setVersions(false)}
        />
      )}
    </section>
  );
};
