// From pqoqubbw/icons (MIT, © pqoqubbw), icons/languages.tsx at 4662fff2,
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

import type { Variants } from "motion/react";
import { motion, useAnimation } from "motion/react";
import type React from "react";
import type { SVGAttributes } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

import { cn } from "../cn";

export interface LanguagesIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface LanguagesIconProps extends SVGAttributes<SVGSVGElement> {
  size?: number;
  /** The svg element, for Legato's hover trigger. */
  rootRef?: React.Ref<SVGSVGElement>;
}

const PATH_VARIANTS: Variants = {
  normal: { opacity: 1, pathLength: 1, pathOffset: 0 },
  animate: (custom: number) => ({
    opacity: [0, 1],
    pathLength: [0, 1],
    pathOffset: [1, 0],
    transition: {
      opacity: { duration: 0.01, delay: custom * 0.1 },
      pathLength: {
        type: "spring",
        duration: 0.5,
        bounce: 0,
        delay: custom * 0.1,
      },
    },
  }),
};

const SVG_VARIANTS: Variants = {
  normal: { opacity: 1 },
  animate: {
    opacity: 1,
    transition: {
      staggerChildren: 0.1,
      delayChildren: 0.2,
    },
  },
};

const LanguagesIcon = forwardRef<LanguagesIconHandle, LanguagesIconProps>(
  (
    { onMouseEnter, onMouseLeave, className, size = 24, rootRef, ...props },
    ref,
  ) => {
    const svgControls = useAnimation();
    const pathControls = useAnimation();

    const isControlledRef = useRef(false);

    useImperativeHandle(ref, () => {
      isControlledRef.current = true;

      return {
        startAnimation: () => {
          svgControls.start("animate");
          pathControls.start("animate");
        },
        stopAnimation: () => {
          svgControls.start("normal");
          pathControls.start("normal");
        },
      };
    });

    const handleMouseEnter = useCallback(
      (e: React.MouseEvent<SVGSVGElement>) => {
        if (isControlledRef.current) {
          onMouseEnter?.(e);
        } else {
          svgControls.start("animate");
          pathControls.start("animate");
        }
      },
      [onMouseEnter, pathControls, svgControls],
    );

    const handleMouseLeave = useCallback(
      (e: React.MouseEvent<SVGSVGElement>) => {
        if (isControlledRef.current) {
          onMouseLeave?.(e);
        } else {
          svgControls.start("normal");
          pathControls.start("normal");
        }
      },
      [svgControls, pathControls, onMouseLeave],
    );

    return (
      <motion.svg
        className={cn("lucide lucide-languages", className)}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        ref={rootRef}
        animate={svgControls}
        fill="none"
        height={size}
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2"
        variants={SVG_VARIANTS}
        viewBox="0 0 24 24"
        width={size}
        xmlns="http://www.w3.org/2000/svg"
        {...(props as Record<string, unknown>)}
      >
        <motion.path
          animate={pathControls}
          custom={3}
          d="m5 8 6 6"
          variants={PATH_VARIANTS}
        />
        <motion.path
          animate={pathControls}
          custom={2}
          d="m4 14 6-6 3-3"
          variants={PATH_VARIANTS}
        />
        <motion.path
          animate={pathControls}
          custom={1}
          d="M2 5h12"
          variants={PATH_VARIANTS}
        />
        <motion.path
          animate={pathControls}
          custom={0}
          d="M7 2h1"
          variants={PATH_VARIANTS}
        />
        <motion.path
          animate={pathControls}
          custom={3}
          d="m22 22-5-10-5 10"
          variants={PATH_VARIANTS}
        />
        <motion.path
          animate={pathControls}
          custom={3}
          d="M14 18h6"
          variants={PATH_VARIANTS}
        />
      </motion.svg>
    );
  },
);

LanguagesIcon.displayName = "LanguagesIcon";

export { LanguagesIcon };
