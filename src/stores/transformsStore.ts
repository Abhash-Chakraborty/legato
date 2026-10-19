import { create } from "zustand";
import { commands, events, type TransformRuns } from "@/bindings";

/** Transform runs and per-transform use, for the Transforms page. Reloads
 *  whenever the backend records a run or Undo changes one. */
interface TransformsStore {
  runs: TransformRuns | null;
  load: () => Promise<void>;
}

export const useTransformsStore = create<TransformsStore>()((set) => ({
  runs: null,
  load: async () => {
    const result = await commands.getTransformRuns().catch(() => null);
    if (result?.status === "ok") set({ runs: result.data });
  },
}));

let listening = false;

/** Loads the runs once and keeps them current. */
export const watchTransformRuns = () => {
  const { runs, load } = useTransformsStore.getState();
  if (!runs) load();
  if (listening) return;
  listening = true;
  events.transformRunsChanged
    .listen(() => load())
    .catch(() => {
      listening = false;
    });
};
