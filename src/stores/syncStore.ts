import { create } from "zustand";
import { commands, events, type ClipRow, type SyncState } from "@/bindings";

/** Settings › Devices: the paired phone, what syncs and recent clips. */
interface SyncStore {
  state: SyncState | null;
  clips: ClipRow[];
  load: () => Promise<void>;
}

export const useSyncStore = create<SyncStore>()((set) => ({
  state: null,
  clips: [],
  load: async () => {
    const [state, clips] = await Promise.all([
      commands.getSyncState(),
      commands.getRecentClips(),
    ]);
    set({
      ...(state.status === "ok" ? { state: state.data } : {}),
      ...(clips.status === "ok" ? { clips: clips.data } : {}),
    });
  },
}));

let listening = false;

/** Loads the sync state and keeps it current. It loads again on every
 *  showing: words, snippets and notes change without a sync event, and
 *  their counts on Devices went stale. */
export const watchSync = () => {
  useSyncStore.getState().load();
  if (listening) return;
  listening = true;
  events.syncChanged
    .listen(() => useSyncStore.getState().load())
    .catch(() => {
      listening = false;
    });
};
