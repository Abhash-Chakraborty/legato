import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import {
  AudioLines,
  BookA,
  ClipboardPaste,
  Eraser,
  Replace,
  type LucideIcon,
} from "lucide-react";
import { commands, type TodayFix } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useSettingsStore } from "../../../stores/settingsStore";
import { useModelStore } from "../../../stores/modelStore";
import { useModelStatusStore } from "../../../stores/modelStatusStore";
import {
  useDictionaryStore,
  watchDictionary,
} from "../../../stores/dictionaryStore";
import {
  useNavStore,
  type Page,
  type SettingsSection,
} from "../../../stores/navStore";
import { getLanguageLabel } from "../../../lib/constants/languages";
import { formatKeyCombination } from "../../../lib/utils/keyboard";
import { getTranslatedModelName } from "../../../lib/utils/modelTranslation";

const BARS = 6;

/** Six bars that follow the microphone while Legato listens. Levels only
 *  arrive during a dictation; the meter rests flat otherwise. */
const LevelMeter: React.FC = () => {
  const [level, setLevel] = useState(0);

  useEffect(() => {
    commands.watchMicLevel(true).catch(() => {});
    let decay: ReturnType<typeof setTimeout> | undefined;
    const unlisten = listen<number[]>("mic-level", ({ payload }) => {
      const peak = Math.max(0, ...payload.slice(0, 8));
      setLevel(Math.min(1, peak));
      clearTimeout(decay);
      decay = setTimeout(() => setLevel(0), 300);
    });
    return () => {
      clearTimeout(decay);
      commands.watchMicLevel(false).catch(() => {});
      unlisten.then((fn) => fn());
    };
  }, []);

  const lit = Math.round(level * BARS);
  return (
    <span className="flex h-3.5 items-end gap-[3px]" aria-hidden="true">
      {Array.from({ length: BARS }, (_, i) => (
        <span
          key={i}
          className={`w-[3px] rounded-full transition-colors duration-[var(--dur-1)] ${i < lit ? "bg-teal-600" : "bg-line-strong"}`}
          style={{ height: `${6 + i * 1.6}px` }}
        />
      ))}
    </span>
  );
};

const Fact: React.FC<{
  label: string;
  value: React.ReactNode;
  detail?: React.ReactNode;
  aside?: React.ReactNode;
}> = ({ label, value, detail, aside }) => (
  <div className="flex items-center gap-3 px-3.5 py-2.5">
    <div className="min-w-0 flex-1">
      <div className="text-xs text-text-3">{label}</div>
      <div className="truncate text-sm font-medium text-text">{value}</div>
      {detail && <div className="truncate text-xs text-text-3">{detail}</div>}
    </div>
    {aside}
  </div>
);

const Step: React.FC<{
  icon: LucideIcon;
  title: string;
  detail: string;
  onClick: () => void;
}> = ({ icon: Icon, title, detail, onClick }) => (
  <li>
    <button
      type="button"
      onClick={onClick}
      className="group flex w-full items-center gap-3 rounded-[var(--r-sm)] px-1.5 py-1.5 text-start transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0"
    >
      <span className="grid h-7 w-7 flex-none place-items-center rounded-full border border-line text-text-2 group-hover:text-text">
        <Icon className="h-3.5 w-3.5 stroke-[1.75]" aria-hidden="true" />
      </span>
      <span className="min-w-0">
        <span className="block text-sm text-text">{title}</span>
        <span className="block truncate text-xs text-text-3">{detail}</span>
      </span>
    </button>
  </li>
);

/** Home's right rail: what is set up right now, the steps a dictation goes
 *  through (each a link to where it is changed), and today's fixes. */
export const HomeRail: React.FC<{ fixedToday: TodayFix[] }> = ({
  fixedToday,
}) => {
  const { t } = useTranslation();
  const dictionary = useDictionaryStore((s) => s.view);
  useEffect(() => watchDictionary(), []);
  const { settings } = useSettings();
  const audioDevices = useSettingsStore((s) => s.audioDevices);
  const { models, currentModel } = useModelStore();
  const status = useModelStatusStore((s) => s.status);
  const go = useNavStore((s) => s.go);
  const open = (page: Page, section?: SettingsSection) => () =>
    go(page, section);

  const model = models.find((m) => m.id === currentModel);
  const modelName = model
    ? getTranslatedModelName(model, t)
    : t("home.rail.noModel");
  const modelState =
    status === "ready"
      ? t("home.rail.modelLoaded")
      : status === "loading"
        ? t("home.rail.modelLoading")
        : status === "error"
          ? t("home.rail.modelError")
          : t("home.rail.modelIdle");

  const selectedMic = settings?.selected_microphone;
  const micName =
    !selectedMic || selectedMic === "default"
      ? (audioDevices.find((d) => d.is_default)?.name ??
        t("home.rail.defaultMic"))
      : selectedMic;

  const language = settings?.selected_language ?? "auto";
  const languageName =
    language === "auto"
      ? t("home.rail.autoLanguage")
      : (getLanguageLabel(language) ?? language);

  const providerId = settings?.post_process_provider_id ?? "";
  const provider = settings?.post_process_providers?.find(
    (p) => p.id === providerId,
  );
  const aiModel = settings?.post_process_models?.[providerId] ?? "";
  const aiOn = settings?.post_process_enabled ?? false;

  const binding = settings?.bindings?.transcribe?.current_binding ?? "";
  const keys = formatKeyCombination(binding).split(" + ").filter(Boolean);
  const activation = settings?.shortcut_activation ?? "hold_or_toggle";

  const prompt = settings?.post_process_prompts?.find(
    (p) => p.id === settings?.post_process_selected_prompt_id,
  );
  const words = dictionary?.entries.length ?? 0;
  const pasteMethod = settings?.paste_method ?? "ctrl_v";
  const pasteLabel = t(
    {
      ctrl_v: "settings.advanced.pasteMethod.options.clipboard",
      ctrl_shift_v: "settings.advanced.pasteMethod.options.clipboardCtrlShiftV",
      shift_insert:
        "settings.advanced.pasteMethod.options.clipboardShiftInsert",
      direct: "settings.advanced.pasteMethod.options.direct",
      none: "settings.advanced.pasteMethod.options.none",
    }[pasteMethod],
  );

  return (
    <aside aria-label={t("home.rail.label")} className="flex flex-col gap-7">
      <section>
        <h2 className="mb-2.5 text-sm font-semibold">{t("home.rail.now")}</h2>
        <div className="divide-y divide-line overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface">
          <Fact
            label={t("home.rail.model")}
            value={modelName}
            detail={modelState}
          />
          <Fact
            label={t("home.rail.microphone")}
            value={micName}
            aside={<LevelMeter />}
          />
          <Fact label={t("home.rail.language")} value={languageName} />
          <Fact
            label={t("home.rail.ai")}
            value={
              aiOn && provider ? (
                <span className="inline-flex items-center gap-1.5">
                  <span
                    aria-hidden="true"
                    className="h-1.5 w-1.5 rounded-full bg-ok"
                  />
                  {aiModel
                    ? t("home.rail.aiModel", {
                        provider: provider.label,
                        model: aiModel,
                      })
                    : provider.label}
                </span>
              ) : (
                t("home.rail.aiOff")
              )
            }
          />
          <Fact
            label={t("home.rail.shortcut")}
            value={
              keys.length ? (
                <span className="mt-0.5 inline-flex flex-wrap gap-1">
                  {keys.map((key) => (
                    <kbd key={key} className="kbd">
                      {key}
                    </kbd>
                  ))}
                </span>
              ) : (
                t("home.rail.noShortcut")
              )
            }
            detail={t(`home.rail.activation.${activation}`)}
          />
        </div>
      </section>

      <section>
        <h2 className="mb-2 text-sm font-semibold">
          {t("home.rail.pipeline")}
        </h2>
        <ol className="flex flex-col">
          <Step
            icon={AudioLines}
            title={t("home.rail.steps.transcribe")}
            detail={modelName}
            onClick={open("models")}
          />
          <Step
            icon={BookA}
            title={t("home.rail.steps.dictionary")}
            detail={t("home.rail.steps.dictionaryDetail", { count: words })}
            onClick={open("dictionary")}
          />
          <Step
            icon={Eraser}
            title={t("home.rail.steps.cleanup")}
            detail={
              settings?.filler_word_removal_enabled
                ? t("home.rail.steps.cleanupOn")
                : t("home.rail.steps.off")
            }
            onClick={open("settings", "transcription")}
          />
          <Step
            icon={Replace}
            title={t("home.rail.steps.transform")}
            detail={
              settings?.post_process_enabled
                ? prompt
                  ? t("home.rail.steps.transformWith", { name: prompt.name })
                  : t("home.rail.steps.transformOn")
                : t("home.rail.steps.off")
            }
            onClick={open("transforms")}
          />
          <Step
            icon={ClipboardPaste}
            title={t("home.rail.steps.paste")}
            detail={pasteLabel}
            onClick={open("settings", "output")}
          />
        </ol>
      </section>

      {fixedToday.length > 0 && (
        <section>
          <h2 className="mb-2 text-sm font-semibold">
            {t("home.rail.fixedToday", { count: fixedToday.length })}
          </h2>
          <ul className="flex flex-col gap-1.5 text-[13px]">
            {fixedToday.slice(0, 8).map((fix, i) => (
              <li
                key={`${fix.history_id}-${i}`}
                className="flex min-w-0 items-baseline gap-1.5"
              >
                <span className="truncate text-text-3 line-through decoration-text-4">
                  {fix.from}
                </span>
                <span aria-hidden="true" className="text-text-4">
                  →
                </span>
                <span className="sr-only">{t("home.rail.fixedTo")}</span>
                <span className="fixed-word truncate font-medium text-text">
                  {fix.to}
                </span>
                {fix.app && (
                  <span className="ms-auto flex-none truncate text-xs text-text-3">
                    {fix.app}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
    </aside>
  );
};
