// The bar's and the bubble's strings. Those windows are always loaded, so
// they take only the sections they show (SMALL_LOCALE in vite.config.ts),
// not every string in the app. A key from another section shows as its
// name there: add the section to SMALL_LOCALE when one is needed.
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "virtual:small-locale";

i18n.use(initReactI18next).init({
  resources: { en: { translation: en } },
  lng: "en",
  interpolation: {
    escapeValue: false, // React already escapes values
  },
  react: {
    useSuspense: false,
  },
});

export default i18n;
