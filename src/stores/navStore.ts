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
  | "insights"
  | "models"
  | "settings";

/** The sections inside Settings, in sidebar order. */
export type SettingsSection =
  | "general"
  | "features"
  | "shortcuts"
  | "audio"
  | "transcription"
  | "output"
  | "ai"
  | "calls"
  | "connections"
  | "bar"
  | "devices"
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
  /** The sidebar shows icons only; remembered on this PC. */
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
}

const COLLAPSED_KEY = "legato.sidebar.collapsed";
const readCollapsed = () => {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === "1";
  } catch {
    return false;
  }
};

export const useNavStore = create<NavStore>()((set) => ({
  page: "home",
  settingsSection: "general",
  hint: null,
  goTo: (page, hint) => set({ page, hint }),
  clearHint: () => set({ hint: null }),
  sidebarCollapsed: readCollapsed(),
  toggleSidebar: () =>
    set((state) => {
      const sidebarCollapsed = !state.sidebarCollapsed;
      try {
        localStorage.setItem(COLLAPSED_KEY, sidebarCollapsed ? "1" : "0");
      } catch {
        // Not remembered; it still toggles.
      }
      return { sidebarCollapsed };
    }),
  go: (page, section) =>
    set((state) => ({
      page,
      settingsSection: section ?? state.settingsSection,
    })),
}));
