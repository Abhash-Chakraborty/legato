// From pqoqubbw/icons (MIT, © pqoqubbw), icons/audio-lines.tsx at 4662fff2,
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

import { motion, useAnimation } from "motion/react";
import type React from "react";
import type { SVGAttributes } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

import { cn } from "../cn";

export interface AudioLinesIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface AudioLinesIconProps extends SVGAttributes<SVGSVGElement> {
  size?: number;
  /** The svg element, for Legato's hover trigger. */
  rootRef?: React.Ref<SVGSVGElement>;
}

const AudioLinesIcon = forwardRef<AudioLinesIconHandle, AudioLinesIconProps>(
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
        className={cn("lucide lucide-audio-lines", className)}
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
        <path d="M2 10v3" />
        <motion.path
          animate={controls}
          d="M6 6v11"
          variants={{
            normal: { d: "M6 6v11" },
            animate: {
              d: ["M6 6v11", "M6 10v3", "M6 6v11"],
              transition: {
                duration: 1.5,
                repeat: Number.POSITIVE_INFINITY,
              },
            },
          }}
        />
        <motion.path
          animate={controls}
          d="M10 3v18"
          variants={{
            normal: { d: "M10 3v18" },
            animate: {
              d: ["M10 3v18", "M10 9v5", "M10 3v18"],
              transition: {
                duration: 1,
                repeat: Number.POSITIVE_INFINITY,
              },
            },
          }}
        />
        <motion.path
          animate={controls}
          d="M14 8v7"
          variants={{
            normal: { d: "M14 8v7" },
            animate: {
              d: ["M14 8v7", "M14 6v11", "M14 8v7"],
              transition: {
                duration: 0.8,
                repeat: Number.POSITIVE_INFINITY,
              },
            },
          }}
        />
        <motion.path
          animate={controls}
          d="M18 5v13"
          variants={{
            normal: { d: "M18 5v13" },
            animate: {
              d: ["M18 5v13", "M18 7v9", "M18 5v13"],
              transition: {
                duration: 1.5,
                repeat: Number.POSITIVE_INFINITY,
              },
            },
          }}
        />
        <path d="M22 10v3" />
      </svg>
    );
  },
);

AudioLinesIcon.displayName = "AudioLinesIcon";

export { AudioLinesIcon };
