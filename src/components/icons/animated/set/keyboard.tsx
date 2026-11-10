// From pqoqubbw/icons (MIT, © pqoqubbw), icons/keyboard.tsx at 4662fff2,
// adapted by scripts/animated-icons.ts: the svg is the root, as in
// lucide-react. Don't edit by hand; change the script and run it again.

import { AnimatePresence, motion, useAnimation } from "motion/react";
import type React from "react";
import type { SVGAttributes } from "react";
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";

import { cn } from "../cn";

export interface KeyboardIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface KeyboardIconProps extends SVGAttributes<SVGSVGElement> {
  size?: number;
  /** The svg element, for Legato's hover trigger. */
  rootRef?: React.Ref<SVGSVGElement>;
}

const KEYBOARD_PATHS = [
  { id: "key1", d: "M10 8h.01" },
  { id: "key2", d: "M12 12h.01" },
  { id: "key3", d: "M14 8h.01" },
  { id: "key4", d: "M16 12h.01" },
  { id: "key5", d: "M18 8h.01" },
  { id: "key6", d: "M6 8h.01" },
  { id: "key7", d: "M7 16h10" },
  { id: "key8", d: "M8 12h.01" },
];

const KeyboardIcon = forwardRef<KeyboardIconHandle, KeyboardIconProps>(
  (
    { onMouseEnter, onMouseLeave, className, size = 24, rootRef, ...props },
    ref,
  ) => {
    const [isHovered, setIsHovered] = useState(false);
    const controls = useAnimation();

    const isControlledRef = useRef(false);

    useImperativeHandle(ref, () => {
      isControlledRef.current = true;

      return {
        startAnimation: () => setIsHovered(true),
        stopAnimation: () => setIsHovered(false),
      };
    });

    const handleMouseEnter = useCallback(
      (e: React.MouseEvent<SVGSVGElement>) => {
        if (isControlledRef.current) {
          onMouseEnter?.(e);
        } else {
          setIsHovered(true);
        }
      },
      [onMouseEnter],
    );

    const handleMouseLeave = useCallback(
      (e: React.MouseEvent<SVGSVGElement>) => {
        if (isControlledRef.current) {
          onMouseLeave?.(e);
        } else {
          setIsHovered(false);
        }
      },
      [onMouseLeave],
    );

    useEffect(() => {
      const animateKeys = async () => {
        if (isHovered) {
          await controls.start((i) => ({
            opacity: [1, 0.2, 1],
            transition: {
              duration: 1.5,
              times: [0, 0.5, 1],
              delay: i * 0.2 * Math.random(),
              repeat: 1,
              repeatType: "reverse",
            },
          }));
        } else {
          controls.stop();
          controls.set({ opacity: 1 });
        }
      };

      animateKeys();
    }, [isHovered, controls]);

    return (
      <svg
        className={cn("lucide lucide-keyboard", className)}
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
        <rect height="16" rx="2" width="20" x="2" y="4" />
        <AnimatePresence>
          {KEYBOARD_PATHS.map((path, index) => (
            <motion.path
              animate={controls}
              custom={index}
              d={path.d}
              initial={{ opacity: 1 }}
              key={path.id}
            />
          ))}
        </AnimatePresence>
      </svg>
    );
  },
);

KeyboardIcon.displayName = "KeyboardIcon";

export { KeyboardIcon };
