import React from "react";
import { ChevronDown } from "lucide-react";

type ModelStatus =
  | "ready"
  | "loading"
  | "downloading"
  | "verifying"
  | "error"
  | "unloaded"
  | "none";

interface ModelStatusButtonProps {
  status: ModelStatus;
  displayText: string;
  isDropdownOpen: boolean;
  onClick: () => void;
  className?: string;
}

/** The title bar's model pill: a status dot, the model, and its menu. */
const ModelStatusButton: React.FC<ModelStatusButtonProps> = ({
  status,
  displayText,
  isDropdownOpen,
  onClick,
  className = "",
}) => {
  // Green only when the model is loaded and ready; amber while it is busy
  // getting there; red when it can't run.
  const dot = (() => {
    switch (status) {
      case "ready":
        return "bg-ok shadow-[0_0_0_3px_color-mix(in_srgb,var(--ok)_20%,transparent)]";
      case "loading":
      case "downloading":
      case "verifying":
        return "bg-warn shadow-[0_0_0_3px_color-mix(in_srgb,var(--warn)_20%,transparent)] motion-safe:animate-pulse";
      case "error":
      case "none":
        return "bg-danger shadow-[0_0_0_3px_color-mix(in_srgb,var(--danger)_20%,transparent)]";
      default:
        return "bg-text-4";
    }
  })();

  return (
    <button
      type="button"
      onClick={onClick}
      aria-haspopup="menu"
      aria-expanded={isDropdownOpen}
      className={`inline-flex h-7 items-center gap-2 rounded-full ps-2.5 pe-2 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 aria-expanded:bg-surface-3 ${className}`}
    >
      <span className={`h-[7px] w-[7px] flex-none rounded-full ${dot}`} />
      <span className="max-w-56 truncate">{displayText}</span>
      <ChevronDown
        aria-hidden="true"
        className={`h-3.5 w-3.5 flex-none text-text-3 transition-transform duration-[var(--dur-2)] ${isDropdownOpen ? "rotate-180" : ""}`}
      />
    </button>
  );
};

export default ModelStatusButton;
