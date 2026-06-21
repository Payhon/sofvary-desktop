import {
  formatPolicyActionLabel,
  formatPolicyDecisionReasons,
  formatPolicyDecisionRisks,
  formatPolicyDecisionSummary,
  formatPolicyDecisionTitle,
  formatPolicyDialogTitle,
} from "../../core/policy/policyLogic";
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
  const dialogTitle = formatPolicyDialogTitle(title, t);
  return (
    <section
      className="permission-dialog"
      aria-label={t("permission.aria")}
      role="dialog"
      aria-modal="true"
      data-no-drag
    >
      <div className="permission-dialog__surface">
        <header className="permission-dialog__header">
          <div>
            <span>{t("permission.title")}</span>
            <h2>{dialogTitle}</h2>
          </div>
          <button type="button" aria-label={t("permission.cancelApproval")} onClick={onCancel}>
            X
          </button>
        </header>

        <div className="permission-dialog__body">
          {decisions.map((decision) => {
            const reasons = formatPolicyDecisionReasons(decision, t);
            const risks = formatPolicyDecisionRisks(decision, t);
            return (
              <article key={`${decision.action}:${decision.subject ?? decision.title}`}>
                <div className="permission-dialog__decision-heading">
                  <strong>{formatPolicyDecisionTitle(decision, t)}</strong>
                  <small>{formatPolicyActionLabel(decision.action, t)}</small>
                </div>
                <p>{formatPolicyDecisionSummary(decision, t)}</p>
                {decision.subject ? <code>{decision.subject}</code> : null}
                {reasons.length > 0 ? (
                  <ul>
                    {reasons.map((reason, index) => (
                      <li key={`${decision.action}:reason:${index}`}>{reason}</li>
                    ))}
                  </ul>
                ) : null}
                {risks.length > 0 ? (
                  <div className="permission-dialog__risks">
                    {risks.map((risk, index) => (
                      <span key={`${decision.action}:risk:${index}`}>{risk}</span>
                    ))}
                  </div>
                ) : null}
              </article>
            );
          })}
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
