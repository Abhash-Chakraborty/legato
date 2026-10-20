import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import legatoIcon from "../../assets/legato-icon.svg";
import ModelSelector from "../model-selector";
import UpdateChecker from "../update-checker";

// A brand name, so it stays out of the translation files.
const APP_NAME = "Legato";

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
 * window controls.
 */
export const TitleBar: React.FC<TitleBarProps> = ({ minimal = false }) => {
  const { t } = useTranslation();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
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
  }, []);

  const win = getCurrentWindow();

  return (
    <header
      // A maximized frameless window on Windows reaches a few pixels past the
      // top of the screen, so the bar gets that much padding to stay visible.
      className={`flex h-12 flex-none select-none items-stretch ${maximized ? "h-[55px] pt-[7px]" : ""}`}
      data-tauri-drag-region
    >
      {!minimal && (
        <div
          className="flex items-center gap-2.5 ps-[22px] pe-3 text-[17px] font-semibold tracking-[-0.01em] text-text"
          data-tauri-drag-region
        >
          <img
            src={legatoIcon}
            alt=""
            width={22}
            height={22}
            className="pointer-events-none h-[22px] w-[22px] rounded-[6px]"
          />
          <span className="pointer-events-none">{APP_NAME}</span>
        </div>
      )}
      <div className="flex-1" data-tauri-drag-region />
      {!minimal && (
        <div className="flex items-center gap-1 pe-2">
          <UpdateChecker />
          <ModelSelector />
        </div>
      )}
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
    </header>
  );
};
