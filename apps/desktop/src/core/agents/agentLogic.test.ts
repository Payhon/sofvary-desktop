import test from "node:test";
import assert from "node:assert/strict";
import { createTranslator } from "@sofvary/i18n";
import type { AgentConfig, AgentConfigState, DiscoveredAgent } from "../../types";
import {
  discoverableAgentsToAdd,
  formatAgentTestRecord,
  formatDiscoveredAgentStatus,
  getAgentStatusLine,
  getSelectableAgents,
  getSelectedAgentId,
  sortAgents,
} from "./agentLogic";

const baseAgent: AgentConfig = {
  id: "codex",
  provider: "codex",
  label: "Codex",
  enabled: true,
  acp: {
    executable: "/bin/codex-acp",
    args: [],
    env: {},
    source: "external-path",
  },
  cli: null,
  allowCliFallback: false,
  lastTest: null,
};

test("sortAgents puts default and enabled agents first", () => {
  const agents = [
    { ...baseAgent, id: "opencode", label: "OpenCode" },
    { ...baseAgent, id: "claude-code", label: "Claude Code", enabled: false },
    baseAgent,
  ];

  assert.deepEqual(
    sortAgents(agents, "opencode").map((agent) => agent.id),
    ["opencode", "codex", "claude-code"],
  );
});

test("getSelectedAgentId falls back to configured default", () => {
  const state: AgentConfigState = {
    defaultAgentId: "codex",
    agents: [baseAgent],
  };

  assert.equal(getSelectedAgentId(null, state), "codex");
});

test("getSelectableAgents excludes disabled and incomplete agents", () => {
  const state: AgentConfigState = {
    defaultAgentId: "codex",
    agents: [
      baseAgent,
      { ...baseAgent, id: "missing", label: "Missing", acp: null, cli: null },
      { ...baseAgent, id: "disabled", label: "Disabled", enabled: false },
    ],
  };

  assert.deepEqual(
    getSelectableAgents(state).map((agent) => agent.id),
    ["codex"],
  );
});

test("getAgentStatusLine reports missing config", () => {
  assert.equal(getAgentStatusLine(null), "Agent is not configured");
});

test("agent status formatters support zh translator", () => {
  const t = createTranslator("zh-CN");
  assert.equal(getAgentStatusLine(null, t), "未配置 Agent");
  assert.equal(getAgentStatusLine({ ...baseAgent, enabled: false }, t), "Agent 未启用");
  assert.equal(
    formatAgentTestRecord({ ok: true, transport: "acp", detail: "ok", checkedAt: "2026-06-20T00:00:00Z" }, t),
    "ACP 通讯正常",
  );
  assert.equal(
    formatDiscoveredAgentStatus({ config: baseAgent, detected: true, status: "ACP available via /bin/codex-acp" }, t),
    "通过 /bin/codex-acp 发现 ACP",
  );
});

test("formatDiscoveredAgentStatus defaults to English", () => {
  assert.equal(
    formatDiscoveredAgentStatus({ config: baseAgent, detected: true, status: "CLI fallback available via /bin/codex" }),
    "CLI fallback available via /bin/codex",
  );
});

test("discoverableAgentsToAdd hides configured agents", () => {
  const discovered: DiscoveredAgent[] = [
    { config: baseAgent, detected: true, status: "found" },
    {
      config: { ...baseAgent, id: "opencode", provider: "opencode", label: "OpenCode" },
      detected: true,
      status: "found",
    },
  ];

  assert.deepEqual(
    discoverableAgentsToAdd(discovered, [baseAgent]).map((agent) => agent.config.id),
    ["opencode"],
  );
});
