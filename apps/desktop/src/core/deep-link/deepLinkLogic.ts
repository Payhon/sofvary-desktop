import type { DeepLinkInstallPreflight, InstallPermissionSummary } from "../../types";

export type DeepLinkStatusKind = "idle" | "reviewing" | "ready" | "installing" | "success" | "error";

export interface DeepLinkStatusState {
  kind: DeepLinkStatusKind;
  detail?: string;
}

export function formatDeepLinkStatus(state: DeepLinkStatusState): string {
  switch (state.kind) {
    case "reviewing":
      return "Reviewing Sofvary deep link...";
    case "ready":
      return state.detail ?? "Review permissions before installing.";
    case "installing":
      return "Installing capsule into a new local workspace...";
    case "success":
      return state.detail ?? "Capsule installed and preview opened.";
    case "error":
      return state.detail ?? "Deep link install failed.";
    case "idle":
      return "Paste a Sofvary install link to review.";
  }
}

export function formatPermissionSummary(summary: InstallPermissionSummary): string[] {
  return [
    `Workspace read: ${formatList(summary.workspaceRead)}`,
    `Workspace write: ${formatList(summary.workspaceWrite)}`,
    `Local database: ${summary.localDatabase}`,
    `Network: ${summary.network}`,
    `Device access: ${summary.deviceAccess}`,
    `System access: ${summary.systemAccess}`,
    `Plugins: ${formatList(summary.pluginPacks)}`,
    `Requested: ${formatList(summary.requested)}`,
  ];
}

export function describePreflight(preflight: DeepLinkInstallPreflight): string {
  return `${preflight.app.name} v${preflight.version.version} / ${preflight.artifact.kind} / sha256 ${preflight.artifact.sha256.slice(0, 12)}`;
}

function formatList(values: string[]): string {
  return values.length > 0 ? values.join(", ") : "none";
}
