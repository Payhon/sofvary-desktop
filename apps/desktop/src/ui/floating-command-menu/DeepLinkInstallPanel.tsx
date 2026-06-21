import { describePreflight, formatPermissionSummary } from "../../core/deep-link/deepLinkLogic";
import type { DeepLinkInstallPreflight } from "../../types";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";

interface DeepLinkInstallPanelProps {
  value: string;
  statusLine: string;
  preflight: DeepLinkInstallPreflight | null;
  disabled: boolean;
  canInstall: boolean;
  onChange: (value: string) => void;
  onReview: () => void;
  onInstall: () => void;
  onClear: () => void;
}

export function DeepLinkInstallPanel({
  value,
  statusLine,
  preflight,
  disabled,
  canInstall,
  onChange,
  onReview,
  onInstall,
  onClear,
}: DeepLinkInstallPanelProps) {
  const { t } = useDesktopLocale();
  return (
    <section className="deep-link-panel" aria-label={t("deepLink.title")}>
      <div className="workspace-list__header">
        <div>
          <strong>{t("deepLink.title")}</strong>
          <small>{statusLine}</small>
        </div>
        <button type="button" disabled={disabled || !value} onClick={onClear} data-no-drag>
          {t("action.clear")}
        </button>
      </div>
      <input
        className="deep-link-input"
        type="url"
        value={value}
        placeholder={t("deepLink.placeholder")}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        data-no-drag
      />
      <div className="deep-link-actions">
        <button type="button" disabled={disabled || !value.trim()} onClick={onReview} data-no-drag>
          {t("action.review")}
        </button>
        <button type="button" disabled={disabled || !canInstall} onClick={onInstall} data-no-drag>
          {t("action.install")}
        </button>
      </div>
      {preflight ? (
        <div className="deep-link-summary">
          <strong>{describePreflight(preflight)}</strong>
          <ul>
            {formatPermissionSummary(preflight.permissionSummary).map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
