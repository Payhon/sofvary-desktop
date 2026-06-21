import type {
  ResolvedUiTheme,
  UiAccentPreference,
  UiAppearancePreferences,
  UiDensityPreference,
  UiGlassPreference,
  UiMotionPreference,
  UiRadiusPreference,
  UiShadowPreference,
  UiThemePreference,
} from "../../types";

type Translator = (key: string, params?: Record<string, string | number | boolean | null | undefined>, fallback?: string) => string;

export const uiThemePreferences: Array<{
  preference: UiThemePreference;
}> = [
  {
    preference: "system",
  },
  {
    preference: "dark",
  },
  {
    preference: "light",
  },
];

export const uiThemePreferenceCycle: UiThemePreference[] = [
  "system",
  "light",
  "dark",
];

export const uiAccentPreferences: Array<{ preference: UiAccentPreference }> = [
  { preference: "blue" },
  { preference: "teal" },
  { preference: "violet" },
  { preference: "amber" },
  { preference: "rose" },
];

export const uiDensityPreferences: Array<{ preference: UiDensityPreference }> = [
  { preference: "compact" },
  { preference: "comfortable" },
  { preference: "spacious" },
];

export const uiGlassPreferences: Array<{ preference: UiGlassPreference }> = [
  { preference: "solid" },
  { preference: "balanced" },
  { preference: "transparent" },
];

export const uiRadiusPreferences: Array<{ preference: UiRadiusPreference }> = [
  { preference: "sharp" },
  { preference: "soft" },
  { preference: "rounded" },
];

export const uiShadowPreferences: Array<{ preference: UiShadowPreference }> = [
  { preference: "flat" },
  { preference: "soft" },
  { preference: "deep" },
];

export const uiMotionPreferences: Array<{ preference: UiMotionPreference }> = [
  { preference: "reduced" },
  { preference: "balanced" },
  { preference: "expressive" },
];

export const defaultUiAppearancePreferences: UiAppearancePreferences = {
  themePreference: "system",
  accent: "blue",
  density: "comfortable",
  glass: "balanced",
  radius: "soft",
  shadow: "soft",
  motion: "balanced",
};

export function normalizeUiThemePreference(value: unknown): UiThemePreference {
  if (value === "dark" || value === "light" || value === "system") {
    return value;
  }

  return "system";
}

export function normalizeUiAccentPreference(value: unknown): UiAccentPreference {
  return normalizePreference(value, ["blue", "teal", "violet", "amber", "rose"], "blue");
}

export function normalizeUiDensityPreference(value: unknown): UiDensityPreference {
  return normalizePreference(value, ["compact", "comfortable", "spacious"], "comfortable");
}

export function normalizeUiGlassPreference(value: unknown): UiGlassPreference {
  return normalizePreference(value, ["solid", "balanced", "transparent"], "balanced");
}

export function normalizeUiRadiusPreference(value: unknown): UiRadiusPreference {
  return normalizePreference(value, ["sharp", "soft", "rounded"], "soft");
}

export function normalizeUiShadowPreference(value: unknown): UiShadowPreference {
  return normalizePreference(value, ["flat", "soft", "deep"], "soft");
}

export function normalizeUiMotionPreference(value: unknown): UiMotionPreference {
  return normalizePreference(value, ["reduced", "balanced", "expressive"], "balanced");
}

export function normalizeUiAppearancePreferences(
  value: unknown,
  legacyThemePreference: UiThemePreference = defaultUiAppearancePreferences.themePreference,
): UiAppearancePreferences {
  const source = isRecord(value) ? value : {};
  return {
    themePreference: normalizeUiThemePreferenceWithFallback(source.themePreference, legacyThemePreference),
    accent: normalizeUiAccentPreference(source.accent),
    density: normalizeUiDensityPreference(source.density),
    glass: normalizeUiGlassPreference(source.glass),
    radius: normalizeUiRadiusPreference(source.radius),
    shadow: normalizeUiShadowPreference(source.shadow),
    motion: normalizeUiMotionPreference(source.motion),
  };
}

function normalizeUiThemePreferenceWithFallback(
  value: unknown,
  fallback: UiThemePreference,
): UiThemePreference {
  if (value === "dark" || value === "light" || value === "system") {
    return value;
  }

  return normalizeUiThemePreference(fallback);
}

export function normalizeResolvedUiTheme(value: unknown): ResolvedUiTheme {
  return value === "light" ? "light" : "dark";
}

export function getNextUiThemePreference(
  preference: UiThemePreference,
): UiThemePreference {
  const currentIndex = uiThemePreferenceCycle.indexOf(
    normalizeUiThemePreference(preference),
  );

  return (
    uiThemePreferenceCycle[(currentIndex + 1) % uiThemePreferenceCycle.length] ??
    "system"
  );
}

export function resolveUiTheme(
  preference: UiThemePreference,
  systemTheme: ResolvedUiTheme,
): ResolvedUiTheme {
  return preference === "system" ? systemTheme : preference;
}

export function formatUiThemePreference(preference: UiThemePreference, t: Translator = fallbackThemeT): string {
  switch (preference) {
    case "dark":
      return t("theme.dark");
    case "light":
      return t("theme.light");
    case "system":
    default:
      return t("theme.system");
  }
}

export function formatUiThemePreferenceDetail(preference: UiThemePreference, t: Translator = fallbackThemeT): string {
  switch (preference) {
    case "dark":
      return t("theme.darkDetail");
    case "light":
      return t("theme.lightDetail");
    case "system":
    default:
      return t("theme.systemDetail");
  }
}

export function formatResolvedUiTheme(theme: ResolvedUiTheme, t: Translator = fallbackThemeT): string {
  return theme === "light" ? t("theme.light") : t("theme.dark");
}

export function formatUiThemeStatus(
  preference: UiThemePreference,
  resolvedTheme: ResolvedUiTheme,
  t: Translator = fallbackThemeT,
): string {
  if (preference === "system") {
    return t("theme.statusSystem", {
      theme: resolvedTheme === "light" ? t("theme.currentLight") : t("theme.currentDark"),
    });
  }

  return t("theme.statusFixed", {
    theme: formatResolvedUiTheme(resolvedTheme, t),
  });
}

export function formatUiAccentPreference(preference: UiAccentPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.accent.${preference}`);
}

export function formatUiAccentPreferenceDetail(preference: UiAccentPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.accent.${preference}Detail`);
}

export function formatUiDensityPreference(preference: UiDensityPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.density.${preference}`);
}

export function formatUiDensityPreferenceDetail(preference: UiDensityPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.density.${preference}Detail`);
}

export function formatUiGlassPreference(preference: UiGlassPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.glass.${preference}`);
}

export function formatUiGlassPreferenceDetail(preference: UiGlassPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.glass.${preference}Detail`);
}

export function formatUiRadiusPreference(preference: UiRadiusPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.radius.${preference}`);
}

export function formatUiRadiusPreferenceDetail(preference: UiRadiusPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.radius.${preference}Detail`);
}

export function formatUiShadowPreference(preference: UiShadowPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.shadow.${preference}`);
}

export function formatUiShadowPreferenceDetail(preference: UiShadowPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.shadow.${preference}Detail`);
}

export function formatUiMotionPreference(preference: UiMotionPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.motion.${preference}`);
}

export function formatUiMotionPreferenceDetail(preference: UiMotionPreference, t: Translator = fallbackThemeT): string {
  return t(`appearance.motion.${preference}Detail`);
}

export function buildUiAppearanceCssVariables(
  preferences: UiAppearancePreferences,
): Record<string, string> {
  const normalized = normalizeUiAppearancePreferences(preferences);
  const accent = accentTokens[normalized.accent];
  const density = densityTokens[normalized.density];
  const glass = glassTokens[normalized.glass];
  const radius = radiusTokens[normalized.radius];
  const shadow = shadowTokens[normalized.shadow];
  const motion = motionTokens[normalized.motion];

  return {
    "--sv-accent-rgb": accent.rgb,
    "--sv-accent": `rgb(${accent.rgb})`,
    "--sv-accent-contrast": accent.contrast,
    "--sv-accent-soft": `rgb(${accent.rgb} / 0.18)`,
    "--sv-accent-medium": `rgb(${accent.rgb} / 0.34)`,
    "--sv-command-panel-alpha": glass.commandPanelAlpha,
    "--sv-command-feature-alpha": glass.featureAlpha,
    "--sv-card-alpha": glass.cardAlpha,
    "--sv-panel-blur": glass.blur,
    "--sv-card-radius": radius.cardRadius,
    "--sv-control-radius": radius.controlRadius,
    "--sv-density-gap": density.gap,
    "--sv-density-padding": density.padding,
    "--sv-control-height": density.controlHeight,
    "--sv-elevated-shadow": shadow.elevated,
    "--sv-soft-shadow": shadow.soft,
    "--sv-motion-duration": motion.duration,
    "--sv-motion-easing": motion.easing,
  };
}

export function getUiAppearanceDataAttributes(preferences: UiAppearancePreferences): Record<string, string> {
  const normalized = normalizeUiAppearancePreferences(preferences);
  return {
    "data-appearance-accent": normalized.accent,
    "data-appearance-density": normalized.density,
    "data-appearance-glass": normalized.glass,
    "data-appearance-radius": normalized.radius,
    "data-appearance-shadow": normalized.shadow,
    "data-appearance-motion": normalized.motion,
  };
}

function normalizePreference<T extends string>(
  value: unknown,
  allowed: readonly T[],
  fallback: T,
): T {
  return typeof value === "string" && allowed.includes(value as T) ? (value as T) : fallback;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

const accentTokens: Record<UiAccentPreference, { rgb: string; contrast: string }> = {
  blue: { rgb: "59 130 246", contrast: "#dbeafe" },
  teal: { rgb: "20 184 166", contrast: "#ccfbf1" },
  violet: { rgb: "124 58 237", contrast: "#ede9fe" },
  amber: { rgb: "245 158 11", contrast: "#fef3c7" },
  rose: { rgb: "244 63 94", contrast: "#ffe4e6" },
};

const densityTokens: Record<UiDensityPreference, { gap: string; padding: string; controlHeight: string }> = {
  compact: { gap: "8px", padding: "8px", controlHeight: "28px" },
  comfortable: { gap: "10px", padding: "10px", controlHeight: "32px" },
  spacious: { gap: "12px", padding: "12px", controlHeight: "36px" },
};

const glassTokens: Record<UiGlassPreference, { commandPanelAlpha: string; featureAlpha: string; cardAlpha: string; blur: string }> = {
  solid: { commandPanelAlpha: "0.98", featureAlpha: "0.98", cardAlpha: "0.07", blur: "10px" },
  balanced: { commandPanelAlpha: "0.96", featureAlpha: "0.94", cardAlpha: "0.045", blur: "22px" },
  transparent: { commandPanelAlpha: "0.9", featureAlpha: "0.86", cardAlpha: "0.032", blur: "30px" },
};

const radiusTokens: Record<UiRadiusPreference, { cardRadius: string; controlRadius: string }> = {
  sharp: { cardRadius: "4px", controlRadius: "4px" },
  soft: { cardRadius: "8px", controlRadius: "7px" },
  rounded: { cardRadius: "8px", controlRadius: "8px" },
};

const shadowTokens: Record<UiShadowPreference, { elevated: string; soft: string }> = {
  flat: { elevated: "none", soft: "none" },
  soft: {
    elevated: "0 22px 64px rgb(0 0 0 / 0.34)",
    soft: "0 12px 34px rgb(0 0 0 / 0.22)",
  },
  deep: {
    elevated: "0 30px 82px rgb(0 0 0 / 0.5)",
    soft: "0 20px 54px rgb(0 0 0 / 0.34)",
  },
};

const motionTokens: Record<UiMotionPreference, { duration: string; easing: string }> = {
  reduced: { duration: "0ms", easing: "linear" },
  balanced: { duration: "160ms", easing: "ease" },
  expressive: { duration: "260ms", easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
};

function fallbackThemeT(key: string, params: Record<string, string | number | boolean | null | undefined> = {}): string {
  const fallback: Record<string, string> = {
    "theme.system": "System",
    "theme.dark": "Dark",
    "theme.light": "Light",
    "theme.systemDetail": "Follow macOS/Windows appearance",
    "theme.darkDetail": "Use dark Sofvary UI",
    "theme.lightDetail": "Use light Sofvary UI",
    "theme.currentLight": "current light",
    "theme.currentDark": "current dark",
    "theme.statusSystem": "System / {theme}",
    "theme.statusFixed": "Fixed {theme}",
    "appearance.accent.blue": "Blue",
    "appearance.accent.blueDetail": "Default Sofvary blue",
    "appearance.accent.teal": "Teal",
    "appearance.accent.tealDetail": "Cool operation tint",
    "appearance.accent.violet": "Violet",
    "appearance.accent.violetDetail": "AI-oriented tint",
    "appearance.accent.amber": "Amber",
    "appearance.accent.amberDetail": "Warm focus tint",
    "appearance.accent.rose": "Rose",
    "appearance.accent.roseDetail": "High-energy tint",
    "appearance.density.compact": "Compact",
    "appearance.density.compactDetail": "Tighter spacing",
    "appearance.density.comfortable": "Comfortable",
    "appearance.density.comfortableDetail": "Balanced spacing",
    "appearance.density.spacious": "Spacious",
    "appearance.density.spaciousDetail": "Larger controls",
    "appearance.glass.solid": "Solid",
    "appearance.glass.solidDetail": "Less transparency",
    "appearance.glass.balanced": "Balanced",
    "appearance.glass.balancedDetail": "Default glass depth",
    "appearance.glass.transparent": "Transparent",
    "appearance.glass.transparentDetail": "More background bleed",
    "appearance.radius.sharp": "Sharp",
    "appearance.radius.sharpDetail": "Smaller corners",
    "appearance.radius.soft": "Soft",
    "appearance.radius.softDetail": "Default corners",
    "appearance.radius.rounded": "Rounded",
    "appearance.radius.roundedDetail": "Fuller controls",
    "appearance.shadow.flat": "Flat",
    "appearance.shadow.flatDetail": "Minimal elevation",
    "appearance.shadow.soft": "Soft",
    "appearance.shadow.softDetail": "Default elevation",
    "appearance.shadow.deep": "Deep",
    "appearance.shadow.deepDetail": "Stronger overlay depth",
    "appearance.motion.reduced": "Reduced",
    "appearance.motion.reducedDetail": "Minimize animation",
    "appearance.motion.balanced": "Balanced",
    "appearance.motion.balancedDetail": "Default response",
    "appearance.motion.expressive": "Expressive",
    "appearance.motion.expressiveDetail": "More animated feedback",
  };
  return (fallback[key] ?? key).replace(/\{([a-zA-Z0-9_.-]+)\}/g, (match, name) =>
    params[name] === undefined || params[name] === null ? match : String(params[name]),
  );
}
