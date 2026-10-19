import { create } from "zustand";
import type { PracticeResult } from "@/bindings";
import type { Scene } from "./setup";

/** A message in the practice chat with Samir. */
export interface ChatMessage {
  dir: "in" | "out";
  text: string;
  at: number;
}

/** A word from the practice: added to the Dictionary (with the entry to
 *  remove on Undo) or skipped. */
export interface WordChoice {
  state: "added" | "skipped";
  text: string;
  entryId?: number;
}

interface SetupStore {
  /** What landed in each scene, so coming back shows it as it was. */
  results: Partial<Record<Scene, PracticeResult>>;
  /** Replies sent to Samir, and his answers. */
  chat: ChatMessage[];
  /** When the practice email was "sent". */
  emailSent: number | null;
  words: Record<string, WordChoice>;
  /** Open Models when setup finishes. */
  openModels: boolean;
  setResult: (scene: Scene, result: PracticeResult) => void;
  send: (scene: Scene) => void;
  /** Samir's answer arriving in the chat. */
  receive: (message: ChatMessage) => void;
  setWord: (key: string, choice: WordChoice | null) => void;
  setOpenModels: (on: boolean) => void;
  reset: () => void;
}

const KEY = "legato.setup";

type Saved = Pick<
  SetupStore,
  "results" | "chat" | "emailSent" | "words" | "openModels"
>;

const EMPTY: Saved = {
  results: {},
  chat: [],
  emailSent: null,
  words: {},
  openModels: false,
};

// Kept in the browser so a restart part-way through setup shows the scenes
// as they were; storage can be missing, and then setup starts clean.
const load = (): Saved => {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? { ...EMPTY, ...(JSON.parse(raw) as Partial<Saved>) } : EMPTY;
  } catch {
    return EMPTY;
  }
};

const save = (s: Saved) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {
    // Nothing to do: the practice still works, it just isn't remembered.
  }
};

export const useSetupStore = create<SetupStore>()((set, get) => {
  const update = (patch: Partial<Saved>) => {
    set(patch);
    const { results, chat, emailSent, words, openModels } = get();
    save({ results, chat, emailSent, words, openModels });
  };
  return {
    ...load(),
    setResult: (scene, result) => {
      update({
        results: { ...get().results, [scene]: result },
        // A new email is a new draft.
        ...(scene === "email" ? { emailSent: null } : {}),
      });
    },
    send: (scene) => {
      const result = get().results[scene];
      if (!result) return;
      const now = Date.now();
      if (scene === "message") {
        const results = { ...get().results };
        delete results.message;
        update({
          chat: [...get().chat, { dir: "out", text: result.text, at: now }],
          results,
        });
      } else if (scene === "email") {
        update({ emailSent: now });
      }
    },
    receive: (message) => update({ chat: [...get().chat, message] }),
    setWord: (key, choice) => {
      const words = { ...get().words };
      if (choice) words[key] = choice;
      else delete words[key];
      update({ words });
    },
    setOpenModels: (on) => update({ openModels: on }),
    reset: () => update(EMPTY),
  };
});

/** Whether a scene has had a dictation land (or a reply sent from it). */
export const sceneDone = (
  s: Pick<SetupStore, "results" | "chat">,
  scene: Scene,
): boolean =>
  !!s.results[scene] ||
  (scene === "message" && s.chat.some((m) => m.dir === "out"));
