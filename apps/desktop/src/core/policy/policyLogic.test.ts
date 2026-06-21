import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { PolicyDecision } from "../../types";
import {
  buildPolicyApprovalSet,
  emptyPolicyApprovals,
  formatPolicyBlockMessage,
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
      summary: "Sofvary is about to install Node.js.",
      reasons: ["Managed runtime environments stay inside Sofvary data."],
      risks: ["nodejs 24.16.0"],
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
});
