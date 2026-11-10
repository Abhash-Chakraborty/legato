// From pqoqubbw/icons (MIT, © pqoqubbw), icons/italic.tsx at 4662fff2,
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

import type { Variants } from "motion/react";
import { motion, useAnimation } from "motion/react";
import type React from "react";
import type { SVGAttributes } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

import { cn } from "../cn";

export interface ItalicIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface ItalicIconProps extends SVGAttributes<SVGSVGElement> {
  size?: number;
  /** The svg element, for Legato's hover trigger. */
  rootRef?: React.Ref<SVGSVGElement>;
}

const LINE_VARIANTS: Variants = {
  normal: { pathLength: 1, opacity: 1, pathOffset: 0 },
  animate: {
    pathLength: [0, 1],
    opacity: [0, 1],
    pathOffset: [1, 0],
  },
};

const ItalicIcon = forwardRef<ItalicIconHandle, ItalicIconProps>(
  (
    { onMouseEnter, onMouseLeave, className, size = 24, rootRef, ...props },
    ref,
  ) => {
    const controls = useAnimation();
    const isControlledRef = useRef(false);

    useImperativeHandle(ref, () => {
      isControlledRef.current = true;

      return {
        startAnimation: () => controls.start("animate"),
        stopAnimation: () => controls.start("normal"),
      };
    });

    const handleMouseEnter = useCallback(
      (e: React.MouseEvent<SVGSVGElement>) => {
        if (isControlledRef.current) {
          onMouseEnter?.(e);
        } else {
          controls.start("animate");
        }
      },
      [controls, onMouseEnter],
    );

    const handleMouseLeave = useCallback(
      (e: React.MouseEvent<SVGSVGElement>) => {
        if (isControlledRef.current) {
          onMouseLeave?.(e);
        } else {
          controls.start("normal");
        }
      },
      [controls, onMouseLeave],
    );

    return (
      <svg
        className={cn("lucide lucide-italic", className)}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        ref={rootRef}
        fill="none"
        height={size}
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2"
        viewBox="0 0 24 24"
        width={size}
        xmlns="http://www.w3.org/2000/svg"
        {...props}
      >
        <motion.line
          animate={controls}
          transition={{ duration: 0.2 }}
          variants={LINE_VARIANTS}
          x1="19"
          x2="10"
          y1="4"
          y2="4"
        />
        <motion.line
          animate={controls}
          transition={{ duration: 0.2 }}
          variants={LINE_VARIANTS}
          x1="14"
          x2="5"
          y1="20"
          y2="20"
        />
        <motion.line
          animate={controls}
          transition={{
            delay: 0.1,
            duration: 0.4,
          }}
          variants={{
            normal: { pathLength: 1, pathOffset: 0 },
            animate: {
              pathLength: [0, 1],
              pathOffset: [1, 0],
            },
          }}
          x1="15"
          x2="9"
          y1="4"
          y2="20"
        />
      </svg>
    );
  },
);

ItalicIcon.displayName = "ItalicIcon";

export { ItalicIcon };
