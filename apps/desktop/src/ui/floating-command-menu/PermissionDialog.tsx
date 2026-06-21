import type { PolicyDecision } from "../../types";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";

interface PermissionDialogProps {
  title: string;
  decisions: PolicyDecision[];
  onApprove: () => void;
  onCancel: () => void;
}

export function PermissionDialog({
  title,
  decisions,
  onApprove,
  onCancel,
}: PermissionDialogProps) {
  const { t } = useDesktopLocale();
  return (
    <section
      className="permission-dialog"
      aria-label={t("permission.aria", {}, "Sofvary security policy approval")}
      role="dialog"
      aria-modal="true"
      data-no-drag
    >
      <div className="permission-dialog__surface">
        <header className="permission-dialog__header">
          <div>
            <span>{t("permission.title")}</span>
            <h2>{title}</h2>
          </div>
          <button type="button" aria-label={t("permission.cancelApproval")} onClick={onCancel}>
            X
          </button>
        </header>

        <div className="permission-dialog__body">
          {decisions.map((decision) => (
            <article key={`${decision.action}:${decision.subject ?? decision.title}`}>
              <div className="permission-dialog__decision-heading">
                <strong>{decision.title}</strong>
                <small>{decision.action}</small>
              </div>
              <p>{decision.summary}</p>
              {decision.subject ? <code>{decision.subject}</code> : null}
              {decision.reasons.length > 0 ? (
                <ul>
                  {decision.reasons.map((reason) => (
                    <li key={reason}>{reason}</li>
                  ))}
                </ul>
              ) : null}
              {decision.risks.length > 0 ? (
                <div className="permission-dialog__risks">
                  {decision.risks.map((risk) => (
                    <span key={risk}>{risk}</span>
                  ))}
                </div>
              ) : null}
            </article>
          ))}
        </div>

        <footer className="permission-dialog__actions">
          <button type="button" onClick={onCancel}>
            {t("action.cancel")}
          </button>
          <button type="button" className="permission-dialog__approve" onClick={onApprove}>
            {t("permission.approve")}
          </button>
        </footer>
      </div>
    </section>
  );
}
