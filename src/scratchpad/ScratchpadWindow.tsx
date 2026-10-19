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
  ChevronDown,
  Copy,
  EllipsisVertical,
  History,
  Mic,
  PencilLine,
  PictureInPicture2,
  Pin,
  PinOff,
  Plus,
  Replace,
  Send,
  Square,
  Trash2,
  X,
} from "lucide-react";
import {
  commands,
  events,
  type ScratchNote,
  type SendTarget,
  type Transform,
} from "@/bindings";
import { AppToaster } from "@/components/shell/AppToaster";
import { ActionMenu } from "@/components/ui/ActionMenu";
import { Segmented } from "@/components/ui/Segmented";
import { AppIcon } from "@/components/pages/home/AppIcon";
import { copyToClipboard } from "@/components/pages/home/clipboard";
import { FailedPasteBanner } from "@/components/scratchpad/FailedPasteBanner";
import { NotePreview } from "@/components/scratchpad/NotePreview";
import { NoteStatus } from "@/components/scratchpad/NoteStatus";
import { VersionHistory } from "@/components/scratchpad/VersionHistory";
import { insertAt } from "@/components/scratchpad/notes";
import { useNoteDraft } from "@/components/scratchpad/useNoteDraft";
import { useScratchStore, watchScratch } from "@/stores/scratchStore";
import { useAiStore, watchAi } from "@/stores/aiStore";
import { providerFor } from "@/components/pages/transforms/model";
import { useProblemText } from "@/components/pages/transforms/TryIt";

type Mode = "write" | "preview";

const tbButton =
  "grid h-[26px] w-[30px] flex-none place-items-center self-center rounded-[5px] text-text-2 transition-[background-color,color,transform] duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 active:scale-[0.94] active:bg-surface-4 active:duration-0";

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
  const [transforms, setTransforms] = useState<Transform[] | null>(null);
  const [tfMenu, setTfMenu] = useState(false);
  const [transforming, setTransforming] = useState<string | null>(null);
  const run = useRef(0);
  const ai = useAiStore((s) => s.status);
  const problemText = useProblemText();
  const editorRef = useRef<HTMLTextAreaElement>(null);

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
    setTargets(await commands.getSendTargets().catch(() => []));
  };

  const pickTarget = async (x: SendTarget) => {
    const result = await commands.setSendTarget(x.handle);
    if (result.status === "error")
      toast.error(t("scratchpad.window.targetGone", { app: x.name }));
    refresh(active?.id);
  };

  const send = async () => {
    if (!active || !target || !draft.text.trim() || sending) return;
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
    if (key === " ") {
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

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-surface text-text">
      <header
        className="flex h-9 flex-none select-none items-end gap-0.5 border-b border-line bg-surface-2 pe-1 ps-1.5"
        data-tauri-drag-region
      >
        <div
          role="tablist"
          aria-label={t("scratchpad.window.tabs")}
          className="flex h-full min-w-0 flex-[0_1_auto] items-end gap-0.5"
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
                className={`group relative -mb-px flex h-[30px] min-w-[70px] max-w-[176px] flex-[0_1_176px] items-center rounded-t-[7px] border border-b-0 pe-[5px] transition-[background-color,color,transform] duration-[var(--dur-2)] hover:duration-0 motion-reduce:transition-none ${
                  drag?.id === note.id
                    ? "cursor-grabbing shadow-[var(--shadow-1)]"
                    : ""
                } ${
                  on
                    ? "border-line bg-surface text-text"
                    : "border-transparent text-text-2 hover:bg-surface-3 hover:text-text"
                }`}
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
                    className={`h-full min-w-0 flex-1 truncate rounded-t-[7px] pe-1.5 ps-[11px] text-start text-[13px] focus-visible:outline-offset-[-3px] ${on ? "font-medium" : ""}`}
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
          <Plus className="h-[15px] w-[15px]" aria-hidden="true" />
        </button>
        <span className="h-full flex-1" data-tauri-drag-region />
        <button
          type="button"
          onClick={toggleKeepOnTop}
          aria-label={t("scratchpad.keepOnTop.short")}
          aria-pressed={keepOnTop}
          title={t("scratchpad.keepOnTop.title")}
          className={`${tbButton} ${keepOnTop ? "bg-teal-50 text-teal-700 hover:bg-teal-50 hover:text-teal-700" : ""}`}
        >
          <PictureInPicture2 className="h-[15px] w-[15px]" aria-hidden="true" />
        </button>
        <div className="relative self-center">
          <button
            type="button"
            onClick={() => setMenu(!menu)}
            disabled={!active}
            aria-label={t("scratchpad.window.more")}
            aria-haspopup="menu"
            aria-expanded={menu}
            title={t("scratchpad.window.more")}
            className={tbButton}
          >
            <EllipsisVertical
              className="h-[15px] w-[15px]"
              aria-hidden="true"
            />
          </button>
          {menu && active && (
            <ActionMenu
              label={t("scratchpad.window.more")}
              onClose={() => setMenu(false)}
              className="end-0 top-full mt-1 w-56"
              items={[
                {
                  id: "pin",
                  label: active.pinned
                    ? t("scratchpad.actions.unpin")
                    : t("scratchpad.actions.pin"),
                  icon: active.pinned ? PinOff : Pin,
                  onSelect: () => togglePin(active),
                },
                {
                  id: "rename",
                  label: t("scratchpad.actions.rename"),
                  icon: PencilLine,
                  shortcut: "F2",
                  onSelect: () => setRenaming(active.id),
                },
                {
                  id: "versions",
                  label: t("scratchpad.actions.versions", {
                    count: active.versions,
                  }),
                  icon: History,
                  disabled: active.versions < 2,
                  onSelect: async () => {
                    await draft.flush();
                    setVersionsOf(
                      useScratchStore
                        .getState()
                        .notes?.find((n) => n.id === active.id) ?? active,
                    );
                  },
                },
                "separator",
                {
                  id: "delete",
                  label: t("scratchpad.actions.delete"),
                  icon: Trash2,
                  danger: true,
                  onSelect: () => remove(active),
                },
              ]}
            />
          )}
        </div>
        <button
          type="button"
          onClick={() => commands.hideScratchpad()}
          aria-label={t("scratchpad.window.close")}
          title={t("scratchpad.window.closeTip")}
          className={`${tbButton} hover:bg-danger hover:text-white`}
        >
          <X className="h-[15px] w-[15px]" aria-hidden="true" />
        </button>
      </header>

      {active ? (
        <>
          <FailedPasteBanner note={active} text={draft.text} />
          <div className="flex h-[42px] flex-none items-center justify-between gap-2.5 pe-3.5 ps-3">
            <Segmented
              label={t("scratchpad.editor.mode")}
              value={mode}
              onChange={setMode}
              options={[
                { value: "write", label: t("scratchpad.editor.write") },
                { value: "preview", label: t("scratchpad.editor.preview") },
              ]}
            />
            <NoteStatus
              note={active}
              text={draft.text}
              state={draft.state}
              onRetry={draft.flush}
            />
          </div>
          <div className="relative min-h-0 flex-1">
            {mode === "write" ? (
              <textarea
                ref={editorRef}
                autoFocus
                value={draft.text}
                onChange={(e) => draft.setText(e.target.value)}
                aria-label={t("scratchpad.editor.label")}
                placeholder={t("scratchpad.editor.placeholder")}
                spellCheck
                className="absolute inset-0 h-full w-full resize-none border-0 bg-transparent px-5 pb-[18px] pt-0.5 text-[15px] leading-[1.6] text-text caret-teal-700 placeholder:text-text-4 note-editor"
              />
            ) : (
              <NotePreview
                text={draft.text}
                onChange={draft.setText}
                className="absolute inset-0 px-5 pb-[18px] pt-0.5"
              />
            )}
          </div>
          <footer className="flex h-[54px] flex-none items-center gap-1.5 border-t border-line px-2.5">
            <button
              type="button"
              onClick={dictate}
              aria-pressed={dictating}
              className={`inline-flex h-8 flex-none items-center gap-[7px] whitespace-nowrap rounded-[var(--r-md)] border pe-2 ps-2.5 text-[13px] font-medium transition-colors duration-[var(--dur-1)] hover:duration-0 active:scale-[0.97] ${
                dictating
                  ? "border-teal-500 bg-teal-50 text-teal-700"
                  : "border-line-strong bg-surface text-text shadow-[var(--shadow-1)] hover:bg-surface-2"
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
              {dictating
                ? t("scratchpad.window.listening")
                : t("scratchpad.window.dictate")}
              {!dictating && (
                <span className="ms-2 hidden gap-[3px] min-[600px]:inline-flex">
                  <kbd className="kbd">{t("scratchpad.keys.ctrl")}</kbd>
                  <kbd className="kbd">{t("scratchpad.keys.space")}</kbd>
                </span>
              )}
            </button>
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
                title={
                  transforming
                    ? t("scratchpad.transform.cancelTip")
                    : t("scratchpad.transform.tip")
                }
                className="inline-flex h-8 items-center gap-1.5 rounded-[var(--r-md)] px-2.5 text-[13px] font-medium text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-45"
              >
                <Replace
                  className={`h-4 w-4 ${transforming ? "animate-pulse text-teal-700" : ""}`}
                  aria-hidden="true"
                />
                <span className="max-w-[140px] truncate">
                  {transforming
                    ? t("scratchpad.transform.running", { name: transforming })
                    : t("scratchpad.transform.label")}
                </span>
                {!transforming && (
                  <ChevronDown
                    className="h-3.5 w-3.5 text-text-3"
                    aria-hidden="true"
                  />
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
            <span className="flex-1" />
            <button
              type="button"
              onClick={copyAll}
              disabled={!draft.text.trim()}
              aria-label={t("scratchpad.copyAll")}
              title={t("scratchpad.copyAll")}
              className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 disabled:cursor-not-allowed disabled:opacity-45"
            >
              <Copy className="h-4 w-4" aria-hidden="true" />
            </button>
            <div className="relative flex">
              <button
                type="button"
                onClick={send}
                disabled={!target || !draft.text.trim() || sending}
                title={
                  target
                    ? t("scratchpad.window.sendTip", { app: target.name })
                    : t("scratchpad.window.noTarget")
                }
                className="inline-flex h-8 max-w-[220px] items-center gap-[7px] rounded-s-[var(--r-md)] border border-transparent bg-teal-fill pe-3 ps-[11px] text-[13px] font-medium text-white transition-colors duration-[var(--dur-1)] hover:bg-teal-fill-hover hover:duration-0 active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:bg-teal-fill"
              >
                {target ? (
                  <AppIcon path={target.path} className="h-4 w-4" />
                ) : (
                  <Send className="h-4 w-4" aria-hidden="true" />
                )}
                <span className="truncate">
                  {target
                    ? t("scratchpad.window.sendTo", { app: target.name })
                    : t("scratchpad.window.send")}
                </span>
              </button>
              <button
                type="button"
                onClick={() => (targets ? setTargets(null) : openTargets())}
                aria-label={t("scratchpad.window.pickTarget")}
                aria-haspopup="menu"
                aria-expanded={!!targets}
                title={t("scratchpad.window.pickTarget")}
                className="grid h-8 w-7 place-items-center rounded-e-[var(--r-md)] border-s border-white/25 bg-teal-fill text-white transition-colors duration-[var(--dur-1)] hover:bg-teal-fill-hover hover:duration-0 active:scale-[0.97]"
              >
                <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
              {targets && (
                <ActionMenu
                  label={t("scratchpad.window.pickTarget")}
                  onClose={() => setTargets(null)}
                  className="bottom-full end-0 mb-1 w-72"
                  items={
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
                          checked: x.current,
                          onSelect: () => pickTarget(x),
                        }))
                  }
                />
              )}
            </div>
          </footer>
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

      {versionsOf && (
        <VersionHistory note={versionsOf} onClose={() => setVersionsOf(null)} />
      )}
      <AppToaster />
    </div>
  );
};
