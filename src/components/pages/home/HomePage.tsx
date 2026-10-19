import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { convertFileSrc } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { Search, X } from "lucide-react";
import { toast } from "sonner";
import { commands, type HistoryEntry, type HistoryFilter } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import { formatKeyCombination } from "../../../lib/utils/keyboard";
import { Dropdown } from "../../ui/Dropdown";
import { copyToClipboard } from "./clipboard";
import {
  dayLabel,
  formatSpeech,
  groupByDay,
  localDay,
  searchTerms,
} from "./format";
import { HistoryRow, type RowActions } from "./HistoryRow";
import { HomeRail } from "./HomeRail";
import {
  isPendingDelete,
  scheduleDelete,
  undoDelete,
  UNDO_MS,
} from "./pendingDeletes";
import { useHistory } from "./useHistory";

const FILTERS: HistoryFilter[] = ["all", "starred", "fixed", "transformed"];
const ALL_APPS = "__all__";

export const HomePage: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { settings } = useSettings();
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<HistoryFilter>("all");
  const [app, setApp] = useState<string | null>(null);
  const [hidden, setHidden] = useState<Set<number>>(new Set());
  const [focusedId, setFocusedId] = useState<number | null>(null);
  const [menuId, setMenuId] = useState<number | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);

  // Search as you type, without a query per keystroke.
  useEffect(() => {
    const timer = setTimeout(() => setSearch(query), 150);
    return () => clearTimeout(timer);
  }, [query]);

  const history = useHistory({ search, filter, app });
  const {
    entries,
    setEntries,
    summary,
    hasMore,
    loadMore,
    loading,
    refreshSummary,
  } = history;

  const visible = useMemo(
    () => entries.filter((e) => !hidden.has(e.id) && !isPendingDelete(e.id)),
    [entries, hidden],
  );
  const groups = useMemo(() => groupByDay(visible), [visible]);

  useTextHighlight(
    listRef,
    "history-search",
    searchTerms(search),
    "[data-history-text], [data-history-app]",
  );

  // "/" or Ctrl+F jumps to the search box.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement).closest(
        "input, textarea, [contenteditable=true]",
      );
      if (
        (e.key === "/" && !typing) ||
        (e.key.toLowerCase() === "f" && e.ctrlKey && !e.shiftKey && !e.altKey)
      ) {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  // Load the next page when the end of the list comes into view.
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel || !hasMore) return;
    const observer = new IntersectionObserver(
      ([first]) => first.isIntersecting && loadMore(),
      { rootMargin: "400px" },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, loadMore]);

  const actions: RowActions = useMemo(
    () => ({
      copy: async (_entry, text) => {
        const ok = await copyToClipboard(text);
        if (!ok) toast.error(t("home.toast.copyFailed"));
        return ok;
      },
      toggleStar: async (entry) => {
        const flip = (list: HistoryEntry[]) =>
          list.map((e) => (e.id === entry.id ? { ...e, saved: !e.saved } : e));
        setEntries(flip);
        const result = await commands
          .toggleHistoryEntrySaved(entry.id)
          .catch(() => null);
        if (result?.status !== "ok") {
          setEntries(flip);
          toast.error(t("home.toast.starFailed"));
        }
        refreshSummary();
      },
      remove: (entry) => {
        setHidden((prev) => new Set(prev).add(entry.id));
        const restore = () =>
          setHidden((prev) => {
            const next = new Set(prev);
            next.delete(entry.id);
            return next;
          });
        scheduleDelete(entry.id, () => {
          restore();
          toast.error(t("home.toast.deleteFailed"));
        });
        toast(t("home.toast.deleted"), {
          duration: UNDO_MS,
          action: {
            label: t("home.toast.undo"),
            onClick: () => {
              undoDelete(entry.id);
              restore();
            },
          },
        });
      },
      retry: async (entry) => {
        const result = await commands
          .retryHistoryEntryTranscription(entry.id)
          .catch((e) => ({ status: "error" as const, error: String(e) }));
        if (result.status !== "ok") {
          toast.error(t("home.toast.retryFailed"), {
            description: String(result.error),
          });
        }
      },
      saveAudio: async (entry) => {
        const stamp = localDay(entry.timestamp);
        const destination = await save({
          defaultPath: `Legato ${stamp} ${entry.id}.wav`,
          filters: [{ name: t("home.row.wavFiles"), extensions: ["wav"] }],
        }).catch(() => null);
        if (!destination) return;
        const result = await commands
          .saveHistoryAudio(entry.id, destination)
          .catch((e) => ({ status: "error" as const, error: String(e) }));
        if (result.status === "ok") toast.success(t("home.toast.audioSaved"));
        else
          toast.error(t("home.toast.audioFailed"), {
            description: String(result.error),
          });
      },
      loadAudio: async (entry) => {
        const result = await commands
          .getAudioFilePath(entry.file_name)
          .catch(() => null);
        return result?.status === "ok"
          ? convertFileSrc(result.data, "asset")
          : null;
      },
    }),
    [t, setEntries, refreshSummary],
  );

  // Rows take focus one at a time; the list moves it with the arrow keys.
  const rowIds = visible.map((e) => e.id);
  const activeId =
    focusedId !== null && rowIds.includes(focusedId) ? focusedId : rowIds[0];

  const focusRow = useCallback((id: number | undefined) => {
    if (id === undefined) return;
    setFocusedId(id);
    listRef.current?.querySelector<HTMLElement>(`[data-row="${id}"]`)?.focus();
  }, []);

  const onListKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-row]");
    if (!row || e.target !== row) return;
    const id = Number(row.dataset.row);
    const entry = visible.find((x) => x.id === id);
    const i = rowIds.indexOf(id);
    if (!entry) return;
    const key = e.key.toLowerCase();
    if (key === "arrowdown" || key === "j") {
      e.preventDefault();
      focusRow(rowIds[Math.min(i + 1, rowIds.length - 1)]);
    } else if (key === "arrowup" || key === "k") {
      e.preventDefault();
      focusRow(rowIds[Math.max(i - 1, 0)]);
    } else if (key === "home") {
      e.preventDefault();
      focusRow(rowIds[0]);
    } else if (key === "end") {
      e.preventDefault();
      focusRow(rowIds[rowIds.length - 1]);
    } else if (key === "delete") {
      e.preventDefault();
      focusRow(rowIds[i + 1] ?? rowIds[i - 1]);
      actions.remove(entry);
    } else if (key === "s" && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      actions.toggleStar(entry);
    } else if (key === "enter" || (key === "c" && e.ctrlKey)) {
      e.preventDefault();
      actions.copy(
        entry,
        entry.post_processed_text ?? entry.transcription_text,
      );
    } else if (key === "contextmenu" || (key === "f10" && e.shiftKey)) {
      e.preventDefault();
      setMenuId(id);
    }
  };

  const today = summary?.today;
  const counts = summary?.counts;
  const filtering = search.trim() !== "" || filter !== "all" || app !== null;
  const now = new Date();
  const binding = formatKeyCombination(
    settings?.bindings?.transcribe?.current_binding ?? "",
  );

  const appOptions = [
    { value: ALL_APPS, label: t("home.filters.allApps") },
    ...(summary?.apps ?? []).map((a) => ({
      value: a.name,
      label: t("home.filters.appCount", { name: a.name, count: a.count }),
    })),
  ];

  return (
    <div className="grid grid-cols-1 gap-12 min-[1180px]:grid-cols-[minmax(0,1fr)_256px]">
      <div className="min-w-0">
        <div className="mb-1.5 flex min-h-10 flex-wrap items-center justify-between gap-x-6 gap-y-3">
          <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
            {t("home.title")}
          </h1>
          <div className="relative w-full max-w-[300px]">
            <Search
              className="pointer-events-none absolute start-[11px] top-1/2 h-[15px] w-[15px] -translate-y-1/2 text-text-3"
              aria-hidden="true"
            />
            <input
              ref={searchRef}
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape" && query) {
                  e.preventDefault();
                  e.stopPropagation();
                  setQuery("");
                } else if (e.key === "ArrowDown") {
                  e.preventDefault();
                  focusRow(rowIds[0]);
                }
              }}
              placeholder={t("home.search.placeholder")}
              aria-label={t("home.search.placeholder")}
              className="input compact w-full ps-[33px] pe-8 [&::-webkit-search-cancel-button]:hidden"
            />
            {query ? (
              <button
                type="button"
                onClick={() => {
                  setQuery("");
                  searchRef.current?.focus();
                }}
                aria-label={t("home.search.clear")}
                title={t("home.search.clear")}
                className="absolute end-1 top-1/2 grid h-6 w-6 -translate-y-1/2 place-items-center rounded-[var(--r-xs)] text-text-3 hover:bg-surface-3 hover:text-text"
              >
                <X className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            ) : (
              <kbd className="kbd pointer-events-none absolute end-2 top-1/2 -translate-y-1/2">
                /
              </kbd>
            )}
          </div>
        </div>

        <p className="mb-6 text-sm text-text-2">
          {today && today.dictations > 0
            ? t("home.today", {
                dictations: t("home.counts.dictations", {
                  count: today.dictations,
                }),
                words: t("home.counts.words", { count: today.words }),
                speech: formatSpeech(today.speech_ms),
              })
            : t("home.nothingToday")}
        </p>

        <div className="mb-4 flex flex-wrap items-center gap-2">
          <div
            role="radiogroup"
            aria-label={t("home.filters.label")}
            className="inline-flex rounded-[var(--r-sm)] border border-line bg-surface-2 p-0.5"
          >
            {FILTERS.filter(
              (f) => f === "all" || f === filter || (counts?.[f] ?? 0) > 0,
            ).map((f) => (
              <button
                key={f}
                type="button"
                role="radio"
                aria-checked={filter === f}
                onClick={() => setFilter(f)}
                className={`inline-flex h-7 items-center gap-1.5 rounded-[calc(var(--r-sm)-2px)] px-2.5 text-[13px] transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                  filter === f
                    ? "bg-surface font-medium text-text shadow-[var(--shadow-1)]"
                    : "text-text-2 hover:text-text"
                }`}
              >
                {t(`home.filters.${f}`)}
                <span className="tabular-nums text-text-3">
                  {counts?.[f] ?? 0}
                </span>
              </button>
            ))}
          </div>
          {(summary?.apps.length ?? 0) > 0 && (
            <Dropdown
              className="w-44"
              options={appOptions}
              selectedValue={app ?? ALL_APPS}
              onSelect={(v) => setApp(v === ALL_APPS ? null : v)}
              ariaLabel={t("home.filters.appLabel")}
            />
          )}
        </div>

        <div ref={listRef} onKeyDown={onListKeyDown}>
          {loading && entries.length === 0 ? null : visible.length === 0 ? (
            filtering ? (
              <div className="grid place-items-center gap-1.5 px-6 py-16 text-center">
                <h2 className="text-[15px] font-medium">
                  {search.trim()
                    ? t("home.noResults.titleSearch", { query: search.trim() })
                    : t("home.noResults.title")}
                </h2>
                <p className="text-[13px] text-text-3">
                  {t("home.noResults.body")}
                </p>
                <button
                  type="button"
                  onClick={() => {
                    setQuery("");
                    setFilter("all");
                    setApp(null);
                  }}
                  className="mt-2 text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
                >
                  {t("home.noResults.clear")}
                </button>
              </div>
            ) : (
              <div className="grid place-items-center gap-1.5 px-6 py-16 text-center">
                <h2 className="text-[15px] font-medium">
                  {t("home.empty.title")}
                </h2>
                <p className="max-w-[420px] text-[13px] text-text-3">
                  {binding
                    ? t("home.empty.body", { shortcut: binding })
                    : t("home.empty.bodyNoShortcut")}
                </p>
              </div>
            )
          ) : (
            groups.map((group) => {
              const label = dayLabel(group.day, now, i18n.language);
              const title =
                label.kind === "date"
                  ? label.text
                  : t(`home.day.${label.kind}`);
              return (
                <section key={group.day} aria-label={title} className="mb-6">
                  <h2 className="mb-1 px-3 text-[15px] font-semibold">
                    {title}
                  </h2>
                  <ul aria-label={title} className="flex flex-col">
                    {group.entries.map((entry) => (
                      <HistoryRow
                        key={entry.id}
                        entry={entry}
                        actions={actions}
                        focused={entry.id === activeId}
                        menuOpen={menuId === entry.id}
                        setMenuOpen={(open) =>
                          setMenuId(open ? entry.id : null)
                        }
                        onFocusRow={() => setFocusedId(entry.id)}
                      />
                    ))}
                  </ul>
                </section>
              );
            })
          )}
          <div ref={sentinelRef} className="h-px" />
        </div>
      </div>

      <div className="hidden min-[1180px]:block">
        <div className="sticky top-0">
          <HomeRail />
        </div>
      </div>
    </div>
  );
};
