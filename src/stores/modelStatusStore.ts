import { create } from "zustand";

export type ModelStatus =
  | "ready"
  | "loading"
  | "downloading"
  | "verifying"
  | "error"
  | "unloaded"
  | "none";

/** The speech model's load state. The title bar's model picker tracks it and
 *  publishes it here, so Home's right rail shows the same thing. */
interface ModelStatusStore {
  status: ModelStatus;
  setStatus: (status: ModelStatus) => void;
}

export const useModelStatusStore = create<ModelStatusStore>()((set) => ({
  status: "unloaded",
  setStatus: (status) => set({ status }),
}));
