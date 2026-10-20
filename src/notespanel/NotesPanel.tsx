import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Check, Loader2, Mic, Pause, Play, Square, X } from "lucide-react";
import { commands, events, type NoteLine } from "@/bindings";
import { clock, longClock } from "@/components/notes/notes";
import { SideMeter } from "@/components/pages/notes/RecordingStrip";
import { AppToaster } from "@/components/shell/AppToaster";
import { Button } from "@/components/ui/Button";
import { takeDictation, useNotesStore, watchNotes } from "@/stores/notesStore";

/** Autosave waits this long after the last key. */
const SAVE_DELAY_MS = 600;

const Line: React.FC<{ line: NoteLine }> = ({ line }) => {
  const { t } = useTranslation();
  return (
    <li className="py-1.5">
      <div className="flex items-baseline gap-2 text-xs">
        <span
          className={`font-medium ${line.who === "you" ? "text-teal-700" : "text-text-2"}`}
        >
          {t(`notes.who.${line.who}`)}
        </span>
        <span className="tabular-nums text-text-3">{clock(line.at_ms)}</span>
      </div>
      <p className="mt-0.5 text-sm leading-relaxed">{line.text}</p>
    </li>
  );
};

/** Your notes on the recording, saved as you type; dictated words land at
 *  the cursor. */
const MyNotes: React.FC<{ noteId: number }> = ({ noteId }) => {
  const { t } = useTranslation();
  const stored = useNotesStore(
    (s) => s.notes?.find((n) => n.id === noteId)?.my_notes ?? "",
  );
  const [text, setLocal] = useState(stored);
  const [saved, setSaved] = useState(false);
  const [dictating, setDictating] = useState(false);
  const pending = useRef<string | null>(null);
  const timer = useRef<number | null>(null);
  const ref = useRef<HTMLTextAreaElement>(null);

  const flush = useCallback(async () => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
    const job = pending.current;
    if (job === null) return;
    pending.current = null;
    const r = await commands.saveNoteMyNotes(noteId, job).catch(() => null);
    if (r?.status === "ok") {
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

  useEffect(() => {
    window.addEventListener("blur", flush);
    return () => {
      window.removeEventListener("blur", flush);
      flush();
    };
  }, [flush]);

  // Saved elsewhere (the Notes page): follow it unless an edit is waiting.
  useEffect(() => {
    if (pending.current === null) setLocal(stored);
  }, [stored]);

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

  return (
    <section className="flex flex-none flex-col border-t border-line px-4 pb-3 pt-2.5">
      <div className="mb-1.5 flex h-7 items-center gap-2">
        <h2 className="text-xs font-semibold text-text-2">
          {t("notes.detail.tab.mine")}
        </h2>
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
        <button
          type="button"
          onClick={() => {
            ref.current?.focus();
            commands.notesDictate(noteId).catch(() => {});
          }}
          aria-pressed={dictating}
          className="inline-flex h-7 items-center gap-1.5 rounded-[var(--r-sm)] px-2 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0"
        >
          {dictating ? (
            <Square className="h-3.5 w-3.5 text-danger" aria-hidden="true" />
          ) : (
            <Mic className="h-3.5 w-3.5" aria-hidden="true" />
          )}
          {dictating ? t("notes.mine.stopDictating") : t("notes.mine.dictate")}
        </button>
      </div>
      <textarea
        ref={ref}
        value={text}
        onChange={(e) => setText(e.target.value)}
        aria-label={t("notes.detail.tab.mine")}
        placeholder={t("notesPanel.minePlaceholder")}
        className="input h-36 w-full resize-none py-2 text-sm leading-relaxed"
      />
    </section>
  );
};

/**
 * The Notes side panel: the recording in progress at the right of the
 * screen, with its live transcript, your notes, Pause and Stop. Closing it
 * leaves the recording running; the bar and the bubble show that.
 */
export const NotesPanel: React.FC = () => {
  const { t } = useTranslation();
  const session = useNotesStore((s) => s.session);
  const tick = useNotesStore((s) => s.tick);
  const live = useNotesStore((s) => s.live);
  const [busy, setBusy] = useState(false);
  const [pinned, setPinned] = useState(true);
  const listRef = useRef<HTMLOListElement>(null);

  useEffect(() => {
    watchNotes();
    // Shown again for a new recording: catch up on it.
    const sub = events.notesSessionChanged.listen(() =>
      useNotesStore.getState().load(),
    );
    return () => {
      sub.then((off) => off());
    };
  }, []);

  useEffect(() => {
    const list = listRef.current;
    if (list && pinned) list.scrollTop = list.scrollHeight;
  }, [live.length, pinned]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") commands.hideNotesPanel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const run = async (fn: () => Promise<{ status: string; error?: string }>) => {
    if (busy) return;
    setBusy(true);
    const r = await fn().catch((e) => ({ status: "error", error: String(e) }));
    setBusy(false);
    if (r.status === "error")
      toast.error(t("notes.toast.actionFailed"), { description: r.error });
  };

  const finishing = session?.phase === "finishing";
  const paused = tick?.paused ?? session?.paused ?? false;
  const elapsed = tick?.elapsed_ms ?? session?.elapsed_ms ?? 0;
  const pending = tick?.pending ?? session?.pending ?? 0;
  const status = !session
    ? t("notesPanel.idle")
    : finishing
      ? t("notes.strip.finishing")
      : paused
        ? t("notes.strip.paused")
        : t("notes.strip.recording");

  return (
    <div className="flex h-screen flex-col overflow-hidden border border-line bg-surface text-text">
      <header className="flex flex-none items-start gap-2 px-4 pb-2 pt-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-[13px] font-medium">
            {finishing ? (
              <Loader2
                className="h-3.5 w-3.5 animate-spin text-teal-700"
                aria-hidden="true"
              />
            ) : (
              session && (
                <span
                  aria-hidden="true"
                  className={`h-2 w-2 rounded-full ${paused ? "bg-warn" : "notes-recdot bg-danger"}`}
                />
              )
            )}
            <span role="status">{status}</span>
            {session && !finishing && (
              <span className="tabular-nums text-text-2">
                · {longClock(elapsed)}
              </span>
            )}
          </div>
          <h1 className="mt-1 truncate text-[15px] font-semibold">
            {session?.title ?? t("notesPanel.title")}
          </h1>
        </div>
        <button
          type="button"
          onClick={() => commands.hideNotesPanel()}
          aria-label={t("notesPanel.close")}
          title={t("notesPanel.close")}
          className="grid h-8 w-8 flex-none place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0"
        >
          <X className="h-4 w-4" aria-hidden="true" />
        </button>
      </header>

      {session && !finishing && (
        <div className="flex flex-none items-center gap-3.5 px-4 pb-2.5">
          <SideMeter
            level={paused ? 0 : (tick?.you ?? 0)}
            label={t("notes.who.you")}
          />
          {session.kind === "meeting" && (
            <SideMeter
              level={paused ? 0 : (tick?.them ?? 0)}
              label={t("notes.who.them")}
            />
          )}
        </div>
      )}

      <section className="relative flex min-h-0 flex-1 flex-col border-t border-line">
        <h2 className="flex-none px-4 pb-1 pt-2.5 text-xs font-semibold text-text-2">
          {t("notes.strip.live")}
        </h2>
        {live.length ? (
          <ol
            ref={listRef}
            aria-live="polite"
            aria-label={t("notes.strip.live")}
            onScroll={(e) => {
              const el = e.currentTarget;
              setPinned(el.scrollHeight - el.scrollTop - el.clientHeight < 24);
            }}
            className="min-h-0 flex-1 overflow-y-auto px-4 pb-2"
          >
            {live.map((line) => (
              <Line key={line.id} line={line} />
            ))}
          </ol>
        ) : (
          <p className="px-4 text-[13px] text-text-3">
            {!session
              ? t("notesPanel.none")
              : pending
                ? t("notes.strip.transcribing")
                : t("notes.strip.waiting")}
          </p>
        )}
        {!pinned && (
          <button
            type="button"
            onClick={() => setPinned(true)}
            className="absolute bottom-2 end-4 rounded-full bg-surface-3 px-2.5 py-1 text-xs font-medium text-text-2 shadow-sm hover:text-text"
          >
            {t("notes.strip.latest")}
          </button>
        )}
      </section>

      {session && <MyNotes key={session.note_id} noteId={session.note_id} />}

      {session && (
        <footer className="flex flex-none items-center gap-2 border-t border-line px-4 py-3">
          {finishing ? (
            <span className="truncate text-[13px] text-text-3">
              {pending
                ? t("notes.strip.linesLeft", { count: pending })
                : t("notes.strip.saving")}
            </span>
          ) : (
            <>
              <p className="min-w-0 flex-1 text-xs leading-snug text-text-3">
                {t("notesPanel.closeHint")}
              </p>
              <Button
                variant="secondary"
                size="sm"
                disabled={busy}
                onClick={() => run(() => commands.pauseNotes(!paused))}
              >
                {paused ? (
                  <Play className="h-4 w-4" aria-hidden="true" />
                ) : (
                  <Pause className="h-4 w-4" aria-hidden="true" />
                )}
                {paused ? t("notes.strip.resume") : t("notes.strip.pause")}
              </Button>
              <Button
                size="sm"
                disabled={busy}
                onClick={() => run(() => commands.stopNotes())}
              >
                <Square className="h-3.5 w-3.5" aria-hidden="true" />
                {t("notes.strip.stop")}
              </Button>
            </>
          )}
        </footer>
      )}
      <AppToaster />
    </div>
  );
};
