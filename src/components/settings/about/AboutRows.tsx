import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { emit } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import { SettingContainer } from "../../ui/SettingContainer";
import { Button } from "../../ui/Button";
import { useSettings } from "../../../hooks/useSettings";
import { useUpdateStore } from "../../../stores/updateStore";

const SOURCE_URL = "https://github.com/Abhash-Chakraborty/legato";

/** The installed version, what the updater is doing, and a button to check
 *  now or install what it found. */
export const VersionRow: React.FC = () => {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");
  const { settings, updateChecksLocked } = useSettings();
  const status = useUpdateStore((state) => state.status);
  const progress = useUpdateStore((state) => state.progress);
  const checksOn =
    (settings?.update_checks_enabled ?? false) && updateChecksLocked === false;

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch((error) => console.error("Failed to get app version:", error));
  }, []);

  const statusText = !checksOn
    ? t("settings.about.updates.off")
    : status === "checking"
      ? t("footer.checkingUpdates")
      : status === "upToDate"
        ? t("footer.upToDate")
        : status === "available"
          ? t("footer.updateAvailableShort")
          : status === "installing"
            ? progress > 0 && progress < 100
              ? t("footer.downloading", { progress })
              : t("footer.installing")
            : status === "checkFailed" || status === "installFailed"
              ? t(`footer.${status}`)
              : t("settings.about.version.description");

  return (
    <SettingContainer
      title={t("settings.about.version.title", { version })}
      description={statusText}
    >
      {status === "available" ? (
        <Button variant="primary" onClick={() => emit("install-update")}>
          {t("settings.about.updates.install")}
        </Button>
      ) : (
        <Button
          variant="secondary"
          disabled={
            !checksOn || status === "checking" || status === "installing"
          }
          onClick={() => emit("check-for-updates")}
        >
          {t("footer.checkForUpdates")}
        </Button>
      )}
    </SettingContainer>
  );
};

export const SourceCodeRow: React.FC = () => {
  const { t } = useTranslation();
  return (
    <SettingContainer
      title={t("settings.about.sourceCode.title")}
      description={t("settings.about.sourceCode.description")}
    >
      <Button variant="secondary" onClick={() => openUrl(SOURCE_URL)}>
        <ExternalLink className="h-4 w-4" aria-hidden="true" />
        {t("settings.about.sourceCode.button")}
      </Button>
    </SettingContainer>
  );
};
