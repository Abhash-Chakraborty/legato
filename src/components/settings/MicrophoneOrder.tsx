import React from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";

interface MicrophoneOrderProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const iconButton =
  "grid h-7 w-7 place-items-center rounded-[var(--r-sm)] text-text-3 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 disabled:pointer-events-none disabled:opacity-40";

/** The microphones you've picked, best first: when the first isn't plugged
 *  in, the next one that is records. Shown once there are two. */
export const MicrophoneOrder: React.FC<MicrophoneOrderProps> = React.memo(
  ({ descriptionMode = "inline", grouped = false }) => {
    const { t } = useTranslation();
    const {
      getSetting,
      updateSetting,
      refreshSettings,
      isUpdating,
      audioDevices,
    } = useSettings();
    const order = getSetting("microphone_order") ?? [];
    if (order.length < 2) return null;

    const busy = isUpdating("microphone_order");
    const present = new Set(audioDevices.map((d) => d.name));
    const save = async (next: string[]) => {
      await updateSetting("microphone_order", next);
      // The first one is now your pick.
      await refreshSettings();
    };
    const move = (i: number, by: number) => {
      const next = [...order];
      [next[i], next[i + by]] = [next[i + by], next[i]];
      save(next);
    };

    return (
      <SettingContainer
        title={t("settings.sound.microphoneOrder.title")}
        description={t("settings.sound.microphoneOrder.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
      >
        <ol
          aria-label={t("settings.sound.microphoneOrder.label")}
          className="grid w-full max-w-[520px] overflow-hidden rounded-[var(--r-md)] border border-line"
        >
          {order.map((name, i) => (
            <li
              key={name}
              className="flex min-h-10 items-center gap-2.5 border-b border-line px-3 py-1.5 text-sm last:border-b-0"
            >
              <span className="w-4 flex-none text-xs tabular-nums text-text-3">
                {i + 1}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate">{name}</span>
                {!present.has(name) && (
                  <span className="block text-xs text-text-3">
                    {t("settings.sound.microphoneOrder.unplugged")}
                  </span>
                )}
              </span>
              <button
                type="button"
                disabled={busy || i === 0}
                onClick={() => move(i, -1)}
                aria-label={t("settings.sound.microphoneOrder.up", { name })}
                title={t("settings.sound.microphoneOrder.up", { name })}
                className={iconButton}
              >
                <ChevronUp className="h-4 w-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                disabled={busy || i === order.length - 1}
                onClick={() => move(i, 1)}
                aria-label={t("settings.sound.microphoneOrder.down", { name })}
                title={t("settings.sound.microphoneOrder.down", { name })}
                className={iconButton}
              >
                <ChevronDown className="h-4 w-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => save(order.filter((n) => n !== name))}
                aria-label={t("settings.sound.microphoneOrder.forget", {
                  name,
                })}
                title={t("settings.sound.microphoneOrder.forget", { name })}
                className={iconButton}
              >
                <X className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ol>
      </SettingContainer>
    );
  },
);
