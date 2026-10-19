import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

export const NativeTitleBar: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  return (
    <ToggleSwitch
      checked={getSetting("native_title_bar") ?? false}
      onChange={(enabled) => updateSetting("native_title_bar", enabled)}
      isUpdating={isUpdating("native_title_bar")}
      label={t("settings.general.nativeTitleBar.label")}
      description={t("settings.general.nativeTitleBar.description")}
      grouped
    />
  );
};
