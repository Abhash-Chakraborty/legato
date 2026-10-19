import React from "react";
import { useTranslation } from "react-i18next";
import { Check, Cpu } from "lucide-react";
import type { ModelInfo } from "@/bindings";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  onModelSelect: (modelId: string) => void;
  /** Opens the Models page. */
  onManage: () => void;
  onClose: () => void;
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  onModelSelect,
  onManage,
  onClose,
}) => {
  const { t } = useTranslation();
  const downloadedModels = models.filter((m) => m.is_downloaded);

  const handleModelClick = (modelId: string) => {
    onModelSelect(modelId);
  };

  return (
    <div
      role="menu"
      className="menu end-0 top-full mt-2 w-72 max-h-[60vh]"
      onKeyDown={(e) => {
        const items = Array.from(
          e.currentTarget.querySelectorAll<HTMLButtonElement>(
            "[role=menuitemradio], [role=menuitem]",
          ),
        );
        const i = items.indexOf(document.activeElement as HTMLButtonElement);
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          const step = e.key === "ArrowDown" ? 1 : -1;
          items[(i + step + items.length) % items.length]?.focus();
        } else if (e.key === "Escape") {
          e.preventDefault();
          onClose();
        }
      }}
    >
      {downloadedModels.length > 0 ? (
        downloadedModels.map((model) => (
          <button
            key={model.id}
            type="button"
            role="menuitemradio"
            aria-checked={currentModelId === model.id}
            onClick={() => handleModelClick(model.id)}
            className="menu-item items-start"
          >
            <span className="min-w-0 flex-1">
              <span className="block text-sm">
                {getTranslatedModelName(model, t)}
                {model.is_custom && (
                  <span className="ms-1.5 text-xs text-text-3">
                    {t("modelSelector.custom")}
                  </span>
                )}
                {model.supports_streaming && (
                  <span className="ms-1.5 text-xs text-teal-700">
                    {t("modelSelector.streaming")}
                  </span>
                )}
              </span>
              <span className="mt-0.5 block text-xs leading-snug text-text-3">
                {getTranslatedModelDescription(model, t)}
              </span>
            </span>
            {currentModelId === model.id && (
              <Check className="check mt-0.5" aria-hidden="true" />
            )}
          </button>
        ))
      ) : (
        <div className="px-2.5 py-1.5 text-sm text-text-3">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
      <div className="mx-1.5 my-1 h-px bg-line" role="separator" />
      <button
        type="button"
        role="menuitem"
        className="menu-item"
        onClick={onManage}
      >
        <Cpu className="h-4 w-4 text-text-2" aria-hidden="true" />
        {t("modelSelector.manage")}
      </button>
    </div>
  );
};

export default ModelDropdown;
