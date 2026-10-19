import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type ScratchNote } from "@/bindings";
import { useScratchStore } from "../../stores/scratchStore";

/** Autosave waits this long after the last key. */
const SAVE_DELAY_MS = 600;

export type SaveState = "saved" | "saving" | "failed";

/** A note's text as it's being edited: saved 600 ms after the last change,
 *  and on switching notes or closing. Text from elsewhere (the other window,
 *  a dictation) replaces it when nothing is waiting to be saved. */
export const useNoteDraft = (note: ScratchNote | null) => {
  const put = useScratchStore((s) => s.put);
  const [text, setLocal] = useState(note?.text ?? "");
  const [state, setState] = useState<SaveState>("saved");
  const pending = useRef<{ id: number; text: string } | null>(null);
  const timer = useRef<number | null>(null);
  const noteId = note?.id ?? null;

  const flush = useCallback(async () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    const job = pending.current;
    if (!job) return;
    pending.current = null;
    setState("saving");
    const result = await commands
      .saveScratchNote(job.id, job.text)
      .catch(() => null);
    if (result?.status === "ok") {
      put(result.data);
      if (!pending.current) setState("saved");
    } else {
      // Kept for Retry, unless a newer edit is already waiting.
      if (!pending.current) pending.current = job;
      setState("failed");
    }
  }, [put]);

  // Another note: save what was typed into the last one, then load this one.
  useEffect(() => {
    setLocal(note?.text ?? "");
    setState("saved");
    return () => {
      flush();
    };
  }, [noteId]);

  // Saved elsewhere: follow it unless an edit here is waiting.
  useEffect(() => {
    if (note && !pending.current) setLocal(note.text);
  }, [note?.text]);

  useEffect(() => {
    const onHide = () => {
      flush();
    };
    window.addEventListener("beforeunload", onHide);
    window.addEventListener("blur", onHide);
    return () => {
      window.removeEventListener("beforeunload", onHide);
      window.removeEventListener("blur", onHide);
    };
  }, [flush]);

  const setText = useCallback(
    (next: string) => {
      if (noteId === null) return;
      setLocal(next);
      pending.current = { id: noteId, text: next };
      setState("saving");
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(flush, SAVE_DELAY_MS);
    },
    [noteId, flush],
  );

  return { text, setText, state, flush };
};
