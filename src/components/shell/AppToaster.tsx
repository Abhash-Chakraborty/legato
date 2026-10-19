import React from "react";
import { Toaster } from "sonner";

/** Toasts, styled once for every window that shows them. */
export const AppToaster: React.FC = () => (
  <Toaster
    theme="system"
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
