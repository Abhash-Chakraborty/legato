import React, { useEffect, useId, useRef } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";

const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "textarea:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

interface SheetProps {
  open: boolean;
  title: string;
  description?: React.ReactNode;
  onClose: () => void;
  /** Ctrl+Enter runs this, as does the form's submit. */
  onSubmit: () => void;
  closeLabel: string;
  footer: React.ReactNode;
  initialFocusRef?: React.RefObject<HTMLElement>;
  /** Room for long text, such as a prompt. */
  wide?: boolean;
  children: React.ReactNode;
}

/** A form that slides in from the right edge, for anything with more than
 *  three fields. Esc closes it and Ctrl+Enter saves; focus stays inside and
 *  goes back to the opener on close. */
export const Sheet: React.FC<SheetProps> = ({
  open,
  title,
  description,
  onClose,
  onSubmit,
  closeLabel,
  footer,
  initialFocusRef,
  wide = false,
  children,
}) => {
  const titleId = useId();
  const descId = useId();
  const panelRef = useRef<HTMLFormElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const onSubmitRef = useRef(onSubmit);
  onSubmitRef.current = onSubmit;

  useEffect(() => {
    if (!open) return;
    const opener =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const frame = requestAnimationFrame(() =>
      (initialFocusRef?.current ?? panelRef.current)?.focus(),
    );

    const onKey = (e: KeyboardEvent) => {
      const panel = panelRef.current;
      if (!panel) return;
      if (e.key === "Escape") {
        // A menu or popover inside the sheet closes first.
        if (e.defaultPrevented) return;
        e.preventDefault();
        onCloseRef.current();
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        onSubmitRef.current();
      } else if (e.key === "Tab") {
        const items = Array.from(
          panel.querySelectorAll<HTMLElement>(FOCUSABLE),
        ).filter((el) => el.offsetParent !== null);
        if (items.length === 0) return;
        const first = items[0];
        const last = items[items.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("keydown", onKey);
      opener?.focus();
    };
  }, [open, initialFocusRef]);

  if (!open) return null;

  return createPortal(
    <div
      className="sheet-backdrop fixed inset-0 z-50 flex justify-end bg-black/25"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <form
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description ? descId : undefined}
        tabIndex={-1}
        onSubmit={(e) => {
          e.preventDefault();
          onSubmit();
        }}
        className={`sheet-panel flex h-full ${wide ? "w-[560px]" : "w-[440px]"} max-w-full flex-col border-s border-line bg-surface shadow-[var(--shadow-3)] outline-none`}
      >
        <div className="flex items-start justify-between gap-3 px-6 pb-4 pt-5">
          <div className="min-w-0">
            <h2 id={titleId} className="text-lg font-semibold">
              {title}
            </h2>
            {description && (
              <p id={descId} className="mt-1 text-[13px] text-text-3">
                {description}
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={closeLabel}
            title={closeLabel}
            className="grid h-8 w-8 flex-none place-items-center rounded-[var(--r-sm)] text-text-3 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0"
          >
            <X className="h-4 w-4" aria-hidden="true" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
          {children}
        </div>
        <div className="flex items-center gap-2 border-t border-line px-6 py-3.5">
          {footer}
        </div>
      </form>
    </div>,
    document.body,
  );
};
