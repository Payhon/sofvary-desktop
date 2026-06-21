import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { createTranslator } from "@sofvary/i18n";
import type { PolicyDecision } from "../../types";
import {
  buildPolicyApprovalSet,
  emptyPolicyApprovals,
  formatPolicyActionLabel,
  formatPolicyBlockMessage,
  formatPolicyDecisionReasons,
  formatPolicyDecisionRisks,
  formatPolicyDecisionSummary,
  formatPolicyDecisionTitle,
  summarizePolicyDecisions,
} from "./policyLogic";

const dependencyDecision: PolicyDecision = {
  action: "dependency-install",
  decision: "requires-confirmation",
  title: "Dependency install requires approval",
  summary: "pnpm install is about to run",
  reasons: ["Dependency changes can alter local app behavior."],
  risks: ["New package code may run during install."],
  subject: "pnpm install",
};

describe("policyLogic", () => {
  it("returns empty approvals when no confirmation is needed", () => {
    assert.deepEqual(emptyPolicyApprovals(), { approved: [] });
    assert.deepEqual(
      summarizePolicyDecisions([{ ...dependencyDecision, decision: "allowed" }]),
      { kind: "clear" },
    );
  });

  it("builds approval grants for confirmation decisions", () => {
    assert.deepEqual(summarizePolicyDecisions([dependencyDecision]), {
      kind: "requires-confirmation",
      decisions: [dependencyDecision],
    });
    assert.deepEqual(buildPolicyApprovalSet([dependencyDecision]), {
      approved: [{ action: "dependency-install", subject: "pnpm install" }],
    });
  });

  it("preserves capsule import subjects after real capsule preflight", () => {
    const capsuleDecision: PolicyDecision = {
      action: "capsule-import",
      decision: "requires-confirmation",
      title: "Capsule import requires approval",
      summary: "Importing an App Capsule creates a local workspace.",
      reasons: ["Capsule imports require shell-owned permission review."],
      risks: ["Workspace: generated app files"],
      subject: "Sofvary App Capsule",
    };

    assert.deepEqual(buildPolicyApprovalSet([capsuleDecision]), {
      approved: [{ action: "capsule-import", subject: "Sofvary App Capsule" }],
    });
  });

  it("preserves runtime environment install subjects", () => {
    const runtimeEnvironmentDecision: PolicyDecision = {
      action: "runtime-environment-install",
      decision: "requires-confirmation",
      title: "Runtime environment install requires approval",
      summary: "Sofvary is about to install a managed runtime environment into application data.",
      reasons: [
        "Managed runtime environments stay inside the Sofvary data directory.",
        "Sofvary will verify artifact hashes before activating sidecar executables.",
        "This does not modify the system PATH or use a global package manager.",
      ],
      risks: [
        "nodejs 24.16.0 for windows-x64 (edaca9bd58ec8e92037dac4e877d52f6b8f430b81c18b57e264b4e2fb111cd56)",
      ],
      subject:
        "runtime-env:nodejs:24.16.0:windows-x64:edaca9bd58ec8e92037dac4e877d52f6b8f430b81c18b57e264b4e2fb111cd56",
    };

    assert.deepEqual(buildPolicyApprovalSet([runtimeEnvironmentDecision]), {
      approved: [
        {
          action: "runtime-environment-install",
          subject:
            "runtime-env:nodejs:24.16.0:windows-x64:edaca9bd58ec8e92037dac4e877d52f6b8f430b81c18b57e264b4e2fb111cd56",
        },
      ],
    });
  });

  it("localizes policy decision text for zh-CN", () => {
    const t = createTranslator("zh-CN", "desktop");
    const runtimeEnvironmentDecision: PolicyDecision = {
      action: "runtime-environment-install",
      decision: "requires-confirmation",
      title: "Runtime environment install requires approval",
      summary: "Sofvary is about to install a managed runtime environment into application data.",
      reasons: [
        "Managed runtime environments stay inside the Sofvary data directory.",
        "Sofvary will verify artifact hashes before activating sidecar executables.",
        "This does not modify the system PATH or use a global package manager.",
      ],
      risks: ["nodejs 24.16.0 for windows-x64 (sha256-value)"],
      subject: "runtime-env:nodejs:24.16.0:windows-x64:sha256-value",
    };

    assert.equal(formatPolicyActionLabel(runtimeEnvironmentDecision.action, t), "运行环境安装");
    assert.equal(formatPolicyDecisionTitle(runtimeEnvironmentDecision, t), "运行环境安装需要确认");
    assert.equal(
      formatPolicyDecisionSummary(runtimeEnvironmentDecision, t),
      "Sofvary 将在应用数据目录中安装托管运行环境。",
    );
    assert.deepEqual(formatPolicyDecisionReasons(runtimeEnvironmentDecision, t), [
      "托管运行环境会保留在 Sofvary 数据目录内。",
      "Sofvary 会在启用 sidecar 可执行文件前校验 artifact hash。",
      "此操作不会修改系统 PATH，也不会使用全局包管理器。",
    ]);
    assert.deepEqual(formatPolicyDecisionRisks(runtimeEnvironmentDecision, t), [
      "nodejs 24.16.0 用于 windows-x64（sha256-value）",
    ]);
  });

  it("prioritizes forbidden decisions", () => {
    const forbidden: PolicyDecision = {
      ...dependencyDecision,
      action: "command-execution",
      decision: "forbidden",
      title: "Command blocked",
      summary: "Global install is not allowed",
    };

    assert.deepEqual(summarizePolicyDecisions([dependencyDecision, forbidden]), {
      kind: "forbidden",
      decisions: [forbidden],
    });
    assert.equal(
      formatPolicyBlockMessage([forbidden]),
      "Command blocked: Global install is not allowed. Dependency changes can alter local app behavior.",
    );
  });

  it("localizes forbidden policy block messages", () => {
    const t = createTranslator("zh-CN", "desktop");
    const forbidden: PolicyDecision = {
      action: "command-execution",
      decision: "forbidden",
      title: "Command blocked",
      summary: "Global package installation is forbidden by the Phase 22 command policy.",
      reasons: ["Global installs can modify shared system or user tooling."],
      risks: ["npm install -g example"],
      subject: "npm install -g example",
    };

    assert.equal(
      formatPolicyBlockMessage([forbidden], t),
      "命令已阻止：Phase 22 命令策略禁止全局安装包。全局安装可能修改共享的系统或用户工具。",
    );
  });
});
