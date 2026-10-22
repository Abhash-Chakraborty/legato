import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Bold,
  Bot,
  ChevronDown,
  Code,
  Copy,
  Eye,
  Heading2,
  History,
  Italic,
  List,
  ListChecks,
  ListOrdered,
  Maximize2,
  Mic,
  Minimize2,
  MoreHorizontal,
  PanelLeft,
  PencilLine,
  PictureInPicture2,
  Pin,
  PinOff,
  Plus,
  Replace,
  Search,
  Send,
  Square,
  SquarePen,
  Trash2,
  Type,
  X,
} from "lucide-react";
import {
  commands,
  events,
  type InyonAgent,
  type ScratchNote,
  type SendTarget,
  type Transform,
} from "@/bindings";
import { AppToaster } from "@/components/shell/AppToaster";
import { ActionMenu, type ActionMenuItem } from "@/components/ui/ActionMenu";
import { AppIcon } from "@/components/pages/home/AppIcon";
import { copyToClipboard } from "@/components/pages/home/clipboard";
import { FailedPasteBanner } from "@/components/scratchpad/FailedPasteBanner";
import { NotePreview } from "@/components/scratchpad/NotePreview";
import { NoteStatus } from "@/components/scratchpad/NoteStatus";
import { VersionHistory } from "@/components/scratchpad/VersionHistory";
import {
  byUpdated,
  insertAt,
  matchesQuery,
  prefixLines,
  wrapSelection,
  type Edit,
} from "@/components/scratchpad/notes";
import { relativeTime } from "@/components/pages/dictionary/list";
import legatoIcon from "@/assets/legato-icon.svg";
import { useNoteDraft } from "@/components/scratchpad/useNoteDraft";
import { useScratchStore, watchScratch } from "@/stores/scratchStore";
import { useAiStore, watchAi } from "@/stores/aiStore";
import { providerFor } from "@/components/pages/transforms/model";
import { useProblemText } from "@/components/pages/transforms/TryIt";
import { agentName } from "@/lib/utils/agents";

type Mode = "write" | "preview";

const tbButton =
  "grid h-7 w-8 flex-none place-items-center self-center rounded-[6px] text-text-2 transition-[background-color,color,transform] duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 active:scale-[0.94] active:bg-surface-4 active:duration-0";

/** A row in the left rail: an icon, and its label while the notes panel
 *  is open. */
const railItem =
  "flex h-9 w-full flex-none items-center gap-3 rounded-[8px] px-[11px] text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 active:scale-[0.98]";

const fmtButton =
  "grid h-7 w-7 flex-none place-items-center rounded-[6px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 active:scale-[0.94]";

const PANEL_KEY = "legato.scratchpad.panel";
const readPanel = () => {
  try {
    return localStorage.getItem(PANEL_KEY) === "1";
  } catch {
    return false;
  }
};

/** The floating Scratchpad: up to five notes as tabs, written or dictated
 *  into, and sent to the app that was in front when it opened. */
export const ScratchpadWindow: React.FC = () => {
  const { t } = useTranslation();
  const notes = useScratchStore((s) => s.notes);
  const put = useScratchStore((s) => s.put);
  const [activeId, setActiveId] = useState<number | null>(null);
  const [keepOnTop, setKeepOnTop] = useState(true);
  const [target, setTarget] = useState<{
    name: string;
    path: string | null;
  } | null>(null);
  const [mode, setMode] = useState<Mode>("write");
  const [dictating, setDictating] = useState(false);
  const [menu, setMenu] = useState(false);
  const [renaming, setRenaming] = useState<number | null>(null);
  const [versionsOf, setVersionsOf] = useState<ScratchNote | null>(null);
  const [sending, setSending] = useState(false);
  const [targets, setTargets] = useState<SendTarget[] | null>(null);
  // Agent sessions in Inyon, when paired; one picked replaces the window.
  const [agents, setAgents] = useState<InyonAgent[]>([]);
  const [agentTarget, setAgentTarget] = useState<InyonAgent | null>(null);
  const [transforms, setTransforms] = useState<Transform[] | null>(null);
  const [tfMenu, setTfMenu] = useState(false);
  const [transforming, setTransforming] = useState<string | null>(null);
  const run = useRef(0);
  const ai = useAiStore((s) => s.status);
  const problemText = useProblemText();
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [panel, setPanelState] = useState(readPanel);
  const [query, setQuery] = useState("");
  const [formatting, setFormatting] = useState(false);
  const [maximized, setMaximized] = useState(false);
  const [rowMenu, setRowMenu] = useState<number | null>(null);
  const now = Date.now() / 1000;
  const { i18n } = useTranslation();

  const setPanel = (open: boolean) => {
    setPanelState(open);
    try {
      localStorage.setItem(PANEL_KEY, open ? "1" : "0");
    } catch {
      // Remembering the panel is a convenience.
    }
  };

  useEffect(() => {
    const win = getCurrentWindow();
    const sync = () =>
      win
        .isMaximized()
        .then(setMaximized)
        .catch(() => {});
    sync();
    const off = win.onResized(sync);
    return () => {
      off.then((fn) => fn());
    };
  }, []);

  /** Every note, pinned first, then the ones edited last, narrowed by the
   *  search. */
  const listed = useMemo(
    () =>
      [...(notes ?? [])]
        .filter((n) => matchesQuery(n, query))
        .sort((a, b) => Number(b.pinned) - Number(a.pinned) || byUpdated(a, b)),
    [notes, query],
  );

  const tabs = useMemo(
    () =>
      (notes ?? [])
        .filter((n) => n.tab !== null)
        .sort((a, b) => (a.tab ?? 0) - (b.tab ?? 0)),
    [notes],
  );
  const active =
    tabs.find((n) => n.id === activeId) ?? tabs[tabs.length - 1] ?? null;
  const draft = useNoteDraft(active);

  const refresh = useCallback(async (show?: number) => {
    const result = await commands.getScratchpadState().catch(() => null);
    if (result?.status !== "ok") return;
    const state = result.data;
    setKeepOnTop(state.keep_on_top);
    setTarget(
      state.target_app
        ? { name: state.target_app, path: state.target_app_path }
        : null,
    );
    setActiveId(show ?? state.active);
  }, []);

  useEffect(() => {
    watchScratch();
    refresh();
    const subs = [
      events.scratchpadShow.listen((e) => {
        useScratchStore.getState().load();
        refresh(e.payload.note_id);
        requestAnimationFrame(() => editorRef.current?.focus());
      }),
      // A dictation into a Notes page's My notes isn't this window's.
      events.scratchpadDictating.listen(
        (e) => !e.payload.notes && setDictating(e.payload.active),
      ),
    ];
    return () => {
      subs.forEach((s) => s.then((off) => off()));
    };
  }, [refresh]);

  // Dictated words land at the cursor of the note they were meant for.
  const latest = useRef({ active, draft });
  latest.current = { active, draft };
  useEffect(() => {
    const sub = events.scratchpadInsert.listen(async (e) => {
      const { note_id, text } = e.payload;
      const { active, draft } = latest.current;
      const editor = editorRef.current;
      if (active?.id === note_id && editor) {
        const next = insertAt(
          draft.text,
          editor.selectionStart,
          editor.selectionEnd,
          text,
        );
        draft.setText(next.text);
        setMode("write");
        requestAnimationFrame(() => {
          editor.focus();
          editor.setSelectionRange(next.cursor, next.cursor);
        });
        return;
      }
      const note = useScratchStore
        .getState()
        .notes?.find((n) => n.id === note_id);
      if (!note) return;
      const end = note.text.length;
      const result = await commands.saveScratchNote(
        note_id,
        insertAt(note.text, end, end, text).text,
      );
      if (result.status === "ok") put(result.data);
    });
    return () => {
      sub.then((off) => off());
    };
  }, [put]);

  const activate = (id: number) => {
    if (transforming) cancelTransform();
    setActiveId(id);
    setRenaming(null);
    commands.openScratchTab(id).catch(() => {});
  };

  const newTab = async () => {
    const result = await commands.newScratchNote();
    if (result.status === "error") {
      toast.error(t("scratchpad.toast.newFailed"), {
        description: result.error,
      });
      return;
    }
    put({ ...result.data });
    setActiveId(result.data.id);
    setMode("write");
    useScratchStore.getState().load();
    requestAnimationFrame(() => editorRef.current?.focus());
  };

  const closeTab = async (id: number) => {
    await draft.flush();
    const i = tabs.findIndex((n) => n.id === id);
    const rest = tabs.filter((n) => n.id !== id);
    await commands.closeScratchTab(id);
    if (rest.length === 0) {
      commands.hideScratchpad();
      return;
    }
    if (active?.id === id) activate(rest[Math.min(i, rest.length - 1)].id);
  };

  // Dragging a tab: where it started, how far it has moved and where it
  // would land. The other tabs slide aside to show the gap.
  const [drag, setDrag] = useState<{
    id: number;
    from: number;
    to: number;
    dx: number;
    width: number;
  } | null>(null);
  const press = useRef<{
    id: number;
    x: number;
    centers: number[];
    width: number;
    moved: boolean;
  } | null>(null);
  const dragged = useRef(false);

  const onTabPointerDown = (
    e: React.PointerEvent<HTMLButtonElement>,
    note: ScratchNote,
  ) => {
    if (e.button !== 0 || tabs.length < 2) return;
    const rects = [
      ...document.querySelectorAll<HTMLElement>("[data-tab-wrap]"),
    ].map((w) => w.getBoundingClientRect());
    const i = tabs.findIndex((n) => n.id === note.id);
    press.current = {
      id: note.id,
      x: e.clientX,
      centers: rects.map((r) => r.left + r.width / 2),
      // The tab and the gap after it.
      width: (rects[i]?.width ?? 0) + 2,
      moved: false,
    };
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onTabPointerMove = (e: React.PointerEvent<HTMLButtonElement>) => {
    const p = press.current;
    if (!p) return;
    const dx = e.clientX - p.x;
    if (!p.moved && Math.abs(dx) < 5) return;
    p.moved = true;
    const from = tabs.findIndex((n) => n.id === p.id);
    const center = (p.centers[from] ?? 0) + dx;
    const to = p.centers.filter((c, j) => j !== from && c < center).length;
    setDrag({ id: p.id, from, to, dx, width: p.width });
  };

  const onTabPointerUp = () => {
    const p = press.current;
    press.current = null;
    const done = drag;
    setDrag(null);
    if (!p?.moved || !done) return;
    dragged.current = true;
    if (done.from === done.to) return;
    // Show the new order now; the backend's reload confirms it.
    const order = tabs.map((n) => n.id);
    order.splice(done.from, 1);
    order.splice(done.to, 0, done.id);
    useScratchStore.setState((s) => ({
      notes:
        s.notes?.map((n) =>
          n.tab === null ? n : { ...n, tab: order.indexOf(n.id) },
        ) ?? null,
    }));
    commands.moveScratchTab(done.id, done.to).catch(() => {});
  };

  const tabShift = (i: number, id: number): React.CSSProperties | undefined => {
    if (!drag) return undefined;
    if (id === drag.id)
      return {
        transform: `translateX(${drag.dx}px)`,
        transition: "none",
        zIndex: 10,
      };
    const { from, to, width } = drag;
    if (from < to && i > from && i <= to)
      return { transform: `translateX(${-width}px)` };
    if (to < from && i >= to && i < from)
      return { transform: `translateX(${width}px)` };
    return undefined;
  };

  const moveTab = (id: number, by: number) => {
    const i = tabs.findIndex((n) => n.id === id);
    const to = i + by;
    if (i < 0 || to < 0 || to >= tabs.length) return;
    commands.moveScratchTab(id, to).catch(() => {});
  };

  const toggleKeepOnTop = () => {
    const next = !keepOnTop;
    setKeepOnTop(next);
    commands.changeScratchpadKeepOnTopSetting(next).catch(() => {});
  };

  const togglePin = async (note: ScratchNote) => {
    const result = await commands.setScratchNotePinned(note.id, !note.pinned);
    if (result.status === "ok") put(result.data);
  };

  const rename = async (note: ScratchNote, title: string) => {
    setRenaming(null);
    if (title.trim() === note.title) return;
    const result = await commands.renameScratchNote(note.id, title);
    if (result.status === "ok") put(result.data);
  };

  const remove = async (note: ScratchNote) => {
    await draft.flush();
    const i = tabs.findIndex((n) => n.id === note.id);
    const result = await commands.deleteScratchNotes([note.id]);
    if (result.status === "error") {
      toast.error(t("scratchpad.toast.deleteFailed"), {
        description: result.error,
      });
      return;
    }
    const rest = tabs.filter((n) => n.id !== note.id);
    if (rest.length) setActiveId(rest[Math.min(i, rest.length - 1)].id);
    const removed = result.data;
    toast(
      t("scratchpad.toast.deleted", {
        title: note.title || t("scratchpad.untitled"),
      }),
      {
        action: {
          label: t("scratchpad.toast.undo"),
          onClick: () => {
            commands.restoreScratchNotes(removed).then(() => {
              setActiveId(note.id);
            });
          },
        },
      },
    );
  };

  const dictate = () => {
    if (!active) return;
    draft.flush();
    setMode("write");
    commands.scratchpadDictate(active.id);
    editorRef.current?.focus();
  };

  const openTransforms = async () => {
    setTfMenu(true);
    watchAi();
    // Fresh each time: transforms change on the Transforms page.
    const settings = await commands.getAppSettings().catch(() => null);
    if (settings?.status === "ok")
      setTransforms((settings.data.transforms ?? []).filter((x) => x.enabled));
  };

  /** Rewrites the selection, or the whole note when nothing is selected,
   *  with Undo. Esc or switching notes drops the result. */
  const transform = async (tf: Transform) => {
    if (!active || transforming) return;
    const editor = editorRef.current;
    const text = draft.text;
    const selected =
      mode === "write" && editor && editor.selectionEnd > editor.selectionStart;
    const [from, to] = selected
      ? [editor.selectionStart, editor.selectionEnd]
      : [0, text.length];
    const part = text.slice(from, to);
    if (!part.trim()) return;
    const id = ++run.current;
    const noteId = active.id;
    setTransforming(tf.name);
    const result = await commands.tryTransform(tf, part).catch(() => null);
    if (id !== run.current) return;
    setTransforming(null);
    if (latest.current.active?.id !== noteId) return;
    const out = result?.status === "ok" ? result.data.text : null;
    if (out === null || result?.status !== "ok") {
      toast.error(t("scratchpad.transform.failed", { name: tf.name }), {
        description:
          result?.status === "ok"
            ? problemText(
                result.data.problem ?? "failed",
                providerFor(tf, [], ai),
              )
            : (result?.error ?? undefined),
      });
      return;
    }
    const before = latest.current.draft.text;
    const next = text.slice(0, from) + out + text.slice(to);
    latest.current.draft.setText(next);
    setMode("write");
    requestAnimationFrame(() => {
      editorRef.current?.focus();
      editorRef.current?.setSelectionRange(from, from + out.length);
    });
    toast(t("scratchpad.transform.done", { name: tf.name }), {
      action: {
        label: t("scratchpad.toast.undo"),
        onClick: () => {
          if (latest.current.active?.id === noteId)
            latest.current.draft.setText(before);
        },
      },
    });
  };

  const cancelTransform = () => {
    run.current++;
    setTransforming(null);
  };

  const copyAll = async () => {
    if (!draft.text.trim()) return;
    if (await copyToClipboard(draft.text)) toast(t("scratchpad.toast.copied"));
  };

  const openTargets = async () => {
    setTargets([]);
    const [windows, sessions] = await Promise.all([
      commands.getSendTargets().catch(() => []),
      commands.inyonAgents().catch(() => []),
    ]);
    setAgents(sessions);
    setTargets(windows);
  };

  const agentLabel = (a: InyonAgent) =>
    t("scratchpad.window.agentLabel", {
      agent: agentName(a.agent),
      label: a.label,
    });

  const sendToAgent = async (a: InyonAgent) => {
    setSending(true);
    await draft.flush();
    const result = await commands.inyonSend(a.host, a.pane, draft.text);
    setSending(false);
    if (result.status === "error")
      toast.error(t(`inyon.reason.${result.error}`, t("inyon.reason.failed")));
    else
      toast.success(
        t(`scratchpad.toast.agent_${result.data}`, {
          agent: agentName(a.agent),
          defaultValue: t("scratchpad.toast.agent_sent", {
            agent: agentName(a.agent),
          }),
        }),
      );
  };

  const pickTarget = async (x: SendTarget) => {
    setAgentTarget(null);
    const result = await commands.setSendTarget(x.handle);
    if (result.status === "error")
      toast.error(t("scratchpad.window.targetGone", { app: x.name }));
    refresh(active?.id);
  };

  const send = async () => {
    if (!active || !draft.text.trim() || sending) return;
    if (agentTarget) return sendToAgent(agentTarget);
    if (!target) return;
    setSending(true);
    await draft.flush();
    const result = await commands.sendScratchNote(draft.text);
    setSending(false);
    if (result.status === "error") {
      // The window hid to paste; bring it back to say why.
      await commands.openScratchpad(active.id);
      toast.error(t("scratchpad.toast.sendFailed"), {
        description: result.error,
      });
    }
  };

  // Window shortcuts. Ctrl+Space dictates, Ctrl+Enter sends, Ctrl+T or
  // Ctrl+N opens a tab, Ctrl+W closes one, Ctrl+Tab moves between them.
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && transforming) {
      e.preventDefault();
      cancelTransform();
      return;
    }
    if (!e.ctrlKey || e.altKey || e.metaKey) return;
    const key = e.key.toLowerCase();
    if (key === "f") {
      e.preventDefault();
      openSearch();
    } else if ((key === "b" || key === "i") && mode === "write") {
      e.preventDefault();
      wrap(key === "b" ? "**" : "_");
    } else if (key === " ") {
      e.preventDefault();
      dictate();
    } else if (key === "enter") {
      e.preventDefault();
      send();
    } else if ((key === "t" || key === "n") && !e.shiftKey) {
      e.preventDefault();
      newTab();
    } else if (key === "w" && active) {
      e.preventDefault();
      closeTab(active.id);
    } else if (key === "tab" && active && tabs.length > 1) {
      e.preventDefault();
      const i = tabs.findIndex((n) => n.id === active.id);
      const next =
        tabs[(i + (e.shiftKey ? -1 : 1) + tabs.length) % tabs.length];
      activate(next.id);
    }
  };

  const keys = useRef(onKeyDown);
  keys.current = onKeyDown;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => keys.current(e);
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const onTabKey = (e: React.KeyboardEvent, note: ScratchNote) => {
    const i = tabs.findIndex((n) => n.id === note.id);
    if (
      e.ctrlKey &&
      e.shiftKey &&
      (e.key === "ArrowLeft" || e.key === "ArrowRight")
    ) {
      e.preventDefault();
      moveTab(note.id, e.key === "ArrowLeft" ? -1 : 1);
      return;
    }
    const to =
      e.key === "ArrowLeft"
        ? i - 1
        : e.key === "ArrowRight"
          ? i + 1
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? tabs.length - 1
              : null;
    if (to !== null && tabs[to]) {
      e.preventDefault();
      activate(tabs[to].id);
      requestAnimationFrame(() =>
        document
          .querySelector<HTMLElement>(`[data-tab="${tabs[to].id}"]`)
          ?.focus(),
      );
    } else if (e.key === "F2") {
      e.preventDefault();
      setRenaming(note.id);
    } else if (e.key === "Delete") {
      e.preventDefault();
      closeTab(note.id);
    }
  };

  const title = (note: ScratchNote) => note.title || t("scratchpad.untitled");

  const openSearch = () => {
    setPanel(true);
    requestAnimationFrame(() => searchRef.current?.focus());
  };

  /** Applies a formatting change to the editor and keeps the selection. */
  const applyEdit = (
    edit: (text: string, from: number, to: number) => Edit,
  ) => {
    const editor = editorRef.current;
    if (!editor || !active) return;
    const next = edit(draft.text, editor.selectionStart, editor.selectionEnd);
    draft.setText(next.text);
    requestAnimationFrame(() => {
      editor.focus();
      editor.setSelectionRange(next.from, next.to);
    });
  };
  const wrap = (mark: string) =>
    applyEdit((text, from, to) => wrapSelection(text, from, to, mark));
  const prefix = (mark: string) =>
    applyEdit((text, from, to) => prefixLines(text, from, to, mark));

  const noteMenu = (note: ScratchNote) => [
    {
      id: "pin",
      label: note.pinned
        ? t("scratchpad.actions.unpin")
        : t("scratchpad.actions.pin"),
      icon: note.pinned ? PinOff : Pin,
      onSelect: () => togglePin(note),
    },
    {
      id: "rename",
      label: t("scratchpad.actions.rename"),
      icon: PencilLine,
      shortcut: "F2",
      onSelect: () => {
        if (note.id !== active?.id) activate(note.id);
        setRenaming(note.id);
      },
    },
    {
      id: "versions",
      label: t("scratchpad.actions.versions", { count: note.versions }),
      icon: History,
      disabled: note.versions < 2,
      onSelect: async () => {
        await draft.flush();
        setVersionsOf(
          useScratchStore.getState().notes?.find((n) => n.id === note.id) ??
            note,
        );
      },
    },
    "separator" as const,
    {
      id: "delete",
      label: t("scratchpad.actions.delete"),
      icon: Trash2,
      danger: true,
      onSelect: () => remove(note),
    },
  ];

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-surface-2 text-text">
      <header
        className="flex h-11 flex-none select-none items-center gap-1 pe-1.5 ps-3"
        data-tauri-drag-region
      >
        <img
          src={legatoIcon}
          alt=""
          width={20}
          height={20}
          className="pointer-events-none me-2 h-5 w-5 flex-none rounded-[5px]"
        />
        <div
          role="tablist"
          aria-label={t("scratchpad.window.tabs")}
          className="flex h-full min-w-0 flex-[0_1_auto] items-center gap-1"
          data-tauri-drag-region
        >
          {tabs.map((note, i) => {
            const on = note.id === active?.id;
            return (
              <div
                key={note.id}
                data-tab-wrap
                style={tabShift(i, note.id)}
                onAuxClick={(e) => e.button === 1 && closeTab(note.id)}
                className={`group relative flex h-8 min-w-[64px] max-w-[160px] flex-[0_1_160px] items-center pe-1 transition-[color,transform] duration-[var(--dur-2)] hover:duration-0 motion-reduce:transition-none ${
                  drag?.id === note.id ? "cursor-grabbing" : ""
                } ${on ? "text-text" : "text-text-2 hover:text-text"}`}
              >
                {renaming === note.id ? (
                  <input
                    autoFocus
                    defaultValue={note.title}
                    placeholder={t("scratchpad.untitled")}
                    aria-label={t("scratchpad.window.renameLabel")}
                    onFocus={(e) => e.currentTarget.select()}
                    onBlur={(e) => rename(note, e.currentTarget.value)}
                    onKeyDown={(e) => {
                      e.stopPropagation();
                      if (e.key === "Enter")
                        rename(note, e.currentTarget.value);
                      if (e.key === "Escape") setRenaming(null);
                    }}
                    className="mx-1 h-6 min-w-0 flex-1 rounded-[var(--r-sm)] border border-teal-600 bg-surface px-2 text-[13px] font-medium outline-none"
                  />
                ) : (
                  <button
                    type="button"
                    role="tab"
                    data-tab={note.id}
                    aria-selected={on}
                    tabIndex={on ? 0 : -1}
                    onClick={() => {
                      // The click that ends a drag doesn't switch tabs.
                      if (dragged.current) {
                        dragged.current = false;
                        return;
                      }
                      activate(note.id);
                    }}
                    onPointerDown={(e) => onTabPointerDown(e, note)}
                    onPointerMove={onTabPointerMove}
                    onPointerUp={onTabPointerUp}
                    onPointerCancel={onTabPointerUp}
                    onDoubleClick={() => setRenaming(note.id)}
                    onKeyDown={(e) => onTabKey(e, note)}
                    title={title(note)}
                    className={`h-full min-w-0 flex-1 truncate px-2 text-start text-[13px] focus-visible:outline-offset-[-3px] ${on ? "font-medium" : ""}`}
                  >
                    {note.pinned && (
                      <Pin
                        className="me-1 inline h-3 w-3 align-[-1px] text-teal-700"
                        aria-label={t("scratchpad.pinned")}
                      />
                    )}
                    {title(note)}
                  </button>
                )}
                <button
                  type="button"
                  tabIndex={-1}
                  onClick={() => closeTab(note.id)}
                  aria-label={t("scratchpad.window.closeTab", {
                    title: title(note),
                  })}
                  title={t("scratchpad.window.closeTabShort")}
                  className={`grid h-5 w-5 flex-none place-items-center rounded text-text-3 transition-opacity duration-[var(--dur-1)] hover:bg-surface-4 hover:text-text active:scale-90 ${
                    on
                      ? "opacity-100"
                      : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
                  }`}
                >
                  <X className="h-[13px] w-[13px]" aria-hidden="true" />
                </button>
                {/* The open tab is underlined, as in a browser's reading view. */}
                <span
                  aria-hidden="true"
                  className={`absolute inset-x-1.5 bottom-0 h-[2px] rounded-full bg-text transition-opacity duration-[var(--dur-2)] ${on ? "opacity-100" : "opacity-0"}`}
                />
              </div>
            );
          })}
        </div>
        <button
          type="button"
          onClick={newTab}
          aria-label={t("scratchpad.window.newTab")}
          title={t("scratchpad.window.newTabTip")}
          className={tbButton}
        >
          <Plus className="h-4 w-4" aria-hidden="true" />
        </button>
        <span className="h-full flex-1" data-tauri-drag-region />
        <button
          type="button"
          onClick={toggleKeepOnTop}
          aria-label={t("scratchpad.keepOnTop.short")}
          aria-pressed={keepOnTop}
          title={t("scratchpad.keepOnTop.title")}
          className={`${tbButton} ${keepOnTop ? "text-teal-700 hover:text-teal-700" : ""}`}
        >
          <PictureInPicture2 className="h-4 w-4" aria-hidden="true" />
        </button>
        <button
          type="button"
          onClick={() => getCurrentWindow().toggleMaximize()}
          aria-label={
            maximized
              ? t("scratchpad.window.restore")
              : t("scratchpad.window.expand")
          }
          title={
            maximized
              ? t("scratchpad.window.restore")
              : t("scratchpad.window.expand")
          }
          className={tbButton}
        >
          {maximized ? (
            <Minimize2 className="h-4 w-4" aria-hidden="true" />
          ) : (
            <Maximize2 className="h-4 w-4" aria-hidden="true" />
          )}
        </button>
        <button
          type="button"
          onClick={() => commands.hideScratchpad()}
          aria-label={t("scratchpad.window.close")}
          title={t("scratchpad.window.closeTip")}
          className={`${tbButton} hover:bg-danger hover:text-white`}
        >
          <X className="h-4 w-4" aria-hidden="true" />
        </button>
      </header>

      <div className="flex min-h-0 flex-1 gap-1.5 pb-2 pe-2 ps-1.5">
        <aside
          aria-label={t("scratchpad.window.rail")}
          className={`flex flex-none flex-col gap-0.5 overflow-hidden transition-[width] duration-[var(--dur-3)] ease-[var(--ease-out)] motion-reduce:transition-none ${panel ? "w-[208px]" : "w-11"}`}
        >
          <button
            type="button"
            onClick={() => setPanel(!panel)}
            aria-expanded={panel}
            aria-label={
              panel
                ? t("scratchpad.window.collapseNotes")
                : t("scratchpad.window.showNotes")
            }
            title={
              panel
                ? t("scratchpad.window.collapseNotes")
                : t("scratchpad.window.showNotes")
            }
            className={railItem}
          >
            <PanelLeft
              className="h-[17px] w-[17px] flex-none"
              aria-hidden="true"
            />
            {panel && (
              <span className="truncate">
                {t("scratchpad.window.collapseNotes")}
              </span>
            )}
          </button>
          <button
            type="button"
            onClick={newTab}
            aria-label={t("scratchpad.newNote")}
            title={t("scratchpad.window.newTabTip")}
            className={railItem}
          >
            <SquarePen
              className="h-[17px] w-[17px] flex-none"
              aria-hidden="true"
            />
            {panel && (
              <span className="truncate">{t("scratchpad.newNote")}</span>
            )}
          </button>
          {panel ? (
            <div className="relative my-1 flex-none px-1">
              <Search
                className="pointer-events-none absolute start-[13px] top-1/2 h-[15px] w-[15px] -translate-y-1/2 text-text-3"
                aria-hidden="true"
              />
              <input
                ref={searchRef}
                type="search"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") {
                    e.stopPropagation();
                    if (query) setQuery("");
                    else editorRef.current?.focus();
                  } else if (e.key === "ArrowDown") {
                    e.preventDefault();
                    document
                      .querySelector<HTMLElement>("[data-note-row]")
                      ?.focus();
                  }
                }}
                placeholder={t("scratchpad.window.searchNotes")}
                aria-label={t("scratchpad.window.searchNotes")}
                className="input compact w-full ps-[30px] [&::-webkit-search-cancel-button]:hidden"
              />
            </div>
          ) : (
            <button
              type="button"
              onClick={openSearch}
              aria-label={t("scratchpad.window.searchNotes")}
              title={t("scratchpad.window.searchTip")}
              className={railItem}
            >
              <Search
                className="h-[17px] w-[17px] flex-none"
                aria-hidden="true"
              />
            </button>
          )}

          {panel && (
            <ul
              aria-label={t("scratchpad.listLabel")}
              className="-mx-0.5 mt-1 min-h-0 flex-1 overflow-y-auto border-t border-line px-0.5 pt-1.5"
              onKeyDown={(e) => {
                if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
                e.preventDefault();
                const rows = [
                  ...document.querySelectorAll<HTMLElement>("[data-note-row]"),
                ];
                const i = rows.indexOf(document.activeElement as HTMLElement);
                const next = e.key === "ArrowDown" ? i + 1 : i - 1;
                if (next < 0) searchRef.current?.focus();
                else rows[Math.min(next, rows.length - 1)]?.focus();
              }}
            >
              {listed.length === 0 ? (
                <li className="px-2.5 py-3 text-[13px] text-text-3">
                  {query
                    ? t("scratchpad.window.noMatches", { query })
                    : t("scratchpad.empty.title")}
                </li>
              ) : (
                listed.map((note) => {
                  const on = note.id === active?.id;
                  return (
                    <li key={note.id} className="group relative">
                      <button
                        type="button"
                        data-note-row
                        onClick={() => activate(note.id)}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          setRowMenu(note.id);
                        }}
                        onKeyDown={(e) => {
                          if (e.key === "Delete") {
                            e.preventDefault();
                            remove(note);
                          } else if (e.key === "F10" && e.shiftKey) {
                            e.preventDefault();
                            setRowMenu(note.id);
                          }
                        }}
                        className={`ring-inside flex w-full flex-col rounded-[8px] py-2 pe-8 ps-2.5 text-start transition-colors duration-[var(--dur-1)] hover:duration-0 ${on ? "bg-surface-3" : "hover:bg-surface-3"}`}
                      >
                        <span className="flex min-w-0 items-center gap-1 text-[13px] font-medium">
                          {note.pinned && (
                            <Pin
                              className="h-3 w-3 flex-none text-teal-700"
                              aria-label={t("scratchpad.pinned")}
                            />
                          )}
                          <span className="truncate">{title(note)}</span>
                        </span>
                        <span className="text-[12px] text-text-3">
                          {relativeTime(note.updated_at, now, i18n.language)}
                        </span>
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          setRowMenu(rowMenu === note.id ? null : note.id)
                        }
                        aria-label={t("scratchpad.window.more")}
                        aria-haspopup="menu"
                        aria-expanded={rowMenu === note.id}
                        title={t("scratchpad.window.more")}
                        className="absolute end-1 top-2 grid h-6 w-6 place-items-center rounded-[6px] text-text-3 opacity-0 transition-opacity duration-[var(--dur-1)] hover:bg-surface-4 hover:text-text focus-visible:opacity-100 group-focus-within:opacity-100 group-hover:opacity-100"
                      >
                        <MoreHorizontal
                          className="h-4 w-4"
                          aria-hidden="true"
                        />
                      </button>
                      {rowMenu === note.id && (
                        <ActionMenu
                          label={t("scratchpad.window.more")}
                          onClose={() => setRowMenu(null)}
                          className="end-0 top-9 w-48"
                          items={noteMenu(note)}
                        />
                      )}
                    </li>
                  );
                })
              )}
            </ul>
          )}
          {!panel && <span className="flex-1" />}

          <div className="relative">
            <button
              type="button"
              onClick={() =>
                transforming
                  ? cancelTransform()
                  : tfMenu
                    ? setTfMenu(false)
                    : openTransforms()
              }
              disabled={!transforming && !draft.text.trim()}
              aria-haspopup={transforming ? undefined : "menu"}
              aria-expanded={transforming ? undefined : tfMenu}
              aria-label={
                transforming
                  ? t("scratchpad.transform.running", { name: transforming })
                  : t("scratchpad.transform.label")
              }
              title={
                transforming
                  ? t("scratchpad.transform.cancelTip")
                  : t("scratchpad.transform.tip")
              }
              className={`${railItem} disabled:cursor-not-allowed disabled:opacity-45`}
            >
              <Replace
                className={`h-[17px] w-[17px] flex-none ${transforming ? "animate-pulse text-teal-700" : ""}`}
                aria-hidden="true"
              />
              {panel && (
                <span className="truncate">
                  {transforming
                    ? t("scratchpad.transform.running", { name: transforming })
                    : t("scratchpad.window.transforms")}
                </span>
              )}
            </button>
            {tfMenu && !transforming && (
              <ActionMenu
                label={t("scratchpad.transform.label")}
                onClose={() => setTfMenu(false)}
                className="bottom-full start-0 mb-1 w-60"
                items={
                  transforms === null
                    ? []
                    : transforms.length === 0
                      ? [
                          {
                            id: "none",
                            label: t("scratchpad.transform.none"),
                            disabled: true,
                            onSelect: () => {},
                          },
                        ]
                      : transforms.map((tf) => ({
                          id: tf.id,
                          label: tf.name,
                          onSelect: () => transform(tf),
                        }))
                }
              />
            )}
          </div>
          <button
            type="button"
            onClick={() => {
              setFormatting(!formatting);
              if (formatting) setMode("write");
            }}
            aria-pressed={formatting}
            aria-label={t("scratchpad.window.formatting")}
            title={t("scratchpad.window.formattingTip")}
            className={`${railItem} ${formatting ? "bg-surface-3 text-text" : ""}`}
          >
            <Type className="h-[17px] w-[17px] flex-none" aria-hidden="true" />
            {panel && (
              <span className="truncate">
                {t("scratchpad.window.formatting")}
              </span>
            )}
          </button>
        </aside>

        <main className="@container relative flex min-w-0 flex-1 flex-col overflow-hidden rounded-[12px] border border-line bg-surface">
          {active ? (
            <>
              <FailedPasteBanner note={active} text={draft.text} />
              {formatting && (
                <div
                  role="toolbar"
                  aria-label={t("scratchpad.window.formatting")}
                  className="flex h-10 flex-none items-center gap-0.5 border-b border-line px-2"
                >
                  {(
                    [
                      ["bold", Bold, () => wrap("**"), "Ctrl+B"],
                      ["italic", Italic, () => wrap("_"), "Ctrl+I"],
                      ["heading", Heading2, () => prefix("## "), ""],
                      ["bullets", List, () => prefix("- "), ""],
                      ["numbered", ListOrdered, () => prefix("1. "), ""],
                      ["tasks", ListChecks, () => prefix("- [ ] "), ""],
                      ["code", Code, () => wrap("`"), ""],
                    ] as const
                  ).map(([id, Icon, run, keys]) => (
                    <button
                      key={id}
                      type="button"
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={run}
                      disabled={mode !== "write"}
                      aria-label={t(`scratchpad.format.${id}`)}
                      title={
                        keys
                          ? `${t(`scratchpad.format.${id}`)} (${keys})`
                          : t(`scratchpad.format.${id}`)
                      }
                      className={`${fmtButton} disabled:opacity-40`}
                    >
                      <Icon className="h-4 w-4" aria-hidden="true" />
                    </button>
                  ))}
                  <span className="mx-1.5 h-4 w-px bg-line" />
                  <button
                    type="button"
                    onClick={() =>
                      setMode(mode === "write" ? "preview" : "write")
                    }
                    aria-pressed={mode === "preview"}
                    className={`inline-flex h-7 items-center gap-1.5 rounded-[6px] px-2 text-[13px] transition-colors duration-[var(--dur-1)] hover:duration-0 ${mode === "preview" ? "bg-surface-3 text-text" : "text-text-2 hover:bg-surface-3 hover:text-text"}`}
                  >
                    <Eye className="h-4 w-4" aria-hidden="true" />
                    {t("scratchpad.editor.preview")}
                  </button>
                </div>
              )}
              <div className="relative min-h-0 flex-1">
                {mode === "write" ? (
                  <>
                    <textarea
                      ref={editorRef}
                      autoFocus
                      value={draft.text}
                      onChange={(e) => draft.setText(e.target.value)}
                      aria-label={t("scratchpad.editor.label")}
                      spellCheck
                      className="note-editor absolute inset-0 h-full w-full resize-none border-0 bg-transparent px-5 pb-16 pt-4 text-[15px] leading-[1.6] text-text caret-teal-700"
                    />
                    {!draft.text && (
                      <div
                        aria-hidden="true"
                        className="pointer-events-none absolute start-5 top-4 flex items-center gap-1.5 text-[15px] text-text-4"
                      >
                        <kbd className="kbd">{t("scratchpad.keys.ctrl")}</kbd>
                        <kbd className="kbd">{t("scratchpad.keys.space")}</kbd>
                        <span>{t("scratchpad.window.toDictate")}</span>
                      </div>
                    )}
                  </>
                ) : (
                  <NotePreview
                    text={draft.text}
                    onChange={draft.setText}
                    className="absolute inset-0 px-5 pb-16 pt-4"
                  />
                )}
              </div>
              <div className="pointer-events-none absolute inset-x-3 bottom-3 flex items-end gap-2">
                {/* Too narrow for the status next to the buttons: it waits for room. */}
                <div className="pointer-events-auto hidden min-w-0 flex-1 truncate ps-2 text-[12px] text-text-3 @[440px]:block">
                  <NoteStatus
                    note={active}
                    text={draft.text}
                    state={draft.state}
                    onRetry={draft.flush}
                  />
                </div>
                <span className="flex-1 @[440px]:hidden" />
                <button
                  type="button"
                  onClick={dictate}
                  aria-pressed={dictating}
                  aria-label={
                    dictating
                      ? t("scratchpad.window.listening")
                      : t("scratchpad.window.dictate")
                  }
                  title={
                    dictating
                      ? t("scratchpad.window.listening")
                      : t("scratchpad.window.dictateTip")
                  }
                  className={`pointer-events-auto grid h-9 w-9 flex-none place-items-center rounded-full border transition-colors duration-[var(--dur-1)] hover:duration-0 active:scale-[0.95] ${
                    dictating
                      ? "border-teal-500 bg-teal-50 text-teal-700"
                      : "border-line-strong bg-surface text-text-2 hover:bg-surface-2 hover:text-text"
                  }`}
                >
                  {dictating ? (
                    <Square
                      className="h-3.5 w-3.5 fill-current"
                      aria-hidden="true"
                    />
                  ) : (
                    <Mic className="h-4 w-4" aria-hidden="true" />
                  )}
                </button>
                <button
                  type="button"
                  onClick={copyAll}
                  disabled={!draft.text.trim()}
                  className="pointer-events-auto inline-flex h-9 flex-none items-center gap-2 rounded-full bg-text px-4 text-[13px] font-medium text-surface transition-[opacity,transform] duration-[var(--dur-1)] hover:opacity-90 active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-40"
                >
                  <Copy className="h-4 w-4" aria-hidden="true" />
                  {t("scratchpad.copy")}
                </button>
                {/* It gives way first: with the notes list open, the app's
                    name shortens rather than pushing the menu button out. */}
                <div className="pointer-events-auto relative flex min-w-0">
                  <button
                    type="button"
                    onClick={send}
                    disabled={
                      !(target || agentTarget) || !draft.text.trim() || sending
                    }
                    title={
                      agentTarget
                        ? t("scratchpad.window.sendTip", {
                            app: agentLabel(agentTarget),
                          })
                        : target
                          ? t("scratchpad.window.sendTip", { app: target.name })
                          : t("scratchpad.window.noTarget")
                    }
                    className="inline-flex h-9 min-w-0 max-w-[200px] items-center gap-[7px] rounded-s-full bg-teal-fill pe-3 ps-3.5 text-[13px] font-medium text-white transition-colors duration-[var(--dur-1)] hover:bg-teal-fill-hover hover:duration-0 active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:bg-teal-fill"
                  >
                    {agentTarget ? (
                      <Bot className="h-4 w-4" aria-hidden="true" />
                    ) : target ? (
                      <AppIcon path={target.path} className="h-4 w-4" />
                    ) : (
                      <Send className="h-4 w-4" aria-hidden="true" />
                    )}
                    {/* "Send to" stays whole; only the app's name shortens. */}
                    {agentTarget || target ? (
                      <span className="flex min-w-0 gap-[0.3em]">
                        <span className="flex-none">
                          {t("scratchpad.window.sendTo")}
                        </span>
                        <span className="truncate">
                          {agentTarget ? agentLabel(agentTarget) : target?.name}
                        </span>
                      </span>
                    ) : (
                      <span className="truncate">
                        {t("scratchpad.window.send")}
                      </span>
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={() => (targets ? setTargets(null) : openTargets())}
                    aria-label={t("scratchpad.window.pickTarget")}
                    aria-haspopup="menu"
                    aria-expanded={!!targets}
                    title={t("scratchpad.window.pickTarget")}
                    className="grid h-9 w-8 place-items-center rounded-e-full border-s border-white/25 bg-teal-fill text-white transition-colors duration-[var(--dur-1)] hover:bg-teal-fill-hover hover:duration-0 active:scale-[0.97]"
                  >
                    <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
                  </button>
                  {targets && (
                    <ActionMenu
                      label={t("scratchpad.window.pickTarget")}
                      onClose={() => setTargets(null)}
                      className="bottom-full end-0 mb-1 w-72"
                      items={([] as ActionMenuItem[]).concat(
                        targets.length === 0
                          ? [
                              {
                                id: "none",
                                label: t("scratchpad.window.noWindows"),
                                disabled: true,
                                onSelect: () => {},
                              },
                            ]
                          : targets.map((x) => ({
                              id: String(x.handle),
                              label: x.title
                                .toLowerCase()
                                .includes(x.name.toLowerCase())
                                ? x.title
                                : t("scratchpad.window.targetLabel", {
                                    app: x.name,
                                    title: x.title,
                                  }),
                              checked: x.current && !agentTarget,
                              onSelect: () => pickTarget(x),
                            })),
                        agents.length === 0
                          ? []
                          : [
                              "separator" as const,
                              ...agents.map((a) => ({
                                id: `agent-${a.host}-${a.pane}`,
                                label: agentLabel(a),
                                checked:
                                  agentTarget?.host === a.host &&
                                  agentTarget?.pane === a.pane,
                                onSelect: () => setAgentTarget(a),
                              })),
                            ],
                      )}
                    />
                  )}
                </div>
              </div>
            </>
          ) : notes ? (
            <div className="grid flex-1 place-items-center px-8 text-center">
              <div className="grid justify-items-center gap-1.5">
                <h2 className="text-[15px] font-medium">
                  {t("scratchpad.window.emptyTitle")}
                </h2>
                <p className="max-w-[320px] text-[13px] text-text-3">
                  {t("scratchpad.window.emptyBody")}
                </p>
                <button
                  type="button"
                  onClick={newTab}
                  className="mt-3 inline-flex h-8 items-center gap-2 rounded-[var(--r-md)] bg-teal-fill px-3 text-[13px] font-medium text-white hover:bg-teal-fill-hover active:scale-[0.97]"
                >
                  <Plus className="h-4 w-4" aria-hidden="true" />
                  {t("scratchpad.newNote")}
                </button>
              </div>
            </div>
          ) : null}
        </main>
      </div>

      {versionsOf && (
        <VersionHistory note={versionsOf} onClose={() => setVersionsOf(null)} />
      )}
      {/* Above the floating Copy and Send bar (12 px from the bottom, 36 px
          tall), which the toasts covered. */}
      <AppToaster bottom={60} />
    </div>
  );
};
