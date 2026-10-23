import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { open } from "@tauri-apps/plugin-dialog";
import { commands, type FolderSuggestion } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";

/** A Word copy of every note in a folder, rewritten as notes change. In
 *  Google Drive's or OneDrive's folder the copies reach the cloud through
 *  that app, so Legato never signs in anywhere. */
export const NotesFolderRow: React.FC<{ grouped?: boolean }> = ({
  grouped = true,
}) => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const [suggestions, setSuggestions] = useState<FolderSuggestion[]>([]);
  const [busy, setBusy] = useState(false);
  const folder = settings?.notes_folder ?? null;

  useEffect(() => {
    commands
      .noteFolderSuggestions()
      .then(setSuggestions)
      .catch(() => {});
  }, []);

  const choose = async (next: string | null) => {
    setBusy(true);
    const result = await commands
      .changeNotesFolderSetting(next)
      .catch(() => null);
    setBusy(false);
    if (result?.status !== "ok") {
      toast.error(t("settings.connections.notesFolder.failed"), {
        description: result?.status === "error" ? result.error : undefined,
      });
      return;
    }
    await refreshSettings();
    if (next) toast.success(t("settings.connections.notesFolder.started"));
  };

  const pick = async () => {
    const picked = await open({ directory: true }).catch(() => null);
    if (typeof picked === "string") choose(picked);
  };

  return (
    <SettingContainer
      title={t("settings.connections.notesFolder.title")}
      description={t("settings.connections.notesFolder.help")}
      grouped={grouped}
      layout="stacked"
    >
      {folder ? (
        <div className="flex flex-wrap items-center gap-2">
          <div className="min-w-0 flex-[1_1_240px] cursor-text select-text truncate rounded-[var(--r-md)] border border-line bg-surface-2 px-3 py-2 font-mono text-xs text-text-2">
            {folder}
          </div>
          <Button
            variant="secondary"
            size="sm"
            onClick={() =>
              commands.openNotesFolder().then((r) => {
                if (r.status === "error")
                  toast.error(t("settings.connections.notesFolder.missing"));
              })
            }
          >
            {t("common.open")}
          </Button>
          <Button variant="secondary" size="sm" disabled={busy} onClick={pick}>
            {t("settings.connections.notesFolder.change")}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={busy}
            onClick={() => choose(null)}
          >
            {t("settings.connections.notesFolder.stop")}
          </Button>
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          {suggestions.map((s) => (
            <Button
              key={s.path}
              size="sm"
              disabled={busy}
              title={s.path}
              onClick={() => choose(s.path)}
            >
              {t("settings.connections.notesFolder.use", {
                service: s.service,
              })}
            </Button>
          ))}
          <Button
            variant={suggestions.length ? "secondary" : "primary"}
            size="sm"
            disabled={busy}
            onClick={pick}
          >
            {t("settings.connections.notesFolder.choose")}
          </Button>
          {!suggestions.length && (
            <span className="text-[13px] text-text-3">
              {t("settings.connections.notesFolder.noDrive")}
            </span>
          )}
        </div>
      )}
    </SettingContainer>
  );
};
