import React, { useEffect, useState } from 'react';
import { setLocale, en, es } from '../i18n';

type Language = 'en' | 'es';

/**
 * Language switcher that persists locale selection across sessions.
 * Updates the page language when the user selects a different language.
 */
export function LanguageSwitcher() {
  const [language, setLanguage] = useState<Language>('en');

  // Load language preference from localStorage on mount
  useEffect(() => {
    const saved = localStorage.getItem('language') as Language | null;
    if (saved && (saved === 'en' || saved === 'es')) {
      setLanguage(saved);
      applyLanguage(saved);
    }
  }, []);

  const applyLanguage = (lang: Language) => {
    const locale = lang === 'es' ? es : en;
    setLocale(locale);
    localStorage.setItem('language', lang);
    // Update HTML lang attribute for accessibility
    document.documentElement.lang = lang;
  };

  const handleLanguageChange = (lang: Language) => {
    setLanguage(lang);
    applyLanguage(lang);
  };

  return (
    <div className="flex gap-2">
      <button
        onClick={() => handleLanguageChange('en')}
        className={`px-3 py-1 rounded text-sm font-medium transition-colors ${
          language === 'en'
            ? 'bg-blue-600 text-white'
            : 'bg-gray-200 text-gray-800 hover:bg-gray-300'
        }`}
        aria-pressed={language === 'en'}
      >
        English
      </button>
      <button
        onClick={() => handleLanguageChange('es')}
        className={`px-3 py-1 rounded text-sm font-medium transition-colors ${
          language === 'es'
            ? 'bg-blue-600 text-white'
            : 'bg-gray-200 text-gray-800 hover:bg-gray-300'
        }`}
        aria-pressed={language === 'es'}
      >
        Español
      </button>
    </div>
  );
}
