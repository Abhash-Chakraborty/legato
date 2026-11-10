// From pqoqubbw/icons (MIT, © pqoqubbw), icons/wifi.tsx at 4662fff2,
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

import { motion, useAnimation } from "motion/react";
import type React from "react";
import type { SVGAttributes } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

import { cn } from "../cn";

export interface WifiIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface WifiIconProps extends SVGAttributes<SVGSVGElement> {
  size?: number;
  /** The svg element, for Legato's hover trigger. */
  rootRef?: React.Ref<SVGSVGElement>;
}

const WIFI_LEVELS = [
  { d: "M12 20h.01", initialOpacity: 1, delay: 0 },
  { d: "M8.5 16.429a5 5 0 0 1 7 0", initialOpacity: 1, delay: 0.1 },
  { d: "M5 12.859a10 10 0 0 1 14 0", initialOpacity: 1, delay: 0.2 },
  { d: "M2 8.82a15 15 0 0 1 20 0", initialOpacity: 1, delay: 0.3 },
];

const WifiIcon = forwardRef<WifiIconHandle, WifiIconProps>(
  (
    { onMouseEnter, onMouseLeave, className, size = 24, rootRef, ...props },
    ref,
  ) => {
    const controls = useAnimation();

    const isControlledRef = useRef(false);

    useImperativeHandle(ref, () => {
      isControlledRef.current = true;

      return {
        startAnimation: async () => {
          await controls.start("fadeOut");
          controls.start("fadeIn");
        },
        stopAnimation: () => controls.start("fadeIn"),
      };
    });

    const handleMouseEnter = useCallback(
      async (e: React.MouseEvent<SVGSVGElement>) => {
        if (isControlledRef.current) {
          onMouseEnter?.(e);
        } else {
          await controls.start("fadeOut");
          controls.start("fadeIn");
        }
      },
      [controls, onMouseEnter],
    );

    const handleMouseLeave = useCallback(
      (e: React.MouseEvent<SVGSVGElement>) => {
        controls.start("fadeIn");
        onMouseLeave?.(e);
      },
      [controls, onMouseLeave],
    );

    return (
      <svg
        className={cn("lucide lucide-wifi", className)}
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
        {WIFI_LEVELS.map((level, index) => (
          <motion.path
            animate={controls}
            d={level.d}
            initial={{ opacity: level.initialOpacity }}
            key={index}
            variants={{
              fadeOut: {
                opacity: index === 0 ? 1 : 0,
                transition: { duration: 0.2 },
              },
              fadeIn: {
                opacity: 1,
                transition: {
                  type: "spring",
                  stiffness: 300,
                  damping: 20,
                  delay: level.delay,
                },
              },
            }}
          />
        ))}
      </svg>
    );
  },
);

WifiIcon.displayName = "WifiIcon";

export { WifiIcon };
