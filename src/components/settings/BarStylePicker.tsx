import React, { useRef } from "react";
import { useTranslation } from "react-i18next";
import type { OverlayStyle } from "@/bindings";

const STYLES: OverlayStyle[] = ["none", "minimal", "live"];

interface BarStylePickerProps {
  value: OverlayStyle;
  onChange: (style: OverlayStyle) => void;
  /** The previews hang from the top edge like the island, or float like
   *  the bar at the top or bottom of the screen. */
  look: "island" | "bar-top" | "bar-bottom";
  disabled?: boolean;
}

/** A small drawing of what each style shows while you dictate, so the
 *  difference is visible before you pick one. */
const Preview: React.FC<{
  style: OverlayStyle;
  look: BarStylePickerProps["look"];
}> = ({ style, look }) => {
  const island = look === "island";
  // The island hangs from the top edge; the bar floats a little in.
  const place = island
    ? "top-0 rounded-b-[10px] rounded-t-none"
    : look === "bar-top"
      ? "top-2 rounded-[10px]"
      : "bottom-2 rounded-[10px]";
  const wave = (bars: number) => (
    <span className="flex items-center gap-[2px]" aria-hidden="true">
      {Array.from({ length: bars }, (_, i) => (
        <span
          key={i}
          className="w-[2px] rounded-full bg-[#6ec1b6]"
          style={{ height: `${[5, 9, 12, 8, 11, 6, 10, 7][i % 8]}px` }}
        />
      ))}
    </span>
  );
  return (
    <span
      aria-hidden="true"
      className="relative block h-[72px] overflow-hidden rounded-[var(--r-sm)] border border-line bg-surface-2"
    >
      {/* The top edge of the screen, for the island to hang from. */}
      {island && <span className="absolute inset-x-0 top-0 h-px bg-line" />}
      {style === "minimal" && (
        <span
          className={`absolute left-1/2 flex h-[20px] -translate-x-1/2 items-center gap-[5px] bg-[#121212] px-[4px] ${place}`}
        >
          <span className="h-[12px] w-[12px] rounded-full bg-white/20" />
          {wave(6)}
          <span className="h-[12px] w-[12px] rounded-full bg-white" />
        </span>
      )}
      {style === "live" && (
        <span
          className={`absolute left-1/2 flex w-[80%] max-w-[118px] -translate-x-1/2 flex-col gap-[4px] bg-[#121212] px-[8px] py-[6px] ${place}`}
        >
          <span className="flex items-center justify-between">
            <span className="h-[4px] w-[4px] rounded-full bg-[#6ec1b6]" />
            {wave(6)}
            <span className="h-[8px] w-[8px] rounded-full bg-white/20" />
          </span>
          <span className="h-[3px] w-full rounded-full bg-white/70" />
          <span className="h-[3px] w-2/3 rounded-full bg-white/35" />
        </span>
      )}
    </span>
  );
};

/** Settings › Legato bar › Style: None, Minimal or Live, each with a
 *  drawing of what it shows. */
export const BarStylePicker: React.FC<BarStylePickerProps> = ({
  value,
  onChange,
  look,
  disabled,
}) => {
  const { t } = useTranslation();
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  const onKeyDown = (e: React.KeyboardEvent, i: number) => {
    const step =
      e.key === "ArrowRight" || e.key === "ArrowDown"
        ? 1
        : e.key === "ArrowLeft" || e.key === "ArrowUp"
          ? -1
          : 0;
    if (!step) return;
    e.preventDefault();
    const next = (i + step + STYLES.length) % STYLES.length;
    onChange(STYLES[next]);
    refs.current[next]?.focus();
  };

  return (
    <div
      role="radiogroup"
      aria-label={t("settings.advanced.overlay.style.title")}
      className="grid grid-cols-1 gap-2.5 min-[520px]:grid-cols-3"
    >
      {STYLES.map((style, i) => {
        const on = style === value;
        return (
          <button
            key={style}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={on}
            tabIndex={on ? 0 : -1}
            disabled={disabled}
            onClick={() => onChange(style)}
            onKeyDown={(e) => onKeyDown(e, i)}
            className={`flex flex-col gap-2 rounded-[var(--r-md)] border p-2 text-left transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0 active:scale-[0.99] ${
              on ? "border-teal-700" : "border-line"
            }`}
          >
            <Preview style={style} look={look} />
            <span className="px-1 pb-0.5">
              <span className="flex items-center gap-2 text-sm font-medium">
                <span
                  aria-hidden="true"
                  className={`grid h-3.5 w-3.5 flex-none place-items-center rounded-full border transition-colors duration-[var(--dur-2)] ${on ? "border-teal-700" : "border-line-strong"}`}
                >
                  <span
                    className={`h-1.5 w-1.5 rounded-full bg-teal-700 transition-transform duration-[var(--dur-2)] ease-[var(--ease-spring)] ${on ? "scale-100" : "scale-0"}`}
                  />
                </span>
                {t(`settings.advanced.overlay.style.options.${style}`)}
              </span>
              <span className="mt-0.5 block text-[12.5px] leading-[1.4] text-text-3">
                {t(`settings.advanced.overlay.style.help.${style}`)}
              </span>
            </span>
          </button>
        );
      })}
    </div>
  );
};
