import React from "react";
import ReactDOM from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import { ScratchpadWindow } from "./ScratchpadWindow";
import {
  applyTheme,
  getStoredTheme,
  syncThemeFromSettings,
} from "@/lib/utils/theme";
import type { Theme } from "@/bindings";
import "@/i18n";
import { installContextMenu } from "@/lib/contextMenu";
import "@/App.css";

// Its own webview, so it sets the theme on its own document, as the overlay
// does: the last-known theme first, then the setting, then live changes.
applyTheme(getStoredTheme());
syncThemeFromSettings();
listen<Theme>("theme-changed", (event) => applyTheme(event.payload));

installContextMenu();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ScratchpadWindow />
  </React.StrictMode>,
);
