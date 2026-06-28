import type { AgentConfigState, AgentInstallStateKind, AgentInstallStatus } from "../../types";

type Translator = (key: string, params?: Record<string, string | number | boolean | null | undefined>, fallback?: string) => string;

const statusPriority: Record<AgentInstallStateKind, number> = {
  installed: 0,
  "not-installed": 2,
  installing: -1,
  failed: 1,
  manual: 2,
  "needs-runtime": 3,
  unsupported: 4,
};

export function sortAgentInstallStatuses(
  statuses: AgentInstallStatus[],
  agentState: AgentConfigState,
): AgentInstallStatus[] {
  return [...statuses].sort((left, right) => {
    const leftDefault = left.catalog.id === agentState.defaultAgentId ? 0 : 1;
    const rightDefault = right.catalog.id === agentState.defaultAgentId ? 0 : 1;
    if (leftDefault !== rightDefault) return leftDefault - rightDefault;

    const leftInstalled = left.detected || left.configured ? 0 : 1;
    const rightInstalled = right.detected || right.configured ? 0 : 1;
    if (leftInstalled !== rightInstalled) return leftInstalled - rightInstalled;

    const priority = statusPriority[left.installState] - statusPriority[right.installState];
    if (priority !== 0) return priority;

    return left.catalog.label.localeCompare(right.catalog.label);
  });
}

export function formatAgentInstallState(status: AgentInstallStatus, t: Translator = fallbackAgentInstallT): string {
  if (status.configured && status.detected) return t("agent.installState.configuredDetected");
  if (status.configured) return t("agent.installState.configuredPending");
  switch (status.installState) {
    case "installed":
      return t("agent.installState.installed");
    case "installing":
      return t("agent.installState.installing");
    case "failed":
      return t("agent.installState.failed");
    case "manual":
      return status.detected ? t("agent.installState.detected") : t("agent.installState.manual");
    case "needs-runtime":
      return t("agent.installState.needsRuntime");
    case "unsupported":
      return t("agent.installState.unsupported");
    case "not-installed":
    default:
      return t("agent.installState.notInstalled");
  }
}

export function formatAgentConnection(status: AgentInstallStatus, t: Translator = fallbackAgentInstallT): string {
  if (status.configured?.provider === "sofvary-pi") return "Built-in";
  if (status.configured?.acp || status.catalog.acp) return "ACP";
  if (status.configured?.cli || status.catalog.cli) return "CLI";
  return t("agent.disconnected");
}

export function formatAgentSource(status: AgentInstallStatus, t: Translator = fallbackAgentInstallT): string {
  if (status.source === "bundled") return t("agent.source.bundled");
  if (status.source === "dev-override") return t("agent.source.devOverride");
  if (status.source === "external-path") return t("agent.source.externalPath");
  if (status.source === "manual") return t("agent.source.manual");
  return t("agent.source.notFound");
}

export function formatAgentTest(status: AgentInstallStatus, t: Translator = fallbackAgentInstallT): string {
  const record = status.configured?.lastTest ?? status.lastTest;
  if (!record) return t("agent.test.untested");
  const transport = record.transport.toUpperCase();
  return record.ok
    ? t("agent.test.ok", { transport })
    : t("agent.test.failed", { transport });
}

export function canInstallAgent(status: AgentInstallStatus, activeAgentInstallId: string | null): boolean {
  if (activeAgentInstallId) return false;
  if (!status.catalog.supported) return false;
  if (status.installState === "installing") return false;
  if (status.detected && status.configured) return false;
  return status.catalog.installCapability !== "unavailable";
}

export function getAgentInstallActionLabel(status: AgentInstallStatus, t: Translator = fallbackAgentInstallT): string {
  if (status.installState === "installing") return t("agent.installing");
  if (status.catalog.managed && status.detected) return t("action.update");
  if (status.catalog.managed) return t("action.install");
  return t("agent.installPage");
}

export function getAgentIconLabel(iconKey: string): string {
  const labels: Record<string, string> = {
    "sofvary-pi": "AI",
    codex: "Cx",
    "claude-code": "Cl",
    cursor: "Cu",
    opencode: "OC",
    "kimi-code": "Ki",
    qoder: "Qo",
    "deepseek-tui": "Ds",
  };
  return labels[iconKey] ?? iconKey.slice(0, 2).toUpperCase();
}

export function summarizeAgentInstall(status: AgentInstallStatus, t: Translator = fallbackAgentInstallT): string {
  const parts = [
    formatAgentInstallState(status, t),
    formatAgentConnection(status, t),
    formatAgentSource(status, t),
  ];
  if (status.version) parts.push(`v${status.version}`);
  return parts.join(" · ");
}

export function formatAgentInstallDetail(status: AgentInstallStatus, t: Translator = fallbackAgentInstallT): string {
  return formatAgentInstallDetailText(status.detail, t);
}

export function formatAgentInstallDetailText(detail: string, t: Translator = fallbackAgentInstallT): string {
  const normalizedDetail = detail.trim();
  if (normalizedDetail === "已发现 Codex ACP；Codex CLI fallback 未发现或不可用。") {
    return t("agent.detail.codexAcpNoCli");
  }
  if (normalizedDetail === "当前系统或 CPU 架构暂不支持。") {
    return t("agent.detail.unsupported");
  }
  if (normalizedDetail === "已在本机发现可用命令。") {
    return t("agent.detail.detected");
  }
  if (normalizedDetail === "需要先安装 Node.js。") {
    return t("agent.detail.nodeRequired");
  }
  if (normalizedDetail === "需要通过官方安装页安装，安装后刷新发现。") {
    return t("agent.detail.manualInstall");
  }
  if (normalizedDetail === "未在 Sofvary 受控目录或 PATH 中发现。") {
    return t("agent.detail.notFound");
  }
  return detail;
}

function fallbackAgentInstallT(
  key: string,
  params: Record<string, string | number | boolean | null | undefined> = {},
): string {
  const fallback: Record<string, string> = {
    "action.install": "Install",
    "action.update": "Update",
    "agent.disconnected": "Disconnected",
    "agent.installing": "Installing",
    "agent.installPage": "Install guide",
    "agent.installState.configuredDetected": "Configured / detected",
    "agent.installState.configuredPending": "Configured / pending detection",
    "agent.installState.installed": "Installed",
    "agent.installState.installing": "Installing",
    "agent.installState.failed": "Install failed",
    "agent.installState.detected": "Detected",
    "agent.installState.manual": "Manual install required",
    "agent.installState.needsRuntime": "Runtime required",
    "agent.installState.unsupported": "Unsupported on this platform",
    "agent.installState.notInstalled": "Not installed",
    "agent.source.bundled": "Sofvary managed directory",
    "agent.source.devOverride": "Development override directory",
    "agent.source.externalPath": "PATH external command",
    "agent.source.manual": "Manual configuration",
    "agent.source.notFound": "Path not found",
    "agent.test.untested": "Untested",
    "agent.test.ok": "{transport} OK",
    "agent.test.failed": "{transport} failed",
    "agent.detail.codexAcpNoCli": "Codex ACP detected; Codex CLI fallback is missing or unavailable.",
    "agent.detail.unsupported": "This OS or CPU architecture is not supported yet.",
    "agent.detail.detected": "A usable command was found on this machine.",
    "agent.detail.nodeRequired": "Node.js must be installed first.",
    "agent.detail.manualInstall": "Install from the official install page, then refresh detection.",
    "agent.detail.notFound": "Not found in the Sofvary managed directory or PATH.",
  };
  return (fallback[key] ?? key).replace(/\{([a-zA-Z0-9_.-]+)\}/g, (match, name) =>
    params[name] === undefined || params[name] === null ? match : String(params[name]),
  );
}
