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
          "bg-background border border-mid-gray/20 rounded-lg shadow-lg px-4 py-3 flex items-center gap-3 text-sm",
        title: "font-medium",
        description: "text-mid-gray",
        actionButton:
          "px-2 py-1 text-xs font-medium rounded-lg border bg-mid-gray/10 border-mid-gray/20 hover:bg-background-ui/30 hover:border-logo-primary cursor-pointer whitespace-nowrap",
        cancelButton:
          "px-2 py-1 text-xs font-medium rounded-lg text-text-2 hover:text-text cursor-pointer whitespace-nowrap",
      },
    }}
  />
);
