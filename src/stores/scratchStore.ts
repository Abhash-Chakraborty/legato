import { create } from "zustand";
import { commands, events, type ScratchNote } from "@/bindings";

/** The Scratchpad's notes, shared by its page and its window. Reloads
 *  whenever the backend says a note or tab changed. */
interface ScratchStore {
  notes: ScratchNote[] | null;
  load: () => Promise<void>;
  /** Puts a note the backend returned in place, ahead of the reload. */
  put: (note: ScratchNote) => void;
}

export const useScratchStore = create<ScratchStore>()((set) => ({
  notes: null,
  load: async () => {
    const result = await commands.getScratchNotes().catch(() => null);
    if (result?.status === "ok") set({ notes: result.data });
  },
  put: (note) =>
    set((state) => ({
      notes: state.notes
        ? state.notes.some((n) => n.id === note.id)
          ? state.notes.map((n) => (n.id === note.id ? note : n))
          : [note, ...state.notes]
        : [note],
    })),
}));

let listening = false;

/** Loads the notes once and keeps them current. */
export const watchScratch = () => {
  const { notes, load } = useScratchStore.getState();
  if (!notes) load();
  if (listening) return;
  listening = true;
  events.scratchpadChanged
    .listen(() => load())
    .catch(() => {
      listening = false;
    });
};
