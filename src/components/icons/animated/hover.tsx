import React, {
  forwardRef,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import type { IconNode, LucideIcon, LucideProps } from "lucide-react";

/** What an animated icon lets its owner do. */
export interface IconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

/** An icon from the animated set, or one built with `craft`. */
export type MovingIcon = React.ForwardRefExoticComponent<
  React.SVGAttributes<SVGSVGElement> & {
    size?: number;
    rootRef?: React.Ref<SVGSVGElement>;
  } & React.RefAttributes<IconHandle>
>;

/** What hovering or focusing sets an icon inside it moving: the control the
 *  icon belongs to. An icon outside any of these moves when it's hovered
 *  itself. A list row is not one, so pointing at a row doesn't set off
 *  every icon in it. */
const TRIGGER = [
  "button",
  "a[href]",
  '[role="button"]',
  '[role="menuitem"]',
  '[role="menuitemradio"]',
  '[role="menuitemcheckbox"]',
  '[role="option"]',
  '[role="radio"]',
  '[role="tab"]',
  '[role="switch"]',
  "summary",
  "[data-icon-trigger]",
].join(",");

const reduced = () =>
  typeof matchMedia === "function" &&
  matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * A Lucide icon that moves when the control it sits in is hovered or
 * focused with the keyboard.
 *
 * It renders the plain Lucide icon until that first happens. Only then is
 * the animated one loaded (`load`, once for all of its kind) and swapped
 * in, so starting up and a page of hundreds of icons cost what they did
 * before. Nothing moves with reduced motion on, in a disabled control, or
 * for an icon that's spinning as a progress mark.
 */
export const hover = (
  Still: LucideIcon,
  load: () => Promise<MovingIcon>,
): LucideIcon => {
  let loaded: MovingIcon | null = null;
  let loading: Promise<void> | null = null;
  const ensure = () =>
    (loading ??= load().then(
      (icon) => {
        loaded = icon;
      },
      () => {
        // Asked for again on the next hover.
        loading = null;
      },
    ));

  const Icon = forwardRef<SVGSVGElement, LucideProps>(
    ({ size, color, absoluteStrokeWidth: _absolute, ...rest }, ref) => {
      const node = useRef<SVGSVGElement | null>(null);
      const handle = useRef<IconHandle>(null);
      const over = useRef(false);
      const mounted = useRef(true);
      const [Moving, setMoving] = useState<MovingIcon | null>(null);
      const awake = Moving !== null;

      useEffect(() => {
        mounted.current = true;
        return () => {
          mounted.current = false;
        };
      }, []);

      const setNode = useCallback(
        (el: SVGSVGElement | null) => {
          node.current = el;
          if (typeof ref === "function") ref(el);
          else if (ref) ref.current = el;
        },
        [ref],
      );

      useEffect(() => {
        const el = node.current;
        if (!el) return;
        const trigger = el.closest<HTMLElement>(TRIGGER) ?? el;
        const start = () => {
          // `.im` icons (the sidebar's) have a motion of their own in CSS.
          if (
            reduced() ||
            el.classList.contains("animate-spin") ||
            el.classList.contains("im") ||
            trigger.matches(':disabled, [aria-disabled="true"]')
          )
            return;
          over.current = true;
          if (handle.current) handle.current.startAnimation();
          else if (loaded) setMoving(() => loaded);
          else
            ensure().then(() => {
              if (loaded && mounted.current && over.current)
                setMoving(() => loaded);
            });
        };
        const stop = () => {
          over.current = false;
          handle.current?.stopAnimation();
        };
        const focus = () => {
          if (trigger.matches(":focus-visible")) start();
        };
        trigger.addEventListener("pointerenter", start);
        trigger.addEventListener("pointerleave", stop);
        trigger.addEventListener("focus", focus);
        trigger.addEventListener("blur", stop);
        return () => {
          trigger.removeEventListener("pointerenter", start);
          trigger.removeEventListener("pointerleave", stop);
          trigger.removeEventListener("focus", focus);
          trigger.removeEventListener("blur", stop);
        };
        // The svg is another element once the animated icon takes over.
      }, [awake]);

      // Just swapped in under the pointer: start now.
      useEffect(() => {
        if (awake && over.current) handle.current?.startAnimation();
      }, [awake]);

      if (!Moving)
        return <Still ref={setNode} size={size} color={color} {...rest} />;
      const { ref: _ref, ...attrs } = rest as Omit<
        LucideProps,
        "size" | "color" | "absoluteStrokeWidth"
      > & { ref?: unknown };
      return (
        <Moving
          ref={handle}
          rootRef={setNode}
          size={size === undefined ? 24 : Number(size)}
          {...(color ? { stroke: color } : {})}
          {...attrs}
        />
      );
    },
  );
  Icon.displayName = Still.displayName;
  return Icon as LucideIcon;
};

/** Loads an icon built from Lucide's own shapes with `craft`'s moves. */
export const crafted = (
  name: string,
  shapes: () => Promise<{ __iconNode: IconNode }>,
): (() => Promise<MovingIcon>) => {
  return async () => {
    const [{ craft, MOVES }, { __iconNode }] = await Promise.all([
      import("./craft"),
      shapes(),
    ]);
    return craft(name, __iconNode, MOVES[name]);
  };
};
