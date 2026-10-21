import { useMemo } from "react";
import { create } from "zustand";
import {
  commands,
  events,
  type Note,
  type NoteLine,
  type NotesTick,
  type SessionInfo,
} from "@/bindings";

/** Notes and the recording in progress, kept current from the backend's
 *  events. */
interface NotesStore {
  notes: Note[] | null;
  session: SessionInfo | null;
  /** The clock and meters, a few times a second while recording. */
  tick: NotesTick | null;
  /** Lines transcribed so far in the recording in progress. */
  live: NoteLine[];
  /** The words of lines still being spoken, at most one per side; they
   *  give way to the line once it ends. */
  previews: NoteLine[];
  /** Bumped when a line is added to, changed in or removed from any note,
   *  so an open transcript reloads. */
  linesVersion: number;
  load: () => Promise<void>;
}

export const useNotesStore = create<NotesStore>()((set, get) => ({
  notes: null,
  session: null,
  tick: null,
  live: [],
  previews: [],
  linesVersion: 0,
  load: async () => {
    const [notes, session] = await Promise.all([
      commands.getNotes().catch(() => null),
      commands.getNotesSession().catch(() => null),
    ]);
    if (notes?.status === "ok") set({ notes: notes.data });
    set((state) => {
      const same = session && session.note_id === state.session?.note_id;
      return {
        session,
        live: same ? state.live : [],
        previews: same ? state.previews : [],
      };
    });
    // Opened mid-recording: the lines heard so far.
    if (session && !get().live.length) {
      const lines = await commands
        .getNoteLines(session.note_id)
        .catch(() => null);
      if (lines?.status === "ok" && get().session?.note_id === session.note_id)
        set({ live: lines.data });
    }
  },
}));

let listening = false;

// The open My notes editor, which puts dictated words at its cursor.
const inserters = new Map<number, (text: string) => void>();

/** Lets an open My notes editor take dictated words; returns the cleanup. */
export const takeDictation = (
  noteId: number,
  insert: (text: string) => void,
) => {
  inserters.set(noteId, insert);
  return () => {
    if (inserters.get(noteId) === insert) inserters.delete(noteId);
  };
};

// With no editor open, the words go at the end of My notes.
const appendDictation = async (noteId: number, text: string) => {
  const note = useNotesStore.getState().notes?.find((n) => n.id === noteId);
  if (!note) return;
  const mine = note.my_notes.trimEnd();
  await commands
    .saveNoteMyNotes(noteId, mine ? `${mine}\n${text.trim()}` : text.trim())
    .catch(() => null);
  useNotesStore.getState().load();
};

/** Loads notes once and keeps them current. */
export const watchNotes = () => {
  const { notes, load } = useNotesStore.getState();
  if (!notes) load();
  if (listening) return;
  listening = true;
  const set = useNotesStore.setState;
  Promise.all([
    events.notesChanged.listen(() => useNotesStore.getState().load()),
    events.notesSessionChanged.listen((e) => {
      const session = e.payload.session;
      set((state) => {
        const same = session && session.note_id === state.session?.note_id;
        return {
          session,
          tick: session ? state.tick : null,
          live: same ? state.live : [],
          previews: same && session.phase === "recording" ? state.previews : [],
        };
      });
    }),
    events.notesTick.listen((e) => set({ tick: e.payload })),
    events.noteLineAdded.listen((e) => {
      const line = e.payload.line;
      set((state) => ({
        linesVersion: state.linesVersion + 1,
        live:
          state.session?.note_id === line.note_id
            ? [...state.live, line].sort((a, b) => a.at_ms - b.at_ms)
            : state.live,
      }));
    }),
    events.noteLineChanged.listen((e) => {
      const line = e.payload.line;
      set((state) => ({
        linesVersion: state.linesVersion + 1,
        live: state.live.map((l) => (l.id === line.id ? line : l)),
      }));
    }),
    events.noteLinesChanged.listen(async (e) => {
      const { note_id } = e.payload;
      set((state) => ({ linesVersion: state.linesVersion + 1 }));
      // Voices found during the recording: the live lines get them too.
      if (useNotesStore.getState().session?.note_id !== note_id) return;
      const lines = await commands.getNoteLines(note_id).catch(() => null);
      if (
        lines?.status === "ok" &&
        useNotesStore.getState().session?.note_id === note_id
      )
        set({ live: lines.data });
    }),
    events.notePreview.listen((e) => {
      const { note_id, who, at_ms, text } = e.payload;
      set((state) => {
        if (state.session?.note_id !== note_id) return {};
        const others = state.previews.filter((p) => p.who !== who);
        if (!text) return { previews: others };
        const preview: NoteLine = {
          id: who === "you" ? -1 : -2,
          note_id,
          at_ms,
          end_ms: at_ms,
          who,
          text,
        };
        return { previews: [...others, preview] };
      });
    }),
    events.noteInsert.listen((e) => {
      const { note_id, text } = e.payload;
      const insert = inserters.get(note_id);
      if (insert) insert(text);
      else appendDictation(note_id, text);
    }),
    events.noteLineRemoved.listen((e) =>
      set((state) => ({
        linesVersion: state.linesVersion + 1,
        live: state.live.filter((l) => l.id !== e.payload.line_id),
      })),
    ),
  ]).catch(() => {
    listening = false;
  });
};

/** A live transcript line, or the words of one still being spoken. */
export interface LiveLine {
  line: NoteLine;
  preview: boolean;
}

/** The lines so far with the ones still being spoken, in time order. */
export const useLiveLines = (): LiveLine[] => {
  const live = useNotesStore((s) => s.live);
  const previews = useNotesStore((s) => s.previews);
  return useMemo(
    () =>
      [
        ...live.map((line) => ({ line, preview: false })),
        ...previews.map((line) => ({ line, preview: true })),
      ].sort((a, b) => a.line.at_ms - b.line.at_ms),
    [live, previews],
  );
};
