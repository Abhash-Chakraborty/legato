import { create } from "zustand";

/**
 * What the updater is doing, written by <UpdateChecker> in the title bar and
 * read by Settings › About, so both show the same state.
 */
export type UpdateStatus =
  | "idle"
  | "checking"
  | "upToDate"
  | "available"
  | "installing"
  /** A check you asked for didn't reach the update server. */
  | "checkFailed"
  | "installFailed";

interface UpdateStore {
  status: UpdateStatus;
  /** Download progress while installing, 0 to 100. */
  progress: number;
  setStatus: (status: UpdateStatus) => void;
  setProgress: (progress: number) => void;
}

export const useUpdateStore = create<UpdateStore>()((set) => ({
  status: "idle",
  progress: 0,
  setStatus: (status) => set({ status }),
  setProgress: (progress) => set({ progress }),
}));
