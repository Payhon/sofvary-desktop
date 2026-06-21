import test from "node:test";
import assert from "node:assert/strict";
import {
  buildUiAppearanceCssVariables,
  defaultUiAppearancePreferences,
  formatUiAccentPreference,
  formatUiAccentPreferenceDetail,
  formatUiDensityPreference,
  formatUiGlassPreference,
  formatResolvedUiTheme,
  formatUiThemePreferenceDetail,
  formatUiThemePreference,
  formatUiThemeStatus,
  getNextUiThemePreference,
  normalizeUiAppearancePreferences,
  normalizeUiAccentPreference,
  normalizeUiDensityPreference,
  normalizeUiGlassPreference,
  normalizeUiMotionPreference,
  normalizeUiRadiusPreference,
  normalizeResolvedUiTheme,
  normalizeUiShadowPreference,
  normalizeUiThemePreference,
  resolveUiTheme,
  uiAccentPreferences,
  uiThemePreferences,
} from "./uiSettingsLogic";
import { createTranslator } from "@sofvary/i18n";

test("normalizeUiThemePreference defaults unknown values to system", () => {
  assert.equal(normalizeUiThemePreference("dark"), "dark");
  assert.equal(normalizeUiThemePreference("light"), "light");
  assert.equal(normalizeUiThemePreference("system"), "system");
  assert.equal(normalizeUiThemePreference("sepia"), "system");
  assert.equal(normalizeUiThemePreference(null), "system");
});

test("resolveUiTheme follows system only for system preference", () => {
  assert.equal(resolveUiTheme("system", "light"), "light");
  assert.equal(resolveUiTheme("system", "dark"), "dark");
  assert.equal(resolveUiTheme("dark", "light"), "dark");
  assert.equal(resolveUiTheme("light", "dark"), "light");
});

test("getNextUiThemePreference cycles system light dark", () => {
  assert.equal(getNextUiThemePreference("system"), "light");
  assert.equal(getNextUiThemePreference("light"), "dark");
  assert.equal(getNextUiThemePreference("dark"), "system");
});

test("theme labels and status text are stable", () => {
  assert.deepEqual(
    uiThemePreferences.map((item) => item.preference),
    ["system", "dark", "light"],
  );
  assert.equal(formatUiThemePreference("system"), "System");
  assert.equal(formatUiThemePreferenceDetail("dark"), "Use dark Sofvary UI");
  assert.equal(formatResolvedUiTheme("dark"), "Dark");
  assert.equal(formatResolvedUiTheme("light"), "Light");
  assert.equal(formatUiThemeStatus("system", "light"), "System / current light");
  assert.equal(formatUiThemeStatus("dark", "dark"), "Fixed Dark");
});

test("theme labels and status text can be localized to Chinese", () => {
  const t = createTranslator("zh-CN", "desktop");
  assert.equal(formatUiThemePreference("system", t), "跟随系统");
  assert.equal(formatUiThemePreferenceDetail("dark", t), "固定使用深色 Sofvary 界面");
  assert.equal(formatResolvedUiTheme("dark", t), "暗色");
  assert.equal(formatResolvedUiTheme("light", t), "亮色");
  assert.equal(formatUiThemeStatus("system", "light", t), "跟随系统 / 当前亮色");
  assert.equal(formatUiThemeStatus("dark", "dark", t), "固定暗色");
});

test("normalizeResolvedUiTheme keeps unsupported values safe", () => {
  assert.equal(normalizeResolvedUiTheme("light"), "light");
  assert.equal(normalizeResolvedUiTheme("dark"), "dark");
  assert.equal(normalizeResolvedUiTheme("unknown"), "dark");
});

test("normalizeUiAppearancePreferences fills defaults and rejects unsupported values", () => {
  assert.deepEqual(normalizeUiAppearancePreferences(null), defaultUiAppearancePreferences);
  assert.deepEqual(
    normalizeUiAppearancePreferences(
      {
        themePreference: "light",
        accent: "teal",
        density: "compact",
        glass: "transparent",
        radius: "sharp",
        shadow: "deep",
        motion: "expressive",
      },
      "dark",
    ),
    {
      themePreference: "light",
      accent: "teal",
      density: "compact",
      glass: "transparent",
      radius: "sharp",
      shadow: "deep",
      motion: "expressive",
    },
  );
  assert.deepEqual(
    normalizeUiAppearancePreferences(
      {
        themePreference: "sepia",
        accent: "lime",
        density: "dense",
        glass: "clear",
        radius: "pill",
        shadow: "huge",
        motion: "wild",
      },
      "dark",
    ),
    {
      ...defaultUiAppearancePreferences,
      themePreference: "dark",
    },
  );
});

test("individual appearance normalizers keep values bounded", () => {
  assert.equal(normalizeUiAccentPreference("rose"), "rose");
  assert.equal(normalizeUiAccentPreference("green"), "blue");
  assert.equal(normalizeUiDensityPreference("spacious"), "spacious");
  assert.equal(normalizeUiDensityPreference("dense"), "comfortable");
  assert.equal(normalizeUiGlassPreference("solid"), "solid");
  assert.equal(normalizeUiGlassPreference("clear"), "balanced");
  assert.equal(normalizeUiRadiusPreference("rounded"), "rounded");
  assert.equal(normalizeUiRadiusPreference("pill"), "soft");
  assert.equal(normalizeUiShadowPreference("flat"), "flat");
  assert.equal(normalizeUiShadowPreference("huge"), "soft");
  assert.equal(normalizeUiMotionPreference("reduced"), "reduced");
  assert.equal(normalizeUiMotionPreference("wild"), "balanced");
});

test("appearance options and CSS variables are stable", () => {
  assert.deepEqual(
    uiAccentPreferences.map((item) => item.preference),
    ["blue", "teal", "violet", "amber", "rose"],
  );
  assert.deepEqual(
    buildUiAppearanceCssVariables({
      themePreference: "dark",
      accent: "teal",
      density: "compact",
      glass: "solid",
      radius: "sharp",
      shadow: "flat",
      motion: "reduced",
    }),
    {
      "--sv-accent-rgb": "20 184 166",
      "--sv-accent": "rgb(20 184 166)",
      "--sv-accent-contrast": "#ccfbf1",
      "--sv-accent-soft": "rgb(20 184 166 / 0.18)",
      "--sv-accent-medium": "rgb(20 184 166 / 0.34)",
      "--sv-command-panel-alpha": "0.98",
      "--sv-command-feature-alpha": "0.98",
      "--sv-card-alpha": "0.07",
      "--sv-panel-blur": "10px",
      "--sv-card-radius": "4px",
      "--sv-control-radius": "4px",
      "--sv-density-gap": "8px",
      "--sv-density-padding": "8px",
      "--sv-control-height": "28px",
      "--sv-elevated-shadow": "none",
      "--sv-soft-shadow": "none",
      "--sv-motion-duration": "0ms",
      "--sv-motion-easing": "linear",
    },
  );
});

test("appearance labels and details can be localized", () => {
  const zh = createTranslator("zh-CN", "desktop");
  assert.equal(formatUiAccentPreference("blue"), "Blue");
  assert.equal(formatUiAccentPreferenceDetail("violet"), "AI-oriented tint");
  assert.equal(formatUiDensityPreference("compact", zh), "紧凑");
  assert.equal(formatUiGlassPreference("transparent", zh), "通透");
});
