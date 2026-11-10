import React, { forwardRef, useImperativeHandle } from "react";
import { motion, useAnimation } from "motion/react";
import type { Transition, Variants } from "motion/react";
import type { IconNode } from "lucide-react";
import { cn } from "./cn";
import type { IconHandle, MovingIcon } from "./hover";

/** How one part of an icon, or the whole of it, moves on hover. */
export interface Move {
  /** What it animates to; arrays are keyframes. It rests at none of it. */
  to: Record<string, number | number[]>;
  transition?: Transition;
  /** What a turn or a squeeze happens around, in the icon's own 24 by 24
   *  units ("7.5px 7.5px"). Without it, the part's own centre. */
  around?: string;
  /** Goes back at once when the pointer leaves, for a move that ends
   *  looking as it began (a star turned by one point). */
  snapBack?: boolean;
}

/** An icon's moves: the whole drawing's, and each part's by its place in
 *  Lucide's list of shapes. */
export interface Moves {
  whole?: Move;
  parts?: Record<number, Move>;
}

const REST: Record<string, number> = {
  x: 0,
  y: 0,
  rotate: 0,
  scale: 1,
  scaleX: 1,
  scaleY: 1,
  pathLength: 1,
  opacity: 1,
};

const variants = (move: Move): Variants => {
  const normal: Record<string, unknown> = {};
  for (const key of Object.keys(move.to)) normal[key] = REST[key] ?? 0;
  normal.transition = move.snapBack ? { duration: 0 } : { duration: 0.2 };
  return {
    normal,
    animate: {
      ...move.to,
      transition: move.transition ?? { duration: 0.45, ease: "easeInOut" },
    },
  } as Variants;
};

const pivot = (move: Move): React.CSSProperties =>
  move.around
    ? { transformBox: "view-box", transformOrigin: move.around }
    : { transformBox: "fill-box", transformOrigin: "center" };

/** A point given in the icon's 24 by 24 units, as shares of the whole
 *  drawing: what the svg itself turns around, at whatever size it shows. */
const share = (around: string): string =>
  around
    .split(/\s+/)
    .map((n) => `${(parseFloat(n) / 24) * 100}%`)
    .join(" ");

const SHAPES = {
  path: motion.path,
  circle: motion.circle,
  rect: motion.rect,
  line: motion.line,
  ellipse: motion.ellipse,
  polyline: motion.polyline,
  polygon: motion.polygon,
} as const;

/**
 * Builds a hover-animated icon from Lucide's own shapes, for an icon the
 * animated set doesn't have. `node` is the icon as lucide-react draws it
 * (its `__iconNode`), so the two look the same at rest.
 */
export const craft = (
  name: string,
  node: IconNode,
  moves: Moves,
): MovingIcon => {
  const whole = moves.whole;
  const Crafted = forwardRef<
    IconHandle,
    React.SVGAttributes<SVGSVGElement> & {
      size?: number;
      rootRef?: React.Ref<SVGSVGElement>;
    }
  >(({ className, size = 24, rootRef, ...props }, ref) => {
    const controls = useAnimation();
    useImperativeHandle(ref, () => ({
      startAnimation: () => controls.start("animate"),
      stopAnimation: () => controls.start("normal"),
    }));
    return (
      <motion.svg
        ref={rootRef}
        className={cn(`lucide lucide-${name}`, className)}
        xmlns="http://www.w3.org/2000/svg"
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        animate={whole ? controls : undefined}
        initial={whole ? "normal" : undefined}
        variants={whole ? variants(whole) : undefined}
        style={{
          overflow: "visible",
          ...(whole?.around ? { transformOrigin: share(whole.around) } : {}),
        }}
        {...(props as Record<string, unknown>)}
      >
        {node.map(([tag, attrs], i) => {
          const { key, ...rest } = attrs as Record<string, string> & {
            key?: string;
          };
          const move = moves.parts?.[i];
          const Shape = SHAPES[tag as keyof typeof SHAPES];
          if (!move || !Shape)
            return React.createElement(tag, { key: key ?? i, ...rest });
          return (
            <Shape
              key={key ?? i}
              {...(rest as Record<string, unknown>)}
              animate={controls}
              initial="normal"
              variants={variants(move)}
              style={pivot(move)}
            />
          );
        })}
      </motion.svg>
    );
  });
  Crafted.displayName = name;
  return Crafted;
};

/* The moves, by what they say. Each takes about half a second and plays
   once per hover. */

const quick: Transition = { duration: 0.35, ease: "easeOut" };

/** Each of `order` after the one before, a tenth of a second apart. */
const oneByOne = (order: number[], move: (delay: number) => Move) =>
  Object.fromEntries(order.map((part, i) => [part, move(i * 0.1)]));

/** A line drawn from its start. */
const draw = (delay = 0): Move => ({
  to: { pathLength: [0, 1], opacity: [0, 1] },
  transition: {
    duration: 0.35,
    ease: "easeOut",
    delay,
    opacity: { duration: 0.05, delay },
  },
});

/** A step that way and back. */
const step = (x: number, y = 0): Move => ({
  to: { x: [0, x, 0], y: [0, y, 0] },
  transition: quick,
});

/** A tool at work: tilts back and forth and settles. */
const work = (around?: string): Move => ({
  to: { rotate: [0, -12, 8, -4, 0] },
  transition: { duration: 0.5, ease: "easeInOut" },
  around,
});

/** A warning: a short shake. */
const shake: Move = {
  to: { rotate: [0, -7, 7, -4, 4, 0] },
  transition: { duration: 0.5, ease: "easeInOut" },
};

/** Pressed and let go. */
const press = (to = 0.82): Move => ({
  to: { scale: [1, to, 1] },
  transition: quick,
});

/** A plus that turns a quarter and looks as it did. */
const plusTurn = (around: string): Move => ({
  to: { rotate: [0, 90] },
  transition: quick,
  around,
  snapBack: true,
});

export const MOVES: Record<string, Moves> = {
  // Writing and editing.
  pencil: { whole: work() },
  "pencil-line": { parts: { 0: draw(0.15), 1: work(), 2: work() } },
  "notebook-pen": { parts: { 5: work() } },
  eraser: {
    whole: { to: { x: [0, -2, 2, -1, 0] }, transition: { duration: 0.45 } },
  },
  scissors: {
    whole: {
      to: { scaleY: [1, 0.8, 1, 0.88, 1] },
      transition: { duration: 0.45 },
    },
  },
  type: { whole: press(1.14) },
  "heading-2": { whole: press(1.14) },
  "spell-check": { parts: { 2: draw() } },
  "book-a": { parts: { 1: draw(), 2: draw(0.15) } },
  "book-plus": {
    parts: { 0: plusTurn("12px 10px"), 2: plusTurn("12px 10px") },
  },
  replace: { parts: { 4: step(0, 1.5), 5: step(0, 1.5) } },
  "clipboard-paste": { parts: { 0: step(2), 2: step(2) } },

  // Lists and structure.
  list: { parts: oneByOne([5, 3, 4], draw) },
  "list-checks": { parts: oneByOne([1, 0], draw) },
  "list-ordered": { parts: oneByOne([2, 0, 1], draw) },
  captions: {
    parts: {
      1: { to: { opacity: [0.15, 1] }, transition: { duration: 0.45 } },
    },
  },
  "chart-column": {
    parts: oneByOne([3, 2, 1], (delay) => ({
      to: { scaleY: [0.35, 1] },
      transition: { duration: 0.35, ease: "easeOut", delay },
      around: "12px 17px",
    })),
  },
  braces: { parts: { 0: step(-1.5), 1: step(1.5) } },
  code: { parts: { 0: step(2), 1: step(-2) } },
  merge: { parts: { 1: draw(), 2: draw(0.2) } },

  // Controls.
  square: { whole: press() },
  minus: { whole: { to: { scaleX: [1, 0.6, 1] }, transition: quick } },
  ellipsis: {
    parts: oneByOne([2, 0, 1], (delay) => ({
      to: { y: [0, -2.5, 0] },
      transition: { duration: 0.3, ease: "easeOut", delay },
    })),
  },
  "ellipsis-vertical": {
    parts: oneByOne([1, 0, 2], (delay) => ({
      to: { scale: [1, 1.7, 1] },
      transition: { duration: 0.3, ease: "easeOut", delay },
    })),
  },
  "settings-2": { parts: { 2: step(-2.5), 3: step(2.5) } },
  "arrow-left-right": {
    parts: { 0: step(-2), 1: step(-2), 2: step(2), 3: step(2) },
  },
  "arrow-up-down": {
    parts: { 0: step(0, 2), 1: step(0, 2), 2: step(0, -2), 3: step(0, -2) },
  },
  "minimize-2": {
    parts: {
      0: step(-1.5, 1.5),
      1: step(-1.5, 1.5),
      2: step(1.5, -1.5),
      3: step(1.5, -1.5),
    },
  },
  "send-horizontal": { whole: step(3) },
  "undo-2": { whole: { to: { rotate: [0, -25, 0] }, transition: quick } },
  "file-down": { parts: { 2: step(0, 2), 3: step(0, 2) } },
  crosshair: {
    whole: {
      to: { rotate: [0, 90], scale: [1, 0.9, 1] },
      transition: quick,
      snapBack: true,
    },
  },

  // Things.
  pin: {
    whole: { to: { rotate: [0, -14, 8, 0], y: [0, -1, 0] }, transition: quick },
  },
  "pin-off": {
    whole: { to: { rotate: [0, -14, 8, 0], y: [0, -1, 0] }, transition: quick },
  },
  tag: { whole: work("7.5px 7.5px") },
  "key-round": { whole: work("16.5px 7.5px") },
  star: {
    whole: {
      to: { rotate: [0, 72], scale: [1, 1.15, 1] },
      transition: { duration: 0.5, ease: "easeOut" },
      snapBack: true,
    },
  },
  folder: { whole: step(0, -1.5) },
  cloud: { whole: step(0, -1.5) },
  mail: { parts: { 0: draw() } },
  camera: { parts: { 1: press(0.7) } },
  video: { parts: { 0: step(1.5) } },
  globe: {
    parts: {
      1: {
        to: { scaleX: [1, 0.15, 1] },
        transition: { duration: 0.6, ease: "easeInOut" },
      },
    },
  },
  smartphone: {
    whole: {
      to: { rotate: [0, -8, 8, -6, 6, 0] },
      transition: { duration: 0.5 },
    },
  },
  plug: { whole: step(0, -2) },
  "flask-conical": { whole: work("12px 4px") },
  bug: {
    whole: {
      to: { rotate: [0, -8, 8, -5, 0], y: [0, -1, 0] },
      transition: { duration: 0.5 },
    },
  },
  "calendar-plus": {
    parts: { 0: plusTurn("19px 19px"), 2: plusTurn("19px 19px") },
  },

  // Status.
  "triangle-alert": { whole: shake },
  "shield-alert": { whole: shake },
  shield: { whole: press(1.1) },
  info: {
    parts: {
      2: { to: { y: [0, -2, 0] }, transition: quick },
      1: {
        to: { scaleY: [1, 0.8, 1] },
        transition: quick,
        around: "12px 16px",
      },
    },
  },
};
