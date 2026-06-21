import test from "node:test";
import assert from "node:assert/strict";
import { createTranslator } from "@sofvary/i18n";
import type { AgentConfigState, AgentInstallStatus } from "../../types";
import {
  canInstallAgent,
  formatAgentInstallDetail,
  formatAgentInstallState,
  formatAgentSource,
  formatAgentTest,
  getAgentIconLabel,
  summarizeAgentInstall,
  sortAgentInstallStatuses,
} from "./agentInstallLogic";

const baseStatus: AgentInstallStatus = {
  catalog: {
    id: "codex",
    label: "Codex",
    iconKey: "codex",
    provider: "codex",
    docsUrl: "https://example.com",
    installCapability: "manual-download",
    recommended: false,
    managed: false,
    supported: true,
    detectCommands: ["codex"],
    acp: { executable: "codex-acp", args: [] },
    cli: { executable: "codex", args: ["exec", "--json"] },
    versionCommand: { executable: "codex", args: ["--version"] },
  },
  configured: null,
  detected: false,
  source: null,
  executable: null,
  version: null,
  installState: "manual",
  detail: "Install from docs.",
  lastTest: null,
  lastInstall: null,
};

const agentState: AgentConfigState = {
  defaultAgentId: "opencode",
  agents: [],
};

test("sortAgentInstallStatuses puts Sofvary Pi and default installed agents first", () => {
  const statuses: AgentInstallStatus[] = [
    baseStatus,
    {
      ...baseStatus,
      catalog: { ...baseStatus.catalog, id: "opencode", label: "OpenCode", provider: "opencode" },
      configured: {
        id: "opencode",
        provider: "opencode",
        label: "OpenCode",
        enabled: true,
        acp: { executable: "/bin/opencode", args: ["acp"], env: {}, source: "external-path" },
        cli: null,
        allowCliFallback: false,
        lastTest: null,
      },
      detected: true,
      installState: "installed",
    },
    {
      ...baseStatus,
      catalog: {
        ...baseStatus.catalog,
        id: "sofvary-pi",
        label: "Sofvary Pi",
        iconKey: "sofvary-pi",
        provider: "sofvary-pi",
        managed: true,
        recommended: true,
        installCapability: "managed",
      },
      installState: "not-installed",
    },
  ];

  assert.deepEqual(
    sortAgentInstallStatuses(statuses, agentState).map((status) => status.catalog.id),
    ["sofvary-pi", "opencode", "codex"],
  );
});

test("formatAgentInstallState reports runtime and failed states", () => {
  assert.equal(
    formatAgentInstallState({ ...baseStatus, installState: "needs-runtime" }),
    "Runtime required",
  );
  assert.equal(formatAgentInstallState({ ...baseStatus, installState: "failed" }), "Install failed");
});

test("agent install formatters support zh translator", () => {
  const t = createTranslator("zh-CN");
  assert.equal(
    formatAgentInstallState({ ...baseStatus, installState: "needs-runtime" }, t),
    "需要运行环境",
  );
  assert.equal(formatAgentInstallState({ ...baseStatus, installState: "failed" }, t), "安装失败");
  assert.equal(formatAgentSource({ ...baseStatus, source: "external-path" }, t), "PATH 外部命令");
  assert.equal(formatAgentTest(baseStatus, t), "未测试");
  assert.equal(
    summarizeAgentInstall(
      {
        ...baseStatus,
        configured: {
          id: "codex",
          provider: "codex",
          label: "Codex",
          enabled: true,
          acp: { executable: "/bin/codex-acp", args: [], env: {}, source: "external-path" },
          cli: null,
          allowCliFallback: false,
          lastTest: null,
        },
        detected: true,
        source: "external-path",
      },
      t,
    ),
    "已配置 / 已发现 · ACP · PATH 外部命令",
  );
});

test("formatAgentInstallDetail localizes known backend detail strings", () => {
  assert.equal(
    formatAgentInstallDetail({
      ...baseStatus,
      detail: "已发现 Codex ACP；Codex CLI fallback 未发现或不可用。",
    }),
    "Codex ACP detected; Codex CLI fallback is missing or unavailable.",
  );

  assert.equal(
    formatAgentInstallDetail(
      {
        ...baseStatus,
        detail: "已在本机发现可用命令。",
      },
      createTranslator("zh-CN"),
    ),
    "已在本机发现可用命令。",
  );
});

test("canInstallAgent disables duplicate installs while active", () => {
  assert.equal(canInstallAgent(baseStatus, "codex"), false);
  assert.equal(canInstallAgent(baseStatus, null), true);
  assert.equal(
    canInstallAgent(
      { ...baseStatus, catalog: { ...baseStatus.catalog, supported: false } },
      null,
    ),
    false,
  );
});

test("getAgentIconLabel falls back for unknown keys", () => {
  assert.equal(getAgentIconLabel("sofvary-pi"), "Pi");
  assert.equal(getAgentIconLabel("new-agent"), "NE");
});
