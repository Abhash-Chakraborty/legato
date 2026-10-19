import { create } from "zustand";
import { commands, events, type DictionaryView } from "@/bindings";

/** The Dictionary, shared by its page, Home's Fix a word and the rail.
 *  It reloads whenever the backend says a word changed, whichever screen or
 *  shortcut changed it. */
interface DictionaryStore {
  view: DictionaryView | null;
  load: () => Promise<void>;
}

export const useDictionaryStore = create<DictionaryStore>()((set) => ({
  view: null,
  load: async () => {
    const result = await commands.getDictionary().catch(() => null);
    if (result?.status === "ok") set({ view: result.data });
  },
}));

let listening = false;

/** Loads the Dictionary once and keeps it current. Safe to call from every
 *  screen that reads it. */
export const watchDictionary = () => {
  const { view, load } = useDictionaryStore.getState();
  if (!view) load();
  if (listening) return;
  listening = true;
  events.dictionaryChanged
    .listen(() => load())
    .catch(() => {
      listening = false;
    });
};
