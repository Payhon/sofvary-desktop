import { safeInvoke } from "../../platform/tauriClient";
import type {
  AgentInteractionMode,
  BuildThreadDetail,
  BuildThreadPreviewRetryResult,
  BuildThreadSummary,
  HandoffActionResult,
  HandoffPromptCopyResult,
  HandoffRescanResult,
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
  agentMode?: AgentInteractionMode | null,
): Promise<BuildThreadSummary> {
  return safeInvoke<BuildThreadSummary>("start_build_thread", {
    payload: { requirement, runtimeKind, mode, policyApprovals, agentId, agentMode },
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
  agentMode?: AgentInteractionMode | null,
): Promise<BuildThreadSummary> {
  return safeInvoke<BuildThreadSummary>("continue_build_thread", {
    payload: { threadId, prompt, policyApprovals, agentMode },
  });
}

export async function cancelBuildThread(threadId: string): Promise<BuildThreadSummary> {
  return safeInvoke<BuildThreadSummary>("cancel_build_thread", { threadId });
}

export async function retryBuildThreadPreview(
  threadId: string,
  policyApprovals?: PolicyApprovalSet,
): Promise<BuildThreadPreviewRetryResult> {
  return safeInvoke<BuildThreadPreviewRetryResult>("retry_build_thread_preview", {
    payload: { threadId, policyApprovals },
  });
}

export async function copyHandoffPrompt(threadId: string): Promise<HandoffPromptCopyResult> {
  return safeInvoke<HandoffPromptCopyResult>("copy_handoff_prompt", { threadId });
}

export async function copyHandoffRepairPrompt(threadId: string): Promise<HandoffPromptCopyResult> {
  return safeInvoke<HandoffPromptCopyResult>("copy_handoff_repair_prompt", { threadId });
}

export async function openHandoffWorkspace(threadId: string): Promise<HandoffActionResult> {
  return safeInvoke<HandoffActionResult>("open_handoff_workspace", { threadId });
}

export async function openHandoffAgent(
  threadId: string,
  policyApprovals?: PolicyApprovalSet,
): Promise<HandoffActionResult> {
  return safeInvoke<HandoffActionResult>("open_handoff_agent", { threadId, policyApprovals });
}

export async function rescanHandoffWorkspace(threadId: string): Promise<HandoffRescanResult> {
  return safeInvoke<HandoffRescanResult>("rescan_handoff_workspace", { threadId });
}
