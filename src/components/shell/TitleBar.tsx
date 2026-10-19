import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettings } from "../../hooks/useSettings";
import ModelSelector from "../model-selector";
import UpdateChecker from "../update-checker";

// Caption button glyphs from Segoe Fluent Icons (Windows 11), which keeps the
// Segoe MDL2 Assets code points that Windows 10 has.
const GLYPH = {
  minimize: "",
  maximize: "",
  restore: "",
  close: "",
};

const captionClasses =
  "grid h-full w-[46px] place-items-center text-[10px] text-text-2 transition-colors duration-[var(--dur-1)] hover:duration-0 [font-family:'Segoe_Fluent_Icons','Segoe_MDL2_Assets',sans-serif]";

interface TitleBarProps {
  /** During onboarding only the window controls show. */
  minimal?: boolean;
}

/**
 * Legato's own title bar: a drag area, the update and model status, and the
 * window controls. With the native title bar on (Settings › General), Windows
 * draws the controls and this keeps only the status.
 */
export const TitleBar: React.FC<TitleBarProps> = ({ minimal = false }) => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const native = settings?.native_title_bar ?? false;
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (native) return;
    const win = getCurrentWindow();
    const sync = () => {
      win
        .isMaximized()
        .then(setMaximized)
        .catch(() => {});
    };
    sync();
    const unlisten = win.onResized(sync);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [native]);

  // Nothing to show: Windows draws the title bar and onboarding has no status.
  if (native && minimal) return null;

  const win = getCurrentWindow();

  return (
    <header
      className="flex h-12 flex-none select-none items-stretch"
      data-tauri-drag-region={native ? undefined : true}
    >
      <div
        className="flex-1"
        data-tauri-drag-region={native ? undefined : true}
      />
      {!minimal && (
        <div className="flex items-center gap-1 pe-2">
          <UpdateChecker />
          <ModelSelector />
        </div>
      )}
      {!native && (
        <div className="flex items-stretch">
          <button
            type="button"
            className={`${captionClasses} hover:bg-surface-3 active:bg-surface-4`}
            aria-label={t("titleBar.minimize")}
            title={t("titleBar.minimize")}
            onClick={() => win.minimize()}
          >
            {GLYPH.minimize}
          </button>
          <button
            type="button"
            className={`${captionClasses} hover:bg-surface-3 active:bg-surface-4`}
            aria-label={
              maximized ? t("titleBar.restore") : t("titleBar.maximize")
            }
            title={maximized ? t("titleBar.restore") : t("titleBar.maximize")}
            onClick={() => win.toggleMaximize()}
          >
            {maximized ? GLYPH.restore : GLYPH.maximize}
          </button>
          <button
            type="button"
            className={`${captionClasses} hover:bg-[#c42b1c] hover:text-white active:bg-[#c42b1c]/90 active:text-white/75`}
            aria-label={t("titleBar.close")}
            title={t("titleBar.close")}
            onClick={() => win.close()}
          >
            {GLYPH.close}
          </button>
        </div>
      )}
    </header>
  );
};
