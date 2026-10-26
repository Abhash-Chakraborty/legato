import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { ResetButton } from "../ui/ResetButton";
import { useSettings } from "../../hooks/useSettings";

interface MicrophoneSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const MicrophoneSelector: React.FC<MicrophoneSelectorProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const {
      getSetting,
      updateSetting,
      resetSetting,
      isUpdating,
      isLoading,
      audioDevices,
      refreshAudioDevices,
      refreshSettings,
    } = useSettings();

    const selectedMicrophone =
      getSetting("selected_microphone") === "default"
        ? "Default"
        : getSetting("selected_microphone") || "Default";

    const handleMicrophoneSelect = async (deviceName: string) => {
      await updateSetting("selected_microphone", deviceName);
      // Picking one puts it first in Microphone order.
      await refreshSettings();
    };

    const handleReset = async () => {
      await resetSetting("selected_microphone");
    };

    const microphoneOptions = audioDevices.map((device) => ({
      value: device.name,
      label: device.name,
    }));
    // Your pick stays while it's unplugged; say so instead of a blank.
    if (
      selectedMicrophone !== "Default" &&
      audioDevices.length > 0 &&
      !audioDevices.some((d) => d.name === selectedMicrophone)
    )
      microphoneOptions.push({
        value: selectedMicrophone,
        label: t("settings.sound.microphone.unplugged", {
          name: selectedMicrophone,
        }),
      });

    return (
      <SettingContainer
        title={t("settings.sound.microphone.title")}
        description={t("settings.sound.microphone.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <div className="flex items-center space-x-1">
          <Dropdown
            options={microphoneOptions}
            selectedValue={selectedMicrophone}
            onSelect={handleMicrophoneSelect}
            placeholder={
              isLoading || audioDevices.length === 0
                ? t("settings.sound.microphone.loading")
                : t("settings.sound.microphone.placeholder")
            }
            disabled={
              isUpdating("selected_microphone") ||
              isLoading ||
              audioDevices.length === 0
            }
            onOpen={refreshAudioDevices}
          />
          <ResetButton
            onClick={handleReset}
            disabled={isUpdating("selected_microphone") || isLoading}
          />
        </div>
      </SettingContainer>
    );
  },
);
