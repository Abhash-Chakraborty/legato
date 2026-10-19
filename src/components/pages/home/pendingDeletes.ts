import { commands } from "@/bindings";

/** How long a deleted dictation can be brought back. */
export const UNDO_MS = 6000;

// Kept outside React so a delete still happens after Home unmounts, and an
// Undo still works if you come back while the toast shows.
const pending = new Map<number, ReturnType<typeof setTimeout>>();

/** Deletes a dictation after the Undo window, unless `undoDelete` runs. */
export const scheduleDelete = (id: number, onError: () => void): void => {
  clearTimeout(pending.get(id));
  pending.set(
    id,
    setTimeout(async () => {
      pending.delete(id);
      const result = await commands.deleteHistoryEntry(id).catch(() => null);
      if (!result || result.status !== "ok") onError();
    }, UNDO_MS),
  );
};

export const undoDelete = (id: number): void => {
  clearTimeout(pending.get(id));
  pending.delete(id);
};

export const isPendingDelete = (id: number): boolean => pending.has(id);
