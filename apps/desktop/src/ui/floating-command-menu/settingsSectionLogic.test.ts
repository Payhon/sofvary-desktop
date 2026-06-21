import assert from "node:assert/strict";
import test from "node:test";
import {
  defaultSettingsSection,
  getSettingsSectionDetailKey,
  getSettingsSectionLabelKey,
  normalizeSettingsSection,
  settingsSectionOrder,
} from "./settingsSectionLogic";

test("settings sections have a stable default and order", () => {
  assert.equal(defaultSettingsSection, "general");
  assert.deepEqual(settingsSectionOrder, ["general", "appearance", "workspace", "runtime", "ai"]);
});

test("normalizeSettingsSection falls back to general for invalid values", () => {
  assert.equal(normalizeSettingsSection("ai"), "ai");
  assert.equal(normalizeSettingsSection("unknown"), "general");
  assert.equal(normalizeSettingsSection(null), "general");
});

test("settings section translation keys are deterministic", () => {
  assert.equal(getSettingsSectionLabelKey("appearance"), "settings.section.appearance");
  assert.equal(getSettingsSectionDetailKey("appearance"), "settings.section.appearanceDetail");
});
