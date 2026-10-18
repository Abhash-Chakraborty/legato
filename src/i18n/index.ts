import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en/translation.json";

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

export default i18n;
