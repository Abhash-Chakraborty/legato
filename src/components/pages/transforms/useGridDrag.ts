import React, { useRef, useState } from "react";

interface Drag {
  id: string;
  from: number;
  to: number;
  dx: number;
  dy: number;
}

interface Press {
  id: string;
  x: number;
  y: number;
  rects: DOMRect[];
  moved: boolean;
}

/** Where `id` lands in `ids` if it moves from `from` to `to`. */
export const reorder = <T>(items: T[], from: number, to: number): T[] => {
  const next = [...items];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
};

/** The slot nearest to a point, by the centres of the slots. */
export const nearestSlot = (rects: DOMRect[], x: number, y: number) => {
  let best = 0;
  let bestDistance = Infinity;
  rects.forEach((r, i) => {
    const distance = Math.hypot(
      r.left + r.width / 2 - x,
      r.top + r.height / 2 - y,
    );
    if (distance < bestDistance) {
      best = i;
      bestDistance = distance;
    }
  });
  return best;
};

/** Dragging cards in a grid by pointer (Tauri's webview takes HTML5 drag
 *  and drop for files). The card follows the pointer and the others slide
 *  into the slots they would take; `onDrop` gets the new index. Presses on
 *  buttons, links and fields are left to them. */
export const useGridDrag = (
  ids: string[],
  selector: string,
  onDrop: (id: string, to: number) => void,
) => {
  const [drag, setDrag] = useState<Drag | null>(null);
  const press = useRef<Press | null>(null);

  const onPointerDown = (e: React.PointerEvent<HTMLElement>, id: string) => {
    if (e.button !== 0 || ids.length < 2) return;
    if ((e.target as HTMLElement).closest("button, a, input, [role=menu]"))
      return;
    press.current = {
      id,
      x: e.clientX,
      y: e.clientY,
      rects: [...document.querySelectorAll<HTMLElement>(selector)].map((el) =>
        el.getBoundingClientRect(),
      ),
      moved: false,
    };
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLElement>) => {
    const p = press.current;
    if (!p) return;
    const dx = e.clientX - p.x;
    const dy = e.clientY - p.y;
    if (!p.moved && Math.hypot(dx, dy) < 5) return;
    p.moved = true;
    const from = ids.indexOf(p.id);
    const r = p.rects[from];
    if (!r) return;
    const to = nearestSlot(
      p.rects,
      r.left + r.width / 2 + dx,
      r.top + r.height / 2 + dy,
    );
    setDrag({ id: p.id, from, to, dx, dy });
  };

  const onPointerUp = () => {
    const p = press.current;
    press.current = null;
    const done = drag;
    setDrag(null);
    if (p?.moved && done && done.from !== done.to) onDrop(done.id, done.to);
  };

  /** The card's offset while a drag is on. */
  const shift = (i: number, id: string): React.CSSProperties | undefined => {
    const rects = press.current?.rects;
    if (!drag || !rects) return undefined;
    if (id === drag.id)
      return {
        transform: `translate(${drag.dx}px, ${drag.dy}px)`,
        transition: "none",
        zIndex: 10,
      };
    const slot = reorder(ids, drag.from, drag.to).indexOf(id);
    const a = rects[i];
    const b = rects[slot];
    if (!a || !b || slot === i) return undefined;
    return { transform: `translate(${b.left - a.left}px, ${b.top - a.top}px)` };
  };

  return {
    dragging: drag?.id ?? null,
    shift,
    handlers: (id: string) => ({
      onPointerDown: (e: React.PointerEvent<HTMLElement>) =>
        onPointerDown(e, id),
      onPointerMove,
      onPointerUp,
      onPointerCancel: onPointerUp,
    }),
  };
};
