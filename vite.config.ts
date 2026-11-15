import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "path";
import { readFileSync } from "fs";
import type { Plugin } from "vite";

const host = process.env.TAURI_DEV_HOST;

/** The sections of translation.json the bar and the bubble use. Those two
 *  windows are always loaded, so they get these strings alone
 *  (src/i18n/small.ts) instead of every string in the app. */
const SMALL_LOCALE = ["overlay", "bubble", "inyon", "ai", "contextMenu"];

const smallLocale = (): Plugin => {
  const file = resolve(__dirname, "src/i18n/locales/en/translation.json");
  return {
    name: "legato-small-locale",
    resolveId: (id) =>
      id === "virtual:small-locale" ? "\0small-locale" : null,
    load(id) {
      if (id !== "\0small-locale") return null;
      this.addWatchFile(file);
      const all = JSON.parse(readFileSync(file, "utf8"));
      const kept = Object.fromEntries(SMALL_LOCALE.map((k) => [k, all[k]]));
      return `export default JSON.parse(${JSON.stringify(JSON.stringify(kept))});`;
    },
  };
};

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss(), smallLocale()],

  // Path aliases
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
      "@/bindings": resolve(__dirname, "./src/bindings.ts"),
    },
  },

  // Multiple entry points for main app and overlay
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        overlay: resolve(__dirname, "src/overlay/index.html"),
        scratchpad: resolve(__dirname, "src/scratchpad/index.html"),
        bubble: resolve(__dirname, "src/bubble/index.html"),
        notespanel: resolve(__dirname, "src/notespanel/index.html"),
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
