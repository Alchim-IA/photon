import React, { createContext, useContext, useState, useEffect, useCallback } from "react";
import frLocale from "../locales/fr.json";
import enLocale from "../locales/en.json";

export type Language = "fr" | "en";

function flattenObject(obj: Record<string, unknown>, prefix = ""): Record<string, string> {
  return Object.entries(obj).reduce((acc, [key, value]) => {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "object" && value !== null && !Array.isArray(value)) {
      Object.assign(acc, flattenObject(value as Record<string, unknown>, fullKey));
    } else {
      acc[fullKey] = String(value);
    }
    return acc;
  }, {} as Record<string, string>);
}

const locales: Record<Language, Record<string, string>> = {
  fr: flattenObject(frLocale as Record<string, unknown>),
  en: flattenObject(enLocale as Record<string, unknown>),
};

interface LanguageContextValue {
  language: Language;
  setLanguage: (lang: Language) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
}

const LanguageContext = createContext<LanguageContextValue>({
  language: "fr",
  setLanguage: () => {},
  t: (key) => key,
});

export function LanguageProvider({ children }: { children: React.ReactNode }) {
  const [language, setLanguageState] = useState<Language>(() => {
    return (localStorage.getItem("app-language") as Language) ?? "fr";
  });

  useEffect(() => {
    document.documentElement.setAttribute("lang", language);
  }, [language]);

  const setLanguage = useCallback((lang: Language) => {
    localStorage.setItem("app-language", lang);
    setLanguageState(lang);
  }, []);

  const t = useCallback(
    (key: string, params?: Record<string, string | number>): string => {
      let value = locales[language]?.[key] ?? key;
      if (params) {
        for (const [k, v] of Object.entries(params)) {
          value = value.replace(new RegExp(`\\{\\{${k}\\}\\}`, "g"), String(v));
        }
      }
      return value;
    },
    [language]
  );

  return (
    <LanguageContext.Provider value={{ language, setLanguage, t }}>
      {children}
    </LanguageContext.Provider>
  );
}

export function useTranslation() {
  return useContext(LanguageContext);
}
