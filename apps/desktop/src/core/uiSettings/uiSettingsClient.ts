import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import type { ResolvedUiTheme, UiAppearancePreferences, UiThemePreference } from "../../types";
import {
  buildUiAppearanceCssVariables,
  defaultUiAppearancePreferences,
  getUiAppearanceDataAttributes,
  normalizeResolvedUiTheme,
  normalizeUiAppearancePreferences,
  normalizeUiThemePreference,
  resolveUiTheme,
} from "./uiSettingsLogic";

export const UI_THEME_STORAGE_KEY = "sofvary.ui.themePreference";
export const UI_APPEARANCE_STORAGE_KEY = "sofvary.ui.appearancePreferences.v1";

type UiAppearanceStyle = CSSProperties & Record<`--${string}`, string>;

function getSystemTheme(): ResolvedUiTheme {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return "dark";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function readStoredThemePreference(): UiThemePreference {
  if (typeof window === "undefined") {
    return "system";
  }

  try {
    return normalizeUiThemePreference(window.localStorage.getItem(UI_THEME_STORAGE_KEY));
  } catch {
    return "system";
  }
}

function readStoredAppearancePreferences(): UiAppearancePreferences {
  const legacyThemePreference = readStoredThemePreference();
  if (typeof window === "undefined") {
    return normalizeUiAppearancePreferences(defaultUiAppearancePreferences, legacyThemePreference);
  }

  try {
    const stored = window.localStorage.getItem(UI_APPEARANCE_STORAGE_KEY);
    return normalizeUiAppearancePreferences(
      stored ? JSON.parse(stored) : defaultUiAppearancePreferences,
      legacyThemePreference,
    );
  } catch {
    return normalizeUiAppearancePreferences(defaultUiAppearancePreferences, legacyThemePreference);
  }
}

function writeStoredAppearancePreferences(preferences: UiAppearancePreferences) {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(UI_APPEARANCE_STORAGE_KEY, JSON.stringify(preferences));
  } catch {
    // Appearance choices are UI preferences only; storage failures should not break the shell.
  }
}

export function useUiAppearance() {
  const [preferences, setPreferencesState] = useState<UiAppearancePreferences>(() =>
    readStoredAppearancePreferences(),
  );
  const [systemTheme, setSystemTheme] = useState<ResolvedUiTheme>(() => getSystemTheme());

  const resolvedTheme = useMemo(
    () => resolveUiTheme(preferences.themePreference, systemTheme),
    [preferences.themePreference, systemTheme],
  );

  const cssVariables = useMemo(
    () => buildUiAppearanceCssVariables(preferences) as UiAppearanceStyle,
    [preferences],
  );

  const dataAttributes = useMemo(
    () => getUiAppearanceDataAttributes(preferences),
    [preferences],
  );

  const setPreferences = useCallback((nextPreferences: UiAppearancePreferences) => {
    const normalized = normalizeUiAppearancePreferences(nextPreferences);
    writeStoredAppearancePreferences(normalized);
    setPreferencesState(normalized);
  }, []);

  const updatePreferences = useCallback((patch: Partial<UiAppearancePreferences>) => {
    setPreferencesState((current) => {
      const normalized = normalizeUiAppearancePreferences({ ...current, ...patch });
      writeStoredAppearancePreferences(normalized);
      return normalized;
    });
  }, []);

  const setThemePreference = useCallback((nextPreference: UiThemePreference) => {
    updatePreferences({ themePreference: normalizeUiThemePreference(nextPreference) });
  }, [updatePreferences]);

  const setPreference = useCallback((nextPreference: UiThemePreference) => {
    setThemePreference(nextPreference);
  }, [setThemePreference]);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return undefined;
    }

    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const syncSystemTheme = () => {
      setSystemTheme(normalizeResolvedUiTheme(query.matches ? "dark" : "light"));
    };

    syncSystemTheme();

    if (typeof query.addEventListener === "function") {
      query.addEventListener("change", syncSystemTheme);
      return () => query.removeEventListener("change", syncSystemTheme);
    }

    query.addListener(syncSystemTheme);
    return () => query.removeListener(syncSystemTheme);
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") {
      return undefined;
    }

    const onStorage = (event: StorageEvent) => {
      if (event.key === UI_APPEARANCE_STORAGE_KEY || event.key === UI_THEME_STORAGE_KEY) {
        setPreferencesState(readStoredAppearancePreferences());
      }
    };

    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  return {
    preferences,
    preference: preferences.themePreference,
    resolvedTheme,
    systemTheme,
    cssVariables,
    dataAttributes,
    setPreferences,
    updatePreferences,
    setThemePreference,
    setPreference,
  };
}

export function useUiTheme() {
  const appearance = useUiAppearance();
  return {
    preference: appearance.preference,
    resolvedTheme: appearance.resolvedTheme,
    systemTheme: appearance.systemTheme,
    setPreference: appearance.setPreference,
  };
}
