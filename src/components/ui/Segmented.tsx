import React from "react";

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
  disabled?: boolean;
  /** Says why a disabled option can't be chosen. */
  title?: string;
}

interface SegmentedProps<T extends string> {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  label: string;
  className?: string;
}

/** A row of mutually exclusive choices; arrow keys move between them. */
export const Segmented = <T extends string>({
  options,
  value,
  onChange,
  label,
  className = "",
}: SegmentedProps<T>) => {
  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const step =
      e.key === "ArrowRight" || e.key === "ArrowDown"
        ? 1
        : e.key === "ArrowLeft" || e.key === "ArrowUp"
          ? -1
          : 0;
    if (!step) return;
    e.preventDefault();
    const enabled = options.filter((o) => !o.disabled);
    const i = enabled.findIndex((o) => o.value === value);
    const next = enabled[(i + step + enabled.length) % enabled.length];
    if (!next) return;
    onChange(next.value);
    requestAnimationFrame(() =>
      e.currentTarget
        ?.querySelector<HTMLElement>(`[data-value="${next.value}"]`)
        ?.focus(),
    );
  };

  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKeyDown}
      className={`inline-flex rounded-[var(--r-sm)] border border-line bg-surface-2 p-0.5 ${className}`}
    >
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          data-value={o.value}
          aria-checked={value === o.value}
          tabIndex={value === o.value ? 0 : -1}
          disabled={o.disabled}
          title={o.title}
          onClick={() => onChange(o.value)}
          className={`h-7 rounded-[calc(var(--r-sm)-2px)] px-3 text-[13px] transition-colors duration-[var(--dur-1)] hover:duration-0 disabled:cursor-not-allowed disabled:text-text-4 ${
            value === o.value
              ? "bg-surface font-medium text-text shadow-[var(--shadow-1)]"
              : "text-text-2 enabled:hover:text-text"
          }`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
};
