import type {
  PreviewPolicyPayload,
  RuntimeMode,
  WorkspaceSummary,
} from "../../types";

export function buildWorkspacePreviewPolicyPayload(
  workspace: WorkspaceSummary,
  mode: RuntimeMode = "dev",
  agentId?: string | null,
): PreviewPolicyPayload {
  const payload: PreviewPolicyPayload = {
    scope: "runtime-build",
    runtimeKind: workspace.mode,
    mode,
  };

  if (agentId) {
    payload.agentId = agentId;
  }

  return payload;
}
