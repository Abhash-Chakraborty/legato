import React from "react";
import { Toaster } from "sonner";

/** Toasts, styled once for every window that shows them. `bottom` lifts
 *  them clear of controls along the window's bottom edge. */
export const AppToaster: React.FC<{ bottom?: number }> = ({ bottom }) => (
  <Toaster
    theme="system"
    offset={bottom === undefined ? undefined : { bottom }}
    mobileOffset={bottom === undefined ? undefined : { bottom }}
    toastOptions={{
      unstyled: true,
      classNames: {
        toast:
          "bg-surface border border-line rounded-[var(--r-lg)] shadow-[var(--shadow-3)] px-4 py-3 flex items-center gap-3 text-sm",
        title: "font-medium",
        description: "text-text-3",
        actionButton:
          "btn px-2.5 py-1 text-xs font-medium rounded-[var(--r-sm)] border border-transparent bg-teal-50 text-teal-700 hover:bg-teal-100 cursor-pointer whitespace-nowrap",
        cancelButton:
          "px-2 py-1 text-xs font-medium rounded-lg text-text-2 hover:text-text cursor-pointer whitespace-nowrap",
      },
    }}
  />
);
