import { safeInvoke } from "../../platform/tauriClient";
import type { PolicyApprovalSet, RuntimeKind, RuntimeMode, RuntimePreview } from "../../types";

export async function runFakeStaticApp(requirement: string): Promise<RuntimePreview> {
  return safeInvoke<RuntimePreview>("run_fake_static_app", { requirement });
}

export async function runGeneratedApp(
  requirement: string,
  runtimeKind: RuntimeKind | null | undefined,
  mode: RuntimeMode = "dev",
  policyApprovals?: PolicyApprovalSet,
  agentId?: string | null,
  llmProviderId?: string | null,
): Promise<RuntimePreview> {
  return safeInvoke<RuntimePreview>("run_generated_app", {
    payload: { requirement, runtimeKind, mode, policyApprovals, agentId, llmProviderId },
  });
}
