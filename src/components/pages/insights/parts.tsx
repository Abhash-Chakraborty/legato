import React, { useEffect, useRef, useState } from "react";
import { ChevronRight } from "lucide-react";

/** A card on the Insights page. */
export const Card: React.FC<{
  title: string;
  /** Next to the title, such as a toggle or a count. */
  aside?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
}> = ({ title, aside, className = "", children }) => (
  <section
    aria-label={title}
    className={`flex min-w-0 flex-col rounded-[var(--r-lg)] border border-line bg-surface p-5 ${className}`}
  >
    <div className="mb-3 flex min-h-7 items-center justify-between gap-3">
      <h2 className="text-sm font-medium text-text-2">{title}</h2>
      {aside}
    </div>
    {children}
  </section>
);

/** A row that opens another page, with its count. */
export const LinkRow: React.FC<{
  label: string;
  value: string;
  onClick: () => void;
}> = ({ label, value, onClick }) => (
  <button
    type="button"
    onClick={onClick}
    className="-mx-2 flex h-9 items-center gap-3 rounded-[var(--r-sm)] px-2 text-start text-sm transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0 active:scale-[0.99]"
  >
    <span className="min-w-0 flex-1 truncate">{label}</span>
    <span className="tabular-nums text-text-2">{value}</span>
    <ChevronRight className="h-4 w-4 text-text-3" aria-hidden="true" />
  </button>
);

const reducedMotion = () =>
  typeof window !== "undefined" &&
  window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

/** A number that counts up the first time it shows and tweens when it
 *  changes; instant with reduced motion. */
export const useCountUp = (value: number): number => {
  const [shown, setShown] = useState(reducedMotion() ? value : 0);
  const from = useRef(reducedMotion() ? value : 0);
  const first = useRef(true);
  useEffect(() => {
    if (reducedMotion()) {
      setShown(value);
      from.current = value;
      return;
    }
    const start = from.current;
    const duration = first.current ? 600 : 320;
    first.current = false;
    const began = performance.now();
    let frame = 0;
    const step = (now: number) => {
      const p = Math.min(1, (now - began) / duration);
      const eased = 1 - Math.pow(1 - p, 3);
      const next = Math.round(start + (value - start) * eased);
      setShown(next);
      from.current = next;
      if (p < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [value]);
  return shown;
};

/** A big figure that counts up. */
export const Figure: React.FC<{
  value: number;
  locale: string;
  unit?: string;
}> = ({ value, locale, unit }) => {
  const shown = useCountUp(value);
  return (
    <div className="text-[28px] font-semibold leading-none tracking-[-0.01em] tabular-nums">
      {shown.toLocaleString(locale)}
      {unit && (
        <span className="ms-1.5 text-sm font-normal text-text-2">{unit}</span>
      )}
    </div>
  );
};

/** A horizontal bar for a share of the whole. */
export const ShareBar: React.FC<{ share: number }> = ({ share }) => (
  <span
    aria-hidden="true"
    className="block h-1.5 w-full overflow-hidden rounded-full bg-surface-3"
  >
    <span
      className="block h-full rounded-full bg-teal-600 transition-[width] duration-[320ms] ease-out"
      style={{ width: `${Math.max(share * 100, share > 0 ? 2 : 0)}%` }}
    />
  </span>
);
