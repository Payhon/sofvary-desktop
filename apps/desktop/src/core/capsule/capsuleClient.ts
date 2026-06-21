import { openSingleFilePath, saveFilePath } from "../../platform/dialogClient";
import { safeInvoke } from "../../platform/tauriClient";
import type {
  AppCapsuleExportPayload,
  AppCapsuleImportPayload,
  AppCapsuleOperationResult,
  PolicyApprovalSet,
  WorkspaceSummary,
} from "../../types";
import {
  buildExportCapsulePayload,
  buildExportDefaultFileName,
  buildImportCapsulePayload,
} from "./capsuleLogic";

const CAPSULE_FILTERS = [
  {
    name: "Sofvary App Capsule",
    extensions: ["sfcapsule"],
  },
];

export async function selectExportCapsulePath(workspace: WorkspaceSummary): Promise<string | null> {
  const selectedPath = await saveFilePath({
    title: "Export Sofvary App Capsule",
    defaultPath: buildExportDefaultFileName(workspace),
    filters: CAPSULE_FILTERS,
  });

  return selectedPath;
}

export async function selectImportCapsulePath(): Promise<string | null> {
  return openSingleFilePath({
    title: "Import Sofvary App Capsule",
    filters: CAPSULE_FILTERS,
  });
}

export async function exportAppCapsule(
  workspace: WorkspaceSummary,
  outputPath: string,
): Promise<AppCapsuleOperationResult> {
  const payload: AppCapsuleExportPayload = buildExportCapsulePayload(workspace, outputPath);

  return safeInvoke<AppCapsuleOperationResult>("export_app_capsule", { payload });
}

export async function importAppCapsule(
  inputPath: string,
  policyApprovals?: PolicyApprovalSet,
): Promise<AppCapsuleOperationResult> {
  const payload: AppCapsuleImportPayload = buildImportCapsulePayload(inputPath);
  payload.policyApprovals = policyApprovals;

  return safeInvoke<AppCapsuleOperationResult>("import_app_capsule", { payload });
}
