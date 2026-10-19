import React from "react";
import { useTranslation } from "react-i18next";
import { Button } from "./Button";

interface PathDisplayProps {
  path: string;
  onOpen: () => void;
  disabled?: boolean;
}

export const PathDisplay: React.FC<PathDisplayProps> = ({
  path,
  onOpen,
  disabled = false,
}) => {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-2">
      <div className="min-w-0 flex-1 cursor-text select-text break-all rounded-[var(--r-md)] border border-line bg-surface-2 px-3 py-2 font-mono text-xs text-text-2">
        {path}
      </div>
      <Button onClick={onOpen} variant="secondary" disabled={disabled}>
        {t("common.open")}
      </Button>
    </div>
  );
};
