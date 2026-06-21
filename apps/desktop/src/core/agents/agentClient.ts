import { safeInvoke } from "../../platform/tauriClient";
import type { AgentConfig, AgentConfigState, AgentTestRecord, DiscoveredAgent } from "../../types";

export async function discoverAgents(): Promise<DiscoveredAgent[]> {
  return safeInvoke<DiscoveredAgent[]>("discover_agents");
}

export async function listAgentConfigs(): Promise<AgentConfigState> {
  return safeInvoke<AgentConfigState>("list_agent_configs");
}

export async function upsertAgentConfig(config: AgentConfig): Promise<AgentConfigState> {
  return safeInvoke<AgentConfigState>("upsert_agent_config", { config });
}

export async function deleteAgentConfig(agentId: string): Promise<AgentConfigState> {
  return safeInvoke<AgentConfigState>("delete_agent_config", { agentId });
}

export async function setDefaultAgent(agentId: string): Promise<AgentConfigState> {
  return safeInvoke<AgentConfigState>("set_default_agent", { agentId });
}

export async function testAgentConnection(agentId: string): Promise<AgentTestRecord> {
  return safeInvoke<AgentTestRecord>("test_agent_connection", { agentId });
}
