import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  CalendarPlus,
  ListChecks,
  MessageCircleQuestion,
  ChevronDown,
  Search,
  Settings2,
  Tag,
} from "lucide-react";
import { commands, type Note, type NoteKind, type NoteLine } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import { formatKeyCombination } from "../../../lib/utils/keyboard";
import { useNavStore } from "../../../stores/navStore";
import { useNotesStore, watchNotes } from "../../../stores/notesStore";
import { watchCalls } from "../../../stores/callsStore";
import { ActionMenu } from "../../ui/ActionMenu";
import { Button } from "../../ui/Button";
import {
  durationLabel,
  FILTERS,
  inFilter,
  type Filter,
  isYou,
} from "../../notes/notes";
import { NoteDetail } from "./NoteDetail";
import { NoteRow } from "./NoteRow";
import { RecordDialog } from "./RecordDialog";
import { ScheduleDialog } from "../../notes/ScheduleDialog";
import { RecordingStrip } from "./RecordingStrip";
import { CallStrip } from "./CallStrip";
import { TasksView } from "./TasksView";
import { AskNotes } from "./AskNotes";
import { FolderFilter } from "./Folders";
import { ConfirmDelete } from "../../shared/ConfirmDelete";
import {
  RecentlyDeleted,
  RecentlyDeletedLink,
  useTrash,
} from "../../shared/RecentlyDeleted";

/** Search results: matching note ids, with the transcript line that matched
 *  when only the transcript did. */
type Matches = Map<number, NoteLine | null>;

/** Record, with a caret that picks a meeting or a voice memo. */
const RecordButton: React.FC<{
  busy: boolean;
  onRecord: (kind: NoteKind) => void;
}> = ({ busy, onRecord }) => {
  const { t } = useTranslation();
  const [menu, setMenu] = useState(false);
  const caret = useRef<HTMLButtonElement>(null);
  return (
    <div
      className={`relative inline-flex ${busy ? "cursor-not-allowed" : ""}`}
      title={busy ? t("notes.recordBusy") : undefined}
    >
      <Button
        className="rounded-e-none"
        disabled={busy}
        onClick={() => onRecord("meeting")}
      >
        <span
          aria-hidden="true"
          className="h-2 w-2 rounded-full bg-danger shadow-[0_0_0_2px_color-mix(in_srgb,var(--danger)_22%,transparent)]"
        />
        {t("notes.record.button")}
      </Button>
      <button
        ref={caret}
        type="button"
        disabled={busy}
        aria-haspopup="menu"
        aria-expanded={menu}
        aria-label={t("notes.record.choose")}
        title={t("notes.record.choose")}
        onClick={() => setMenu(!menu)}
        className="ms-px grid h-9 w-[30px] place-items-center rounded-e-[var(--r-md)] bg-teal-fill text-white transition-colors duration-[var(--dur-1)] hover:bg-teal-fill-hover hover:duration-0 active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:bg-teal-fill"
      >
        <ChevronDown className="h-[15px] w-[15px]" aria-hidden="true" />
      </button>
      {menu && (
        <ActionMenu
          label={t("notes.record.choose")}
          className="end-0 top-full z-20 mt-1 w-48"
          onClose={(restore) => {
            setMenu(false);
            if (restore) caret.current?.focus();
          }}
          items={(["meeting", "memo"] as NoteKind[]).map((kind) => ({
            id: kind,
            label: t(`notes.record.new.${kind}`),
            onSelect: () => onRecord(kind),
          }))}
        />
      )}
    </div>
  );
};

/** Meetings and voice memos: the recording in progress on top, the list on
 *  the left and the open note on the right. */
export const NotesPage: React.FC = () => {
  const { t, i18n } = useTranslation();
  const go = useNavStore((s) => s.go);
  const hint = useNavStore((s) => s.hint);
  const clearHint = useNavStore((s) => s.clearHint);
  const notes = useNotesStore((s) => s.notes);
  const session = useNotesStore((s) => s.session);
  const folders = useNotesStore((s) => s.folders);
  const { settings } = useSettings();
  const [filter, setFilter] = useState<Filter>("all");
  const [tag, setTag] = useState<string | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const [tagMenu, setTagMenu] = useState(false);
  const [query, setQuery] = useState("");
  const [matches, setMatches] = useState<Matches | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [tasks, setTasks] = useState(false);
  const [askingAll, setAskingAll] = useState(false);
  const [focusedId, setFocusedId] = useState<number | null>(null);
  const [dialog, setDialog] = useState<NoteKind | null>(null);
  const [scheduling, setScheduling] = useState(false);
  const [hidden, setHidden] = useState<Set<number>>(new Set());
  const listRef = useRef<HTMLUListElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const tagButton = useRef<HTMLButtonElement>(null);
  const now = new Date();

  useEffect(watchNotes, []);
  useEffect(watchCalls, []);

  // Opened from the Legato bar or elsewhere on one note.
  useEffect(() => {
    if (!hint) return;
    const id = Number(hint);
    if (Number.isFinite(id)) {
      setSelectedId(id);
      setQuery("");
      setFilter("all");
      setTag(null);
      setFolder(null);
    }
    clearHint();
  }, [hint, clearHint]);

  // Search runs in the backend, across titles, tags, summaries, transcripts
  // and My notes.
  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setMatches(null);
      return;
    }
    let live = true;
    const timer = setTimeout(() => {
      commands
        .searchNotes(q)
        .then((r) => {
          if (!live || r.status !== "ok") return;
          setMatches(new Map(r.data.map((m) => [m.id, m.line])));
        })
        .catch(() => {});
    }, 120);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [query, notes]);

  const all = useMemo(
    () => (notes ?? []).filter((n) => !hidden.has(n.id)),
    [notes, hidden],
  );
  // The notes in the folder picked, or every note.
  const inFolder = useMemo(
    () =>
      folder
        ? all.filter((n) => n.folder?.toLowerCase() === folder.toLowerCase())
        : all,
    [all, folder],
  );
  // A folder renamed or deleted elsewhere stops filtering.
  useEffect(() => {
    if (
      folder &&
      !folders.some((f) => f.name.toLowerCase() === folder.toLowerCase())
    )
      setFolder(null);
  }, [folders, folder]);
  // Your next steps still open, for the Next steps button.
  const openTasks = useMemo(
    () =>
      all.reduce(
        (n, note) =>
          n + note.actions.filter((a) => !a.done && isYou(a.owner)).length,
        0,
      ),
    [all],
  );
  const tags = useMemo(() => {
    const seen = new Map<string, string>();
    for (const n of all)
      for (const tg of n.tags)
        if (!seen.has(tg.toLowerCase())) seen.set(tg.toLowerCase(), tg);
    return [...seen.values()].sort((a, b) => a.localeCompare(b));
  }, [all]);
  const counts = useMemo(
    () =>
      Object.fromEntries(
        FILTERS.map((f) => [
          f,
          inFolder.filter((n) => inFilter(n, f, tag)).length,
        ]),
      ) as Record<Filter, number>,
    [inFolder, tag],
  );
  const shown = useMemo(
    () =>
      inFolder.filter(
        (n) => inFilter(n, filter, tag) && (!matches || matches.has(n.id)),
      ),
    [inFolder, filter, tag, matches],
  );
  const recordingId = session?.note_id ?? null;
  const selected = all.find((n) => n.id === selectedId) ?? shown[0] ?? null;

  // A new recording opens on its own note.
  const lastSession = useRef(recordingId);
  useEffect(() => {
    if (recordingId !== null && recordingId !== lastSession.current)
      setSelectedId(recordingId);
    lastSession.current = recordingId;
  }, [recordingId]);

  useTextHighlight(
    listRef,
    "recordings-search",
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

  // Deleting asks first; the note then waits in Recently deleted.
  const [asking, setAsking] = useState<Note | null>(null);
  const [trashOpen, setTrashOpen] = useState(false);
  const trash = useTrash("note");

  const remove = async (note: Note) => {
    setHidden((h) => new Set(h).add(note.id));
    const result = await commands.deleteNote(note.id);
    if (result.status === "error" || !result.data) {
      setHidden((h) => {
        const next = new Set(h);
        next.delete(note.id);
        return next;
      });
      if (result.status === "error")
        toast.error(t("notes.toast.deleteFailed"), {
          description: result.error,
        });
      return;
    }
    const removed = result.data;
    toast(t("notes.toast.deleted", { title: note.title }), {
      action: {
        label: t("notes.toast.undo"),
        onClick: async () => {
          const back = await commands.restoreNote(removed).catch(() => null);
          if (back?.status === "ok") setSelectedId(note.id);
          else toast.error(t("notes.toast.restoreFailed"));
        },
      },
    });
  };

  // Hidden only bridges the wait for the reloaded list.
  useEffect(() => setHidden(new Set()), [notes]);

  const record = (kind: NoteKind) => {
    if (session) return;
    setDialog(kind);
  };

  // R records, / searches.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement).closest(
        "input, textarea, [contenteditable=true], [role=dialog], [role=menu]",
      );
      if (typing || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key.toLowerCase() === "r" && !session) {
        e.preventDefault();
        setDialog("meeting");
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
      const next = ids[Math.min(i + 1, ids.length - 1)];
      focusRow(next);
      setSelectedId(next);
    } else if (key === "arrowup" || key === "k") {
      e.preventDefault();
      const next = ids[Math.max(i - 1, 0)];
      focusRow(next);
      setSelectedId(next);
    } else if (key === "home") {
      e.preventDefault();
      focusRow(ids[0]);
      setSelectedId(ids[0]);
    } else if (key === "end") {
      e.preventDefault();
      focusRow(ids[ids.length - 1]);
      setSelectedId(ids[ids.length - 1]);
    } else if (key === "enter") {
      e.preventDefault();
      setSelectedId(id);
    } else if (key === "delete" && note.id !== recordingId) {
      e.preventDefault();
      const next = ids[i + 1] ?? ids[i - 1];
      setAsking(note);
    }
  };

  const binding = settings?.bindings?.toggle_notes?.current_binding ?? "";
  const keys = formatKeyCombination(binding).split(" + ").filter(Boolean);
  const totalMs = all.reduce((sum, n) => sum + n.duration_ms, 0);

  const empty = (
    <div className="grid place-items-center gap-1.5 px-6 py-16 text-center">
      <h2 className="text-[15px] font-medium">{t("notes.empty.title")}</h2>
      <p className="max-w-[320px] text-[13px] text-text-3">
        {keys.length
          ? t("notes.empty.body", { keys: keys.join("+") })
          : t("notes.empty.bodyNoKey")}
      </p>
      <Button
        className="mt-3"
        size="sm"
        disabled={!!session}
        onClick={() => record("meeting")}
      >
        {t("notes.record.button")}
      </Button>
    </div>
  );

  return (
    <div className="mx-auto flex w-full max-w-[1240px] flex-col min-[1000px]:min-h-0 min-[1000px]:flex-1">
      <div className="mb-6 flex min-h-10 flex-wrap items-center justify-between gap-4">
        <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
          {t("notes.title")}
        </h1>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setTasks(!tasks)}
            aria-pressed={tasks}
            title={t("notes.tasks.button")}
            className={`inline-flex h-9 items-center gap-1.5 rounded-[var(--r-md)] border px-3 text-[13px] shadow-[var(--shadow-1)] transition-colors duration-[var(--dur-1)] hover:duration-0 active:scale-[0.97] ${
              tasks
                ? "border-teal-700 bg-teal-50 font-medium text-teal-700"
                : "border-line-strong bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
            }`}
          >
            <ListChecks className="h-4 w-4" aria-hidden="true" />
            {t("notes.tasks.button")}
            {openTasks > 0 && (
              <span className="tabular-nums text-text-3">{openTasks}</span>
            )}
          </button>
          <button
            type="button"
            onClick={() => setAskingAll(true)}
            aria-label={t("notes.askAll.title")}
            title={t("notes.askAll.title")}
            className="grid h-9 w-9 place-items-center rounded-[var(--r-md)] border border-line-strong bg-surface text-text-2 shadow-[var(--shadow-1)] transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
          >
            <MessageCircleQuestion className="h-4 w-4" aria-hidden="true" />
          </button>
          <button
            type="button"
            onClick={() => setScheduling(true)}
            aria-label={t("notes.schedule.button")}
            title={t("notes.schedule.button")}
            className="grid h-9 w-9 place-items-center rounded-[var(--r-md)] border border-line-strong bg-surface text-text-2 shadow-[var(--shadow-1)] transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
          >
            <CalendarPlus className="h-4 w-4" aria-hidden="true" />
          </button>
          <button
            type="button"
            onClick={() => go("settings", "calls")}
            aria-label={t("notes.callSettings")}
            title={t("notes.callSettings")}
            className="grid h-9 w-9 place-items-center rounded-[var(--r-md)] border border-line-strong bg-surface text-text-2 shadow-[var(--shadow-1)] transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
          >
            <Settings2 className="h-4 w-4" aria-hidden="true" />
          </button>
          <button
            type="button"
            onClick={() => go("settings", "shortcuts")}
            title={t("notes.shortcut.change")}
            className="hidden items-center gap-1 rounded-[var(--r-sm)] bg-surface-2 py-1 pe-2.5 ps-1.5 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:duration-0 active:scale-[0.98] min-[900px]:inline-flex"
          >
            {keys.length > 0 ? (
              <>
                {keys.map((key) => (
                  <kbd key={key} className="kbd">
                    {key}
                  </kbd>
                ))}
                <span className="ms-1">{t("notes.shortcut.chip")}</span>
              </>
            ) : (
              t("notes.shortcut.none")
            )}
          </button>
          <RecordButton busy={!!session} onRecord={record} />
        </div>
      </div>
      <p className="-mt-3.5 mb-5 max-w-[680px] text-sm text-text-2">
        {t("notes.subtitle")}
      </p>

      {session ? <RecordingStrip session={session} /> : <CallStrip />}

      {!notes ? null : tasks ? (
        <TasksView
          notes={all}
          onOpen={(id) => {
            setTasks(false);
            setQuery("");
            setFilter("all");
            setTag(null);
            setFolder(null);
            setSelectedId(id);
          }}
        />
      ) : all.length === 0 && !session ? (
        <div className="rounded-[var(--r-lg)] border border-line bg-surface">
          {empty}
        </div>
      ) : (
        <div className="grid overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface min-[1000px]:min-h-[320px] min-[1000px]:flex-1 min-[1000px]:grid-cols-[minmax(0,340px)_minmax(0,1fr)] min-[1000px]:grid-rows-[minmax(0,1fr)]">
          <div className="flex min-h-0 min-w-0 flex-col border-line max-[999px]:border-b min-[1000px]:border-e">
            <div className="grid gap-2.5 border-b border-line p-3">
              <div className="relative">
                <Search
                  className="pointer-events-none absolute start-[11px] top-1/2 h-[14px] w-[14px] -translate-y-1/2 text-text-3"
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
                  placeholder={t("notes.search")}
                  aria-label={t("notes.search")}
                  className="input w-full ps-[32px] [&::-webkit-search-cancel-button]:hidden"
                />
              </div>
              {/* Folders and Tags drop to their own line when the column
                  is too narrow for them beside the filter. */}
              <div className="flex flex-wrap items-center justify-between gap-x-2 gap-y-1.5">
                <div
                  role="radiogroup"
                  aria-label={t("notes.filter.label")}
                  className="inline-flex rounded-[var(--r-md)] bg-surface-2 p-[3px]"
                >
                  {FILTERS.map((f) => (
                    <button
                      key={f}
                      type="button"
                      role="radio"
                      aria-checked={filter === f}
                      onClick={() => setFilter(f)}
                      className={`inline-flex h-[26px] items-center gap-1.5 rounded-[var(--r-sm)] px-2.5 text-xs transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                        filter === f
                          ? "bg-surface font-medium text-text shadow-sm"
                          : "text-text-2 hover:text-text"
                      }`}
                    >
                      {t(`notes.filter.${f}`)}
                      <span className="tabular-nums text-text-3">
                        {counts[f]}
                      </span>
                    </button>
                  ))}
                </div>
                <div className="ms-auto flex min-w-0 items-center">
                  <FolderFilter
                    folders={folders}
                    folder={folder}
                    onChange={setFolder}
                  />
                  {tags.length > 0 && (
                    <div className="relative">
                      <button
                        ref={tagButton}
                        type="button"
                        onClick={() => setTagMenu(!tagMenu)}
                        aria-haspopup="menu"
                        aria-expanded={tagMenu}
                        aria-label={tag ? undefined : t("notes.filter.tags")}
                        title={tag ? undefined : t("notes.filter.tags")}
                        className={`inline-flex h-7 max-w-[130px] items-center gap-1 rounded-[var(--r-sm)] px-1.5 text-[13px] transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0 ${
                          tag ? "font-medium text-teal-700" : "text-text-2"
                        }`}
                      >
                        <Tag
                          className="h-3.5 w-3.5 flex-none"
                          aria-hidden="true"
                        />
                        {tag && <span className="truncate">{tag}</span>}
                        <ChevronDown
                          className="h-3.5 w-3.5 flex-none"
                          aria-hidden="true"
                        />
                      </button>
                      {tagMenu && (
                        <ActionMenu
                          label={t("notes.filter.tags")}
                          className="end-0 top-full z-10 mt-1 w-52"
                          onClose={(restore) => {
                            setTagMenu(false);
                            if (restore) tagButton.current?.focus();
                          }}
                          items={[
                            {
                              id: "any",
                              label: t("notes.filter.anyTag"),
                              checked: tag === null,
                              onSelect: () => setTag(null),
                            },
                            "separator",
                            ...tags.map((tg) => ({
                              id: `tag-${tg}`,
                              label: tg,
                              checked: tag?.toLowerCase() === tg.toLowerCase(),
                              onSelect: () => setTag(tg),
                            })),
                          ]}
                        />
                      )}
                    </div>
                  )}
                </div>
              </div>
            </div>

            {shown.length === 0 ? (
              <div className="grid flex-1 place-items-center gap-1.5 px-6 py-14 text-center">
                <h2 className="text-[15px] font-medium">
                  {query.trim()
                    ? t("notes.noResults", { query: query.trim() })
                    : t("notes.noneHere")}
                </h2>
                <p className="max-w-[260px] text-[13px] text-text-3">
                  {query.trim()
                    ? t("notes.noResultsBody")
                    : t("notes.noneHereBody")}
                </p>
                <button
                  type="button"
                  onClick={() => {
                    setQuery("");
                    setFilter("all");
                    setTag(null);
                    setFolder(null);
                  }}
                  className="mt-1 text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
                >
                  {query.trim() ? t("notes.clearSearch") : t("notes.showAll")}
                </button>
              </div>
            ) : (
              <ul
                ref={listRef}
                aria-label={t("notes.listLabel")}
                onKeyDown={onListKeyDown}
                className="max-h-[50vh] min-h-[200px] flex-1 overflow-y-auto min-[1000px]:max-h-none min-[1000px]:min-h-0"
              >
                {shown.map((note) => (
                  <NoteRow
                    key={note.id}
                    note={note}
                    match={matches?.get(note.id) ?? null}
                    query={query}
                    now={now}
                    recording={
                      note.id === recordingId && session?.phase === "recording"
                    }
                    focused={note.id === activeRow}
                    selected={note.id === selected?.id}
                    onFocusRow={() => setFocusedId(note.id)}
                    onOpen={() => setSelectedId(note.id)}
                    onDelete={() => setAsking(note)}
                  />
                ))}
              </ul>
            )}
            <p className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 border-t border-line px-4 py-2.5 text-xs text-text-3">
              <span>
                {t("notes.footer", {
                  notes: t("notes.counts.notes", {
                    count: all.length,
                    formatted: all.length.toLocaleString(i18n.language),
                  }),
                  duration: durationLabel(totalMs),
                })}
              </span>
              <RecentlyDeletedLink
                count={trash.length}
                onOpen={() => setTrashOpen(true)}
              />
            </p>
          </div>

          <div className="min-w-0 min-[1000px]:min-h-0">
            {selected ? (
              <NoteDetail
                key={selected.id}
                note={selected}
                recording={selected.id === recordingId}
                finishing={
                  selected.id === recordingId && session?.phase === "finishing"
                }
                onDelete={() => setAsking(selected)}
              />
            ) : (
              <p className="px-7 py-14 text-center text-[13px] text-text-3">
                {t("notes.pick")}
              </p>
            )}
          </div>
        </div>
      )}

      <ScheduleDialog open={scheduling} onClose={() => setScheduling(false)} />

      <AskNotes
        open={askingAll}
        notes={all}
        onClose={() => setAskingAll(false)}
        onOpen={(id) => {
          setAskingAll(false);
          setTasks(false);
          setQuery("");
          setFilter("all");
          setTag(null);
          setFolder(null);
          setSelectedId(id);
        }}
      />

      <RecordDialog
        open={dialog !== null}
        kind={dialog ?? "meeting"}
        onClose={() => setDialog(null)}
        onStarted={(id) => {
          setTasks(false);
          setSelectedId(id);
          setQuery("");
          setFilter("all");
          setTag(null);
          setFolder(null);
        }}
      />

      <ConfirmDelete
        open={asking !== null}
        title={t("notes.confirmDelete.title", { title: asking?.title ?? "" })}
        description={t("notes.confirmDelete.description")}
        onCancel={() => setAsking(null)}
        onConfirm={() => {
          const note = asking;
          setAsking(null);
          if (!note) return;
          // The next note along takes the selection and focus.
          const i = ids.indexOf(note.id);
          const next = ids[i + 1] ?? ids[i - 1];
          if (selected?.id === note.id && next !== undefined) {
            setSelectedId(next);
            focusRow(next);
          }
          remove(note);
        }}
      />
      <RecentlyDeleted
        kind="note"
        items={trash}
        open={trashOpen}
        onClose={() => setTrashOpen(false)}
        onRestored={(item) => {
          setTrashOpen(false);
          setSelectedId(item.item_id);
        }}
      />
    </div>
  );
};
