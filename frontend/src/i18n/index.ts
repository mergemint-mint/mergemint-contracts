import en, { TranslationKey } from './en';

type Translations = typeof en;

let locale: Translations = en;

/**
 * Lightweight i18n shim — API-compatible subset of react-i18next.
 * Falls back to the English catalog when a key is missing.
 */
export function t(key: TranslationKey, fallback?: string): string {
  return (locale[key] as string) ?? fallback ?? key;
}

/** useTranslation shim — returns the same { t } object every render. */
export function useTranslation() {
  return { t };
}

/**
 * Swap locale at runtime (for future language support).
 * Always falls back to English for missing keys.
 */
export function setLocale(next: Translations): void {
  locale = next;
}

export { en };
export type { TranslationKey };
