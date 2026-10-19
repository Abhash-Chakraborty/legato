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
  /** Bumped when a line is added to or removed from any note, so an open
   *  transcript reloads. */
  linesVersion: number;
  load: () => Promise<void>;
}

export const useNotesStore = create<NotesStore>()((set, get) => ({
  notes: null,
  session: null,
  tick: null,
  live: [],
  linesVersion: 0,
  load: async () => {
    const [notes, session] = await Promise.all([
      commands.getNotes().catch(() => null),
      commands.getNotesSession().catch(() => null),
    ]);
    if (notes?.status === "ok") set({ notes: notes.data });
    set((state) => ({
      session,
      live:
        session && session.note_id === state.session?.note_id ? state.live : [],
    }));
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
      set((state) => ({
        session,
        tick: session ? state.tick : null,
        live:
          session && session.note_id === state.session?.note_id
            ? state.live
            : [],
      }));
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
