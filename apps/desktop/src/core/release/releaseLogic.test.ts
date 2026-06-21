import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { AppReleaseCapability, PackagerToolchainStatus, WorkspaceSummary } from "../../types";
import {
  buildAppReleasePayload,
  buildReleaseDefaultName,
  canStartRelease,
  formatReleaseStatus,
  getCurrentReleasePlatform,
  getReleasePlatformReason,
  sanitizeReleaseFileStem,
} from "./releaseLogic";

const workspace: WorkspaceSummary = {
  appId: "app_123",
  name: "Customer CRM",
  mode: "react-sqlite",
  updatedAt: "2026-06-20T00:00:00Z",
  root: "/tmp/app_123",
};

const capabilities: AppReleaseCapability = {
  currentPlatform: "macos",
  beta: true,
  targetPlatforms: [
    {
      platform: "windows",
      label: "Windows",
      enabled: false,
      current: false,
      reason: "本机发布仅支持当前 OS",
      outputKind: "NSIS .exe (planned)",
    },
    {
      platform: "macos",
      label: "Mac",
      enabled: true,
      current: true,
      reason: "Unsigned beta packaging is available on this machine.",
      outputKind: ".dmg (planned)",
    },
    {
      platform: "linux",
      label: "Linux",
      enabled: false,
      current: false,
      reason: "本机发布仅支持当前 OS",
      outputKind: ".AppImage (planned)",
    },
  ],
  runtimes: [],
};

const readyToolchain: PackagerToolchainStatus = {
  platform: "macos",
  ready: true,
  beta: true,
  installActionAvailable: false,
  detail: "ready",
  requirements: [],
};

describe("releaseLogic", () => {
  it("builds release defaults from workspace names", () => {
    assert.equal(buildReleaseDefaultName(workspace), "Customer CRM");
    assert.equal(sanitizeReleaseFileStem(" Customer CRM! "), "customer-crm");
    assert.equal(sanitizeReleaseFileStem("!!!"), "sofvary-app");
  });

  it("keeps only current platform enabled", () => {
    assert.equal(getCurrentReleasePlatform(capabilities), "macos");
    assert.equal(getReleasePlatformReason(capabilities, "windows"), "本机发布仅支持当前 OS");
    assert.equal(
      canStartRelease({
        appName: "Customer CRM",
        outputDir: "/tmp/out",
        targetPlatform: "windows",
        capabilities,
        toolchainStatus: readyToolchain,
        busy: false,
      }),
      false,
    );
    assert.equal(
      canStartRelease({
        appName: "Customer CRM",
        outputDir: "/tmp/out",
        targetPlatform: "macos",
        capabilities,
        toolchainStatus: readyToolchain,
        busy: false,
      }),
      true,
    );
  });

  it("builds app release payload with policy approvals", () => {
    const payload = buildAppReleasePayload({
      workspace,
      appName: "Customer CRM",
      targetPlatform: "macos",
      outputDir: "/tmp/out",
      includeAiContinuation: true,
      stealthUi: { aiMenuLabel: "Tune app", aiShortcut: "CmdOrCtrl+Shift+U" },
      policyApprovals: { approved: [{ action: "app-release", subject: "subject" }] },
    });

    assert.equal(payload.appId, "app_123");
    assert.equal(payload.includeAiContinuation, true);
    assert.equal(payload.stealthUi.aiMenuLabel, "Tune app");
    assert.equal(payload.stealthUi.aiShortcut, "CmdOrCtrl+Shift+U");
    assert.equal(payload.stealthUi.aiPanelTitle, "AI Optimize");
    assert.deepEqual(payload.policyApprovals?.approved[0], {
      action: "app-release",
      subject: "subject",
    });
  });

  it("formats release status transitions", () => {
    assert.equal(formatReleaseStatus({ kind: "idle" }), "Publishing ready.");
    assert.match(formatReleaseStatus({ kind: "publishing", targetName: "CRM" }), /Publishing CRM/);
    assert.equal(formatReleaseStatus({ kind: "error", detail: "boom" }), "boom");
  });
});
