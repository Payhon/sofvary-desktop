import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { WorkspaceSummary } from "../../types";
import {
  buildExportCapsulePayload,
  buildExportDefaultFileName,
  buildImportCapsulePayload,
  ensureCapsuleExtension,
  formatCapsuleStatus,
} from "./capsuleLogic";

const workspace: WorkspaceSummary = {
  appId: "app_123",
  name: "Customer Manager",
  mode: "react-vite",
  updatedAt: "2026-06-05T00:00:00.000Z",
  root: "/tmp/sofvary/app_123",
};

describe("capsule logic", () => {
  it("appends the .sfcapsule extension when exporting", () => {
    assert.equal(ensureCapsuleExtension("/tmp/customer-manager"), "/tmp/customer-manager.sfcapsule");
    assert.equal(
      ensureCapsuleExtension("/tmp/customer-manager.SFCAPSULE"),
      "/tmp/customer-manager.SFCAPSULE",
    );
  });

  it("builds export command payload with prompt history disabled by default", () => {
    assert.deepEqual(buildExportCapsulePayload(workspace, "/tmp/export"), {
      appId: "app_123",
      includePromptHistory: false,
      outputPath: "/tmp/export.sfcapsule",
    });
  });

  it("builds a dialog-safe default export file name", () => {
    assert.equal(buildExportDefaultFileName(workspace), "customer-manager-app_123.sfcapsule");
  });

  it("accepts only .sfcapsule files for import payloads", () => {
    assert.deepEqual(buildImportCapsulePayload("/tmp/import.sfcapsule"), {
      capsulePath: "/tmp/import.sfcapsule",
    });
    assert.throws(() => buildImportCapsulePayload("/tmp/import.zip"), /Only \.sfcapsule/);
  });

  it("formats concise status messages for menu feedback", () => {
    assert.equal(
      formatCapsuleStatus({ kind: "exporting", targetName: "Customer Manager" }),
      "Exporting Customer Manager...",
    );
    assert.equal(
      formatCapsuleStatus({ kind: "previewing", targetName: "Customer Manager" }),
      "Opening preview Customer Manager...",
    );
    assert.equal(
      formatCapsuleStatus({ kind: "error", detail: "Checksum validation failed." }),
      "Checksum validation failed.",
    );
  });
});
