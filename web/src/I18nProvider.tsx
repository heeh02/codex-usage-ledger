import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { I18nContext, type I18nValue } from './i18n';
import { setUiLanguage, type UiLanguage } from './lib';
import { enMessages } from './locales/en';
import { zhCNMessages, type MessageKey } from './locales/zh-CN';
import { notifyNativeLanguageChanged } from './nativeBridge';

const STORAGE_KEY = 'ledger.language';

function initialLanguage(): UiLanguage {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === 'zh-CN' || saved === 'en') return saved;
    return window.navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en';
  } catch {
    return 'zh-CN';
  }
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, updateLanguage] = useState<UiLanguage>(initialLanguage);
  setUiLanguage(language);

  useEffect(() => {
    document.documentElement.lang = language;
    document.documentElement.dir = 'ltr';
    notifyNativeLanguageChanged(language);
  }, [language]);

  const value = useMemo<I18nValue>(() => ({
    language,
    setLanguage: (next) => {
      updateLanguage(next);
      try {
        window.localStorage.setItem(STORAGE_KEY, next);
      } catch {
        // A blocked localStorage must not prevent language switching for this run.
      }
    },
    t: ((key: MessageKey, parameters?: Record<string, string | number>) => {
      const catalog: Record<string, string> = language === 'zh-CN' ? zhCNMessages : enMessages;
      const template = catalog[key] ?? key;
      return template.replace(/\{([a-zA-Z0-9_]+)\}/g, (placeholder, name: string) => (
        parameters && name in parameters ? String(parameters[name]) : placeholder
      ));
    }) as I18nValue['t'],
  }), [language]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}
