import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface Props {
  grouped?: boolean;
}

/** Saying "press enter" at the end of a dictation sends it. */
export const PressEnterByVoice: React.FC<Props> = React.memo(
  ({ grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    return (
      <ToggleSwitch
        checked={getSetting("press_enter_by_voice") ?? false}
        onChange={(on) => updateSetting("press_enter_by_voice", on)}
        isUpdating={isUpdating("press_enter_by_voice")}
        label={t("settings.advanced.pressEnterByVoice.label")}
        description={t("settings.advanced.pressEnterByVoice.description")}
        descriptionMode="tooltip"
        grouped={grouped}
      />
    );
  },
);

/** In chat apps, each paragraph of a dictation goes as its own message. */
export const StackedMessages: React.FC<Props> = React.memo(
  ({ grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    return (
      <ToggleSwitch
        checked={getSetting("stacked_messages") ?? false}
        onChange={(on) => updateSetting("stacked_messages", on)}
        isUpdating={isUpdating("stacked_messages")}
        label={t("settings.advanced.stackedMessages.label")}
        description={t("settings.advanced.stackedMessages.description")}
        descriptionMode="tooltip"
        grouped={grouped}
      />
    );
  },
);
