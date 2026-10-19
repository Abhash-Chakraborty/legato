import React, { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { toast } from "sonner";
import { commands, type HistoryEntry } from "@/bindings";
import { reveal } from "../../../lib/utils/reveal";
import { guessSpelling, tokenize } from "../../../lib/utils/words";
import { useDictionaryStore } from "../../../stores/dictionaryStore";
import { useNavStore } from "../../../stores/navStore";
import { Button } from "../../ui/Button";
import { finalText } from "./format";

interface FixWordProps {
  entry: HistoryEntry;
  /** Leaves fix mode; `fixed` is true after a fix was saved. */
  onExit: (fixed: boolean) => void;
}

type Selection = [number, number];

/** Fix a word: the transcript's words become buttons. Click one or drag
 *  across a few, or move with the arrow keys and press Enter, then say how it
 *  should be spelled. */
export const FixWord: React.FC<FixWordProps> = ({ entry, onExit }) => {
  const { t } = useTranslation();
  const text = finalText(entry);
  const tokens = useMemo(() => tokenize(text), [text]);
  const [cursor, setCursor] = useState(0);
  const [sel, setSel] = useState<Selection | null>(null);
  const [popover, setPopover] = useState(false);
  const dragging = useRef<number | null>(null);
  const rootRef = useRef<HTMLParagraphElement>(null);

  const focusWord = (k: number) =>
    rootRef.current?.querySelector<HTMLElement>(`[data-k="${k}"]`)?.focus();

  useEffect(() => {
    focusWord(0);
  }, []);

  // Pointer drags end anywhere on the page.
  useEffect(() => {
    const up = () => {
      if (dragging.current === null) return;
      dragging.current = null;
      setPopover(true);
    };
    document.addEventListener("pointerup", up);
    return () => document.removeEventListener("pointerup", up);
  }, []);

  const select = (a: number, b: number) =>
    setSel([Math.min(a, b), Math.max(a, b)]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (popover) return;
    const last = tokens.length - 1;
    const move = (to: number, extend: boolean) => {
      e.preventDefault();
      const k = Math.max(0, Math.min(last, to));
      if (extend) select(sel ? (k < cursor ? sel[1] : sel[0]) : cursor, k);
      setCursor(k);
      focusWord(k);
    };
    if (e.key === "ArrowRight") move(cursor + 1, e.shiftKey);
    else if (e.key === "ArrowLeft") move(cursor - 1, e.shiftKey);
    else if (e.key === "Home") move(0, e.shiftKey);
    else if (e.key === "End") move(last, e.shiftKey);
    else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (!sel || cursor < sel[0] || cursor > sel[1]) select(cursor, cursor);
      setPopover(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onExit(false);
    }
    // Row shortcuts (S, Delete, arrows between rows) stay off while fixing.
    e.stopPropagation();
  };

  if (tokens.length === 0) {
    return <p className="text-[15px] leading-[1.55] text-text">{text}</p>;
  }

  const parts: React.ReactNode[] = [];
  let at = 0;
  tokens.forEach((tok, k) => {
    if (tok.s > at) {
      const inSel = sel && k > sel[0] && k <= sel[1];
      parts.push(
        <span key={`g${k}`} className={inSel ? "bg-teal-100" : ""}>
          {text.slice(at, tok.s)}
        </span>,
      );
    }
    const on = !!sel && k >= sel[0] && k <= sel[1];
    parts.push(
      <button
        key={k}
        type="button"
        data-k={k}
        tabIndex={k === cursor ? 0 : -1}
        aria-pressed={on}
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          e.preventDefault();
          dragging.current = k;
          setPopover(false);
          setCursor(k);
          select(k, k);
          focusWord(k);
        }}
        onPointerEnter={() => {
          if (dragging.current !== null) select(dragging.current, k);
        }}
        className={`rounded-[3px] px-px transition-colors duration-[var(--dur-1)] hover:duration-0 focus-visible:outline-offset-1 ${
          on
            ? "bg-teal-100 text-text"
            : "underline decoration-line-strong decoration-dotted underline-offset-4 hover:bg-surface-3"
        }`}
      >
        {text.slice(tok.s, tok.e)}
      </button>,
    );
    at = tok.e;
  });
  if (at < text.length) parts.push(<span key="end">{text.slice(at)}</span>);

  return (
    <div className="relative">
      <p
        ref={rootRef}
        role="group"
        aria-label={t("home.fix.pickLabel")}
        onKeyDown={onKeyDown}
        className="select-none whitespace-pre-wrap break-words text-[15px] leading-[1.75] text-text"
      >
        {parts}
      </p>
      <p className="mt-1 text-[13px] text-text-3">
        {t("home.fix.pickHelp")}{" "}
        <button
          type="button"
          onClick={() => onExit(false)}
          className="font-medium text-teal-700 underline-offset-2 hover:underline"
        >
          {t("home.fix.done")}
        </button>
      </p>
      {popover && sel && (
        <FixPopover
          entry={entry}
          text={text}
          start={tokens[sel[0]].s}
          end={tokens[sel[1]].e}
          canExtendLeft={sel[0] > 0}
          canExtendRight={sel[1] < tokens.length - 1}
          onExtend={(side) =>
            setSel(
              side < 0
                ? [Math.max(0, sel[0] - 1), sel[1]]
                : [sel[0], Math.min(tokens.length - 1, sel[1] + 1)],
            )
          }
          onCancel={() => {
            setPopover(false);
            focusWord(cursor);
          }}
          onFixed={() => onExit(true)}
        />
      )}
    </div>
  );
};

interface FixPopoverProps {
  entry: HistoryEntry;
  text: string;
  start: number;
  end: number;
  canExtendLeft: boolean;
  canExtendRight: boolean;
  onExtend: (side: -1 | 1) => void;
  onCancel: () => void;
  onFixed: () => void;
}

const FixPopover: React.FC<FixPopoverProps> = ({
  entry,
  text,
  start,
  end,
  canExtendLeft,
  canExtendRight,
  onExtend,
  onCancel,
  onFixed,
}) => {
  const { t } = useTranslation();
  const go = useNavStore((s) => s.go);
  const entries = useDictionaryStore((s) => s.view?.entries ?? []);
  const span = text.slice(start, end);
  const guess = useMemo(() => guessSpelling(span, entries), [span, entries]);
  const [value, setValue] = useState(guess.text);
  const [edited, setEdited] = useState(false);
  const [remember, setRemember] = useState(true);
  const [also, setAlso] = useState(false);
  const [others, setOthers] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const ref = useRef<HTMLDivElement>(null);
  const inputId = useId();

  // A new span brings a new suggestion, unless the spelling was typed.
  useEffect(() => {
    if (!edited) setValue(guess.text);
  }, [guess.text, edited]);

  useEffect(() => {
    requestAnimationFrame(() => {
      inputRef.current?.focus({ preventScroll: true });
      inputRef.current?.select();
    });
    // Near the bottom of the list the popover would open off screen, and it
    // grows as the span widens, so keep it in view while it is open.
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver(() => reveal(el));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const to = value.trim();
  const valid = to !== "" && to !== span;

  useEffect(() => {
    if (!valid) {
      setOthers([]);
      return;
    }
    let live = true;
    const timer = setTimeout(async () => {
      const r = await commands
        .findFixCandidates(span, to, entry.id)
        .catch(() => null);
      if (live) setOthers(r?.status === "ok" ? r.data : []);
    }, 150);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [span, to, valid, entry.id]);

  // Clicks outside close it.
  useEffect(() => {
    const onPointer = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) onCancel();
    };
    document.addEventListener("pointerdown", onPointer, true);
    return () => document.removeEventListener("pointerdown", onPointer, true);
  }, [onCancel]);

  const commit = async (learn: boolean) => {
    if (!valid || busy) {
      inputRef.current?.focus();
      return;
    }
    setBusy(true);
    const result = await commands.fixWord({
      history_id: entry.id,
      shown_before: text,
      shown_after: text.slice(0, start) + to + text.slice(end),
      from: span,
      to,
      learn,
      remember_heard: learn && remember,
      also: learn && also ? others : [],
    });
    setBusy(false);
    if (result.status === "error") {
      toast.error(t("home.fix.failed"), { description: result.error });
      return;
    }
    const r = result.data;
    const more =
      r.others > 0 ? t("home.fix.alsoFixed", { count: r.others }) : "";
    const message = !learn
      ? t("home.fix.toast.here")
      : r.created
        ? t("home.fix.toast.added", { word: to })
        : r.extended
          ? t("home.fix.toast.extended", { heard: span, word: to })
          : t("home.fix.toast.known", { word: to });
    toast.success(message + more, {
      duration: 7000,
      action: {
        label: t("home.toast.undo"),
        onClick: () => {
          commands.undoFixWord(r.undo_token).then((u) => {
            if (u.status === "error") toast.error(u.error);
          });
        },
      },
      ...(learn
        ? {
            cancel: {
              label: t("home.fix.openDictionary"),
              onClick: () => go("dictionary"),
            },
          }
        : {}),
    });
    onFixed();
  };

  return (
    <div
      ref={ref}
      role="dialog"
      aria-label={t("home.fix.popoverLabel", { heard: span })}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
      }}
      onAnimationEnd={(e) => reveal(e.currentTarget)}
      className="menu absolute start-0 top-full z-30 mt-1 w-[320px] scroll-mb-4 p-3.5"
    >
      <div className="mb-3 flex items-center gap-2 text-[13px]">
        <span className="text-text-3">{t("home.fix.heard")}</span>
        <span className="min-w-0 flex-1 truncate font-medium">
          {t("dictionary.row.quoted", { text: span })}
        </span>
        <button
          type="button"
          disabled={!canExtendLeft}
          onClick={() => onExtend(-1)}
          aria-label={t("home.fix.extendLeft")}
          title={t("home.fix.extendLeft")}
          className="grid h-7 w-7 place-items-center rounded-[var(--r-sm)] text-text-2 hover:bg-surface-2 hover:text-text disabled:opacity-40 disabled:hover:bg-transparent"
        >
          <ChevronLeft className="h-4 w-4" aria-hidden="true" />
        </button>
        <button
          type="button"
          disabled={!canExtendRight}
          onClick={() => onExtend(1)}
          aria-label={t("home.fix.extendRight")}
          title={t("home.fix.extendRight")}
          className="grid h-7 w-7 place-items-center rounded-[var(--r-sm)] text-text-2 hover:bg-surface-2 hover:text-text disabled:opacity-40 disabled:hover:bg-transparent"
        >
          <ChevronRight className="h-4 w-4" aria-hidden="true" />
        </button>
      </div>
      <label htmlFor={inputId} className="mb-1.5 block text-[13px] font-medium">
        {t("home.fix.shouldBe")}
      </label>
      <input
        id={inputId}
        ref={inputRef}
        className="input w-full"
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
          setEdited(true);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            commit(true);
          }
        }}
        autoComplete="off"
        spellCheck={false}
      />
      {guess.why && !edited && (
        <p className="mt-1.5 text-[13px] text-text-3">
          {t(`home.fix.why.${guess.why}`, { word: guess.text })}
        </p>
      )}
      <div className="mt-3 flex flex-col gap-2">
        <label className="flex cursor-pointer items-start gap-2 text-[13px]">
          <input
            type="checkbox"
            className="check"
            checked={remember}
            onChange={(e) => setRemember(e.target.checked)}
          />
          <span>{t("home.fix.remember", { heard: span })}</span>
        </label>
        {others.length > 0 && (
          <label className="flex cursor-pointer items-start gap-2 text-[13px]">
            <input
              type="checkbox"
              className="check"
              checked={also}
              onChange={(e) => setAlso(e.target.checked)}
            />
            <span>{t("home.fix.also", { count: others.length })}</span>
          </label>
        )}
      </div>
      <div className="mt-3.5 flex flex-wrap gap-2">
        <Button
          size="sm"
          disabled={!valid || busy}
          onClick={() => commit(true)}
        >
          {t("home.fix.learn")}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          disabled={!valid || busy}
          onClick={() => commit(false)}
        >
          {t("home.fix.here")}
        </Button>
        <Button size="sm" variant="ghost" onClick={onCancel}>
          {t("home.fix.cancel")}
        </Button>
      </div>
    </div>
  );
};
