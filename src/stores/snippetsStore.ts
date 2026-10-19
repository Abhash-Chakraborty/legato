import { create } from "zustand";
import { commands, events, type SnippetsView } from "@/bindings";

/** The snippets, for their page. Reloads whenever the backend says one was
 *  added, changed, used or removed. */
interface SnippetsStore {
  view: SnippetsView | null;
  load: () => Promise<void>;
}

export const useSnippetsStore = create<SnippetsStore>()((set) => ({
  view: null,
  load: async () => {
    const result = await commands.getSnippets().catch(() => null);
    if (result?.status === "ok") set({ view: result.data });
  },
}));

let listening = false;

/** Loads the snippets once and keeps them current. */
export const watchSnippets = () => {
  const { view, load } = useSnippetsStore.getState();
  if (!view) load();
  if (listening) return;
  listening = true;
  events.snippetsChanged
    .listen(() => load())
    .catch(() => {
      listening = false;
    });
};
