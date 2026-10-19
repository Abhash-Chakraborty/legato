import React, { useEffect, useRef } from "react";
import type { LucideIcon } from "lucide-react";

export type ActionMenuItem =
  | {
      id: string;
      label: string;
      icon: LucideIcon;
      /** Shown on the right, such as "Del". */
      shortcut?: string;
      danger?: boolean;
      disabled?: boolean;
      onSelect: () => void;
    }
  | "separator";

interface ActionMenuProps {
  items: ActionMenuItem[];
  label: string;
  /** Called with `true` when focus should go back to the opener. */
  onClose: (restoreFocus: boolean) => void;
  className?: string;
}

/** A menu of commands: arrow keys, Home and End move, Enter runs, Esc and
 *  Tab close. Clicks outside close it without moving focus. */
export const ActionMenu: React.FC<ActionMenuProps> = ({
  items,
  label,
  onClose,
  className = "end-0 top-full mt-1",
}) => {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ref.current
      ?.querySelector<HTMLButtonElement>("[role=menuitem]:not(:disabled)")
      ?.focus();
    const onPointer = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose(false);
    };
    document.addEventListener("pointerdown", onPointer, true);
    return () => document.removeEventListener("pointerdown", onPointer, true);
  }, [onClose]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const buttons = Array.from(
      e.currentTarget.querySelectorAll<HTMLButtonElement>(
        "[role=menuitem]:not(:disabled)",
      ),
    );
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const move = (to: number) => {
      e.preventDefault();
      buttons[(to + buttons.length) % buttons.length]?.focus();
    };
    if (e.key === "ArrowDown") move(i + 1);
    else if (e.key === "ArrowUp") move(i - 1);
    else if (e.key === "Home") move(0);
    else if (e.key === "End") move(buttons.length - 1);
    else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose(true);
    } else if (e.key === "Tab") onClose(false);
    // Keys typed in the menu shouldn't reach the list behind it.
    e.stopPropagation();
  };

  return (
    <div
      ref={ref}
      role="menu"
      aria-label={label}
      className={`menu w-60 ${className}`}
      onKeyDown={onKeyDown}
    >
      {items.map((item, index) =>
        item === "separator" ? (
          <div
            key={`sep-${index}`}
            role="separator"
            className="mx-1.5 my-1 h-px bg-line"
          />
        ) : (
          <button
            key={item.id}
            type="button"
            role="menuitem"
            disabled={item.disabled}
            className={`menu-item ${item.danger ? "text-danger" : ""}`}
            onClick={() => {
              onClose(true);
              item.onSelect();
            }}
          >
            <item.icon
              className={`h-4 w-4 flex-none stroke-[1.75] ${item.danger ? "" : "text-text-2"}`}
              aria-hidden="true"
            />
            <span className="min-w-0 flex-1 truncate">{item.label}</span>
            {item.shortcut && (
              <span className="text-xs text-text-3">{item.shortcut}</span>
            )}
          </button>
        ),
      )}
    </div>
  );
};
