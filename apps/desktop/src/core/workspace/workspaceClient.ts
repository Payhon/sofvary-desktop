import { safeInvoke } from "../../platform/tauriClient";
import type {
  AppBoxManifest,
  PolicyApprovalSet,
  RuntimeMode,
  RuntimePreview,
  WorkspaceSummary,
} from "../../types";

export async function createWorkspace(name: string): Promise<AppBoxManifest> {
  return safeInvoke<AppBoxManifest>("create_workspace", { name });
}

export async function listWorkspaces(): Promise<WorkspaceSummary[]> {
  return safeInvoke<WorkspaceSummary[]>("list_workspaces");
}

export async function deleteWorkspace(workspace: WorkspaceSummary): Promise<AppBoxManifest> {
  return safeInvoke<AppBoxManifest>("delete_workspace", { appId: workspace.appId });
}

export async function previewWorkspace(
  workspace: WorkspaceSummary,
  mode: RuntimeMode = "dev",
  policyApprovals?: PolicyApprovalSet,
): Promise<RuntimePreview> {
  return safeInvoke<RuntimePreview>("preview_workspace", {
    payload: { appId: workspace.appId, mode, policyApprovals },
  });
}
