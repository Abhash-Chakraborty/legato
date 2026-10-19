import React from "react";
import { useTranslation } from "react-i18next";
import ResetIcon from "../icons/ResetIcon";

interface ResetButtonProps {
  onClick: () => void;
  disabled?: boolean;
  className?: string;
  ariaLabel?: string;
  children?: React.ReactNode;
}

export const ResetButton: React.FC<ResetButtonProps> = React.memo(
  ({ onClick, disabled = false, className = "", ariaLabel, children }) => {
    const { t } = useTranslation();
    const label = ariaLabel ?? t("common.resetToDefault");
    return (
      <button
        type="button"
        aria-label={label}
        title={label}
        className={`inline-grid h-[30px] w-[30px] place-items-center rounded-[var(--r-sm)] text-text-2 transition-[background-color,color,transform] duration-[var(--dur-1)] ${
          disabled
            ? "cursor-not-allowed opacity-35"
            : "cursor-pointer hover:bg-surface-3 hover:text-text hover:duration-0 active:scale-[0.94] active:duration-0"
        } ${className}`}
        onClick={onClick}
        disabled={disabled}
      >
        {children ?? <ResetIcon />}
      </button>
    );
  },
);
