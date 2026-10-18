import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown, type DropdownOption } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import type { PasteMethod } from "@/bindings";

interface PasteMethodProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const PasteMethodSetting: React.FC<PasteMethodProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const selectedMethod = (getSetting("paste_method") ||
      "ctrl_v") as PasteMethod;

    const pasteMethodOptions: DropdownOption[] = [
      {
        value: "ctrl_v",
        label: t("settings.advanced.pasteMethod.options.clipboard"),
      },
      {
        value: "direct",
        label: t("settings.advanced.pasteMethod.options.direct"),
      },
      {
        value: "none",
        label: t("settings.advanced.pasteMethod.options.none"),
      },
      {
        value: "ctrl_shift_v",
        label: t("settings.advanced.pasteMethod.options.clipboardCtrlShiftV"),
      },
      {
        value: "shift_insert",
        label: t("settings.advanced.pasteMethod.options.clipboardShiftInsert"),
      },
    ];

    return (
      <SettingContainer
        title={t("settings.advanced.pasteMethod.title")}
        description={t("settings.advanced.pasteMethod.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        tooltipPosition="bottom"
      >
        <Dropdown
          options={pasteMethodOptions}
          selectedValue={selectedMethod}
          onSelect={(value) =>
            updateSetting("paste_method", value as PasteMethod)
          }
          disabled={isUpdating("paste_method")}
        />
      </SettingContainer>
    );
  },
);
