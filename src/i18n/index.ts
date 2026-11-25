import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en.json";

/** English only: this fork ships a single locale. Strings still go through
 *  `t()` so components stay unchanged, but there is no language choice. */
export const i18nReady = i18n.use(initReactI18next).init({
  resources: { en: { translation: en } },
  lng: "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
