import { createContext, useContext } from 'react';
import type { UiLanguage } from './lib';
import type { MessageKey } from './locales/zh-CN';

export interface Translate {
  (key: MessageKey, parameters?: Record<string, string | number>): string;
}

export interface I18nValue {
  language: UiLanguage;
  setLanguage: (language: UiLanguage) => void;
  t: Translate;
}

export const I18nContext = createContext<I18nValue | null>(null);

export function useI18n(): I18nValue {
  const value = useContext(I18nContext);
  if (!value) throw new Error('useI18n must be used inside I18nProvider');
  return value;
}
