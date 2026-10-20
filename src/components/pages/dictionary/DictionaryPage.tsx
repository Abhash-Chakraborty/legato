import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { open as openFile, save } from "@tauri-apps/plugin-dialog";
import {
  ArrowUpDown,
  Download,
  EllipsisVertical,
  FlaskConical,
  Plus,
  Search,
  Upload,
  X,
} from "lucide-react";
import { toast } from "sonner";
import {
  commands,
  type DictionaryEntry,
  type PendingWord,
  type Spelling,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useNavStore } from "../../../stores/navStore";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import { formatKeyCombination } from "../../../lib/utils/keyboard";
import {
  useDictionaryStore,
  watchDictionary,
} from "../../../stores/dictionaryStore";
import { ActionMenu } from "../../ui/ActionMenu";
import { Button } from "../../ui/Button";
import { LearningCard } from "./LearningCard";
import { ReviewList } from "./ReviewList";
import { otherForms } from "./SpellingList";
import {
  inTab,
  matchesQuery,
  sortEntries,
  SORTS,
  TABS,
  type Sort,
  type Tab,
} from "./list";
import { TestSentence } from "./TestSentence";
import { toastSaved, WordEditor } from "./WordEditor";
import { WordRow } from "./WordRow";

const INTRO_KEY = "legato.dictionary.introHidden";

const readIntroHidden = () => {
  try {
    return localStorage.getItem(INTRO_KEY) === "1";
  } catch {
    return false;
  }
};

/** Editor state: closed, adding, or editing one word; `pending` is a word
 *  from Needs review being edited before it's added. */
type Editing = null | {
  entry: DictionaryEntry | null;
  pending?: PendingWord;
  /** A term the model wrote more than one way, edited before it's added. */
  spelling?: Spelling;
};

/** How words should be spelled: each word, the ways the model mishears it,
 *  and how often Legato fixed it. */
export const DictionaryPage: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const view = useDictionaryStore((s) => s.view);
  const hint = useNavStore((s) => s.hint);
  const clearHint = useNavStore((s) => s.clearHint);
  const [tab, setTab] = useState<Tab>(hint === "review" ? "review" : "all");
  const [sort, setSort] = useState<Sort>("pinned");
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<Editing>(null);
  const [focusedId, setFocusedId] = useState<number | null>(null);
  const [menu, setMenu] = useState<"more" | "sort" | null>(null);
  const [introHidden, setIntroHidden] = useState(readIntroHidden);
  const [testing, setTesting] = useState(false);
  const listRef = useRef<HTMLUListElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const testRef = useRef<HTMLInputElement>(null);
  const now = Math.floor(Date.now() / 1000);

  useEffect(() => watchDictionary(), []);

  // The Legato bar's Review opens Needs review.
  useEffect(() => {
    if (hint !== "review") return;
    setTab("review");
    clearHint();
  }, [hint, clearHint]);

  const entries = useMemo(() => view?.entries ?? [], [view]);
  const shown = useMemo(
    () =>
      sortEntries(
        entries.filter((e) => inTab(e, tab) && matchesQuery(e, query)),
        sort,
      ),
    [entries, tab, query, sort],
  );
  const counts = {
    all: entries.length,
    learned: entries.filter((e) => inTab(e, "learned")).length,
    mine: entries.filter((e) => inTab(e, "mine")).length,
    review: (view?.pending.length ?? 0) + (view?.spellings.length ?? 0),
  };
  const reviewing = tab === "review";
  const totalHits = entries.reduce((sum, e) => sum + e.hits, 0);

  useTextHighlight(
    listRef,
    "dictionary-search",
    query.trim() ? [query.trim()] : [],
    "[data-word-text], [data-word-heard]",
  );

  const binding = settings?.bindings?.add_to_dictionary?.current_binding ?? "";
  const keys = formatKeyCombination(binding).split(" + ").filter(Boolean);

  const ids = shown.map((e) => e.id);
  const activeId =
    focusedId !== null && ids.includes(focusedId) ? focusedId : ids[0];
  const focusRow = useCallback((id: number | undefined) => {
    if (id === undefined) return;
    setFocusedId(id);
    requestAnimationFrame(() =>
      listRef.current
        ?.querySelector<HTMLElement>(`[data-row="${id}"]`)
        ?.focus(),
    );
  }, []);

  const remove = useCallback(
    async (entry: DictionaryEntry) => {
      const result = await commands.deleteDictionaryEntries([entry.id]);
      if (result.status === "error") {
        toast.error(t("dictionary.toast.deleteFailed"), {
          description: result.error,
        });
        return;
      }
      const removed = result.data;
      toast(t("dictionary.toast.deleted", { word: entry.text }), {
        action: {
          label: t("dictionary.toast.undo"),
          onClick: () => {
            commands.restoreDictionaryEntries(removed).catch(() => {});
          },
        },
      });
    },
    [t],
  );

  const togglePin = useCallback(
    async (entry: DictionaryEntry) => {
      const result = await commands.setDictionaryEntryPinned(
        entry.id,
        !entry.pinned,
      );
      if (result.status === "error") {
        toast.error(result.error);
        return;
      }
      toast.success(
        entry.pinned
          ? t("dictionary.toast.unpinned", { word: entry.text })
          : t("dictionary.toast.pinned", { word: entry.text }),
      );
    },
    [t],
  );

  const importWords = async () => {
    const path = await openFile({
      multiple: false,
      filters: [
        {
          name: t("dictionary.import.files"),
          extensions: ["csv", "txt", "json"],
        },
      ],
    }).catch(() => null);
    if (typeof path !== "string") return;
    const result = await commands.importDictionary(path);
    if (result.status === "error") {
      toast.error(t("dictionary.import.failed"), {
        description: result.error,
      });
      return;
    }
    toast.success(t("dictionary.import.done", { ...result.data }));
  };

  const exportWords = async () => {
    const path = await save({
      defaultPath: "Legato dictionary.json",
      filters: [{ name: t("dictionary.export.files"), extensions: ["json"] }],
    }).catch(() => null);
    if (!path) return;
    const result = await commands.exportDictionary(path);
    if (result.status === "ok")
      toast.success(t("dictionary.export.done", { count: result.data }));
    else
      toast.error(t("dictionary.export.failed"), {
        description: result.error,
      });
  };

  // N adds a word from anywhere on the page.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement).closest(
        "input, textarea, [contenteditable=true], [role=dialog], [role=menu]",
      );
      if (typing || editing || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key.toLowerCase() === "n") {
        e.preventDefault();
        setEditing({ entry: null });
      } else if (e.key === "/") {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [editing]);

  const onListKeyDown = (e: React.KeyboardEvent<HTMLUListElement>) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-row]");
    if (!row || e.target !== row) return;
    const id = Number(row.dataset.row);
    const entry = shown.find((x) => x.id === id);
    const i = ids.indexOf(id);
    if (!entry) return;
    const key = e.key.toLowerCase();
    if (key === "arrowdown" || key === "j") {
      e.preventDefault();
      focusRow(ids[Math.min(i + 1, ids.length - 1)]);
    } else if (key === "arrowup" || key === "k") {
      e.preventDefault();
      focusRow(ids[Math.max(i - 1, 0)]);
    } else if (key === "home") {
      e.preventDefault();
      focusRow(ids[0]);
    } else if (key === "end") {
      e.preventDefault();
      focusRow(ids[ids.length - 1]);
    } else if (key === "enter") {
      e.preventDefault();
      setEditing({ entry });
    } else if (key === "delete") {
      e.preventDefault();
      focusRow(ids[i + 1] ?? ids[i - 1]);
      remove(entry);
    } else if (key === "p" && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      togglePin(entry);
    }
  };

  const hideIntro = () => {
    setIntroHidden(true);
    try {
      localStorage.setItem(INTRO_KEY, "1");
    } catch {
      // Shown again next time; nothing else depends on it.
    }
  };

  const examples = entries.filter((e) => e.heard.length > 0).slice(0, 3);

  return (
    <div className="mx-auto w-full max-w-[1120px]">
      <div className="mb-6 flex min-h-10 flex-wrap items-center justify-between gap-4">
        <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
          {t("dictionary.title")}
        </h1>
        <div className="flex items-center gap-2">
          {keys.length > 0 && (
            <span
              className="hidden items-center gap-1 rounded-[var(--r-sm)] bg-surface-2 py-1 ps-1.5 pe-2.5 text-[13px] text-text-2 min-[900px]:inline-flex"
              title={t("dictionary.selectionHelp")}
            >
              {keys.map((key) => (
                <kbd key={key} className="kbd">
                  {key}
                </kbd>
              ))}
              <span className="ms-1">{t("dictionary.selectionChip")}</span>
            </span>
          )}
          <div className="relative">
            <button
              type="button"
              onClick={() => setMenu(menu === "more" ? null : "more")}
              aria-label={t("dictionary.more")}
              title={t("dictionary.more")}
              aria-haspopup="menu"
              aria-expanded={menu === "more"}
              className="grid h-9 w-9 place-items-center rounded-[var(--r-md)] border border-line-strong bg-surface text-text-2 shadow-[var(--shadow-1)] transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
            >
              <EllipsisVertical className="h-4 w-4" aria-hidden="true" />
            </button>
            {menu === "more" && (
              <ActionMenu
                label={t("dictionary.more")}
                onClose={() => setMenu(null)}
                items={[
                  {
                    id: "import",
                    label: t("dictionary.import.action"),
                    icon: Upload,
                    onSelect: importWords,
                  },
                  {
                    id: "export",
                    label: t("dictionary.export.action"),
                    icon: Download,
                    disabled: entries.length === 0,
                    onSelect: exportWords,
                  },
                ]}
              />
            )}
          </div>
          <Button onClick={() => setEditing({ entry: null })}>
            <Plus className="h-4 w-4" aria-hidden="true" />
            {t("dictionary.add")}
          </Button>
        </div>
      </div>
      <p className="-mt-3.5 mb-6 max-w-[640px] text-sm text-text-2">
        {t("dictionary.subtitle")}
      </p>

      {!introHidden && (
        <div className="relative mb-4 rounded-[var(--r-lg)] bg-surface-2 p-5">
          <button
            type="button"
            onClick={hideIntro}
            aria-label={t("dictionary.intro.hide")}
            title={t("dictionary.intro.hide")}
            className="absolute end-3 top-3 grid h-7 w-7 place-items-center rounded-[var(--r-sm)] text-text-3 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0"
          >
            <X className="h-4 w-4" aria-hidden="true" />
          </button>
          <div className="flex flex-wrap items-start gap-x-8 gap-y-4 pe-8">
            <div className="min-w-0 flex-[1_1_320px]">
              <h2 className="text-[15px] font-semibold">
                {t("dictionary.intro.title")}
              </h2>
              <p className="mt-1 max-w-[540px] text-sm text-text-2">
                {t("dictionary.intro.body")}
              </p>
              <Button
                variant="secondary"
                size="sm"
                className="mt-3"
                aria-expanded={testing}
                onClick={() => {
                  setTesting(!testing);
                  if (!testing)
                    requestAnimationFrame(() => testRef.current?.focus());
                }}
              >
                <FlaskConical className="h-3.5 w-3.5" aria-hidden="true" />
                {t("dictionary.intro.test")}
              </Button>
            </div>
            {examples.length > 0 && (
              <dl className="flex-none rounded-[var(--r-md)] border border-line bg-surface px-4 py-3 text-[13px]">
                {examples.map((e) => (
                  <div key={e.id} className="flex items-baseline gap-2 py-0.5">
                    <dt className="text-text-3 line-through decoration-text-4">
                      {e.heard[0]}
                    </dt>
                    <dd className="font-medium text-teal-700">{e.text}</dd>
                  </div>
                ))}
              </dl>
            )}
          </div>
          {testing && (
            <div className="mt-4 border-t border-line pt-4">
              <TestSentence
                inputRef={testRef}
                label={t("dictionary.test.label")}
                placeholder={t("dictionary.test.placeholder")}
              />
            </div>
          )}
        </div>
      )}

      {view && <LearningCard view={view} />}

      <div className="mb-3 flex flex-wrap items-end justify-between gap-3 border-b border-line">
        <div
          role="tablist"
          aria-label={t("dictionary.tabs.label")}
          className="flex gap-5"
        >
          {TABS.map((id) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={tab === id}
              onClick={() => setTab(id)}
              className={`-mb-px inline-flex items-center gap-1.5 border-b-2 pb-2.5 text-sm transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                tab === id
                  ? "border-teal-600 font-medium text-text"
                  : "border-transparent text-text-2 hover:text-text"
              }`}
            >
              {t(`dictionary.tabs.${id}`)}
              {id === "review" ? (
                counts.review > 0 && (
                  <span className="rounded-full bg-teal-50 px-1.5 text-xs font-medium tabular-nums leading-[18px] text-teal-700">
                    {counts.review}
                  </span>
                )
              ) : (
                <span className="tabular-nums text-xs text-text-3">
                  {counts[id]}
                </span>
              )}
            </button>
          ))}
        </div>
        <div
          className={`mb-2 flex items-center gap-1.5 ${reviewing ? "invisible" : ""}`}
        >
          <div className="relative w-[220px]">
            <Search
              className="pointer-events-none absolute start-[10px] top-1/2 h-[14px] w-[14px] -translate-y-1/2 text-text-3"
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
                  focusRow(ids[0]);
                }
              }}
              placeholder={t("dictionary.search")}
              aria-label={t("dictionary.search")}
              className="input compact w-full ps-[30px] [&::-webkit-search-cancel-button]:hidden"
            />
          </div>
          <div className="relative">
            <button
              type="button"
              onClick={() => setMenu(menu === "sort" ? null : "sort")}
              aria-label={t("dictionary.sort.label")}
              title={t("dictionary.sort.label")}
              aria-haspopup="menu"
              aria-expanded={menu === "sort"}
              className="grid h-[30px] w-[30px] place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0"
            >
              <ArrowUpDown className="h-4 w-4" aria-hidden="true" />
            </button>
            {menu === "sort" && (
              <ActionMenu
                label={t("dictionary.sort.label")}
                onClose={() => setMenu(null)}
                className="end-0 top-full mt-1 w-52"
                items={SORTS.map((s) => ({
                  id: s,
                  label: t(`dictionary.sort.${s}`),
                  checked: sort === s,
                  onSelect: () => setSort(s),
                }))}
              />
            )}
          </div>
        </div>
      </div>

      {!view ? null : reviewing ? (
        <ReviewList
          pending={view.pending}
          spellings={view.spellings}
          now={now}
          enabled={settings?.learn_from_edits ?? true}
          mode={settings?.learn_mode ?? "ask"}
          onEdit={(pending) => setEditing({ entry: null, pending })}
          onEditSpelling={(spelling) => setEditing({ entry: null, spelling })}
          onTurnOn={() => updateSetting("learn_from_edits", true)}
        />
      ) : entries.length === 0 ? (
        <div className="grid place-items-center gap-1.5 px-6 py-16 text-center">
          <h2 className="text-[15px] font-medium">
            {t("dictionary.empty.title")}
          </h2>
          <p className="max-w-[440px] text-[13px] text-text-3">
            {t("dictionary.empty.body")}
          </p>
          <Button className="mt-3" onClick={() => setEditing({ entry: null })}>
            <Plus className="h-4 w-4" aria-hidden="true" />
            {t("dictionary.add")}
          </Button>
        </div>
      ) : shown.length === 0 ? (
        <div className="grid place-items-center gap-1.5 px-6 py-16 text-center">
          <h2 className="text-[15px] font-medium">
            {query.trim()
              ? t("dictionary.noResults.titleSearch", { query: query.trim() })
              : t("dictionary.noResults.title")}
          </h2>
          <button
            type="button"
            onClick={() => {
              setQuery("");
              setTab("all");
            }}
            className="mt-1 text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
          >
            {t("dictionary.noResults.clear")}
          </button>
        </div>
      ) : (
        <>
          <ul
            ref={listRef}
            aria-label={t(`dictionary.tabs.${tab}`)}
            onKeyDown={onListKeyDown}
            className="overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface"
          >
            {shown.map((entry) => (
              <WordRow
                key={entry.id}
                entry={entry}
                now={now}
                focused={entry.id === activeId}
                onFocusRow={() => setFocusedId(entry.id)}
                onEdit={() => setEditing({ entry })}
                onPin={() => togglePin(entry)}
                onDelete={() => remove(entry)}
              />
            ))}
          </ul>
          <p className="mt-3 text-[13px] text-text-3">
            {t("dictionary.footer", {
              words: t("dictionary.counts.words", { count: entries.length }),
              fixes: t("dictionary.counts.fixes", { count: totalHits }),
              learned: t("dictionary.counts.learned", {
                count: counts.learned,
              }),
            })}
          </p>
        </>
      )}

      {editing && view && (
        <WordEditor
          key={
            editing.entry?.id ??
            editing.pending?.id ??
            editing.spelling?.word ??
            "new"
          }
          entry={editing.entry}
          initial={
            editing.pending
              ? {
                  text: editing.pending.word,
                  heard: [editing.pending.heard],
                }
              : editing.spelling && {
                  text: editing.spelling.word,
                  heard: otherForms(editing.spelling),
                }
          }
          entries={entries}
          pinLimit={view.pin_limit}
          maxChars={view.max_word_chars}
          onClose={() => setEditing(null)}
          onEditOther={(entry) => setEditing({ entry })}
          onDelete={(entry) => {
            setEditing(null);
            remove(entry);
          }}
          onSaved={(entry, created) => {
            if (editing.pending)
              commands
                .dismissPendingWords([editing.pending.id])
                .catch(() => {});
            // Saved under another spelling: stop suggesting this one.
            if (
              editing.spelling &&
              entry.text.toLowerCase() !== editing.spelling.word.toLowerCase()
            )
              commands.addNeverLearnWord(editing.spelling.word).catch(() => {});
            setEditing(null);
            toastSaved(t, entry, created);
            setFocusedId(entry.id);
          }}
        />
      )}
    </div>
  );
};
