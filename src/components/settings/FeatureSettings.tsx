import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

/** The settings Settings › Features switches, each a whole feature. */
export type FeatureKey =
  | "notes_enabled"
  | "scratchpad_enabled"
  | "post_process_enabled"
  | "insights_enabled";

/** One feature on or off: its page, shortcuts and buttons go with it. */
export const FeatureToggle: React.FC<{ feature: FeatureKey; name: string }> = ({
  feature,
  name,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  return (
    <ToggleSwitch
      checked={getSetting(feature) ?? feature !== "post_process_enabled"}
      onChange={(on) => updateSetting(feature, on)}
      isUpdating={isUpdating(feature)}
      label={t(`settings.features.${name}.title`)}
      description={t(`settings.features.${name}.help`)}
      descriptionMode="inline"
      grouped
    />
  );
};

/** The island at the top of the screen, or the bar as before. */
export const IslandToggle: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  return (
    <ToggleSwitch
      checked={getSetting("bar_look") === "island"}
      onChange={(on) => updateSetting("bar_look", on ? "island" : "bar")}
      isUpdating={isUpdating("bar_look")}
      label={t("settings.features.island.title")}
      description={t("settings.features.island.help")}
      descriptionMode="inline"
      grouped
    />
  );
};
