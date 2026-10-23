import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { BookPlus } from "lucide-react";
import { commands, type NoteLine, type WordKind } from "@/bindings";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Segmented } from "../ui/Segmented";
import { linesToFix } from "./notes";

const KINDS: (WordKind | "word")[] = [
  "word",
  "person",
  "project",
  "organisation",
  "term",
];

/** Longer selections are sentences, not a word to add. */
const MAX_CHARS = 48;
const MAX_WORDS = 4;
const EDGE = 8;

/** Lets a line's menu offer the words selected in it. */
export const ADD_WORD_EVENT = "legato:add-word";

interface Pick {
  text: string;
  rect: DOMRect;
}

/** The words selected in one transcript line, if they could be a word to
 *  add: a few words inside a single line of `root`. */
export const pickedWords = (root: HTMLElement | null): Pick | null => {
  const selection = window.getSelection();
  if (!root || !selection || selection.isCollapsed || !selection.rangeCount)
    return null;
  const text = selection.toString().trim().replace(/\s+/g, " ");
  if (!text || text.length > MAX_CHARS || text.split(" ").length > MAX_WORDS)
    return null;
  const lineOf = (node: Node | null) =>
    (node instanceof Element ? node : node?.parentElement)?.closest(
      "[data-line-text]",
    ) ?? null;
  const line = lineOf(selection.anchorNode);
  if (!line || line !== lineOf(selection.focusNode) || !root.contains(line))
    return null;
  return { text, rect: selection.getRangeAt(0).getBoundingClientRect() };
};

/** Select a word in the transcript, or a few, and a small button offers to
 *  add it to the Dictionary: spelled as it should be, with what it is, and
 *  written that way in every line of the note. */
export const AddWordFromSelection: React.FC<{
  noteId: number;
  lines: NoteLine[];
  root: React.RefObject<HTMLElement | null>;
}> = ({ noteId, lines, root }) => {
  const { t } = useTranslation();
  const [pick, setPick] = useState<Pick | null>(null);
  const [open, setOpen] = useState(false);
  const [word, setWord] = useState("");
  const [kind, setKind] = useState<WordKind | "word">("word");
  const [busy, setBusy] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  const panel = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);

  const start = (next: Pick) => {
    setPick(next);
    setWord(next.text);
    setKind("word");
    setPos(null);
    setOpen(true);
    requestAnimationFrame(() => input.current?.select());
  };
  const close = () => {
    setOpen(false);
    setPick(null);
  };

  // Follow the selection while the panel is closed.
  useEffect(() => {
    if (open) return;
    const read = (e: Event) => {
      if (e instanceof MouseEvent && e.button !== 0) return;
      setPick(pickedWords(root.current));
    };
    const onChange = () => {
      if (window.getSelection()?.isCollapsed) setPick(null);
    };
    document.addEventListener("mouseup", read);
    document.addEventListener("keyup", read);
    document.addEventListener("selectionchange", onChange);
    window.addEventListener("scroll", onChange, true);
    return () => {
      document.removeEventListener("mouseup", read);
      document.removeEventListener("keyup", read);
      document.removeEventListener("selectionchange", onChange);
      window.removeEventListener("scroll", onChange, true);
    };
  }, [open, root]);

  // A line's menu asks for the words selected in it.
  useEffect(() => {
    const onAsk = () => {
      const next = pickedWords(root.current);
      if (next) start(next);
    };
    window.addEventListener(ADD_WORD_EVENT, onAsk);
    return () => window.removeEventListener(ADD_WORD_EVENT, onAsk);
  }, [root]);

  // Esc or a click outside closes the panel.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      close();
    };
    const onDown = (e: PointerEvent) => {
      if (!panel.current?.contains(e.target as Node)) close();
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onDown, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onDown, true);
    };
  }, [open]);

  // Place the panel under the selection, or above it when it won't fit.
  useLayoutEffect(() => {
    if (!open || !pick || !panel.current) return;
    const box = panel.current.getBoundingClientRect();
    const below = pick.rect.bottom + 8;
    const top =
      below + box.height > window.innerHeight - EDGE
        ? Math.max(EDGE, pick.rect.top - 8 - box.height)
        : below;
    const left = Math.min(
      Math.max(EDGE, pick.rect.left),
      window.innerWidth - EDGE - box.width,
    );
    setPos({ top, left });
  }, [open, pick]);

  const save = async () => {
    if (!pick || busy || !word.trim()) return;
    setBusy(true);
    const result = await commands
      .addNoteWord(
        noteId,
        pick.text,
        word.trim(),
        kind === "word" ? null : kind,
      )
      .catch(() => null);
    setBusy(false);
    if (result?.status !== "ok") {
      toast.error(t("notes.addWord.failed"), {
        description: result?.status === "error" ? result.error : undefined,
      });
      return;
    }
    const { undo_token, lines: fixed } = result.data;
    toast(
      fixed
        ? t("notes.addWord.addedFixed", { word: word.trim(), count: fixed })
        : t("notes.addWord.added", { word: word.trim() }),
      {
        action: {
          label: t("notes.toast.undo"),
          onClick: () => commands.undoNoteWordFix(undo_token),
        },
      },
    );
    window.getSelection()?.removeAllRanges();
    close();
  };

  if (!pick) return null;
  const count = linesToFix(lines, pick.text, word);
  const same = word.trim().toLowerCase() === pick.text.toLowerCase();

  return createPortal(
    open ? (
      <div
        ref={panel}
        role="dialog"
        aria-label={t("notes.addWord.title", { word: pick.text })}
        style={{ top: pos?.top ?? -9999, left: pos?.left ?? -9999 }}
        className="fixed z-50 w-[min(380px,calc(100vw-16px))] rounded-[var(--r-lg)] border border-line-strong bg-surface p-3 shadow-[var(--shadow-3)]"
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            save();
          }
        }}
      >
        <p className="text-sm font-medium">{t("notes.addWord.heading")}</p>
        <p className="mt-0.5 text-[13px] text-text-2">
          {t("notes.addWord.heard", { heard: pick.text })}
        </p>
        <label className="mt-3 mb-1 block text-[13px] font-medium">
          {t("notes.addWord.shouldBe")}
          <Input
            ref={input}
            value={word}
            maxLength={MAX_CHARS * 2}
            onChange={(e) => setWord(e.target.value)}
            className="mt-1 w-full"
          />
        </label>
        <p className="mt-3 mb-1 text-[13px] font-medium">
          {t("dictionary.editor.kind")}
        </p>
        <Segmented
          label={t("dictionary.editor.kind")}
          value={kind}
          onChange={setKind}
          options={KINDS.map((k) => ({
            value: k,
            label: t(`dictionary.kind.${k}`),
          }))}
          className="w-full"
        />
        <p className="mt-3 text-[13px] text-text-3">
          {count
            ? t("notes.addWord.fixes", { count, heard: pick.text })
            : same
              ? t("notes.addWord.keeps")
              : t("notes.addWord.learns", { heard: pick.text })}
        </p>
        <div className="mt-3 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={close}>
            {t("common.cancel")}
          </Button>
          <Button size="sm" disabled={busy || !word.trim()} onClick={save}>
            {t("notes.addWord.add")}
          </Button>
        </div>
      </div>
    ) : (
      <button
        type="button"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => start(pick)}
        style={{
          top: Math.max(EDGE, pick.rect.top - 36),
          left: Math.min(
            Math.max(EDGE, pick.rect.left + pick.rect.width / 2 - 70),
            window.innerWidth - EDGE - 140,
          ),
        }}
        className="fixed z-50 inline-flex h-7 items-center gap-1.5 rounded-[var(--r-md)] border border-line-strong bg-surface px-2.5 text-[13px] font-medium text-text shadow-[var(--shadow-2)] hover:bg-surface-2"
      >
        <BookPlus className="h-3.5 w-3.5 text-teal-700" aria-hidden="true" />
        {t("notes.addWord.button")}
      </button>
    ),
    document.body,
  );
};
