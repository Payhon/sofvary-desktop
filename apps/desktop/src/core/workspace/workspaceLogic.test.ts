import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { WorkspaceSummary } from "../../types";
import { buildWorkspacePreviewPolicyPayload } from "./workspaceLogic";

const workspace: WorkspaceSummary = {
  appId: "app_123",
  name: "Training Admin",
  mode: "react-project",
  updatedAt: "2026-06-28T00:00:00.000Z",
  root: "/tmp/sofvary/app_123",
};

describe("workspaceLogic", () => {
  it("builds the runtime policy payload for existing workspace preview", () => {
    assert.deepEqual(buildWorkspacePreviewPolicyPayload(workspace), {
      scope: "runtime-build",
      runtimeKind: "react-project",
      mode: "dev",
    });
  });

  it("preserves an agent id when a build thread is linked to the workspace", () => {
    assert.deepEqual(buildWorkspacePreviewPolicyPayload(workspace, "dev", "sofvary-agent"), {
      scope: "runtime-build",
      runtimeKind: "react-project",
      mode: "dev",
      agentId: "sofvary-agent",
    });
  });
});
