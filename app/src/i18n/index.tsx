// Minimal typed i18n (no dependency): flat keys, `{name}` interpolation.
import { createContext, useContext, useEffect, useMemo, type ReactNode } from "react";
import type { UiLanguage } from "../api";
import { en } from "./en";
import { ru, type MessageKey } from "./ru";

export type { MessageKey };
export type Translate = (key: MessageKey, params?: Record<string, string | number>) => string;

const DICTIONARIES: Record<UiLanguage, Record<MessageKey, string>> = { ru, en };

export function createTranslate(language: UiLanguage): Translate {
  const dictionary = DICTIONARIES[language];
  return (key, params) => {
    const template = dictionary[key];
    if (!params) return template;
    return template.replace(/\{(\w+)\}/g, (match, name: string) => {
      const value = params[name];
      return value === undefined ? match : String(value);
    });
  };
}

const I18nContext = createContext<Translate>(createTranslate("ru"));

export function I18nProvider({ language, children }: { language: UiLanguage; children: ReactNode }) {
  const translate = useMemo(() => createTranslate(language), [language]);
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  return <I18nContext.Provider value={translate}>{children}</I18nContext.Provider>;
}

export function useT(): Translate {
  return useContext(I18nContext);
}
