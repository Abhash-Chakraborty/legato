import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

type Side = "top" | "bottom" | "left" | "right";

const GAP = 6;
const EDGE = 8;
const DELAY = 500;
/** Moving from one control to the next shows its tip almost at once. */
const DELAY_AFTER = 100;
const HIDE_AFTER = 5000;

let lastClosed = 0;

/** Where the tip goes beside its control, flipped to the other side when
 *  it wouldn't fit, and kept inside the window. */
const place = (
  target: DOMRect,
  tip: DOMRect,
  side: Side,
): { top: number; left: number } => {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  let s = side;
  if (s === "left" && target.left - GAP - tip.width < EDGE) s = "right";
  else if (s === "right" && target.right + GAP + tip.width > vw - EDGE)
    s = "left";
  else if (s === "top" && target.top - GAP - tip.height < EDGE) s = "bottom";
  else if (s === "bottom" && target.bottom + GAP + tip.height > vh - EDGE)
    s = "top";
  const across =
    s === "left" || s === "right"
      ? {
          top: target.top + target.height / 2 - tip.height / 2,
          left:
            s === "left" ? target.left - GAP - tip.width : target.right + GAP,
        }
      : {
          top:
            s === "top" ? target.top - GAP - tip.height : target.bottom + GAP,
          left: target.left + target.width / 2 - tip.width / 2,
        };
  return {
    top: Math.min(Math.max(across.top, EDGE), vh - EDGE - tip.height),
    left: Math.min(Math.max(across.left, EDGE), vw - EDGE - tip.width),
  };
};

/** A short label for an icon control, with its key if it has one. Shows
 *  after half a second of hover or on keyboard focus, and goes on a key
 *  press, a click, or after five seconds. The control keeps its own
 *  aria-label; the tip is for the eye only. */
export const Tooltip: React.FC<{
  label: string;
  shortcut?: string;
  side?: Side;
  children: React.ReactNode;
}> = ({ label, shortcut, side = "top", children }) => {
  const anchor = useRef<HTMLSpanElement>(null);
  const tip = useRef<HTMLDivElement>(null);
  const timer = useRef<number | undefined>(undefined);
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  const show = (delay: number) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setOpen(true), delay);
  };
  const hide = () => {
    window.clearTimeout(timer.current);
    setOpen((was) => {
      if (was) lastClosed = Date.now();
      return false;
    });
    setPos(null);
  };

  useEffect(() => () => window.clearTimeout(timer.current), []);

  useEffect(() => {
    if (!open) return;
    const away = window.setTimeout(hide, HIDE_AFTER);
    window.addEventListener("keydown", hide, true);
    window.addEventListener("scroll", hide, true);
    return () => {
      window.clearTimeout(away);
      window.removeEventListener("keydown", hide, true);
      window.removeEventListener("scroll", hide, true);
    };
  }, [open]);

  useLayoutEffect(() => {
    if (!open || !anchor.current || !tip.current) return;
    setPos(
      place(
        anchor.current.getBoundingClientRect(),
        tip.current.getBoundingClientRect(),
        side,
      ),
    );
  }, [open, side, label]);

  return (
    <span
      ref={anchor}
      className="inline-flex"
      onPointerEnter={(e) => {
        if (e.pointerType === "mouse")
          show(Date.now() - lastClosed < 400 ? DELAY_AFTER : DELAY);
      }}
      onPointerLeave={hide}
      onPointerDown={hide}
      onFocus={(e) => {
        if (e.target.matches(":focus-visible")) show(DELAY_AFTER);
      }}
      onBlur={hide}
    >
      {children}
      {open &&
        createPortal(
          <div
            ref={tip}
            aria-hidden="true"
            style={{
              top: pos?.top ?? -9999,
              left: pos?.left ?? -9999,
            }}
            className={`pointer-events-none fixed z-[9999] flex max-w-[260px] items-center gap-2 rounded-[var(--r-sm)] border border-line-strong bg-surface px-2 py-1 text-[12px] leading-[1.4] text-text shadow-[var(--shadow-2)] transition-opacity duration-[var(--dur-2)] motion-reduce:transition-none ${pos ? "opacity-100" : "opacity-0"}`}
          >
            <span>{label}</span>
            {shortcut && (
              <kbd className="rounded-[4px] border border-line bg-surface-2 px-1 font-sans text-[12px] leading-[16px] text-text-3">
                {shortcut}
              </kbd>
            )}
          </div>,
          document.body,
        )}
    </span>
  );
};
