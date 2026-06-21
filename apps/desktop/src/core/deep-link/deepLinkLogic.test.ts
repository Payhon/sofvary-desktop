import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { describePreflight, formatDeepLinkStatus, formatPermissionSummary } from "./deepLinkLogic";
import type { DeepLinkInstallPreflight } from "../../types";

describe("deepLinkLogic", () => {
  it("formats installer status without implying policy enforcement", () => {
    assert.equal(formatDeepLinkStatus({ kind: "idle" }), "Paste a Sofvary install link to review.");
    assert.equal(
      formatDeepLinkStatus({ kind: "installing" }),
      "Installing capsule into a new local workspace...",
    );
  });

  it("formats permission summaries from capsule metadata", () => {
    const lines = formatPermissionSummary({
      workspaceRead: ["source/generated"],
      workspaceWrite: [],
      localDatabase: "none; includeData=false",
      network: "local-only",
      deviceAccess: "not granted in Phase 20",
      systemAccess: "not granted in Phase 20",
      requested: [],
      pluginPacks: ["sofvary.plugin.local-report@0.1.0"],
    });

    assert.deepEqual(lines, [
      "Workspace read: source/generated",
      "Workspace write: none",
      "Local database: none; includeData=false",
      "Network: local-only",
      "Device access: not granted in Phase 20",
      "System access: not granted in Phase 20",
      "Plugins: sofvary.plugin.local-report@0.1.0",
      "Requested: none",
    ]);
  });

  it("describes preflight metadata using exact app version and artifact hash", () => {
    const preflight = {
      app: { id: "app-intent-notes", name: "Intent Notes", summary: "", visibility: "public" },
      version: { version: "0.2.0" },
      artifact: { kind: "app-capsule", sha256: "a".repeat(64) },
    } as DeepLinkInstallPreflight;

    assert.equal(describePreflight(preflight), "Intent Notes v0.2.0 / app-capsule / sha256 aaaaaaaaaaaa");
  });
});
