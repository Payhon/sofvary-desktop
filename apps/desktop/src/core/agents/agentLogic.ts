import type { AgentConfig, AgentConfigState, AgentTestRecord, DiscoveredAgent } from "../../types";

type Translator = (key: string, params?: Record<string, string | number | boolean | null | undefined>, fallback?: string) => string;

export function sortAgents(agents: AgentConfig[], defaultAgentId?: string | null): AgentConfig[] {
  return [...agents].sort((left, right) => {
    const leftDefault = left.id === defaultAgentId ? 0 : 1;
    const rightDefault = right.id === defaultAgentId ? 0 : 1;
    if (leftDefault !== rightDefault) return leftDefault - rightDefault;
    const leftEnabled = left.enabled ? 0 : 1;
    const rightEnabled = right.enabled ? 0 : 1;
    if (leftEnabled !== rightEnabled) return leftEnabled - rightEnabled;
    return left.label.localeCompare(right.label);
  });
}

export function getDefaultAgent(state: AgentConfigState): AgentConfig | null {
  return state.agents.find((agent) => agent.id === state.defaultAgentId) ?? null;
}

export function getSelectableAgents(state: AgentConfigState): AgentConfig[] {
  return sortAgents(state.agents, state.defaultAgentId).filter((agent) => isAgentReady(agent));
}

export function isAgentReady(agent: AgentConfig): boolean {
  return (
    agent.enabled &&
    Boolean(agent.acp || (agent.provider === "sofvary-pi" && agent.cli) || (agent.allowCliFallback && agent.cli))
  );
}

export function getSelectedAgentId(
  selectedAgentId: string | null,
  state: AgentConfigState,
): string | null {
  if (selectedAgentId && state.agents.some((agent) => agent.id === selectedAgentId && isAgentReady(agent))) {
    return selectedAgentId;
  }
  return state.defaultAgentId ?? getSelectableAgents(state)[0]?.id ?? null;
}

export function getAgentStatusLine(agent: AgentConfig | null, t: Translator = fallbackAgentT): string {
  if (!agent) return t("agent.status.notConfigured");
  if (!isAgentReady(agent)) return t("agent.status.disabled");
  if (!agent.lastTest) return t("agent.test.untested");
  return formatAgentTestRecord(agent.lastTest, t);
}

export function formatAgentTestRecord(record: AgentTestRecord, t: Translator = fallbackAgentT): string {
  const transport = record.transport.toUpperCase();
  return record.ok
    ? t("agent.test.communicationOk", { transport })
    : t("agent.test.communicationFailed", { transport });
}

export function discoverableAgentsToAdd(
  discovered: DiscoveredAgent[],
  configured: AgentConfig[],
): DiscoveredAgent[] {
  const configuredIds = new Set(configured.map((agent) => agent.id));
  return discovered
    .filter((agent) => agent.detected && !configuredIds.has(agent.config.id))
    .sort((left, right) => left.config.label.localeCompare(right.config.label));
}

export function formatDiscoveredAgentStatus(agent: DiscoveredAgent, t: Translator = fallbackAgentT): string {
  const acpPrefix = "ACP available via ";
  if (agent.status.startsWith(acpPrefix)) {
    return t("agent.discovered.acp", { path: agent.status.slice(acpPrefix.length) });
  }

  const cliPrefix = "CLI fallback available via ";
  if (agent.status.startsWith(cliPrefix)) {
    return t("agent.discovered.cli", { path: agent.status.slice(cliPrefix.length) });
  }

  if (agent.status === "Not found on this machine") {
    return t("agent.discovered.notFound");
  }

  return agent.status;
}

function fallbackAgentT(
  key: string,
  params: Record<string, string | number | boolean | null | undefined> = {},
): string {
  const fallback: Record<string, string> = {
    "agent.status.notConfigured": "Agent is not configured",
    "agent.status.disabled": "Agent is disabled",
    "agent.test.untested": "Untested",
    "agent.test.communicationOk": "{transport} communication OK",
    "agent.test.communicationFailed": "{transport} communication failed",
    "agent.discovered.acp": "ACP available via {path}",
    "agent.discovered.cli": "CLI fallback available via {path}",
    "agent.discovered.notFound": "Not found on this machine",
  };
  return (fallback[key] ?? key).replace(/\{([a-zA-Z0-9_.-]+)\}/g, (match, name) =>
    params[name] === undefined || params[name] === null ? match : String(params[name]),
  );
}
