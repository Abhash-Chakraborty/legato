import React, { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { commands } from "@/bindings";

const BARS = 4;

/** Four bars that move with your voice while the Scratchpad takes a
 *  dictation, beside the stop square. They sit low and still in silence. */
export const DictationMeter: React.FC = () => {
  const [levels, setLevels] = useState<number[]>(() => Array(BARS).fill(0));

  useEffect(() => {
    commands.watchMicLevel(true).catch(() => {});
    const unlisten = getCurrentWebviewWindow().listen<number[]>(
      "mic-level",
      ({ payload }) => {
        // The bands run low to high; a few spread across them.
        const step = Math.max(1, Math.floor(payload.length / BARS));
        setLevels(
          Array.from({ length: BARS }, (_, i) =>
            Math.min(1, payload[i * step] ?? 0),
          ),
        );
      },
    );
    return () => {
      commands.watchMicLevel(false).catch(() => {});
      unlisten.then((off) => off());
    };
  }, []);

  return (
    <span className="flex h-3.5 items-center gap-[2px]" aria-hidden="true">
      {levels.map((level, i) => (
        <span
          key={i}
          className="w-[2.5px] rounded-full bg-current transition-[height] duration-[80ms] ease-out"
          style={{ height: `${3 + level * 11}px` }}
        />
      ))}
    </span>
  );
};
