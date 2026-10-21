import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands, type Voice } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useSpeakerModel } from "../../../hooks/useSpeakerModel";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

const megabytes = (bytes: number) => Math.round(bytes / 1_000_000);

/** Whether Notes tells voices apart, once the speaker model is here. */
export const SpeakersToggleRow: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  return (
    <ToggleSwitch
      checked={settings?.notes_speakers ?? true}
      onChange={(on) => updateSetting("notes_speakers", on)}
      isUpdating={isUpdating("notes_speakers")}
      label={t("settings.speakers.toggle.title")}
      description={t("settings.speakers.toggle.help")}
      descriptionMode="inline"
      grouped
    />
  );
};

/** The speaker model: downloaded on request, with progress and Cancel, and
 *  removed again with a second click. */
export const SpeakerModelRow: React.FC = () => {
  const { t } = useTranslation();
  const model = useSpeakerModel();
  const [confirm, setConfirm] = useState(false);
  if (!model) return null;
  const size = megabytes(model.size);
  const percent = model.size
    ? Math.min(100, Math.round((model.done / model.size) * 100))
    : 0;

  const download = async () => {
    const result = await commands.downloadSpeakerModel();
    if (result.status === "error")
      toast.error(t("settings.speakers.model.failed"), {
        description: result.error,
      });
  };

  const remove = async () => {
    setConfirm(false);
    const result = await commands.deleteSpeakerModel();
    if (result.status === "error")
      toast.error(t("settings.speakers.model.deleteFailed"), {
        description: result.error,
      });
  };

  const description = model.downloading ? (
    t("settings.speakers.model.downloading", {
      done: megabytes(model.done),
      size,
    })
  ) : model.downloaded ? (
    t("settings.speakers.model.ready", { size })
  ) : model.error ? (
    <span className="text-danger">
      {t("settings.speakers.model.error", { error: model.error })}
    </span>
  ) : (
    t("settings.speakers.model.help", { size })
  );

  return (
    <SettingContainer
      title={t("settings.speakers.model.title")}
      description={
        <>
          {description}{" "}
          <span className="text-text-3">
            {t("settings.speakers.model.licence")}
          </span>
        </>
      }
      descriptionMode="inline"
      grouped
    >
      {model.downloading ? (
        <div className="flex items-center gap-3">
          <div
            role="progressbar"
            aria-label={t("settings.speakers.model.title")}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent}
            className="h-1.5 w-28 overflow-hidden rounded-full bg-surface-3"
          >
            <div
              className="h-full rounded-full bg-teal-fill transition-[width] duration-[var(--dur-2)] ease-out"
              style={{ width: `${percent}%` }}
            />
          </div>
          <span className="w-9 text-end text-xs tabular-nums text-text-2">
            {percent}%
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => commands.cancelSpeakerModelDownload()}
          >
            {t("settings.speakers.model.cancel")}
          </Button>
        </div>
      ) : model.downloaded ? (
        confirm ? (
          <div className="flex items-center gap-2">
            <Button variant="danger" size="sm" onClick={remove}>
              {t("settings.speakers.model.confirmDelete")}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setConfirm(false)}>
              {t("settings.speakers.model.keep")}
            </Button>
          </div>
        ) : (
          <Button variant="ghost" size="sm" onClick={() => setConfirm(true)}>
            {t("settings.speakers.model.delete")}
          </Button>
        )
      ) : (
        <Button variant="secondary" size="sm" onClick={download}>
          {model.error
            ? t("settings.speakers.model.retry")
            : t("settings.speakers.model.download", { size })}
        </Button>
      )}
    </SettingContainer>
  );
};

/** Whether naming a speaker keeps a clip of their voice. */
export const RememberVoicesRow: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  return (
    <ToggleSwitch
      checked={settings?.notes_remember_voices ?? true}
      onChange={(on) => updateSetting("notes_remember_voices", on)}
      isUpdating={isUpdating("notes_remember_voices")}
      label={t("settings.speakers.remember.title")}
      description={t("settings.speakers.remember.help")}
      descriptionMode="inline"
      grouped
    />
  );
};

/** The voices saved so far, each forgotten with a second click. */
export const SavedVoicesRow: React.FC = () => {
  const { t, i18n } = useTranslation();
  const [voices, setVoices] = useState<Voice[] | null>(null);
  const [confirm, setConfirm] = useState<number | null>(null);

  const load = () =>
    commands
      .listVoices()
      .then((r) => r.status === "ok" && setVoices(r.data))
      .catch(() => {});
  useEffect(() => {
    load();
  }, []);

  const forget = async (voice: Voice) => {
    setConfirm(null);
    const result = await commands.forgetVoice(voice.id);
    if (result.status === "error") {
      toast.error(t("settings.speakers.voices.forgetFailed"), {
        description: result.error,
      });
      return;
    }
    toast(t("settings.speakers.voices.forgotten", { name: voice.name }));
    load();
  };

  const count = voices?.length ?? 0;
  return (
    <div>
      <SettingContainer
        title={t("settings.speakers.voices.title")}
        description={
          count
            ? t("settings.speakers.voices.help")
            : t("settings.speakers.voices.none")
        }
        descriptionMode="inline"
        grouped
      >
        <span className="text-[13px] tabular-nums text-text-2">
          {t("settings.speakers.voices.count", { count })}
        </span>
      </SettingContainer>
      {count > 0 && (
        <ul className="ms-6 border-s border-line">
          {voices?.map((voice) => (
            <li
              key={voice.id}
              className="flex items-center gap-4 border-b border-line py-2.5 ps-4 last:border-b-0"
            >
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium">{voice.name}</div>
                <div className="mt-0.5 text-[13px] text-text-3">
                  {t("settings.speakers.voices.heard", {
                    seconds: Math.round(voice.ms / 1000),
                    date: new Date(voice.used_at * 1000).toLocaleDateString(
                      i18n.language,
                      { month: "short", day: "numeric" },
                    ),
                  })}
                </div>
              </div>
              {confirm === voice.id ? (
                <div className="flex items-center gap-2">
                  <Button
                    variant="danger"
                    size="sm"
                    onClick={() => forget(voice)}
                  >
                    {t("settings.speakers.voices.confirm")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setConfirm(null)}
                  >
                    {t("settings.speakers.model.keep")}
                  </Button>
                </div>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setConfirm(voice.id)}
                  aria-label={t("settings.speakers.voices.forgetLabel", {
                    name: voice.name,
                  })}
                >
                  {t("settings.speakers.voices.forget")}
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};
