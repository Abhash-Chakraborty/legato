// From pqoqubbw/icons (MIT, © pqoqubbw), icons/phone-incoming.tsx at 4662fff2,
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

import { motion, useAnimation, type Variants } from "motion/react";
import type React from "react";
import type { SVGAttributes } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";
import { cn } from "../cn";

export interface PhoneIncomingIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface PhoneIncomingIconProps extends SVGAttributes<SVGSVGElement> {
  size?: number;
  /** The svg element, for Legato's hover trigger. */
  rootRef?: React.Ref<SVGSVGElement>;
}

const PHONE_MISSED_VARIANTS: Variants = {
  initial: { scale: 1 },
  animate: {
    scale: [1, 1.1, 1],
    transition: {
      duration: 0.9,
      ease: "easeInOut",
    },
  },
};

const ARROW_VARIANTS: Variants = {
  normal: {
    scale: 1,
    translateX: 0,
    translateY: 0,
  },
  animate: {
    scale: [1, 1.2, 1],
    translateX: [0, -1, 0],
    translateY: [0, 1, 0],
    transition: {
      duration: 0.8,
      ease: "easeInOut",
    },
  },
};

const PhoneIncomingIcon = forwardRef<
  PhoneIncomingIconHandle,
  PhoneIncomingIconProps
>(
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
      <motion.svg
        className={cn("lucide lucide-phone-incoming", className)}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        ref={rootRef}
        animate={controls}
        fill="none"
        height="24"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2"
        viewBox="0 0 24 24"
        width="24"
        xmlns="http://www.w3.org/2000/svg"
        {...(props as Record<string, unknown>)}
      >
        <motion.g
          animate={controls}
          initial={{ y: 0, opacity: 1 }}
          variants={ARROW_VARIANTS}
        >
          <motion.path d="M16 2v6h6" />
          <motion.path d="m22 2-6 6" />
        </motion.g>
        <motion.path
          animate={controls}
          d="M13.832 16.568a1 1 0 0 0 1.213-.303l.355-.465A2 2 0 0 1 17 15h3a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2A18 18 0 0 1 2 4a2 2 0 0 1 2-2h3a2 2 0 0 1 2 2v3a2 2 0 0 1-.8 1.6l-.468.351a1 1 0 0 0-.292 1.233 14 14 0 0 0 6.392 6.384"
          initial="initial"
          variants={PHONE_MISSED_VARIANTS}
        />
      </motion.svg>
    );
  },
);

PhoneIncomingIcon.displayName = "PhoneIncomingIcon";

export { PhoneIncomingIcon };
