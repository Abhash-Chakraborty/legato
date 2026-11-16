/**
 * Which system the page runs on, read from the webview itself so the bar's
 * and the bubble's small pages need no plugin for it. WebKit on a Mac says
 * "MacIntel" on Apple silicon too.
 */
export const isMac =
  typeof navigator !== "undefined" && /^Mac/i.test(navigator.platform ?? "");

if (isMac && typeof document !== "undefined") {
  // For the few styles that differ, such as room for the traffic lights.
  document.documentElement.dataset.platform = "mac";
}
