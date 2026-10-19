import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import type { BackgroundAudio as Choice } from "@/bindings";

const CHOICES: Choice[] = ["lower", "pause", "mute", "leave"];

interface BackgroundAudioProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

/** What music and videos in other apps do while you dictate. */
export const BackgroundAudio: React.FC<BackgroundAudioProps> = React.memo(
  ({ descriptionMode = "inline", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const value = (getSetting("background_audio") ?? "lower") as Choice;

    return (
      <SettingContainer
        title={t("settings.sound.backgroundAudio.title")}
        description={t(`settings.sound.backgroundAudio.help.${value}`)}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown
          className="w-[220px]"
          ariaLabel={t("settings.sound.backgroundAudio.title")}
          options={CHOICES.map((choice) => ({
            value: choice,
            label: t(`settings.sound.backgroundAudio.options.${choice}`),
          }))}
          selectedValue={value}
          onSelect={(v) => updateSetting("background_audio", v as Choice)}
          disabled={isUpdating("background_audio")}
        />
      </SettingContainer>
    );
  },
);
