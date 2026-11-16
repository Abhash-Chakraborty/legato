import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en/translation.json";
import mac from "./locales/en/mac.json";
import { isMac } from "../lib/utils/platform";

i18n.use(initReactI18next).init({
  resources: { en: { translation: en } },
  lng: "en",
  interpolation: {
    escapeValue: false, // React already escapes values
  },
  react: {
    useSuspense: false, // Disable suspense for SSR compatibility
  },
});

// A Mac's own wording for the strings that name Windows, its keys or its
// places (the tray, Credential Manager, "this PC"), laid over the rest.
if (isMac) {
  i18n.addResourceBundle("en", "translation", mac, true, true);
}

export default i18n;
