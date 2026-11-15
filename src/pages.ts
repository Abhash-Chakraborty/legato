import { lazy } from "react";

const loaders = {
  Dictionary: () =>
    import("./components/pages/dictionary/DictionaryPage").then((m) => ({
      default: m.DictionaryPage,
    })),
  Snippets: () =>
    import("./components/pages/snippets/SnippetsPage").then((m) => ({
      default: m.SnippetsPage,
    })),
  Style: () =>
    import("./components/pages/style/StylePage").then((m) => ({
      default: m.StylePage,
    })),
  Scratchpad: () =>
    import("./components/pages/scratchpad/ScratchpadPage").then((m) => ({
      default: m.ScratchpadPage,
    })),
  Transforms: () =>
    import("./components/pages/transforms/TransformsPage").then((m) => ({
      default: m.TransformsPage,
    })),
  Notes: () =>
    import("./components/pages/notes/NotesPage").then((m) => ({
      default: m.NotesPage,
    })),
  Insights: () =>
    import("./components/pages/insights/InsightsPage").then((m) => ({
      default: m.InsightsPage,
    })),
  Models: () =>
    import("./components/settings/models/ModelsSettings").then((m) => ({
      default: m.ModelsSettings,
    })),
  Settings: () =>
    import("./components/settings/SettingsPage").then((m) => ({
      default: m.SettingsPage,
    })),
};

/** Every page but Home, each loaded the first time it's shown. */
export const pages = {
  Dictionary: lazy(loaders.Dictionary),
  Snippets: lazy(loaders.Snippets),
  Style: lazy(loaders.Style),
  Scratchpad: lazy(loaders.Scratchpad),
  Transforms: lazy(loaders.Transforms),
  Notes: lazy(loaders.Notes),
  Insights: lazy(loaders.Insights),
  Models: lazy(loaders.Models),
  Settings: lazy(loaders.Settings),
};

/** Loads the pages one after another while the app has nothing to do, so
 *  the first click on each is as quick as the second. Not while the window
 *  is hidden: closed to the tray, Legato loads nothing it isn't showing. */
export const warmPages = () => {
  const queue: (() => Promise<unknown>)[] = Object.values(loaders);
  const idle = (run: () => void) =>
    typeof window.requestIdleCallback === "function"
      ? window.requestIdleCallback(run, { timeout: 2000 })
      : window.setTimeout(run, 300);
  const next = () => {
    const load = queue.shift();
    if (!load) return;
    if (document.hidden) {
      queue.unshift(load);
      document.addEventListener("visibilitychange", () => idle(next), {
        once: true,
      });
      return;
    }
    load()
      .catch(() => {})
      .finally(() => idle(next));
  };
  idle(next);
};
