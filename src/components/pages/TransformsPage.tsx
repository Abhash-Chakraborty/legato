import React from "react";
import { useTranslation } from "react-i18next";
import { Page } from "../shell/Page";
import { SettingsGroup } from "../ui/SettingsGroup";
import { Button } from "../ui/Button";
import { ShortcutInput } from "../settings/ShortcutInput";
import { PostProcessingSettingsPrompts } from "../settings/PostProcessingSettingsPrompts";
import { useSettings } from "../../hooks/useSettings";
import { useNavStore } from "../../stores/navStore";
import { AiStatusLine } from "../shared/AiStatusLine";

/**
 * Prompts your AI provider runs on a dictation before it is pasted. Each
 * prompt becomes a transform in a later step; for now this is Handy's
 * post-processing, with its switch in the page header.
 */
export const TransformsPage: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const go = useNavStore((state) => state.go);
  const enabled = getSetting("post_process_enabled") ?? false;

  return (
    <Page
      title={t("transforms.title")}
      subtitle={t("transforms.subtitle")}
      actions={
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          aria-label={t("transforms.switch")}
          title={t("transforms.switch")}
          className="toggle"
          onClick={() => {
            if (!isUpdating("post_process_enabled"))
              updateSetting("post_process_enabled", !enabled);
          }}
        />
      }
    >
      {!enabled && (
        <p className="mb-6 rounded-[var(--r-md)] bg-surface-2 px-4 py-3 text-sm text-text-2">
          {t("transforms.off")}
        </p>
      )}

      <SettingsGroup title={t("transforms.groups.provider")}>
        <div className="flex min-h-[60px] items-center justify-between gap-8 py-3.5">
          <AiStatusLine />
          <Button variant="secondary" onClick={() => go("settings", "ai")}>
            {t("transforms.changeProvider")}
          </Button>
        </div>
      </SettingsGroup>

      <SettingsGroup title={t("transforms.groups.shortcut")}>
        <ShortcutInput shortcutId="transcribe_with_post_process" grouped />
      </SettingsGroup>

      <SettingsGroup title={t("transforms.groups.prompts")}>
        <PostProcessingSettingsPrompts />
      </SettingsGroup>
    </Page>
  );
};
