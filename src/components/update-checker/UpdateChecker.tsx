import React, { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { arch } from "@tauri-apps/plugin-os";
import { useSettings } from "../../hooks/useSettings";
import { useUpdateStore, type UpdateStatus } from "../../stores/updateStore";
import { commands } from "../../bindings";
import {
  resolvePortableInstallerUrl,
  PORTABLE_RELEASES_URL,
} from "./portableInstaller";

interface UpdateCheckerProps {
  className?: string;
}

const NOTIFIED_KEY = "legato.updateNotified";

/** A Windows notification for a new version, once per version and only
 *  while Legato isn't in front, where the title bar already shows it. */
const notifyOnce = (version: string) => {
  if (document.hasFocus()) return;
  try {
    if (localStorage.getItem(NOTIFIED_KEY) === version) return;
    localStorage.setItem(NOTIFIED_KEY, version);
  } catch {
    // Without storage it may say so again after a restart.
  }
  commands.notifyUpdate(version).catch(() => {});
};

const UpdateChecker: React.FC<UpdateCheckerProps> = ({ className = "" }) => {
  const { t } = useTranslation();
  // Update checking state
  const [isChecking, setIsChecking] = useState(false);
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [isInstalling, setIsInstalling] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [showUpToDate, setShowUpToDate] = useState(false);
  // A check you asked for, or an install, that failed: said until the next
  // try, so the button doesn't look like it did nothing.
  const [failed, setFailed] = useState<"check" | "install" | null>(null);
  const [showPortableUpdateDialog, setShowPortableUpdateDialog] =
    useState(false);
  const [portableInstallerUrl, setPortableInstallerUrl] = useState<string>(
    PORTABLE_RELEASES_URL,
  );

  const { settings, isLoading, updateChecksLocked } = useSettings();
  // Wait for the lock state too (null = not loaded yet), otherwise the first
  // render could fire an update check before LEGATO_DISABLE_UPDATER is known.
  const settingsLoaded =
    !isLoading && settings !== null && updateChecksLocked !== null;
  // Forced-off by system configuration (LEGATO_DISABLE_UPDATER) overrides the
  // stored preference without persisting it, mirroring the backend's effective
  // updater state.
  const updateChecksEnabled =
    (settings?.update_checks_enabled ?? false) && updateChecksLocked === false;

  const upToDateTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const isManualCheckRef = useRef(false);
  const downloadedBytesRef = useRef(0);
  const contentLengthRef = useRef(0);

  useEffect(() => {
    // Wait for settings to load before doing anything
    if (!settingsLoaded) return;

    if (!updateChecksEnabled) {
      if (upToDateTimeoutRef.current) {
        clearTimeout(upToDateTimeoutRef.current);
      }
      setIsChecking(false);
      setUpdateAvailable(false);
      setShowUpToDate(false);
      return;
    }

    checkForUpdates();

    // Listen for update check events (the tray menu and Settings › About)
    const updateUnlisten = listen("check-for-updates", () => {
      handleManualUpdateCheck();
    });
    // Settings › About's Install button
    const installUnlisten = listen("install-update", () => {
      installUpdate();
    });

    return () => {
      if (upToDateTimeoutRef.current) {
        clearTimeout(upToDateTimeoutRef.current);
      }
      updateUnlisten.then((fn) => fn());
      installUnlisten.then((fn) => fn());
    };
  }, [settingsLoaded, updateChecksEnabled]);

  // Update checking functions
  const checkForUpdates = async () => {
    if (!updateChecksEnabled || isChecking) return;

    try {
      setIsChecking(true);
      if (isManualCheckRef.current) setFailed(null);
      const update = await check();

      if (update) {
        setUpdateAvailable(true);
        setShowUpToDate(false);
        // Portable installs can't self-update in place — the manual dialog links
        // straight at the matching installer from this manifest instead.
        setPortableInstallerUrl(
          resolvePortableInstallerUrl(update.rawJson, arch()),
        );
        if (!isManualCheckRef.current) notifyOnce(update.version);
      } else {
        setUpdateAvailable(false);

        if (isManualCheckRef.current) {
          setShowUpToDate(true);
          if (upToDateTimeoutRef.current) {
            clearTimeout(upToDateTimeoutRef.current);
          }
          upToDateTimeoutRef.current = setTimeout(() => {
            setShowUpToDate(false);
          }, 3000);
        }
      }
    } catch (error) {
      console.error("Failed to check for updates:", error);
      if (isManualCheckRef.current) setFailed("check");
    } finally {
      setIsChecking(false);
      isManualCheckRef.current = false;
    }
  };

  const handleManualUpdateCheck = () => {
    if (!updateChecksEnabled) return;
    isManualCheckRef.current = true;
    checkForUpdates();
  };

  const installUpdate = async () => {
    if (!updateChecksEnabled) return;

    const portable = await commands.isPortable();
    if (portable) {
      setShowPortableUpdateDialog(true);
      return;
    }

    try {
      setIsInstalling(true);
      setFailed(null);
      setDownloadProgress(0);
      downloadedBytesRef.current = 0;
      contentLengthRef.current = 0;
      const update = await check();

      if (!update) {
        console.log("No update available during install attempt");
        return;
      }

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            downloadedBytesRef.current = 0;
            contentLengthRef.current = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloadedBytesRef.current += event.data.chunkLength;
            const progress =
              contentLengthRef.current > 0
                ? Math.round(
                    (downloadedBytesRef.current / contentLengthRef.current) *
                      100,
                  )
                : 0;
            setDownloadProgress(Math.min(progress, 100));
            break;
        }
      });
      await relaunch();
    } catch (error) {
      console.error("Failed to install update:", error);
      setFailed("install");
    } finally {
      setIsInstalling(false);
      setDownloadProgress(0);
      downloadedBytesRef.current = 0;
      contentLengthRef.current = 0;
    }
  };

  // Share the state with Settings › About.
  const status: UpdateStatus = !updateChecksEnabled
    ? "idle"
    : isInstalling
      ? "installing"
      : isChecking
        ? "checking"
        : updateAvailable
          ? "available"
          : failed === "check"
            ? "checkFailed"
            : failed === "install"
              ? "installFailed"
              : showUpToDate
                ? "upToDate"
                : "idle";
  useEffect(() => {
    useUpdateStore.getState().setStatus(status);
  }, [status]);
  useEffect(() => {
    useUpdateStore.getState().setProgress(downloadProgress);
  }, [downloadProgress]);

  const installingText =
    downloadProgress > 0 && downloadProgress < 100
      ? t("footer.downloading", { progress: downloadProgress })
      : downloadProgress === 100
        ? t("footer.installing")
        : t("footer.preparing");

  // When no installer could be resolved for this target the button falls back to
  // the releases index, so the dialog has to say "browse" rather than "download".
  const hasDirectInstaller = portableInstallerUrl !== PORTABLE_RELEASES_URL;

  return (
    <>
      {showPortableUpdateDialog && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-background border border-mid-gray/20 rounded-lg p-6 max-w-md w-full mx-4 space-y-4">
            <h2 className="text-base font-semibold">
              {t("footer.portableUpdateTitle")}
            </h2>
            <p className="text-sm text-text/70">
              {hasDirectInstaller
                ? t("footer.portableUpdateMessage")
                : t("footer.portableUpdateBrowseMessage")}
            </p>
            <div className="flex gap-2 justify-end">
              <button
                className="px-3 py-1.5 text-sm rounded border border-mid-gray/20 hover:bg-mid-gray/10 transition-colors"
                onClick={() => setShowPortableUpdateDialog(false)}
              >
                {t("common.close")}
              </button>
              <button
                className="px-3 py-1.5 text-sm rounded bg-logo-primary text-white hover:bg-logo-primary/80 transition-colors"
                onClick={() => {
                  openUrl(portableInstallerUrl);
                  setShowPortableUpdateDialog(false);
                }}
              >
                {hasDirectInstaller
                  ? t("footer.portableUpdateButton")
                  : t("footer.portableUpdateBrowseButton")}
              </button>
            </div>
          </div>
        </div>
      )}
      {/* The title bar only mentions updates when there is something to
          act on or a check you asked for has finished. */}
      {status === "available" && (
        <button
          type="button"
          onClick={installUpdate}
          className={`inline-flex h-7 items-center gap-2 rounded-full bg-teal-50 px-3 text-[13px] font-medium text-teal-700 transition-colors duration-[var(--dur-1)] hover:bg-teal-100 hover:duration-0 ${className}`}
        >
          <span className="h-[7px] w-[7px] rounded-full bg-teal-600" />
          {t("footer.updateAvailableShort")}
        </button>
      )}
      {status === "installing" && (
        <span
          role="status"
          className={`inline-flex h-7 items-center gap-2 px-2 text-[13px] tabular-nums text-text-2 ${className}`}
        >
          {installingText}
        </span>
      )}
      {(status === "checkFailed" || status === "installFailed") && (
        <span
          role="status"
          className={`inline-flex h-7 items-center px-2 text-[13px] text-text-2 ${className}`}
        >
          {t(`footer.${status}`)}
        </span>
      )}
      {status === "upToDate" && (
        <span
          role="status"
          className={`inline-flex h-7 items-center px-2 text-[13px] text-text-3 ${className}`}
        >
          {t("footer.upToDate")}
        </span>
      )}
    </>
  );
};

export default UpdateChecker;
