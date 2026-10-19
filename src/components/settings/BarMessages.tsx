import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

/** Whether the bar confirms each paste, with Undo. */
export const ConfirmPaste: React.FC<{ grouped?: boolean }> = ({ grouped }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  return (
    <ToggleSwitch
      checked={getSetting("bar_confirm_paste") ?? true}
      onChange={(on) => updateSetting("bar_confirm_paste", on)}
      isUpdating={isUpdating("bar_confirm_paste")}
      label={t("settings.bar.confirmPaste.title")}
      description={t("settings.bar.confirmPaste.help")}
      grouped={grouped}
    />
  );
};

/** Whether the bar shows a transform's changes, with Keep and Undo. */
export const ShowChanges: React.FC<{ grouped?: boolean }> = ({ grouped }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  return (
    <ToggleSwitch
      checked={getSetting("bar_show_changes") ?? true}
      onChange={(on) => updateSetting("bar_show_changes", on)}
      isUpdating={isUpdating("bar_show_changes")}
      label={t("settings.bar.showChanges.title")}
      description={t("settings.bar.showChanges.help")}
      grouped={grouped}
    />
  );
};
