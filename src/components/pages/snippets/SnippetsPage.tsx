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
  Plus,
  Search,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { commands, type Snippet } from "@/bindings";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import {
  useDictionaryStore,
  watchDictionary,
} from "../../../stores/dictionaryStore";
import { useNavStore } from "../../../stores/navStore";
import { useSnippetsStore, watchSnippets } from "../../../stores/snippetsStore";
import { ActionMenu } from "../../ui/ActionMenu";
import { Button } from "../../ui/Button";
import { matchesQuery, sortSnippets, SORTS, type Sort } from "./list";
import { SnippetEditor, toastSaved } from "./SnippetEditor";
import { SnippetRow } from "./SnippetRow";

/** Editor state: closed, adding, or editing one snippet. */
type Editing = null | { snippet: Snippet | null };

/** Phrases that paste saved text: each trigger, what it pastes, and how often
 *  and where it was last used. */
export const SnippetsPage: React.FC = () => {
  const { t } = useTranslation();
  const go = useNavStore((s) => s.go);
  const view = useSnippetsStore((s) => s.view);
  const words = useDictionaryStore((s) => s.view?.entries);
  const [sort, setSort] = useState<Sort>("used");
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<Editing>(null);
  const [focusedId, setFocusedId] = useState<number | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [menu, setMenu] = useState<"more" | "sort" | null>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const now = Math.floor(Date.now() / 1000);

  useEffect(() => {
    watchSnippets();
    watchDictionary();
  }, []);

  const snippets = useMemo(() => view?.snippets ?? [], [view]);
  const shown = useMemo(
    () =>
      sortSnippets(
        snippets.filter((s) => matchesQuery(s, query)),
        sort,
      ),
    [snippets, query, sort],
  );
  const totalUses = snippets.reduce((sum, s) => sum + s.uses, 0);

  // Selection only holds snippets that still exist.
  const chosen = useMemo(
    () => snippets.filter((s) => selected.has(s.id)),
    [snippets, selected],
  );

  useTextHighlight(
    listRef,
    "snippets-search",
    query.trim() ? [query.trim()] : [],
    "[data-snippet-trigger], [data-snippet-text]",
  );

  const ids = shown.map((s) => s.id);
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

  const select = (id: number, on: boolean) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });

  const remove = useCallback(
    async (list: Snippet[]) => {
      if (list.length === 0) return;
      const result = await commands.deleteSnippets(list.map((s) => s.id));
      if (result.status === "error") {
        toast.error(t("snippets.toast.deleteFailed"), {
          description: result.error,
        });
        return;
      }
      const removed = result.data;
      setSelected(new Set());
      toast(
        list.length === 1
          ? t("snippets.toast.deleted", { trigger: list[0].trigger })
          : t("snippets.toast.deletedMany", { count: list.length }),
        {
          action: {
            label: t("snippets.toast.undo"),
            onClick: () => {
              commands.restoreSnippets(removed).catch(() => {});
            },
          },
        },
      );
    },
    [t],
  );

  const importSnippets = async () => {
    const path = await openFile({
      multiple: false,
      filters: [
        { name: t("snippets.import.files"), extensions: ["json", "csv"] },
      ],
    }).catch(() => null);
    if (typeof path !== "string") return;
    const result = await commands.importSnippets(path);
    if (result.status === "error") {
      toast.error(t("snippets.import.failed"), { description: result.error });
      return;
    }
    toast.success(t("snippets.import.done", { ...result.data }));
  };

  const exportSnippets = async (list: Snippet[] | null) => {
    const path = await save({
      defaultPath: "Legato snippets.json",
      filters: [{ name: t("snippets.export.files"), extensions: ["json"] }],
    }).catch(() => null);
    if (!path) return;
    const result = await commands.exportSnippets(
      path,
      list ? list.map((s) => s.id) : null,
    );
    if (result.status === "ok")
      toast.success(t("snippets.export.done", { count: result.data }));
    else
      toast.error(t("snippets.export.failed"), { description: result.error });
  };

  // N adds a snippet from anywhere on the page; / searches.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement).closest(
        "input, textarea, [contenteditable=true], [role=dialog], [role=menu]",
      );
      if (typing || editing || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key.toLowerCase() === "n") {
        e.preventDefault();
        setEditing({ snippet: null });
      } else if (e.key === "/") {
        e.preventDefault();
        searchRef.current?.focus();
      } else if (e.key === "Escape" && selected.size > 0) {
        e.preventDefault();
        setSelected(new Set());
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [editing, selected]);

  const onListKeyDown = (e: React.KeyboardEvent<HTMLUListElement>) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-row]");
    if (!row || e.target !== row) return;
    const id = Number(row.dataset.row);
    const snippet = shown.find((x) => x.id === id);
    const i = ids.indexOf(id);
    if (!snippet) return;
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
      setEditing({ snippet });
    } else if (key === " " || key === "x") {
      e.preventDefault();
      select(id, !selected.has(id));
    } else if (key === "delete") {
      e.preventDefault();
      focusRow(ids[i + 1] ?? ids[i - 1]);
      remove(chosen.length > 0 ? chosen : [snippet]);
    }
  };

  return (
    <div className="mx-auto w-full max-w-[1120px]">
      <div className="mb-6 flex min-h-10 flex-wrap items-center justify-between gap-4">
        <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
          {t("snippets.title")}
        </h1>
        <div className="flex items-center gap-2">
          <div className="relative">
            <button
              type="button"
              onClick={() => setMenu(menu === "more" ? null : "more")}
              aria-label={t("snippets.more")}
              title={t("snippets.more")}
              aria-haspopup="menu"
              aria-expanded={menu === "more"}
              className="grid h-9 w-9 place-items-center rounded-[var(--r-md)] border border-line-strong bg-surface text-text-2 shadow-[var(--shadow-1)] transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
            >
              <EllipsisVertical className="h-4 w-4" aria-hidden="true" />
            </button>
            {menu === "more" && (
              <ActionMenu
                label={t("snippets.more")}
                onClose={() => setMenu(null)}
                items={[
                  {
                    id: "import",
                    label: t("snippets.import.action"),
                    icon: Upload,
                    onSelect: importSnippets,
                  },
                  {
                    id: "export",
                    label: t("snippets.export.action"),
                    icon: Download,
                    disabled: snippets.length === 0,
                    onSelect: () => exportSnippets(null),
                  },
                ]}
              />
            )}
          </div>
          <Button onClick={() => setEditing({ snippet: null })}>
            <Plus className="h-4 w-4" aria-hidden="true" />
            {t("snippets.add")}
          </Button>
        </div>
      </div>
      <p className="-mt-3.5 mb-6 max-w-[640px] text-sm text-text-2">
        {t("snippets.subtitleBefore")}
        <button
          type="button"
          onClick={() => go("dictionary")}
          className="font-medium text-teal-700 underline-offset-2 hover:underline"
        >
          {t("snippets.subtitleLink")}
        </button>
        {t("snippets.subtitleAfter")}
      </p>

      <div
        className={`mb-3 flex min-h-[34px] flex-wrap items-center justify-between gap-3 ${snippets.length === 0 ? "hidden" : ""}`}
      >
        {chosen.length > 0 ? (
          <div
            role="toolbar"
            aria-label={t("snippets.selection.label")}
            className="flex items-center gap-1.5"
          >
            <button
              type="button"
              onClick={() => setSelected(new Set())}
              aria-label={t("snippets.selection.clear")}
              title={t("snippets.selection.clear")}
              className="grid h-[30px] w-[30px] place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0"
            >
              <X className="h-4 w-4" aria-hidden="true" />
            </button>
            <span className="me-2 text-sm font-medium tabular-nums">
              {t("snippets.selection.count", { count: chosen.length })}
            </span>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => exportSnippets(chosen)}
            >
              <Download className="h-3.5 w-3.5" aria-hidden="true" />
              {t("snippets.selection.export")}
            </Button>
            <Button
              variant="danger-ghost"
              size="sm"
              onClick={() => remove(chosen)}
            >
              <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
              {t("snippets.selection.delete")}
            </Button>
          </div>
        ) : (
          <div className="relative w-[300px] max-w-full">
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
              placeholder={t("snippets.search")}
              aria-label={t("snippets.search")}
              className="input compact w-full ps-[30px] [&::-webkit-search-cancel-button]:hidden"
            />
          </div>
        )}
        <div className="relative">
          <button
            type="button"
            onClick={() => setMenu(menu === "sort" ? null : "sort")}
            aria-haspopup="menu"
            aria-expanded={menu === "sort"}
            aria-label={t("snippets.sort.label")}
            className="inline-flex h-[30px] items-center gap-1.5 rounded-[var(--r-sm)] px-2 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0"
          >
            <ArrowUpDown className="h-3.5 w-3.5" aria-hidden="true" />
            {t(`snippets.sort.${sort}`)}
          </button>
          {menu === "sort" && (
            <ActionMenu
              label={t("snippets.sort.label")}
              onClose={() => setMenu(null)}
              className="end-0 top-full mt-1 w-48"
              items={SORTS.map((s) => ({
                id: s,
                label: t(`snippets.sort.${s}`),
                checked: sort === s,
                onSelect: () => setSort(s),
              }))}
            />
          )}
        </div>
      </div>

      {!view ? null : snippets.length === 0 ? (
        <div className="grid place-items-center gap-1.5 rounded-[var(--r-lg)] border border-line px-6 py-16 text-center">
          <h2 className="text-[15px] font-medium">
            {t("snippets.empty.title")}
          </h2>
          <p className="max-w-[440px] text-[13px] text-text-3">
            {t("snippets.empty.body")}
          </p>
          <div className="mt-3 flex gap-2">
            <Button onClick={() => setEditing({ snippet: null })}>
              <Plus className="h-4 w-4" aria-hidden="true" />
              {t("snippets.add")}
            </Button>
            <Button variant="secondary" onClick={importSnippets}>
              <Upload className="h-4 w-4" aria-hidden="true" />
              {t("snippets.import.action")}
            </Button>
          </div>
        </div>
      ) : shown.length === 0 ? (
        <div className="grid place-items-center gap-1.5 px-6 py-16 text-center">
          <h2 className="text-[15px] font-medium">
            {t("snippets.noResults", { query: query.trim() })}
          </h2>
          <button
            type="button"
            onClick={() => setQuery("")}
            className="mt-1 text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
          >
            {t("snippets.clearSearch")}
          </button>
        </div>
      ) : (
        <>
          <ul
            ref={listRef}
            aria-label={t("snippets.title")}
            aria-multiselectable="true"
            onKeyDown={onListKeyDown}
            className="overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface"
          >
            {shown.map((snippet) => (
              <SnippetRow
                key={snippet.id}
                snippet={snippet}
                now={now}
                focused={snippet.id === activeId}
                selected={selected.has(snippet.id)}
                selecting={chosen.length > 0}
                onFocusRow={() => setFocusedId(snippet.id)}
                onSelect={(on) => select(snippet.id, on)}
                onEdit={() => setEditing({ snippet })}
                onDelete={() => remove([snippet])}
              />
            ))}
          </ul>
          <p className="mt-3 text-[13px] text-text-3">
            {t("snippets.footer", {
              snippets: t("snippets.counts.snippets", {
                count: snippets.length,
              }),
              uses: t("snippets.counts.uses", { count: totalUses }),
            })}
          </p>
        </>
      )}

      {editing && view && (
        <SnippetEditor
          key={editing.snippet?.id ?? "new"}
          snippet={editing.snippet}
          snippets={snippets}
          words={words ?? []}
          maxTriggerChars={view.max_trigger_chars}
          maxTextChars={view.max_text_chars}
          variables={view.variables}
          onClose={() => setEditing(null)}
          onEditOther={(snippet) => setEditing({ snippet })}
          onDelete={(snippet) => {
            setEditing(null);
            remove([snippet]);
          }}
          onSaved={(snippet, created) => {
            setEditing(null);
            toastSaved(t, snippet, created);
            setFocusedId(snippet.id);
          }}
        />
      )}
    </div>
  );
};
