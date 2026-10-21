import React, { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands, type NoteLine } from "@/bindings";
import { useNotesStore } from "../../stores/notesStore";
import { speakerLabel, type SpeakerNames } from "./notes";

/** What transcripts call each side and each voice, in the UI's words. */
export const useSpeakerNames = (): SpeakerNames => {
  const { t } = useTranslation();
  return {
    you: t("notes.who.you"),
    them: t("notes.who.them"),
    voice: (n) => t("notes.speakers.voice", { n }),
  };
};

/** Who said a line. A voice the speaker model found is renamed in place,
 *  during the recording or after it, and the name goes to all its lines. */
export const SpeakerTag: React.FC<{
  line: NoteLine;
  className?: string;
}> = ({ line, className = "" }) => {
  const { t } = useTranslation();
  const names = useSpeakerNames();
  const speakers = useNotesStore(
    (s) => s.notes?.find((n) => n.id === line.note_id)?.speakers,
  );
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  // Esc ends the edit; the blur that follows mustn't save it.
  const open = useRef(false);
  const label = speakerLabel(line, speakers, names);
  const tone = line.who === "you" ? "text-teal-700" : "text-text-2";
  const voice = line.speaker;

  if (voice == null || line.id < 0)
    return (
      <span className={`${className} truncate font-medium ${tone}`}>
        {label}
      </span>
    );

  const entry = speakers?.find((s) => s.speaker === voice);
  const named = entry?.name ?? "";
  const recognised = !!entry?.recognised;

  const save = async () => {
    if (!open.current) return;
    open.current = false;
    setEditing(false);
    const name = draft.trim();
    if (name === named) return;
    const result = await commands.nameNoteSpeaker(line.note_id, voice, name);
    if (result.status === "error")
      toast.error(t("notes.speakers.renameFailed"), {
        description: result.error,
      });
  };

  if (editing)
    return (
      <input
        autoFocus
        value={draft}
        maxLength={60}
        placeholder={names.voice(voice)}
        aria-label={t("notes.speakers.renameLabel", { name: label })}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={save}
        onFocus={(e) => e.currentTarget.select()}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.currentTarget.blur();
          } else if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            open.current = false;
            setEditing(false);
          }
        }}
        className={`${className} -mx-1 min-w-0 rounded-[var(--r-xs)] border border-teal-600 bg-surface px-1 font-medium outline-none`}
      />
    );

  return (
    <button
      type="button"
      onClick={() => {
        open.current = true;
        setDraft(named);
        setEditing(true);
      }}
      title={
        recognised
          ? t("notes.speakers.recognised", { name: label })
          : t("notes.speakers.rename", { name: label })
      }
      className={`${className} -mx-1 inline-flex min-w-0 items-center gap-1 rounded-[var(--r-xs)] px-1 text-start font-medium ${tone} transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0`}
    >
      {recognised && (
        <span
          className="h-1.5 w-1.5 flex-none rounded-full bg-teal-600"
          aria-hidden="true"
        />
      )}
      <span className="truncate">{label}</span>
    </button>
  );
};
