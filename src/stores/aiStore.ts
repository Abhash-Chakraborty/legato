import { create } from "zustand";
import { commands, events, type AiStatus } from "@/bindings";

/** The AI status line's data, shared by every screen that shows it. It
 *  reloads when the backend says the provider, model, key or the provider's
 *  answers changed. */
interface AiStore {
  status: AiStatus | null;
  /** Last four characters of each saved key, by provider. */
  keyHints: Record<string, string>;
  /** Whether a Test connection is running. */
  testing: boolean;
  load: () => Promise<void>;
  /** Runs Test connection and returns the models the provider listed. */
  test: () => Promise<{ status: AiStatus; models: string[] } | null>;
}

export const useAiStore = create<AiStore>()((set, get) => ({
  status: null,
  keyHints: {},
  testing: false,
  load: async () => {
    const [status, hints] = await Promise.all([
      commands.getAiStatus().catch(() => null),
      commands.getAiKeyHints().catch(() => null),
    ]);
    set({
      ...(status ? { status } : {}),
      ...(hints ? { keyHints: hints as Record<string, string> } : {}),
    });
  },
  test: async () => {
    if (get().testing) return null;
    set({ testing: true });
    try {
      const result = await commands.testAiConnection();
      set({ status: result.status });
      return result;
    } catch {
      return null;
    } finally {
      set({ testing: false });
    }
  },
}));

let listening = false;

/** Loads the status once and keeps it current. Safe to call from every
 *  screen that shows it. */
export const watchAi = () => {
  const { status, load } = useAiStore.getState();
  if (!status) load();
  if (listening) return;
  listening = true;
  events.aiStatusChanged
    .listen(() => load())
    .catch(() => {
      listening = false;
    });
};
