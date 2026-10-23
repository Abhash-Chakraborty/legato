import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import type { LiveCaption } from "@/bindings";

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

/** The desktop bubble: a dot that stays on screen, with Dictate, Notes and
 *  the Scratchpad on hover. */
export const ShowBubble: React.FC<{ grouped?: boolean }> = ({ grouped }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  // The island stands in for the bubble, so it reads as off while the
  // island is on, whatever is saved for when it goes.
  const island =
    getSetting("bar_look") === "island" &&
    getSetting("overlay_style") !== "none";
  return (
    <ToggleSwitch
      checked={(getSetting("bubble_enabled") ?? true) && !island}
      onChange={(on) => updateSetting("bubble_enabled", on)}
      isUpdating={isUpdating("bubble_enabled")}
      label={t("settings.bar.bubble.title")}
      description={t("settings.bar.bubble.help")}
      grouped={grouped}
    />
  );
};

/** The live transcript box while the bubble has a dictation: off, or at the
 *  bottom or top of the screen. */
export const LiveCaptionPlace: React.FC<{ grouped?: boolean }> = ({
  grouped,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const options = (["off", "bottom", "top"] as const).map((value) => ({
    value,
    label: t(`settings.bar.liveCaption.options.${value}`),
  }));
  return (
    <SettingContainer
      title={t("settings.bar.liveCaption.title")}
      description={t("settings.bar.liveCaption.help")}
      grouped={grouped}
    >
      <Dropdown
        options={options}
        selectedValue={getSetting("live_caption") ?? "off"}
        onSelect={(value) =>
          updateSetting("live_caption", value as LiveCaption)
        }
        disabled={isUpdating("live_caption")}
      />
    </SettingContainer>
  );
};
