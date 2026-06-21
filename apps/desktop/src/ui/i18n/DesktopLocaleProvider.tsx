import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  DEFAULT_LOCALE,
  type Locale,
  type TranslationParams,
  createTranslator,
  normalizeLocale,
} from "@sofvary/i18n";

export const DESKTOP_LOCALE_STORAGE_KEY = "sofvary.ui.locale";

interface DesktopLocaleContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: string, params?: TranslationParams, fallback?: string) => string;
}

const DesktopLocaleContext = createContext<DesktopLocaleContextValue | null>(null);

export function DesktopLocaleProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => readStoredDesktopLocale());

  const setLocale = (nextLocale: Locale) => {
    const normalized = normalizeLocale(nextLocale);
    writeStoredDesktopLocale(normalized);
    setLocaleState(normalized);
    window.dispatchEvent(new CustomEvent("sofvary:locale-change", { detail: normalized }));
  };

  useEffect(() => {
    document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : "en";
  }, [locale]);

  useEffect(() => {
    const syncLocale = (value: unknown) => setLocaleState(normalizeLocale(value));
    const onStorage = (event: StorageEvent) => {
      if (event.key === DESKTOP_LOCALE_STORAGE_KEY) {
        syncLocale(event.newValue);
      }
    };
    const onLocaleChange = (event: Event) => {
      syncLocale(event instanceof CustomEvent ? event.detail : readStoredDesktopLocale());
    };
    window.addEventListener("storage", onStorage);
    window.addEventListener("sofvary:locale-change", onLocaleChange);
    return () => {
      window.removeEventListener("storage", onStorage);
      window.removeEventListener("sofvary:locale-change", onLocaleChange);
    };
  }, []);

  const value = useMemo<DesktopLocaleContextValue>(() => {
    const translate = createTranslator(locale);
    return {
      locale,
      setLocale,
      t: translate,
    };
  }, [locale]);

  return <DesktopLocaleContext.Provider value={value}>{children}</DesktopLocaleContext.Provider>;
}

export function useDesktopLocale(): DesktopLocaleContextValue {
  const context = useContext(DesktopLocaleContext);
  if (!context) {
    throw new Error("useDesktopLocale must be used inside DesktopLocaleProvider");
  }
  return context;
}

export function readStoredDesktopLocale(): Locale {
  if (typeof window === "undefined") {
    return DEFAULT_LOCALE;
  }
  try {
    return normalizeLocale(window.localStorage.getItem(DESKTOP_LOCALE_STORAGE_KEY));
  } catch {
    return DEFAULT_LOCALE;
  }
}

function writeStoredDesktopLocale(locale: Locale) {
  try {
    window.localStorage.setItem(DESKTOP_LOCALE_STORAGE_KEY, locale);
  } catch {
    // Language is a presentation preference; storage failures should not block the shell.
  }
}
