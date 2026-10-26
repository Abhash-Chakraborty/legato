import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { Slider } from "../ui/Slider";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import { useModelStore } from "../../stores/modelStore";
import { BarStylePicker } from "./BarStylePicker";
import type {
  BarLook,
  IslandIdle,
  OverlayPosition,
  OverlayStyle,
} from "@/bindings";

interface ShowOverlayProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ShowOverlay: React.FC<ShowOverlayProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const { currentModel, models } = useModelStore();
    const model = models.find((m) => m.id === currentModel);

    const positionOptions = [
      {
        value: "bottom",
        label: t("settings.advanced.overlay.position.options.bottom"),
      },
      {
        value: "top",
        label: t("settings.advanced.overlay.position.options.top"),
      },
    ];

    const lookOptions = [
      {
        value: "bar",
        label: t("settings.advanced.overlay.look.options.bar"),
      },
      {
        value: "island",
        label: t("settings.advanced.overlay.look.options.island"),
      },
    ];

    const idleOptions = [
      {
        value: "small",
        label: t("settings.advanced.overlay.idle.options.small"),
      },
      {
        value: "hide",
        label: t("settings.advanced.overlay.idle.options.hide"),
      },
    ];

    const selectedLook: BarLook =
      getSetting("bar_look") === "island" ? "island" : "bar";
    const selectedIdle: IslandIdle =
      getSetting("island_idle") === "hide" ? "hide" : "small";
    const selectedStyle = (getSetting("overlay_style") ||
      "live") as OverlayStyle;
    // Only "top" and "bottom" are selectable; anything else (empty, or a legacy
    // "none" from before the position was retired) falls back to "bottom".
    const selectedPosition: OverlayPosition =
      getSetting("overlay_position") === "top" ? "top" : "bottom";

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.overlay.style.title")}
          description={t("settings.advanced.overlay.style.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
        >
          <BarStylePicker
            value={selectedStyle}
            onChange={(value) => updateSetting("overlay_style", value)}
            look={
              selectedLook === "island"
                ? "island"
                : selectedPosition === "top"
                  ? "bar-top"
                  : "bar-bottom"
            }
            disabled={isUpdating("overlay_style")}
          />
          {/* With the bar's shape, the bubble takes dictations while it's on. */}
          {selectedStyle !== "none" &&
            selectedLook === "bar" &&
            (getSetting("bubble_enabled") ?? true) && (
              <p className="mt-2 text-[13px] leading-[1.45] text-text-2">
                {t("settings.advanced.overlay.style.bubbleTakes")}
              </p>
            )}
          {/* Live's words come only from a model that writes as you speak. */}
          {selectedStyle === "live" && model && !model.supports_streaming && (
            <p className="mt-2 text-[13px] leading-[1.45] text-text-2">
              {t("settings.advanced.overlay.style.noStreaming", {
                model: model.name,
              })}
            </p>
          )}
        </SettingContainer>

        {selectedStyle !== "none" && (
          <SettingContainer
            title={t("settings.advanced.overlay.look.title")}
            description={t("settings.advanced.overlay.look.description")}
            descriptionMode={descriptionMode}
            grouped={grouped}
          >
            <Dropdown
              options={lookOptions}
              selectedValue={selectedLook}
              onSelect={(value) => updateSetting("bar_look", value as BarLook)}
              disabled={isUpdating("bar_look")}
            />
          </SettingContainer>
        )}

        {selectedStyle !== "none" && selectedLook === "island" && (
          <SettingContainer
            title={t("settings.advanced.overlay.idle.title")}
            description={t("settings.advanced.overlay.idle.description")}
            descriptionMode={descriptionMode}
            grouped={grouped}
          >
            <Dropdown
              options={idleOptions}
              selectedValue={selectedIdle}
              onSelect={(value) =>
                updateSetting("island_idle", value as IslandIdle)
              }
              disabled={isUpdating("island_idle")}
            />
          </SettingContainer>
        )}

        {selectedStyle !== "none" && selectedLook === "bar" && (
          <SettingContainer
            title={t("settings.advanced.overlay.position.title")}
            description={t("settings.advanced.overlay.position.description")}
            descriptionMode={descriptionMode}
            grouped={grouped}
          >
            <Dropdown
              options={positionOptions}
              selectedValue={selectedPosition}
              onSelect={(value) =>
                updateSetting("overlay_position", value as OverlayPosition)
              }
              disabled={isUpdating("overlay_position")}
            />
          </SettingContainer>
        )}

        {(selectedStyle !== "none" && selectedLook === "island") ||
        (getSetting("live_caption") ?? "off") !== "off" ? (
          <Slider
            value={getSetting("bar_opacity") ?? 0.8}
            onChange={(value) => updateSetting("bar_opacity", value)}
            min={0.3}
            max={1}
            step={0.05}
            label={t("settings.advanced.overlay.opacity.title")}
            description={t("settings.advanced.overlay.opacity.description")}
            descriptionMode={descriptionMode}
            grouped={grouped}
            formatValue={(value) => `${Math.round(value * 100)}%`}
          />
        ) : null}
      </>
    );
  },
);
