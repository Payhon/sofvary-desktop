import { safeInvoke } from "../../platform/tauriClient";
import type {
  BuildThreadDetail,
  BuildThreadSummary,
  PolicyApprovalSet,
  RuntimeIntentSelection,
  RuntimeKind,
  RuntimeMode,
} from "../../types";

export async function analyzeBuildIntent(requirement: string): Promise<RuntimeIntentSelection> {
  return safeInvoke<RuntimeIntentSelection>("analyze_build_intent", {
    payload: { requirement },
  });
}

export async function startBuildThread(
  requirement: string,
  runtimeKind: RuntimeKind | null | undefined,
  mode: RuntimeMode = "dev",
  policyApprovals?: PolicyApprovalSet,
  agentId?: string | null,
): Promise<BuildThreadSummary> {
  return safeInvoke<BuildThreadSummary>("start_build_thread", {
    payload: { requirement, runtimeKind, mode, policyApprovals, agentId },
  });
}

export async function listBuildThreads(): Promise<BuildThreadSummary[]> {
  return safeInvoke<BuildThreadSummary[]>("list_build_threads");
}

export async function getBuildThread(threadId: string): Promise<BuildThreadDetail> {
  return safeInvoke<BuildThreadDetail>("get_build_thread", { threadId });
}

export async function deleteBuildThread(threadId: string): Promise<void> {
  return safeInvoke<void>("delete_build_thread", { threadId });
}

export async function continueBuildThread(
  threadId: string,
  prompt: string,
  policyApprovals?: PolicyApprovalSet,
): Promise<BuildThreadSummary> {
  return safeInvoke<BuildThreadSummary>("continue_build_thread", {
    payload: { threadId, prompt, policyApprovals },
  });
}

export async function cancelBuildThread(threadId: string): Promise<BuildThreadSummary> {
  return safeInvoke<BuildThreadSummary>("cancel_build_thread", { threadId });
}
