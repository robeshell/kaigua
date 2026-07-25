import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./locales/en.json";
import ja from "./locales/ja.json";
import zhHans from "./locales/zh-Hans.json";

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    ja: { translation: ja },
    "zh-Hans": { translation: zhHans },
  },
  lng: "zh-Hans",
  fallbackLng: "zh-Hans",
  // Locale files use flat dotted keys (`err.apiKey`); do not treat `.` as nesting.
  keySeparator: false,
  nsSeparator: false,
  interpolation: { escapeValue: false },
});

export default i18n;
