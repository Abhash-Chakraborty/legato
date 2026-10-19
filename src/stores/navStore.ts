import { create } from "zustand";

/** The pages in the sidebar. */
export type Page =
  | "home"
  | "dictionary"
  | "snippets"
  | "style"
  | "transforms"
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
  | "bar"
  | "privacy"
  | "about"
  | "debug";

interface NavStore {
  page: Page;
  settingsSection: SettingsSection;
  /** Opens a page; for Settings, optionally a section of it. */
  go: (page: Page, section?: SettingsSection) => void;
}

export const useNavStore = create<NavStore>()((set) => ({
  page: "home",
  settingsSection: "general",
  go: (page, section) =>
    set((state) => ({
      page,
      settingsSection: section ?? state.settingsSection,
    })),
}));
