import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { save } from "@tauri-apps/plugin-dialog";
import {
  Copy,
  FileDown,
  FileText,
  MoreVertical,
  Pencil,
  Plus,
  Tag,
  Trash2,
  X,
} from "lucide-react";
import { commands, type Note } from "@/bindings";
import { ActionMenu } from "../../ui/ActionMenu";
import { AppIcon } from "../home/AppIcon";
import { copyToClipboard } from "../home/clipboard";
import { durationLabel, summaryText, transcriptText } from "../../notes/notes";
import { MyNotesTab } from "./MyNotesTab";
import { SummaryTab } from "./SummaryTab";
import { TranscriptTab } from "./TranscriptTab";
import { NotePlayer, useNoteAudio } from "./NoteAudio";
import { useSpeakerNames } from "../../notes/SpeakerTag";

type Tab = "summary" | "transcript" | "mine";

/** A time in the transcript to show, from an Ask answer. */
export interface Jump {
  ms: number;
  /** Bumped on every jump, so the same time can be jumped to twice. */
  n: number;
}

const iconButton =
  "grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]";

/** The title, which turns into a field on click; Enter saves, Esc cancels. */
const Title: React.FC<{
  note: Note;
  editing: boolean;
  setEditing: (on: boolean) => void;
}> = ({ note, editing, setEditing }) => {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(note.title);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!editing) return;
    setDraft(note.title);
    requestAnimationFrame(() => ref.current?.select());
  }, [editing, note.title]);

  const commit = async () => {
    setEditing(false);
    const title = draft.trim();
    if (!title || title === note.title) return;
    const result = await commands.renameNote(note.id, title);
    if (result.status === "error")
      toast.error(t("notes.toast.renameFailed"), {
        description: result.error,
      });
  };

  return editing ? (
    <input
      ref={ref}
      value={draft}
      aria-label={t("notes.detail.titleLabel")}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          commit();
        } else if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          setEditing(false);
        }
      }}
      className="input -ms-2 h-9 w-full px-2 text-xl font-semibold"
    />
  ) : (
    <h2>
      <button
        type="button"
        onClick={() => setEditing(true)}
        title={t("notes.detail.rename")}
        className="-ms-1.5 max-w-full truncate rounded-[var(--r-sm)] px-1.5 text-start text-xl font-semibold leading-9 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0"
      >
        {note.title}
      </button>
    </h2>
  );
};

/** Tags as chips, and Add tag, which offers the tags other notes use. */
const Tags: React.FC<{
  note: Note;
  adding: boolean;
  setAdding: (on: boolean) => void;
}> = ({ note, adding, setAdding }) => {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");
  const [known, setKnown] = useState<string[]>([]);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!adding) return;
    setDraft("");
    commands
      .getNoteTags()
      .then((r) => r.status === "ok" && setKnown(r.data))
      .catch(() => {});
    requestAnimationFrame(() => ref.current?.focus());
  }, [adding]);

  const add = async () => {
    const tag = draft.trim();
    setAdding(false);
    if (!tag) return;
    const result = await commands.addNoteTag(note.id, tag);
    if (result.status === "error")
      toast.error(t("notes.toast.tagFailed"), { description: result.error });
  };

  const offered = known.filter(
    (k) => !note.tags.some((tag) => tag.toLowerCase() === k.toLowerCase()),
  );

  return (
    <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
      {note.tags.map((tag) => (
        <span
          key={tag}
          className="inline-flex h-6 items-center gap-1 rounded-[var(--r-xs)] bg-surface-2 pe-1 ps-2 text-xs font-medium text-text-2"
        >
          {tag}
          <button
            type="button"
            onClick={() => commands.removeNoteTag(note.id, tag).catch(() => {})}
            aria-label={t("notes.detail.removeTag", { tag })}
            className="grid h-4 w-4 place-items-center rounded-[3px] text-text-3 hover:bg-surface-3 hover:text-text"
          >
            <X className="h-3 w-3" aria-hidden="true" />
          </button>
        </span>
      ))}
      {adding ? (
        <>
          <input
            ref={ref}
            value={draft}
            list={`note-tags-${note.id}`}
            aria-label={t("notes.detail.addTag")}
            placeholder={t("notes.detail.tagPlaceholder")}
            maxLength={40}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={add}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                add();
              } else if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                setDraft("");
                setAdding(false);
              }
            }}
            className="input compact h-6 w-36 text-xs"
          />
          <datalist id={`note-tags-${note.id}`}>
            {offered.map((k) => (
              <option key={k} value={k} />
            ))}
          </datalist>
        </>
      ) : (
        <button
          type="button"
          onClick={() => setAdding(true)}
          className="inline-flex h-6 items-center gap-1 rounded-[var(--r-xs)] border border-dashed border-line-strong px-2 text-xs text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0"
        >
          <Plus className="h-3 w-3" aria-hidden="true" />
          {t("notes.detail.addTag")}
        </button>
      )}
    </div>
  );
};

interface NoteDetailProps {
  note: Note;
  /** This note is being recorded, or its last lines transcribed. */
  recording: boolean;
  /** Recording stopped; the last lines are being transcribed. */
  finishing: boolean;
  onDelete: () => void;
}

/** One note: its title, when and where it was recorded, tags, and the
 *  Summary, Transcript and My notes tabs. While it records, only My notes. */
export const NoteDetail: React.FC<NoteDetailProps> = ({
  note,
  recording,
  finishing,
  onDelete,
}) => {
  const { t, i18n } = useTranslation();
  const [tab, setTab] = useState<Tab>("summary");
  const [menu, setMenu] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [tagging, setTagging] = useState(false);
  const [jump, setJump] = useState<Jump | null>(null);
  const menuButton = useRef<HTMLButtonElement>(null);
  const shown: Tab = recording ? "mine" : tab;

  const names = useSpeakerNames();
  const started = new Date(note.started_at * 1000);
  const day = started.toLocaleDateString(i18n.language, {
    weekday: "long",
    month: "short",
    day: "numeric",
  });
  const time = started.toLocaleTimeString(i18n.language, {
    hour: "numeric",
    minute: "2-digit",
  });

  const copySummary = async () => {
    const text = summaryText(note, {
      decisions: t("notes.summary.decisions"),
      actions: t("notes.summary.actions"),
    });
    if (text && (await copyToClipboard(text)))
      toast(t("notes.toast.summaryCopied"));
  };

  const copyFormatted = async () => {
    const result = await commands.copyNoteFormatted(note.id);
    if (result.status === "ok") toast(t("notes.toast.docsCopied"));
    else toast.error(result.error);
  };

  const copyTranscript = async () => {
    const result = await commands.getNoteLines(note.id);
    if (result.status !== "ok" || !result.data.length) return;
    if (
      await copyToClipboard(transcriptText(result.data, names, note.speakers))
    )
      toast(t("notes.toast.transcriptCopied"));
  };

  const exportMarkdown = async () => {
    const name = note.title.replace(/[\\/:*?"<>|]+/g, " ").trim() || "Note";
    const path = await save({
      defaultPath: `${name}.md`,
      filters: [{ name: t("notes.detail.markdownFiles"), extensions: ["md"] }],
    }).catch(() => null);
    if (!path) return;
    const result = await commands.exportNote(note.id, path);
    if (result.status === "ok") toast.success(t("notes.toast.exported"));
    else
      toast.error(t("notes.toast.exportFailed"), {
        description: result.error,
      });
  };

  const player = useNoteAudio(note);

  const showTime = (ms: number) => {
    setTab("transcript");
    setJump((j) => ({ ms, n: (j?.n ?? 0) + 1 }));
  };

  const meta = recording
    ? [t("notes.detail.startedAt", { time }), t(`notes.kind.${note.kind}`)]
    : [day, time, durationLabel(note.duration_ms)];
  const tabs: Tab[] = recording ? ["mine"] : ["summary", "transcript", "mine"];

  return (
    <div className="flex min-w-0 flex-col min-[1000px]:h-full">
      {/* The title, details and tabs stay put; the tab's content scrolls. */}
      <div className="flex-none px-7 pt-6">
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <Title note={note} editing={renaming} setEditing={setRenaming} />
          </div>
          <div className="relative flex-none">
            <button
              ref={menuButton}
              type="button"
              onClick={() => setMenu(!menu)}
              aria-haspopup="menu"
              aria-expanded={menu}
              aria-label={t("notes.detail.more")}
              title={t("notes.detail.more")}
              className={iconButton}
            >
              <MoreVertical className="h-4 w-4" aria-hidden="true" />
            </button>
            {menu && (
              <ActionMenu
                label={t("notes.detail.more")}
                className="end-0 top-full z-10 mt-1 w-56"
                onClose={(restore) => {
                  setMenu(false);
                  if (restore) menuButton.current?.focus();
                }}
                items={[
                  {
                    id: "rename",
                    label: t("notes.detail.rename"),
                    icon: Pencil,
                    onSelect: () => setRenaming(true),
                  },
                  {
                    id: "tag",
                    label: t("notes.detail.addTag"),
                    icon: Tag,
                    onSelect: () => setTagging(true),
                  },
                  "separator",
                  {
                    id: "copy-summary",
                    label: t("notes.detail.copySummary"),
                    icon: Copy,
                    disabled: !note.summary.trim(),
                    onSelect: copySummary,
                  },
                  {
                    id: "copy-docs",
                    label: t("notes.detail.copyDocs"),
                    icon: FileText,
                    disabled: !note.summary.trim(),
                    onSelect: copyFormatted,
                  },
                  {
                    id: "copy-transcript",
                    label: t("notes.detail.copyTranscript"),
                    icon: Copy,
                    disabled: !note.lines,
                    onSelect: copyTranscript,
                  },
                  {
                    id: "export",
                    label: t("notes.detail.export"),
                    icon: FileDown,
                    disabled: recording,
                    onSelect: exportMarkdown,
                  },
                  "separator",
                  {
                    id: "delete",
                    label: t("notes.detail.delete"),
                    icon: Trash2,
                    danger: true,
                    disabled: recording,
                    onSelect: onDelete,
                  },
                ]}
              />
            )}
          </div>
        </div>
        <div className="mt-1 flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-1 text-[13px] text-text-3">
          {recording && (
            <>
              <span
                className={`font-medium ${finishing ? "text-teal-700" : "text-danger"}`}
              >
                {finishing
                  ? t("notes.status.transcribing")
                  : t("notes.status.recording")}
              </span>
              <span aria-hidden="true">·</span>
            </>
          )}
          {meta.map((part, i) => (
            <React.Fragment key={i}>
              {i > 0 && <span aria-hidden="true">·</span>}
              <span className="tabular-nums">{part}</span>
            </React.Fragment>
          ))}
          {note.app && (
            <>
              <span aria-hidden="true">·</span>
              <span className="inline-flex items-center gap-1.5 text-text-2">
                <AppIcon path={note.app_path} className="h-4 w-4" />
                {note.app}
              </span>
            </>
          )}
          <span aria-hidden="true">·</span>
          <span>
            {note.kind === "meeting"
              ? t("notes.detail.youAndOthers")
              : t("notes.detail.justYou")}
          </span>
        </div>
        <Tags note={note} adding={tagging} setAdding={setTagging} />
        {!recording && <NotePlayer note={note} player={player} />}

        <div
          role="tablist"
          aria-label={t("notes.detail.tabs")}
          className="mt-5 flex gap-1 border-b border-line"
        >
          {tabs.map((id) => (
            <button
              key={id}
              type="button"
              role="tab"
              id={`note-tab-${id}`}
              aria-selected={shown === id}
              aria-controls={`note-panel-${id}`}
              onClick={() => setTab(id)}
              onKeyDown={(e) => {
                const step =
                  e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
                if (!step) return;
                e.preventDefault();
                const next =
                  tabs[(tabs.indexOf(id) + step + tabs.length) % tabs.length];
                setTab(next);
                document.getElementById(`note-tab-${next}`)?.focus();
              }}
              tabIndex={shown === id ? 0 : -1}
              className={`-mb-px inline-flex items-center gap-1.5 border-b-2 px-2.5 pb-2.5 pt-1 text-sm transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                shown === id
                  ? "border-teal-600 font-medium text-text"
                  : "border-transparent text-text-2 hover:text-text"
              }`}
            >
              {t(`notes.detail.tab.${id}`)}
              {id === "transcript" && note.lines > 0 && (
                <span className="text-xs tabular-nums text-text-3">
                  {note.lines.toLocaleString(i18n.language)}
                </span>
              )}
            </button>
          ))}
        </div>
      </div>

      <div
        role="tabpanel"
        id={`note-panel-${shown}`}
        aria-labelledby={`note-tab-${shown}`}
        className="px-7 pb-7 pt-5 min-[1000px]:min-h-0 min-[1000px]:flex-1 min-[1000px]:overflow-y-auto"
      >
        {shown === "summary" ? (
          <SummaryTab
            note={note}
            onCopy={copySummary}
            onCopyFormatted={copyFormatted}
            onTime={showTime}
            onPlay={note.audio_file ? player.playFrom : undefined}
            onShowTranscript={() => setTab("transcript")}
          />
        ) : shown === "transcript" ? (
          <TranscriptTab
            note={note}
            jump={jump}
            onCopy={copyTranscript}
            player={player}
          />
        ) : (
          <MyNotesTab note={note} recording={recording} />
        )}
      </div>
    </div>
  );
};
