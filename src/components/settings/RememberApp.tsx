import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

/** Whether history records the app each dictation went into. */
export const RememberApp: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  return (
    <ToggleSwitch
      checked={getSetting("remember_app") ?? true}
      onChange={(enabled) => updateSetting("remember_app", enabled)}
      isUpdating={isUpdating("remember_app")}
      label={t("settings.privacy.rememberApp.title")}
      description={t("settings.privacy.rememberApp.description")}
      descriptionMode="inline"
      grouped
    />
  );
};
