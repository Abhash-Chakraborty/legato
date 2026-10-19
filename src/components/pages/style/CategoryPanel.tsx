import React, { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pencil } from "lucide-react";
import {
  commands,
  type AfterPaste,
  type AppCategory,
  type CategoryStyle,
  type HistoryEntry,
  type PasteMethod,
  type StyleApp,
  type Tone,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useModelStore } from "../../../stores/modelStore";
import { useNavStore } from "../../../stores/navStore";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { Segmented } from "../../ui/Segmented";
import { SettingContainer } from "../../ui/SettingContainer";
import { AppIcon } from "../home/AppIcon";
import { relativeTime } from "../dictionary/list";
import { PASTE_METHODS, tonesFor } from "./model";

const INHERIT = "inherit";
const AFTER: AfterPaste[] = ["nothing", "enter", "ctrl_enter"];
const MAX_ICONS = 6;

/** Runs the preview for one text, tone and kind of app. */
const usePreview = (
  text: string,
  category: AppCategory,
  tone: Tone | null,
  deps: string,
) => {
  const [out, setOut] = useState<string | null>(null);
  useEffect(() => {
    if (!text.trim()) {
      setOut("");
      return;
    }
    let live = true;
    const timer = setTimeout(() => {
      commands
        .previewStyle(text, category, tone, null)
        .then((r) => live && setOut(r))
        .catch(() => {});
    }, 120);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [text, category, tone, deps]);
  return out;
};

const ToneRow: React.FC<{
  category: AppCategory;
  tone: Tone;
  checked: boolean;
  sample: string;
  deps: string;
  disabled: boolean;
  onSelect: () => void;
}> = ({ category, tone, checked, sample, deps, disabled, onSelect }) => {
  const { t } = useTranslation();
  const example = usePreview(sample, category, tone, deps);
  return (
    <label
      className={`flex cursor-pointer items-start gap-3 border-b border-line px-4 py-3.5 transition-colors duration-[var(--dur-1)] last:border-b-0 hover:duration-0 ${
        checked ? "bg-teal-50" : "hover:bg-surface-2"
      } ${disabled ? "pointer-events-none" : ""}`}
    >
      <input
        type="radio"
        name={`tone-${category}`}
        className="radio mt-0.5"
        checked={checked}
        disabled={disabled}
        onChange={onSelect}
      />
      <span className="w-[200px] flex-none">
        <span className="block text-sm font-medium">
          {t(`style.tones.${tone}.label`)}
        </span>
        <span className="mt-0.5 block text-[13px] text-text-3">
          {t(`style.tones.${tone}.help`)}
        </span>
      </span>
      <span className="min-w-0 flex-1 whitespace-pre-wrap break-words text-sm leading-relaxed text-text-2">
        {example}
      </span>
    </label>
  );
};

interface CategoryPanelProps {
  category: AppCategory;
  style: CategoryStyle;
  apps: StyleApp[];
  latest: HistoryEntry | null;
  disabled: boolean;
  onChange: (style: CategoryStyle) => void;
  onEditApps: () => void;
}

/** One kind of app: its apps, tone, a try-it box and paste settings. */
export const CategoryPanel: React.FC<CategoryPanelProps> = ({
  category,
  style,
  apps,
  latest,
  disabled,
  onChange,
  onEditApps,
}) => {
  const { t, i18n } = useTranslation();
  const { settings } = useSettings();
  const go = useNavStore((s) => s.go);
  const tryId = useId();
  const seed = latest?.heard_text ?? latest?.transcription_text ?? "";
  const [text, setText] = useState(seed);
  const [edited, setEdited] = useState(false);
  const deps = JSON.stringify([
    settings?.cleanup_level,
    settings?.cleanup_rules,
    settings?.custom_filler_words,
    settings?.filler_word_removal_enabled,
  ]);
  const tryOut = usePreview(text, category, style.tone, deps);
  const now = Math.floor(Date.now() / 1000);

  // A new dictation in this kind of app replaces the text until it's edited.
  useEffect(() => {
    if (!edited) setText(seed);
  }, [seed, edited]);

  const mine = apps.filter((a) => a.category === category);
  const level = settings?.cleanup_level ?? "light";

  const globalMethod = settings?.paste_method ?? "ctrl_v";
  const globalAfter: AfterPaste = !settings?.auto_submit
    ? "nothing"
    : settings.auto_submit_key === "enter"
      ? "enter"
      : "ctrl_enter";
  const globalSpace = settings?.append_trailing_space ?? false;

  const models = useModelStore((s) => s.models);
  const globalModel = settings?.selected_model ?? "";
  const modelName = (id: string) => models.find((m) => m.id === id)?.name ?? id;
  // Downloaded models, and the chosen one even if it was deleted since.
  const pickable = models.filter(
    (m) =>
      m.id !== globalModel && (m.is_downloaded || m.id === style.speech_model),
  );

  return (
    <div className={disabled ? "opacity-55" : ""} aria-disabled={disabled}>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          {mine.length > 0 && (
            <span className="flex items-center gap-1.5" aria-hidden="true">
              {mine.slice(0, MAX_ICONS).map((a) => (
                <AppIcon
                  key={`${a.rule.kind}:${a.rule.pattern}`}
                  path={a.exe_path || null}
                  className="h-5 w-5"
                />
              ))}
            </span>
          )}
          <span className="truncate text-[13px] text-text-3">
            {mine.length === 0
              ? t("style.apps.none")
              : t("style.apps.count", {
                  count: mine.length,
                  names: mine
                    .slice(0, 3)
                    .map((a) => a.name)
                    .join(", "),
                })}
          </span>
        </div>
        <Button
          variant="secondary"
          size="sm"
          onClick={onEditApps}
          disabled={disabled}
        >
          <Pencil className="h-3.5 w-3.5" aria-hidden="true" />
          {t("style.apps.edit")}
        </Button>
      </div>

      <div className="mb-2 flex items-baseline justify-between gap-3">
        <h3 className="text-[15px] font-semibold">{t("style.tone.title")}</h3>
        <span className="text-xs text-text-3">{t("style.tone.note")}</span>
      </div>
      <div
        role="radiogroup"
        aria-label={t("style.tone.title")}
        className="mb-8 overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface"
      >
        {tonesFor(category).map((tone) => (
          <ToneRow
            key={tone}
            category={category}
            tone={tone}
            checked={style.tone === tone}
            sample={t(`style.samples.${category}`)}
            deps={deps}
            disabled={disabled}
            onSelect={() => onChange({ ...style, tone })}
          />
        ))}
      </div>

      <div className="mb-2 flex flex-wrap items-baseline justify-between gap-3">
        <h3 className="text-[15px] font-semibold">{t("style.try.title")}</h3>
        {latest && !edited && (
          <span className="text-xs text-text-3">
            {latest.app_name
              ? t("style.try.from", {
                  app: latest.app_site ?? latest.app_name,
                  when: relativeTime(latest.timestamp, now, i18n.language),
                })
              : t("style.try.fromLast", {
                  when: relativeTime(latest.timestamp, now, i18n.language),
                })}
          </span>
        )}
      </div>
      <div className="mb-1.5 grid gap-4 min-[760px]:grid-cols-2">
        <div>
          <label
            htmlFor={tryId}
            className="mb-1.5 block text-[13px] text-text-2"
          >
            {t("style.try.youSay")}
          </label>
          <textarea
            id={tryId}
            className="input min-h-[96px] w-full resize-y py-2 leading-relaxed"
            value={text}
            disabled={disabled}
            onChange={(e) => {
              setText(e.target.value);
              setEdited(true);
            }}
            placeholder={t("style.try.placeholder")}
            spellCheck={false}
          />
        </div>
        <div>
          <span className="mb-1.5 block text-[13px] text-text-2">
            {t("style.try.pasted")}
          </span>
          <output
            htmlFor={tryId}
            aria-live="polite"
            className="block min-h-[96px] whitespace-pre-wrap break-words rounded-[var(--r-md)] bg-surface-2 px-3 py-2 text-sm leading-relaxed"
          >
            {tryOut}
          </output>
        </div>
      </div>
      <p className="mb-8 text-xs text-text-3">
        {t("style.try.settings", {
          level: t(`style.cleanup.levels.${level}.label`),
          tone: t(`style.tones.${style.tone}.label`),
        })}
      </p>

      <div className="mb-2 flex flex-wrap items-baseline justify-between gap-3">
        <h3 className="text-[15px] font-semibold">{t("style.paste.title")}</h3>
        <span className="text-xs text-text-3">
          {t("style.paste.inheritBefore")}
          <button
            type="button"
            className="link"
            onClick={() => go("settings", "output")}
          >
            {t("style.paste.inheritLink")}
          </button>
        </span>
      </div>
      <div className="rounded-[var(--r-lg)] border border-line bg-surface px-4">
        <SettingContainer
          title={t("style.paste.method.title")}
          description={t("style.paste.method.help")}
          disabled={disabled}
        >
          <Dropdown
            className="w-[240px]"
            ariaLabel={t("style.paste.method.title")}
            disabled={disabled}
            selectedValue={style.paste_method ?? INHERIT}
            onSelect={(v) =>
              onChange({
                ...style,
                paste_method: v === INHERIT ? null : (v as PasteMethod),
              })
            }
            options={[
              {
                value: INHERIT,
                label: t("style.paste.inherit", {
                  value: t(`style.paste.methods.${globalMethod}`),
                }),
              },
              ...PASTE_METHODS.map((m) => ({
                value: m,
                label: t(`style.paste.methods.${m}`),
              })),
            ]}
          />
        </SettingContainer>
        <div className="border-t border-line">
          <SettingContainer
            title={t("style.paste.after.title")}
            description={t("style.paste.after.help")}
            disabled={disabled}
          >
            <Dropdown
              className="w-[240px]"
              ariaLabel={t("style.paste.after.title")}
              disabled={disabled}
              selectedValue={style.after_paste ?? INHERIT}
              onSelect={(v) =>
                onChange({
                  ...style,
                  after_paste: v === INHERIT ? null : (v as AfterPaste),
                })
              }
              options={[
                {
                  value: INHERIT,
                  label: t("style.paste.inherit", {
                    value: t(`style.paste.afterOptions.${globalAfter}`),
                  }),
                },
                ...AFTER.map((a) => ({
                  value: a,
                  label: t(`style.paste.afterOptions.${a}`),
                })),
              ]}
            />
          </SettingContainer>
        </div>
        <div className="border-t border-line">
          <SettingContainer
            title={t("style.paste.space.title")}
            description={t("style.paste.space.help", {
              value: globalSpace
                ? t("style.paste.space.on")
                : t("style.paste.space.off"),
            })}
            disabled={disabled}
          >
            <Segmented
              label={t("style.paste.space.title")}
              value={
                style.trailing_space === null ||
                style.trailing_space === undefined
                  ? INHERIT
                  : style.trailing_space
                    ? "on"
                    : "off"
              }
              onChange={(v) =>
                onChange({
                  ...style,
                  trailing_space: v === INHERIT ? null : v === "on",
                })
              }
              options={[
                { value: INHERIT, label: t("style.paste.space.inherit") },
                { value: "on", label: t("style.paste.space.on") },
                { value: "off", label: t("style.paste.space.off") },
              ]}
            />
          </SettingContainer>
        </div>
      </div>

      <div className="mb-2 mt-8 flex flex-wrap items-baseline justify-between gap-3">
        <h3 className="text-[15px] font-semibold">{t("style.model.title")}</h3>
        <span className="text-xs text-text-3">
          {t("style.paste.inheritBefore")}
          <button type="button" className="link" onClick={() => go("models")}>
            {t("style.model.inheritLink")}
          </button>
        </span>
      </div>
      <div className="rounded-[var(--r-lg)] border border-line bg-surface px-4">
        <SettingContainer
          title={t("style.model.pick.title")}
          description={
            pickable.length
              ? t("style.model.pick.help")
              : t("style.model.pick.none")
          }
          disabled={disabled}
        >
          <Dropdown
            className="w-[240px]"
            ariaLabel={t("style.model.pick.title")}
            disabled={disabled || !pickable.length}
            selectedValue={style.speech_model ?? INHERIT}
            onSelect={(v) =>
              onChange({ ...style, speech_model: v === INHERIT ? null : v })
            }
            options={[
              {
                value: INHERIT,
                label: t("style.paste.inherit", {
                  value: modelName(globalModel),
                }),
              },
              ...pickable.map((m) => ({
                value: m.id,
                label: m.is_downloaded
                  ? m.name
                  : t("style.model.pick.deleted", { name: m.name }),
              })),
            ]}
          />
        </SettingContainer>
      </div>
    </div>
  );
};
