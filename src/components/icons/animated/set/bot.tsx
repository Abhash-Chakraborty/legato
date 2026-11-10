// From pqoqubbw/icons (MIT, © pqoqubbw), icons/bot.tsx at 4662fff2,
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

import { motion, useAnimation } from "motion/react";
import type React from "react";
import type { SVGAttributes } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

import { cn } from "../cn";

export interface BotIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface BotIconProps extends SVGAttributes<SVGSVGElement> {
  size?: number;
  /** The svg element, for Legato's hover trigger. */
  rootRef?: React.Ref<SVGSVGElement>;
}

const BotIcon = forwardRef<BotIconHandle, BotIconProps>(
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
        className={cn("lucide lucide-bot", className)}
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
        <path d="M12 8V4H8" />
        <rect height="12" rx="2" width="16" x="4" y="8" />
        <path d="M2 14h2" />
        <path d="M20 14h2" />

        <motion.line
          animate={controls}
          initial="normal"
          variants={{
            normal: { y1: 13, y2: 15 },
            animate: {
              y1: [13, 14, 13],
              y2: [15, 14, 15],
              transition: {
                duration: 0.5,
                ease: "easeInOut",
                delay: 0.2,
              },
            },
          }}
          x1={15}
          x2={15}
        />

        <motion.line
          animate={controls}
          initial="normal"
          variants={{
            normal: { y1: 13, y2: 15 },
            animate: {
              y1: [13, 14, 13],
              y2: [15, 14, 15],
              transition: {
                duration: 0.5,
                ease: "easeInOut",
                delay: 0.2,
              },
            },
          }}
          x1={9}
          x2={9}
        />
      </svg>
    );
  },
);

BotIcon.displayName = "Bot";

export { BotIcon };
