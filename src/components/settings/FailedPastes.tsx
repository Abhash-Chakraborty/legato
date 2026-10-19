import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

/** Whether the Scratchpad opens when a paste fails. The text is saved there
 *  either way. */
export const FailedPastes: React.FC<{ grouped?: boolean }> = ({
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  return (
    <ToggleSwitch
      checked={getSetting("scratchpad_failed_pastes") ?? true}
      onChange={(enabled) => updateSetting("scratchpad_failed_pastes", enabled)}
      isUpdating={isUpdating("scratchpad_failed_pastes")}
      label={t("settings.output.failedPastes.title")}
      description={t("settings.output.failedPastes.description")}
      descriptionMode="inline"
      grouped={grouped}
    />
  );
};
