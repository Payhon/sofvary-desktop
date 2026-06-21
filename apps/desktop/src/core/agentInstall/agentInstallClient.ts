import { safeInvoke } from "../../platform/tauriClient";
import type {
  AgentInstallCatalogItem,
  AgentInstallStatus,
  PolicyApprovalSet,
} from "../../types";

export async function listAgentInstallCatalog(): Promise<AgentInstallCatalogItem[]> {
  return safeInvoke<AgentInstallCatalogItem[]>("list_agent_install_catalog");
}

export async function getAgentInstallStatuses(): Promise<AgentInstallStatus[]> {
  return safeInvoke<AgentInstallStatus[]>("get_agent_install_statuses");
}

export async function refreshAgentInstallStatuses(): Promise<AgentInstallStatus[]> {
  return safeInvoke<AgentInstallStatus[]>("refresh_agent_install_statuses");
}

export async function startAgentInstall(
  agentId: string,
  policyApprovals: PolicyApprovalSet,
): Promise<AgentInstallStatus> {
  return safeInvoke<AgentInstallStatus>("start_agent_install", {
    payload: { agentId, policyApprovals },
  });
}

export async function cancelAgentInstall(agentId: string): Promise<AgentInstallStatus[]> {
  return safeInvoke<AgentInstallStatus[]>("cancel_agent_install", { agentId });
}

export async function openAgentInstallPage(agentId: string): Promise<void> {
  await safeInvoke<void>("open_agent_install_page", { agentId });
}
