import type { ReactNode } from "react";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";
import {
  getSettingsSectionDetailKey,
  getSettingsSectionLabelKey,
  normalizeSettingsSection,
  settingsSectionOrder,
  type SettingsSectionKey,
} from "./settingsSectionLogic";

interface SettingsSurfaceProps {
  activeSection: SettingsSectionKey;
  children: Partial<Record<SettingsSectionKey, ReactNode>>;
  onSectionChange: (section: SettingsSectionKey) => void;
}

export function SettingsSurface({
  activeSection,
  children,
  onSectionChange,
}: SettingsSurfaceProps) {
  const { t } = useDesktopLocale();
  const normalizedSection = normalizeSettingsSection(activeSection);
  const activeLabel = t(getSettingsSectionLabelKey(normalizedSection));
  const activeDetail = t(getSettingsSectionDetailKey(normalizedSection));

  return (
    <section className="settings-surface" aria-label={t("settings.title")}>
      <nav className="settings-surface__nav" aria-label={t("settings.navigation")}>
        {settingsSectionOrder.map((section) => {
          const label = t(getSettingsSectionLabelKey(section));
          const detail = t(getSettingsSectionDetailKey(section));
          const isActive = normalizedSection === section;
          return (
            <button
              key={section}
              type="button"
              className={isActive ? "is-active" : ""}
              aria-current={isActive ? "page" : undefined}
              onClick={() => onSectionChange(section)}
              data-no-drag
            >
              <strong>{label}</strong>
              <small>{detail}</small>
            </button>
          );
        })}
      </nav>
      <div className="settings-surface__content">
        <header className="settings-surface__heading">
          <strong>{activeLabel}</strong>
          <small>{activeDetail}</small>
        </header>
        <div className="settings-surface__panel">
          {children[normalizedSection] ?? null}
        </div>
      </div>
    </section>
  );
}
