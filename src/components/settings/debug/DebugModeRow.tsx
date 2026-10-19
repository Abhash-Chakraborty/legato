import React from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../../ui/SettingContainer";
import { Button } from "../../ui/Button";
import { useSettings } from "../../../hooks/useSettings";

/** Says how this section appeared and turns it off again. */
export const DebugModeRow: React.FC = () => {
  const { t } = useTranslation();
  const { updateSetting } = useSettings();
  return (
    <SettingContainer
      title={t("settings.debug.mode.title")}
      description={t("settings.debug.mode.description")}
    >
      <Button
        variant="secondary"
        onClick={() => updateSetting("debug_mode", false)}
      >
        {t("settings.debug.mode.turnOff")}
      </Button>
    </SettingContainer>
  );
};
