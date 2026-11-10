import React, { useId, useRef } from "react";
import { X } from "@/icons";

/** Forms typed as chips: Enter or a comma adds one, Backspace on an empty
 *  field removes the last. What's still typed counts as a chip too. */
export const ChipField: React.FC<{
  label: string;
  help: string;
  chips: string[];
  setChips: (chips: string[]) => void;
  input: string;
  setInput: (input: string) => void;
  placeholder: string;
  morePlaceholder: string;
  removeLabel: (chip: string) => string;
  /** Forms that won't be kept as typed, each with why. */
  warnings: string[];
}> = ({
  label,
  help,
  chips,
  setChips,
  input,
  setInput,
  placeholder,
  morePlaceholder,
  removeLabel,
  warnings,
}) => {
  const id = useId();
  const ref = useRef<HTMLInputElement>(null);
  const addChip = () => {
    const value = input.trim().replace(/,$/, "").trim();
    if (!value) return;
    if (!chips.some((h) => h.toLowerCase() === value.toLowerCase()))
      setChips([...chips, value]);
    setInput("");
  };
  return (
    <div>
      <label htmlFor={id} className="mb-1.5 block text-[13px] font-medium">
        {label}
      </label>
      <div
        className="flex min-h-[38px] cursor-text flex-wrap items-center gap-1.5 rounded-[var(--r-md)] border border-line-strong bg-surface px-2 py-[5px] transition-[border-color,box-shadow] duration-[var(--dur-1)] focus-within:border-teal-600 focus-within:shadow-[0_0_0_3px_var(--teal-50)]"
        onClick={() => ref.current?.focus()}
      >
        {chips.map((h, i) => (
          <span
            key={h}
            className="inline-flex h-6 items-center gap-1 rounded-[var(--r-xs)] bg-surface-3 ps-2 pe-1 text-[13px]"
          >
            {h}
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                setChips(chips.filter((_, j) => j !== i));
                ref.current?.focus();
              }}
              aria-label={removeLabel(h)}
              title={removeLabel(h)}
              className="group grid h-[18px] w-[18px] place-items-center rounded text-text-3 hover:bg-surface-4 hover:text-text"
            >
              <X className="h-3 w-3" aria-hidden="true" />
            </button>
          </span>
        ))}
        <input
          id={id}
          ref={ref}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if ((e.key === "Enter" && !e.ctrlKey) || e.key === ",") {
              e.preventDefault();
              addChip();
            } else if (e.key === "Backspace" && !input && chips.length > 0) {
              setChips(chips.slice(0, -1));
            }
          }}
          onBlur={addChip}
          placeholder={chips.length ? morePlaceholder : placeholder}
          className="h-[26px] min-w-[140px] flex-1 border-0 bg-transparent text-sm text-text outline-none placeholder:text-text-4"
          autoComplete="off"
          spellCheck={false}
          aria-describedby={`${id}-help`}
        />
      </div>
      <p id={`${id}-help`} className="mt-1.5 text-[13px] text-text-3">
        {help}
      </p>
      {warnings.map((w) => (
        <p key={w} role="status" className="mt-1 text-[13px] text-warn">
          {w}
        </p>
      ))}
    </div>
  );
};
