import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, History } from "lucide-react";
import { toast } from "sonner";
import {
  commands,
  type AppCategory,
  type AppRule,
  type CategoryStyle,
  type StyleApp,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useSettingsStore } from "../../../stores/settingsStore";
import { AppIcon } from "../home/AppIcon";
import { relativeTime } from "../dictionary/list";
import { CategoryPanel } from "./CategoryPanel";
import { CleanupCard } from "./CleanupCard";
import { EditApps } from "./EditApps";
import { asCategory, CATEGORIES, styleOf } from "./model";
import { useRecentDictations } from "./useRecent";

/** Saves one kind of app's style, showing it at once and rolling back if
 *  the backend refuses. */
const saveStyle = async (category: AppCategory, style: CategoryStyle) => {
  const store = useSettingsStore.getState();
  const before = store.settings;
  if (!before) return;
  const styles = { ...styleMap(before.styles), [category]: style };
  useSettingsStore.setState({ settings: { ...before, styles } });
  const result = await commands.changeCategoryStyleSetting(category, style);
  if (result.status === "error") {
    toast.error(result.error);
    useSettingsStore.setState({ settings: before });
  } else {
    useSettingsStore.setState({ savedAt: Date.now() });
  }
};

const styleMap = (styles: Parameters<typeof styleOf>[0]) =>
  Object.fromEntries(CATEGORIES.map((c) => [c, styleOf(styles, c)])) as Record<
    AppCategory,
    CategoryStyle
  >;

/** Formatting per kind of app: cleanup for every dictation, then a tone and
 *  paste settings for each kind, picked from the app you dictate into. */
export const StylePage: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const recent = useRecentDictations();
  const [apps, setApps] = useState<StyleApp[]>([]);
  const [chosen, setChosen] = useState<AppCategory | null>(null);
  const [editing, setEditing] = useState(false);
  const [explain, setExplain] = useState(false);

  const loadApps = useCallback(async () => {
    const result = await commands.getStyleApps().catch(() => null);
    if (result?.status === "ok") setApps(result.data);
  }, []);

  useEffect(() => {
    loadApps();
  }, [loadApps, recent]);

  const latest = recent[0] ?? null;
  const tab = chosen ?? (latest ? asCategory(latest.category) : "personal");
  const latestIn = useMemo(
    () => recent.find((e) => asCategory(e.category) === tab) ?? null,
    [recent, tab],
  );
  const enabled = settings?.style_enabled ?? true;
  const now = Math.floor(Date.now() / 1000);

  const onRules = (rules: AppRule[]) => {
    const state = useSettingsStore.getState();
    if (state.settings)
      useSettingsStore.setState({
        settings: { ...state.settings, app_rules: rules },
        savedAt: Date.now(),
      });
    loadApps();
  };

  const onTabKey = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const i = CATEGORIES.indexOf(tab);
    const next = CATEGORIES[(i + step + CATEGORIES.length) % CATEGORIES.length];
    setChosen(next);
    requestAnimationFrame(() =>
      document.getElementById(`style-tab-${next}`)?.focus(),
    );
  };

  if (!settings) return null;
  const latestCategory = latest ? asCategory(latest.category) : null;

  return (
    <div className="mx-auto w-full max-w-[1120px]">
      <div className="mb-6 flex min-h-10 flex-wrap items-center justify-between gap-4">
        <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
          {t("style.title")}
        </h1>
        <label className="flex cursor-pointer items-center gap-3 text-sm text-text-2">
          {enabled ? t("style.on") : t("style.off")}
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            aria-label={t("style.toggle")}
            className="toggle"
            onClick={() => updateSetting("style_enabled", !enabled)}
          />
        </label>
      </div>
      <p className="-mt-3.5 mb-6 max-w-[640px] text-sm text-text-2">
        {t("style.subtitle")}
      </p>

      {latest && latestCategory && (
        <div className="mb-6 rounded-[var(--r-lg)] bg-surface-2 px-4 py-2.5 text-[13px]">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <History className="h-4 w-4 text-text-3" aria-hidden="true" />
            <span className="text-text-3">{t("style.last.label")}</span>
            {latest.app_name && (
              <span className="inline-flex items-center gap-1.5 font-medium">
                <AppIcon path={latest.app_path} />
                {latest.app_site ?? latest.app_name}
              </span>
            )}
            <ArrowRight
              className="h-3.5 w-3.5 text-text-4"
              aria-hidden="true"
            />
            <button
              type="button"
              className="link"
              onClick={() => setChosen(latestCategory)}
            >
              {t(`style.categories.${latestCategory}`)}
            </button>
            <span className="text-text-3">
              {enabled
                ? t("style.last.tone", {
                    tone: t(
                      `style.tones.${styleOf(settings.styles, latestCategory).tone}.label`,
                    ),
                    when: relativeTime(latest.timestamp, now, i18n.language),
                  })
                : relativeTime(latest.timestamp, now, i18n.language)}
            </span>
            <button
              type="button"
              className="link ms-auto"
              aria-expanded={explain}
              onClick={() => setExplain(!explain)}
            >
              {t("style.last.how")}
            </button>
          </div>
          {explain && (
            <p className="mt-2 max-w-[720px] text-text-2">
              {t("style.last.howBody")}
            </p>
          )}
        </div>
      )}

      <CleanupCard latest={recent.find((e) => e.heard_text) ?? null} />

      <h2 className="mb-3 text-[17px] font-semibold">
        {t("style.perKind.title")}
      </h2>
      {!enabled && (
        <p className="mb-4 rounded-[var(--r-md)] bg-surface-2 px-4 py-2.5 text-[13px] text-text-2">
          {t("style.perKind.off")}
        </p>
      )}
      <div
        role="tablist"
        aria-label={t("style.perKind.title")}
        onKeyDown={onTabKey}
        className="mb-5 flex gap-5 overflow-x-auto border-b border-line"
      >
        {CATEGORIES.map((c) => (
          <button
            key={c}
            id={`style-tab-${c}`}
            type="button"
            role="tab"
            aria-selected={tab === c}
            aria-controls="style-panel"
            tabIndex={tab === c ? 0 : -1}
            onClick={() => setChosen(c)}
            className={`-mb-px flex-none border-b-2 pb-2.5 text-sm whitespace-nowrap transition-colors duration-[var(--dur-1)] hover:duration-0 ${
              tab === c
                ? "border-teal-600 font-medium text-text"
                : "border-transparent text-text-2 hover:text-text"
            }`}
          >
            {t(`style.categories.${c}`)}
          </button>
        ))}
      </div>
      <div
        id="style-panel"
        role="tabpanel"
        aria-labelledby={`style-tab-${tab}`}
        className="pb-6"
      >
        <CategoryPanel
          key={tab}
          category={tab}
          style={styleOf(settings.styles, tab)}
          apps={apps}
          latest={latestIn}
          disabled={!enabled}
          onChange={(style) => saveStyle(tab, style)}
          onEditApps={() => setEditing(true)}
        />
      </div>

      {editing && (
        <EditApps
          category={tab}
          apps={apps}
          onClose={() => setEditing(false)}
          onRules={onRules}
        />
      )}
    </div>
  );
};
