import type { PolicyApprovalSet, PolicyDecision } from "../../types";

export type PolicyConfirmationState =
  | { kind: "clear" }
  | { kind: "requires-confirmation"; decisions: PolicyDecision[] }
  | { kind: "forbidden"; decisions: PolicyDecision[] };

export function emptyPolicyApprovals(): PolicyApprovalSet {
  return { approved: [] };
}

export function summarizePolicyDecisions(decisions: PolicyDecision[]): PolicyConfirmationState {
  const forbidden = decisions.filter((decision) => decision.decision === "forbidden");
  if (forbidden.length > 0) {
    return { kind: "forbidden", decisions: forbidden };
  }

  const requiresConfirmation = decisions.filter(
    (decision) => decision.decision === "requires-confirmation",
  );
  if (requiresConfirmation.length > 0) {
    return { kind: "requires-confirmation", decisions: requiresConfirmation };
  }

  return { kind: "clear" };
}

export function buildPolicyApprovalSet(decisions: PolicyDecision[]): PolicyApprovalSet {
  return {
    approved: decisions
      .filter((decision) => decision.decision === "requires-confirmation")
      .map((decision) => ({
        action: decision.action,
        subject: decision.subject ?? null,
      })),
  };
}

export function formatPolicyBlockMessage(decisions: PolicyDecision[]): string {
  const primary = decisions[0];
  if (!primary) {
    return "Security policy blocked the requested action.";
  }

  const reason = primary.reasons[0] ? ` ${primary.reasons[0]}` : "";
  return `${primary.title}: ${primary.summary}.${reason}`;
}
