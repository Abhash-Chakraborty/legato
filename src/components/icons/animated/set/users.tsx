// From pqoqubbw/icons (MIT, © pqoqubbw), icons/users.tsx at 4662fff2,
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

import type { Variants } from "motion/react";
import { motion, useAnimation } from "motion/react";
import type React from "react";
import type { SVGAttributes } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

import { cn } from "../cn";

export interface UsersIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface UsersIconProps extends SVGAttributes<SVGSVGElement> {
  size?: number;
  /** The svg element, for Legato's hover trigger. */
  rootRef?: React.Ref<SVGSVGElement>;
}

const PATH_VARIANTS: Variants = {
  normal: {
    translateX: 0,
    transition: {
      type: "spring",
      stiffness: 200,
      damping: 13,
    },
  },
  animate: {
    translateX: [-6, 0],
    transition: {
      delay: 0.1,
      type: "spring",
      stiffness: 200,
      damping: 13,
    },
  },
};

const UsersIcon = forwardRef<UsersIconHandle, UsersIconProps>(
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
        className={cn("lucide lucide-users", className)}
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
        <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
        <circle cx="9" cy="7" r="4" />
        <motion.path
          animate={controls}
          d="M22 21v-2a4 4 0 0 0-3-3.87"
          variants={PATH_VARIANTS}
        />
        <motion.path
          animate={controls}
          d="M16 3.13a4 4 0 0 1 0 7.75"
          variants={PATH_VARIANTS}
        />
      </svg>
    );
  },
);

UsersIcon.displayName = "UsersIcon";

export { UsersIcon };
