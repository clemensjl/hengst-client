import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import de from "./de.json";
import en from "./en.json";

export type Lang = "de" | "en";

/** The setting wins; without one the system language decides (de* is German, everything else English). */
export function pickLanguage(setting: Lang | null, navigatorLanguage: string): Lang {
  if (setting) return setting;
  return navigatorLanguage.toLowerCase().startsWith("de") ? "de" : "en";
}

/** Switches the UI language and `<html lang>`. */
export function setLanguage(lang: Lang): void {
  document.documentElement.lang = lang;
  if (i18n.language !== lang) void i18n.changeLanguage(lang);
}

/** Flat keys with prefixes (`nav.`, `settings.` ...): no key separator. Synchronous, resources are bundled. */
export function initI18n(language: Lang | null, navigatorLanguage: string): typeof i18n {
  const lng = pickLanguage(language, navigatorLanguage);
  if (i18n.isInitialized) {
    setLanguage(lng);
    return i18n;
  }
  void i18n.use(initReactI18next).init({
    lng,
    fallbackLng: "en",
    resources: { de: { translation: de }, en: { translation: en } },
    keySeparator: false,
    nsSeparator: false,
    interpolation: { escapeValue: false },
    initAsync: false,
  });
  document.documentElement.lang = lng;
  return i18n;
}

export default i18n;
