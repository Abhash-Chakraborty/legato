import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Mic, Square } from "lucide-react";
import { commands, events, type Note } from "@/bindings";
import { takeDictation, useNotesStore } from "../../../stores/notesStore";
import { Button } from "../../ui/Button";

/** Autosave waits this long after the last key. */
const SAVE_DELAY_MS = 600;

const words = (text: string) => text.split(/\s+/).filter(Boolean).length;

/** Your own notes on a recording: typed or dictated, saved as you go. */
export const MyNotesTab: React.FC<{ note: Note; recording: boolean }> = ({
  note,
  recording,
}) => {
  const { t, i18n } = useTranslation();
  const session = useNotesStore((s) => s.session);
  const [text, setLocal] = useState(note.my_notes);
  const [saved, setSaved] = useState(false);
  const [dictating, setDictating] = useState(false);
  const pending = useRef<string | null>(null);
  const timer = useRef<number | null>(null);
  const ref = useRef<HTMLTextAreaElement>(null);
  const noteId = note.id;

  const flush = useCallback(async () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    const job = pending.current;
    if (job === null) return;
    pending.current = null;
    const result = await commands
      .saveNoteMyNotes(noteId, job)
      .catch(() => null);
    if (result?.status === "ok") {
      setSaved(true);
      window.setTimeout(() => setSaved(false), 1600);
    } else if (pending.current === null) pending.current = job;
  }, [noteId]);

  const setText = useCallback(
    (next: string) => {
      setLocal(next);
      pending.current = next;
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(flush, SAVE_DELAY_MS);
    },
    [flush],
  );

  // Save on leaving the note or the window.
  useEffect(() => {
    const onHide = () => {
      flush();
    };
    window.addEventListener("blur", onHide);
    return () => {
      window.removeEventListener("blur", onHide);
      flush();
    };
  }, [flush]);

  // Saved elsewhere (a dictation while the page was closed): follow it
  // unless an edit here is waiting.
  useEffect(() => {
    if (pending.current === null) setLocal(note.my_notes);
  }, [note.my_notes]);

  // Dictated words go at the cursor.
  const latest = useRef(text);
  latest.current = text;
  useEffect(
    () =>
      takeDictation(noteId, (words) => {
        const el = ref.current;
        const current = latest.current;
        const start = el?.selectionStart ?? current.length;
        const end = el?.selectionEnd ?? current.length;
        const before = current.slice(0, start);
        const space = before && !/\s$/.test(before) ? " " : "";
        const insert = space + words.trim();
        setText(before + insert + current.slice(end));
        requestAnimationFrame(() => {
          if (!el) return;
          el.focus();
          el.selectionStart = el.selectionEnd = start + insert.length;
        });
      }),
    [noteId, setText],
  );

  useEffect(() => {
    const sub = events.scratchpadDictating.listen((e) => {
      if (e.payload.notes) setDictating(e.payload.active);
      else if (!e.payload.active) setDictating(false);
    });
    return () => {
      sub.then((off) => off());
    };
  }, []);

  const count = words(text);
  const live = recording && session?.note_id === note.id;

  return (
    <div>
      <p className="mb-2.5 text-[13px] text-text-3">
        {live ? t("notes.mine.helpRecording") : t("notes.mine.help")}
      </p>
      <textarea
        ref={ref}
        value={text}
        onChange={(e) => setText(e.target.value)}
        aria-label={t("notes.detail.tab.mine")}
        placeholder={t("notes.mine.placeholder")}
        className="input min-h-[220px] w-full resize-y py-2.5 leading-relaxed"
      />
      <div className="mt-2 flex items-center gap-3">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => {
            ref.current?.focus();
            commands.notesDictate(note.id).catch(() => {});
          }}
          aria-pressed={dictating}
        >
          {dictating ? (
            <Square className="h-3.5 w-3.5 text-danger" aria-hidden="true" />
          ) : (
            <Mic className="h-3.5 w-3.5" aria-hidden="true" />
          )}
          {dictating ? t("notes.mine.stopDictating") : t("notes.mine.dictate")}
        </Button>
        <span className="flex-1" />
        <span className={`saved ${saved ? "show" : ""}`} role="status">
          {saved && (
            <>
              <Check
                className="h-[13px] w-[13px] stroke-[2.5]"
                aria-hidden="true"
              />
              {t("notes.mine.saved")}
            </>
          )}
        </span>
        <span className="text-xs tabular-nums text-text-3">
          {t("notes.mine.words", {
            count,
            formatted: count.toLocaleString(i18n.language),
          })}
        </span>
      </div>
      {live && session && (
        <p className="mt-5 border-t border-line pt-4 text-[13px] text-text-3">
          {session.system_audio
            ? t("notes.mine.recordingBoth", {
                mic: session.microphone,
                system: session.system_audio,
              })
            : t("notes.mine.recordingMic", { mic: session.microphone })}
        </p>
      )}
    </div>
  );
};
