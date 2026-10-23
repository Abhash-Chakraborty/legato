import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Plus, Search } from "lucide-react";
import {
  commands,
  events,
  type ScratchNote,
  type ScratchpadOpen,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import { formatKeyCombination } from "../../../lib/utils/keyboard";
import { useNavStore } from "../../../stores/navStore";
import { useScratchStore, watchScratch } from "../../../stores/scratchStore";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { copyToClipboard } from "../home/clipboard";
import { byUpdated, matchesQuery } from "../../scratchpad/notes";
import { NoteCard } from "./NoteCard";
import { NoteRow } from "./NoteRow";

type Tab = "all" | "pinned";
const OPENS: ScratchpadOpen[] = ["resume", "new", "pinned"];

/** Notes kept in the Scratchpad: a list on the left, the selected note on
 *  the right, and what the shortcut opens. */
export const ScratchpadPage: React.FC = () => {
  const { t, i18n } = useTranslation();
  const go = useNavStore((s) => s.go);
  const notes = useScratchStore((s) => s.notes);
  const put = useScratchStore((s) => s.put);
  const { settings, updateSetting, isUpdating, refreshSettings } =
    useSettings();
  const [tab, setTab] = useState<Tab>("all");
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [focusedId, setFocusedId] = useState<number | null>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const now = Math.floor(Date.now() / 1000);

  useEffect(() => {
    watchScratch();
    // Keep on top can change from the window's button.
    const sub = events.scratchpadChanged.listen(() => refreshSettings());
    return () => {
      sub.then((off) => off());
    };
  }, [refreshSettings]);

  const all = useMemo(() => [...(notes ?? [])].sort(byUpdated), [notes]);
  const pinnedCount = all.filter((n) => n.pinned).length;
  const shown = useMemo(
    () =>
      all.filter((n) => (tab === "all" || n.pinned) && matchesQuery(n, query)),
    [all, tab, query],
  );
  const selected =
    all.find((n) => n.id === selectedId) ?? shown[0] ?? all[0] ?? null;

  useTextHighlight(
    listRef,
    "notes-search",
    query.trim() ? [query.trim()] : [],
    "[data-note-title], [data-note-text]",
  );

  const ids = shown.map((n) => n.id);
  const activeRow =
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

  const newNote = async () => {
    const result = await commands.newScratchNote();
    if (result.status === "error") {
      toast.error(t("scratchpad.toast.newFailed"), {
        description: result.error,
      });
      return;
    }
    put(result.data);
    setQuery("");
    setTab("all");
    setSelectedId(result.data.id);
    requestAnimationFrame(() => editorRef.current?.focus());
  };

  const pin = async (note: ScratchNote) => {
    const result = await commands.setScratchNotePinned(note.id, !note.pinned);
    if (result.status === "ok") put(result.data);
  };

  const copy = async (note: ScratchNote) => {
    if (note.text.trim() && (await copyToClipboard(note.text)))
      toast(t("scratchpad.toast.copied"));
  };

  const remove = async (note: ScratchNote) => {
    const result = await commands.deleteScratchNotes([note.id]);
    if (result.status === "error") {
      toast.error(t("scratchpad.toast.deleteFailed"), {
        description: result.error,
      });
      return;
    }
    const removed = result.data;
    toast(
      t("scratchpad.toast.deleted", {
        title: note.title || t("scratchpad.untitled"),
      }),
      {
        action: {
          label: t("scratchpad.toast.undo"),
          onClick: () => {
            commands.restoreScratchNotes(removed).catch(() => {});
          },
        },
      },
    );
  };

  // N starts a note from anywhere on the page; / searches.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement).closest(
        "input, textarea, [contenteditable=true], [role=dialog], [role=menu]",
      );
      if (typing || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key.toLowerCase() === "n") {
        e.preventDefault();
        newNote();
      } else if (e.key === "/") {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  });

  const onListKeyDown = (e: React.KeyboardEvent<HTMLUListElement>) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-row]");
    if (!row || e.target !== row) return;
    const id = Number(row.dataset.row);
    const note = shown.find((n) => n.id === id);
    const i = ids.indexOf(id);
    if (!note) return;
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
      setSelectedId(id);
      requestAnimationFrame(() => editorRef.current?.focus());
    } else if (key === "p") {
      e.preventDefault();
      pin(note);
    } else if (key === "delete") {
      e.preventDefault();
      focusRow(ids[i + 1] ?? ids[i - 1]);
      remove(note);
    }
  };

  const binding = settings?.bindings?.open_scratchpad?.current_binding ?? "";
  const keys = formatKeyCombination(binding).split(" + ").filter(Boolean);
  const keyText = keys.join("+");
  const failedPastes = settings?.scratchpad_failed_pastes ?? true;
  const openWith = settings?.scratchpad_open ?? "resume";
  const opensOn = t(`scratchpad.settings.opensOn.${openWith}`);

  return (
    <div className="mx-auto flex w-full max-w-[1120px] flex-col min-[1000px]:min-h-0 min-[1000px]:flex-1">
      <div className="mb-6 flex min-h-10 flex-wrap items-center justify-between gap-4">
        <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
          {t("scratchpad.title")}
        </h1>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => go("settings", "shortcuts")}
            title={t("scratchpad.shortcut.change")}
            className="hidden items-center gap-1 rounded-[var(--r-sm)] bg-surface-2 py-1 pe-2.5 ps-1.5 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:duration-0 active:scale-[0.98] min-[900px]:inline-flex"
          >
            {keys.length > 0 ? (
              <>
                {keys.map((key) => (
                  <kbd key={key} className="kbd">
                    {key}
                  </kbd>
                ))}
                <span className="ms-1">{t("scratchpad.shortcut.chip")}</span>
              </>
            ) : (
              t("scratchpad.shortcut.none")
            )}
          </button>
          <Button onClick={newNote}>
            <Plus className="h-4 w-4" aria-hidden="true" />
            {t("scratchpad.newNote")}
          </Button>
        </div>
      </div>
      <p className="-mt-3.5 mb-6 max-w-[680px] text-sm text-text-2">
        {t("scratchpad.subtitle")}{" "}
        {failedPastes
          ? t("scratchpad.subtitleFailedOn")
          : t("scratchpad.subtitleFailedOff")}{" "}
        <button
          type="button"
          onClick={() => go("settings", "output")}
          className="font-medium text-teal-700 underline-offset-2 hover:underline"
        >
          {t("scratchpad.subtitleChange")}
        </button>
      </p>

      {/* The list and the note fit the window: only the list scrolls. */}
      <div className="grid gap-8 min-[1000px]:min-h-0 min-[1000px]:flex-1 min-[1000px]:grid-cols-[minmax(0,360px)_minmax(0,1fr)] min-[1000px]:gap-10">
        <div className="flex min-w-0 flex-col min-[1000px]:min-h-0">
          <div
            role="group"
            aria-label={t("scratchpad.settings.title")}
            className="mb-4 flex-none divide-y divide-line rounded-[var(--r-lg)] border border-line"
          >
            <div className="flex h-11 items-center justify-between gap-3 pe-1.5 ps-3.5">
              <span
                className="text-[13px] text-text-2"
                title={
                  keyText
                    ? t("scratchpad.settings.open.help", {
                        keys: keyText,
                        on: opensOn,
                      })
                    : t("scratchpad.settings.open.helpNoKey", { on: opensOn })
                }
              >
                {t("scratchpad.settings.open.title")}
              </span>
              <Dropdown
                className="w-[176px] [&_.select-btn]:h-8 [&_.select-btn]:min-w-0 [&_.select-btn]:text-[13px]"
                menuClassName="right-0 w-[220px]"
                ariaLabel={t("scratchpad.settings.open.title")}
                selectedValue={openWith}
                disabled={isUpdating("scratchpad_open")}
                onSelect={(value) =>
                  updateSetting("scratchpad_open", value as ScratchpadOpen)
                }
                options={OPENS.map((value) => ({
                  value,
                  label: t(`scratchpad.settings.open.options.${value}`),
                }))}
              />
            </div>
            <label
              className="flex h-11 items-center justify-between gap-3 pe-3 ps-3.5 text-[13px] text-text-2"
              title={t("scratchpad.keepOnTop.help")}
            >
              {t("scratchpad.keepOnTop.short")}
              <button
                type="button"
                role="switch"
                aria-checked={settings?.scratchpad_keep_on_top ?? true}
                aria-label={t("scratchpad.keepOnTop.title")}
                aria-busy={isUpdating("scratchpad_keep_on_top") || undefined}
                className="toggle"
                onClick={() => {
                  if (!isUpdating("scratchpad_keep_on_top"))
                    updateSetting(
                      "scratchpad_keep_on_top",
                      !(settings?.scratchpad_keep_on_top ?? true),
                    );
                }}
              />
            </label>
          </div>
          <div className="mb-2 flex flex-none items-end justify-between gap-3 border-b border-line">
            <div
              role="tablist"
              aria-label={t("scratchpad.tabs.label")}
              className="flex gap-5"
            >
              {(["all", "pinned"] as Tab[]).map((id) => (
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
                  {t(`scratchpad.tabs.${id}`)}
                  <span className="text-xs tabular-nums text-text-3">
                    {id === "all" ? all.length || "" : pinnedCount || ""}
                  </span>
                </button>
              ))}
            </div>
            <div
              className={`relative mb-2 w-[180px] ${all.length ? "" : "invisible"}`}
            >
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
                placeholder={t("scratchpad.search")}
                aria-label={t("scratchpad.search")}
                className="input compact w-full ps-[30px] [&::-webkit-search-cancel-button]:hidden"
              />
            </div>
          </div>

          {!notes ? null : all.length === 0 ? (
            <div className="grid place-items-center gap-1.5 rounded-[var(--r-lg)] border border-line px-6 py-14 text-center">
              <h2 className="text-[15px] font-medium">
                {t("scratchpad.empty.title")}
              </h2>
              <p className="max-w-[300px] text-[13px] text-text-3">
                {keyText
                  ? t("scratchpad.empty.body", { keys: keyText })
                  : t("scratchpad.empty.bodyNoKey")}
              </p>
              <Button className="mt-3" size="sm" onClick={newNote}>
                <Plus className="h-4 w-4" aria-hidden="true" />
                {t("scratchpad.newNote")}
              </Button>
            </div>
          ) : shown.length === 0 && query.trim() ? (
            <div className="grid place-items-center gap-1.5 px-6 py-14 text-center">
              <h2 className="text-[15px] font-medium">
                {t("scratchpad.noResults", { query: query.trim() })}
              </h2>
              <p className="max-w-[300px] text-[13px] text-text-3">
                {t("scratchpad.noResultsBody")}
              </p>
              <button
                type="button"
                onClick={() => setQuery("")}
                className="mt-1 text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
              >
                {t("scratchpad.clearSearch")}
              </button>
            </div>
          ) : shown.length === 0 ? (
            <div className="grid place-items-center gap-1.5 px-6 py-14 text-center">
              <h2 className="text-[15px] font-medium">
                {t("scratchpad.emptyPinned.title")}
              </h2>
              <p className="max-w-[300px] text-[13px] text-text-3">
                {t("scratchpad.emptyPinned.body")}
              </p>
              <button
                type="button"
                onClick={() => setTab("all")}
                className="mt-1 text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
              >
                {t("scratchpad.emptyPinned.showAll")}
              </button>
            </div>
          ) : (
            <>
              <ul
                ref={listRef}
                aria-label={t("scratchpad.listLabel")}
                onKeyDown={onListKeyDown}
                className="overflow-y-auto overscroll-contain rounded-[var(--r-lg)] border border-line bg-surface min-[1000px]:min-h-0 min-[1000px]:flex-1"
              >
                {shown.map((note) => (
                  <NoteRow
                    key={note.id}
                    note={note}
                    query={query}
                    now={now}
                    focused={note.id === activeRow}
                    selected={note.id === selected?.id}
                    onFocusRow={() => setFocusedId(note.id)}
                    onOpen={() => setSelectedId(note.id)}
                    onPin={() => pin(note)}
                    onCopy={() => copy(note)}
                    onDelete={() => remove(note)}
                  />
                ))}
              </ul>
              <p className="mt-3 flex-none text-[13px] text-text-3">
                {t("scratchpad.footer", {
                  notes: t("scratchpad.counts.notes", {
                    count: all.length,
                    formatted: all.length.toLocaleString(i18n.language),
                  }),
                  pinned: pinnedCount.toLocaleString(i18n.language),
                })}
              </p>
            </>
          )}
        </div>

        <div className="flex min-w-0 flex-col min-[1000px]:min-h-0">
          {selected ? (
            <NoteCard
              key={selected.id}
              note={selected}
              textareaRef={editorRef}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
};
