import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronDown } from "lucide-react";

export interface DropdownOption {
  value: string;
  label: string;
  description?: string;
  disabled?: boolean;
}

interface DropdownProps {
  options: DropdownOption[];
  className?: string;
  menuClassName?: string;
  selectedValue: string | null;
  onSelect: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  onOpen?: () => void;
  /** Names the control for screen readers when no visible label points at it. */
  ariaLabel?: string;
}

export const Dropdown: React.FC<DropdownProps> = ({
  options,
  selectedValue,
  onSelect,
  className = "",
  menuClassName,
  placeholder = "Select an option...",
  disabled = false,
  onOpen,
  ariaLabel,
}) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Move focus into the menu on open, onto the chosen option if there is one.
  useEffect(() => {
    if (!isOpen || !menuRef.current) return;
    const items = Array.from(
      menuRef.current.querySelectorAll<HTMLButtonElement>(
        "[role=option]:not(:disabled)",
      ),
    );
    const chosen = items.find((b) => b.dataset.value === selectedValue);
    (chosen ?? items[0])?.focus({ preventScroll: false });
  }, [isOpen, selectedValue]);

  const selectedOption = options.find(
    (option) => option.value === selectedValue,
  );

  const close = (refocus: boolean) => {
    setIsOpen(false);
    if (refocus) buttonRef.current?.focus();
  };

  const handleSelect = (value: string) => {
    onSelect(value);
    close(true);
  };

  const handleToggle = () => {
    if (disabled) return;
    if (!isOpen) onOpen?.();
    setIsOpen(!isOpen);
  };

  const onMenuKeyDown = (e: React.KeyboardEvent) => {
    const items = Array.from(
      menuRef.current?.querySelectorAll<HTMLButtonElement>(
        "[role=option]:not(:disabled)",
      ) ?? [],
    );
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close(true);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!items.length) return;
      const step = e.key === "ArrowDown" ? 1 : -1;
      items[(i + step + items.length) % items.length].focus();
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      items[e.key === "Home" ? 0 : items.length - 1]?.focus();
    } else if (e.key === "Tab") {
      close(false);
    }
  };

  return (
    <div className={`relative ${className}`} ref={dropdownRef}>
      <button
        ref={buttonRef}
        type="button"
        className="select-btn w-full"
        onClick={handleToggle}
        onKeyDown={(e) => {
          if ((e.key === "ArrowDown" || e.key === "ArrowUp") && !isOpen) {
            e.preventDefault();
            handleToggle();
          }
        }}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-label={ariaLabel}
      >
        <span className="truncate">{selectedOption?.label || placeholder}</span>
        <ChevronDown className="chev" aria-hidden="true" />
      </button>
      {isOpen && !disabled && (
        <div
          ref={menuRef}
          role="listbox"
          className={`menu top-full mt-1 ${menuClassName ?? "left-0 right-0"}`}
          onKeyDown={onMenuKeyDown}
        >
          {options.length === 0 ? (
            <div className="px-2.5 py-1.5 text-sm text-text-3">
              {t("common.noOptionsFound")}
            </div>
          ) : (
            options.map((option) => (
              <button
                key={option.value}
                type="button"
                role="option"
                aria-selected={selectedValue === option.value}
                data-value={option.value}
                className={`menu-item ${option.description ? "items-start" : ""}`}
                onClick={() => handleSelect(option.value)}
                disabled={option.disabled}
              >
                <span className="min-w-0 flex-1">
                  <span className="block whitespace-normal break-words">
                    {option.label}
                  </span>
                  {option.description && (
                    <span className="mt-0.5 block whitespace-normal text-xs leading-snug text-text-3">
                      {option.description}
                    </span>
                  )}
                </span>
                {selectedValue === option.value && (
                  <Check className="check" aria-hidden="true" />
                )}
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
};
