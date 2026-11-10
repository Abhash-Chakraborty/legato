import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  ArrowLeftRight,
  BookPlus,
  EllipsisVertical,
  Pencil,
  Trash2,
  X,
} from "@/icons";
import { commands, type NoteLine, type WordFix } from "@/bindings";
import { useNotesStore } from "../../stores/notesStore";
import { ActionMenu, type ActionMenuItem } from "../ui/ActionMenu";
import type { NoteAudio } from "../pages/notes/NoteAudio";
import { ADD_WORD_EVENT, pickedWords } from "./AddWord";
import {
  lineWords,
  speakerLabel,
  wordAt,
  wordAtOffset,
  type LineWord,
  type Voice,
} from "./notes";
import { useSpeakerNames } from "./SpeakerTag";

type Playback = Pick<NoteAudio, "audio" | "playing" | "playFrom">;

/** A word plays from a moment before the model says it starts, so its
 *  first sound isn't cut. */
const LEAD_IN_MS = 150;

/** The playing line, word by word, with the word under the playhead
 *  marked. Follows the audio every frame while it plays and re-renders
 *  only when the word changes. */
const SpokenWords: React.FC<{
  text: string;
  words: LineWord[];
  player: Playback;
}> = ({ text, words, player }) => {
  const [active, setActive] = useState(-1);
  const { audio, playing } = player;

  useEffect(() => {
    const el = audio.current;
    if (!el) return;
    let frame = 0;
    const update = () => setActive(wordAt(words, el.currentTime * 1000));
    const follow = () => {
      update();
      frame = requestAnimationFrame(follow);
    };
    update();
    if (playing) frame = requestAnimationFrame(follow);
    el.addEventListener("seeked", update);
    return () => {
      cancelAnimationFrame(frame);
      el.removeEventListener("seeked", update);
    };
  }, [audio, playing, words]);

  const parts: React.ReactNode[] = [];
  let last = 0;
  words.forEach((w, i) => {
    if (w.from > last) parts.push(text.slice(last, w.from));
    parts.push(
      <span
        key={i}
        data-w={i}
        className={`rounded-[var(--r-xs)] transition-[background-color,box-shadow] duration-[var(--dur-1)] motion-reduce:transition-none ${
          i === active
            ? "bg-teal-100 shadow-[0_0_0_2px_var(--teal-100)]"
            : "shadow-[0_0_0_2px_transparent]"
        }`}
      >
        {w.text}
      </span>,
    );
    last = w.to;
  });
  if (last < text.length) parts.push(text.slice(last));
  return <>{parts}</>;
};

/** How many characters of `root`'s text come before the caret. */
const offsetIn = (root: HTMLElement, caret: Range) => {
  const before = document.createRange();
  before.setStart(root, 0);
  before.setEnd(caret.startContainer, caret.startOffset);
  return before.toString().length;
};

/** Whether a click at (x, y) landed on `word` of a line shown as plain
 *  text, not in the empty space after it. */
const onWord = (root: HTMLElement, word: LineWord, x: number, y: number) => {
  const node = root.firstChild;
  if (root.childNodes.length !== 1 || !(node instanceof Text)) return true;
  const range = document.createRange();
  range.setStart(node, Math.min(word.from, node.length));
  range.setEnd(node, Math.min(word.to, node.length));
  return [...range.getClientRects()].some(
    (r) =>
      x >= r.left - 2 &&
      x <= r.right + 2 &&
      y >= r.top - 2 &&
      y <= r.bottom + 2,
  );
};

/** A transcript line's words, which can be corrected in place (double-click,
 *  or Edit in its menu), given to someone else or deleted, during the
 *  recording and after it. A corrected word is offered for the note's other
 *  lines and the Dictionary. With the note's audio, clicking a word plays
 *  from it, and the playing line marks the word being said. */
export const LineText: React.FC<{
  line: NoteLine;
  /** The words of a line still being spoken: shown, not editable. */
  preview?: boolean;
  /** Who the line can be given to, once voices were told apart. */
  voices?: Voice[];
  /** Selected words can be added to the Dictionary (an
   *  AddWordFromSelection is mounted around the line). */
  canAddWord?: boolean;
  className?: string;
  /** The note's recording, when it has one. */
  player?: Playback;
  /** This line is the one playing. */
  current?: boolean;
}> = ({
  line,
  preview,
  voices = [],
  canAddWord = false,
  className = "",
  player,
  current = false,
}) => {
  const { t } = useTranslation();
  const names = useSpeakerNames();
  const speakers = useNotesStore(
    (s) => s.notes?.find((n) => n.id === line.note_id)?.speakers,
  );
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(line.text);
  const [menu, setMenu] = useState(false);
  // Words selected in this line when its menu opened.
  const [picked, setPicked] = useState<string | null>(null);
  const textRef = useRef<HTMLSpanElement>(null);
  const [fix, setFix] = useState<WordFix | null>(null);
  const [learned, setLearned] = useState(false);
  const [busy, setBusy] = useState(false);
  const box = useRef<HTMLTextAreaElement>(null);
  // Esc ends the edit; the blur that may follow mustn't save it.
  const open = useRef(false);
  const more = useRef<HTMLButtonElement>(null);
  const other = line.who === "you" ? "them" : "you";
  const words = useMemo(() => lineWords(line), [line]);

  // A click on a word plays the recording from it; a drag that selects
  // words, or the second click of a double-click, doesn't.
  const seek = (e: React.MouseEvent<HTMLSpanElement>) => {
    const root = textRef.current;
    if (!player || !root || e.detail !== 1 || !words.length) return;
    if (!(window.getSelection()?.isCollapsed ?? true)) return;
    const hit = (e.target as HTMLElement).closest("[data-w]");
    let i = hit ? Number(hit.getAttribute("data-w")) : -1;
    if (i < 0) {
      const caret = document.caretRangeFromPoint?.(e.clientX, e.clientY);
      if (!caret || !root.contains(caret.startContainer)) return;
      i = wordAtOffset(words, offsetIn(root, caret));
      if (words[i] && !onWord(root, words[i], e.clientX, e.clientY)) return;
    }
    const word = words[i];
    if (word) player.playFrom(Math.max(line.at_ms, word.start - LEAD_IN_MS));
  };

  // Fit the box to the words as they're typed.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [editing, draft]);

  const edit = () => {
    open.current = true;
    setDraft(line.text);
    setEditing(true);
    requestAnimationFrame(() => {
      const el = box.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
    });
  };

  const remove = async () => {
    const result = await commands.deleteNoteLine(line.id);
    if (result.status === "error") {
      toast.error(t("notes.line.deleteFailed"), { description: result.error });
      return;
    }
    toast(t("notes.line.deleted"), {
      action: {
        label: t("notes.toast.undo"),
        onClick: () => commands.restoreNoteLine(result.data),
      },
    });
  };

  const save = async () => {
    if (!open.current) return;
    open.current = false;
    const text = draft.trim();
    setEditing(false);
    if (text === line.text) return;
    if (!text) {
      remove();
      return;
    }
    const result = await commands.editNoteLine(
      line.id,
      line.who,
      line.speaker ?? null,
      text,
    );
    if (result.status === "error") {
      toast.error(t("notes.line.saveFailed"), { description: result.error });
      return;
    }
    setLearned(false);
    setFix(result.data.fix);
    const before = line.text;
    toast(t("notes.line.edited"), {
      action: {
        label: t("notes.toast.undo"),
        onClick: () => {
          setFix(null);
          commands
            .editNoteLine(line.id, line.who, line.speaker ?? null, before)
            .catch(() => {});
        },
      },
    });
  };

  const giveTo = async (to: Voice) => {
    const result = await commands.editNoteLine(
      line.id,
      to.who,
      to.speaker,
      line.text,
    );
    if (result.status === "error")
      toast.error(t("notes.line.saveFailed"), { description: result.error });
  };

  const applyFix = async (learn: boolean) => {
    if (!fix || busy) return;
    setBusy(true);
    const result = await commands.fixNoteWord(
      { ...fix, others: learn ? [] : fix.others },
      learn,
    );
    setBusy(false);
    if (result.status === "error") {
      toast.error(t("notes.fix.failed"), { description: result.error });
      return;
    }
    const { undo_token, lines } = result.data;
    toast(
      learn
        ? t("notes.fix.learned", { word: fix.word })
        : t("notes.fix.fixed", { count: lines }),
      {
        action: {
          label: t("notes.toast.undo"),
          onClick: () => commands.undoNoteWordFix(undo_token),
        },
      },
    );
    // Hide the offer once nothing in it is left to do.
    if (learn) setLearned(true);
    const others = learn ? fix.others : [];
    const canLearn = !learn && !learned;
    setFix(others.length || canLearn ? { ...fix, others } : null);
  };

  const items: ActionMenuItem[] = [
    ...(picked
      ? [
          {
            id: "add-word",
            label: t("notes.addWord.menu", { word: picked }),
            icon: BookPlus,
            onSelect: () => window.dispatchEvent(new Event(ADD_WORD_EVENT)),
          },
          "separator" as const,
        ]
      : []),
    { id: "edit", label: t("notes.line.edit"), icon: Pencil, onSelect: edit },
    ...(voices.length
      ? voices.map((v) => ({
          id: `who-${v.who}-${v.speaker ?? ""}`,
          label: t("notes.line.saidBy", {
            who: speakerLabel({ ...line, ...v }, speakers, names),
          }),
          checked: v.who === line.who && v.speaker === (line.speaker ?? null),
          onSelect: () => giveTo(v),
        }))
      : [
          {
            id: "who",
            label: t("notes.line.saidBy", { who: t(`notes.who.${other}`) }),
            icon: ArrowLeftRight,
            onSelect: () => giveTo({ who: other, speaker: null }),
          },
        ]),
    "separator",
    {
      id: "delete",
      label: t("notes.line.delete"),
      icon: Trash2,
      danger: true,
      onSelect: remove,
    },
  ];

  if (preview)
    return <span className={`${className} text-text-3`}>{line.text}</span>;

  return (
    <div
      className="group/line relative min-w-0"
      onContextMenu={(e) => {
        if (editing) return;
        e.preventDefault();
        setPicked(
          canAddWord ? (pickedWords(textRef.current)?.text ?? null) : null,
        );
        setMenu(true);
      }}
    >
      {editing ? (
        <textarea
          ref={box}
          value={draft}
          rows={1}
          aria-label={t("notes.line.editLabel")}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={save}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              e.currentTarget.blur();
            } else if (e.key === "Escape") {
              e.preventDefault();
              e.stopPropagation();
              open.current = false;
              setDraft(line.text);
              setEditing(false);
            }
          }}
          className={`${className} -mx-1.5 -my-1 block w-[calc(100%+12px)] resize-none rounded-[var(--r-sm)] border border-teal-600 bg-surface px-1.5 py-1 outline-none`}
        />
      ) : (
        <span
          ref={textRef}
          data-line-text
          onClick={seek}
          onDoubleClick={edit}
          className={`${className} block select-text pe-7`}
        >
          {current && player ? (
            <SpokenWords text={line.text} words={words} player={player} />
          ) : (
            line.text
          )}
        </span>
      )}
      {!editing && (
        <button
          ref={more}
          type="button"
          onClick={() => {
            setPicked(null);
            setMenu(!menu);
          }}
          aria-label={t("notes.line.menu")}
          aria-expanded={menu}
          aria-haspopup="menu"
          title={t("notes.line.menu")}
          className={`absolute end-0 top-0 grid h-6 w-6 place-items-center rounded-[var(--r-xs)] text-text-3 transition-opacity duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text focus-visible:opacity-100 group-hover/line:opacity-100 group-hover/line:duration-0 ${menu ? "opacity-100" : "opacity-0"}`}
        >
          <EllipsisVertical className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
      )}
      {menu && (
        <ActionMenu
          items={items}
          label={t("notes.line.menu")}
          onClose={(restore) => {
            setMenu(false);
            if (restore) more.current?.focus();
          }}
          className="end-0 top-7"
        />
      )}
      {fix && !editing && (
        <div className="mt-1.5 flex flex-wrap items-center gap-x-1 gap-y-1 text-[13px]">
          <span className="me-1 text-text-2">
            {t("notes.fix.offer", { heard: fix.heard, word: fix.word })}
          </span>
          {fix.others.length > 0 && (
            <button
              type="button"
              disabled={busy}
              onClick={() => applyFix(false)}
              className="rounded-[var(--r-sm)] px-1.5 py-0.5 font-medium text-teal-700 transition-colors duration-[var(--dur-1)] hover:bg-teal-50 hover:duration-0 disabled:opacity-50"
            >
              {t("notes.fix.others", { count: fix.others.length })}
            </button>
          )}
          {!learned && (
            <button
              type="button"
              disabled={busy}
              onClick={() => applyFix(true)}
              className="rounded-[var(--r-sm)] px-1.5 py-0.5 font-medium text-teal-700 transition-colors duration-[var(--dur-1)] hover:bg-teal-50 hover:duration-0 disabled:opacity-50"
            >
              {t("notes.fix.learn")}
            </button>
          )}
          <button
            type="button"
            onClick={() => setFix(null)}
            aria-label={t("notes.fix.dismiss")}
            title={t("notes.fix.dismiss")}
            className="grid h-6 w-6 place-items-center rounded-[var(--r-xs)] text-text-3 hover:bg-surface-3 hover:text-text"
          >
            <X className="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        </div>
      )}
    </div>
  );
};
