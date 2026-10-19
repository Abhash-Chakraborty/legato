import { useCallback, useEffect, useState } from "react";
import { commands, events, type HistoryEntry } from "@/bindings";

const LIMIT = 80;

/** Recent dictations that have text, newest first, kept current as new ones
 *  are saved. Style takes its examples from these. */
export const useRecentDictations = () => {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);

  const load = useCallback(async () => {
    const result = await commands
      .getHistoryEntries({
        cursor: null,
        limit: LIMIT,
        search: null,
        filter: "all",
        app: null,
      })
      .catch(() => null);
    if (result?.status === "ok")
      setEntries(
        result.data.entries.filter((e) => e.transcription_text.trim()),
      );
  }, []);

  useEffect(() => {
    load();
    const unlisten = events.historyUpdatePayload.listen(() => load());
    return () => {
      unlisten.then((off) => off()).catch(() => {});
    };
  }, [load]);

  return entries;
};
