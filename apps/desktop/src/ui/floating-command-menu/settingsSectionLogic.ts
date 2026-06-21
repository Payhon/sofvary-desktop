export type SettingsSectionKey = "general" | "appearance" | "workspace" | "runtime" | "ai";

export const defaultSettingsSection: SettingsSectionKey = "general";

export const settingsSectionOrder: SettingsSectionKey[] = [
  "general",
  "appearance",
  "workspace",
  "runtime",
  "ai",
];

const settingsSectionSet = new Set<string>(settingsSectionOrder);

export function normalizeSettingsSection(value: unknown): SettingsSectionKey {
  return typeof value === "string" && settingsSectionSet.has(value)
    ? (value as SettingsSectionKey)
    : defaultSettingsSection;
}

export function getSettingsSectionLabelKey(section: SettingsSectionKey): string {
  return `settings.section.${section}`;
}

export function getSettingsSectionDetailKey(section: SettingsSectionKey): string {
  return `settings.section.${section}Detail`;
}
