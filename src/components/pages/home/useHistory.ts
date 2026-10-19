import { useCallback, useEffect, useRef, useState } from "react";
import {
  commands,
  events,
  type HistoryEntry,
  type HistoryFilter,
  type HistorySummary,
} from "@/bindings";

const PAGE_SIZE = 40;

export interface HistoryView {
  search: string;
  filter: HistoryFilter;
  app: string | null;
  /** A local date, YYYY-MM-DD. */
  day: string | null;
}

/** Home's rows for a search, filter and app, with paging, live updates from
 *  the dictation pipeline, and the summary above the list. */
export const useHistory = (view: HistoryView) => {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [summary, setSummary] = useState<HistorySummary | null>(null);
  // Answers to older requests are dropped when the view has moved on.
  const request = useRef(0);
  const loadingMore = useRef(false);
  const viewRef = useRef(view);
  viewRef.current = view;

  const refreshSummary = useCallback(async () => {
    const result = await commands.getHistorySummary().catch(() => null);
    if (result?.status === "ok") setSummary(result.data);
  }, []);

  const fetchPage = useCallback(async (cursor: number | null) => {
    const { search, filter, app, day } = viewRef.current;
    return commands
      .getHistoryEntries({
        cursor,
        limit: PAGE_SIZE,
        search: search.trim() || null,
        filter,
        app,
        day,
      })
      .catch(() => null);
  }, []);

  const reload = useCallback(async () => {
    const token = ++request.current;
    setLoading(true);
    const result = await fetchPage(null);
    if (token !== request.current) return;
    if (result?.status === "ok") {
      setEntries(result.data.entries);
      setHasMore(result.data.has_more);
    }
    setLoading(false);
  }, [fetchPage]);

  const loadMore = useCallback(async () => {
    if (loadingMore.current || !hasMore) return;
    const last = entries[entries.length - 1];
    if (!last) return;
    loadingMore.current = true;
    const token = request.current;
    const result = await fetchPage(last.id);
    loadingMore.current = false;
    if (token !== request.current || result?.status !== "ok") return;
    setEntries((prev) => {
      const seen = new Set(prev.map((e) => e.id));
      return [...prev, ...result.data.entries.filter((e) => !seen.has(e.id))];
    });
    setHasMore(result.data.has_more);
  }, [entries, hasMore, fetchPage]);

  useEffect(() => {
    reload();
  }, [view.search, view.filter, view.app, view.day, reload]);

  useEffect(() => {
    refreshSummary();
  }, [refreshSummary]);

  useEffect(() => {
    const unlisten = events.historyUpdatePayload.listen(({ payload }) => {
      if (payload.action === "added") {
        const { search, filter, app } = viewRef.current;
        const shows =
          !search.trim() &&
          filter === "all" &&
          (!app || app === payload.entry.app_name);
        if (shows) {
          setEntries((prev) => [
            payload.entry,
            ...prev.filter((e) => e.id !== payload.entry.id),
          ]);
        }
      } else if (payload.action === "updated") {
        setEntries((prev) =>
          prev.map((e) => (e.id === payload.entry.id ? payload.entry : e)),
        );
      } else if (payload.action === "deleted") {
        setEntries((prev) => prev.filter((e) => e.id !== payload.id));
      }
      // "toggled" comes from this page's own optimistic update.
      refreshSummary();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refreshSummary]);

  return {
    entries,
    setEntries,
    hasMore,
    loading,
    loadMore,
    reload,
    summary,
    refreshSummary,
  };
};
