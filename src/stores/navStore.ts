import { create } from "zustand";

/** The pages in the sidebar. */
export type Page =
  | "home"
  | "dictionary"
  | "snippets"
  | "style"
  | "scratchpad"
  | "transforms"
  | "notes"
  | "models"
  | "settings";

/** The sections inside Settings, in sidebar order. */
export type SettingsSection =
  | "general"
  | "shortcuts"
  | "audio"
  | "transcription"
  | "output"
  | "ai"
  | "calls"
  | "bar"
  | "privacy"
  | "about"
  | "debug";

interface NavStore {
  page: Page;
  settingsSection: SettingsSection;
  /** A part of the page to show first, such as the Dictionary's Needs
   *  review; the page clears it once shown. */
  hint: string | null;
  /** Opens a page; for Settings, optionally a section of it. */
  go: (page: Page, section?: SettingsSection) => void;
  /** Opens a page on one of its parts. */
  goTo: (page: Page, hint: string | null) => void;
  clearHint: () => void;
}

export const useNavStore = create<NavStore>()((set) => ({
  page: "home",
  settingsSection: "general",
  hint: null,
  goTo: (page, hint) => set({ page, hint }),
  clearHint: () => set({ hint: null }),
  go: (page, section) =>
    set((state) => ({
      page,
      settingsSection: section ?? state.settingsSection,
    })),
}));
