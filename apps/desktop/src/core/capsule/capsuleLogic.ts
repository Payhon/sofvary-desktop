import type { AppCapsuleExportPayload, AppCapsuleImportPayload, WorkspaceSummary } from "../../types";

export const CAPSULE_EXTENSION = ".sfcapsule";

export type CapsuleStatusKind =
  | "idle"
  | "choosing-export"
  | "exporting"
  | "choosing-import"
  | "importing"
  | "previewing"
  | "deleting"
  | "success"
  | "error"
  | "canceled";

export interface CapsuleStatusState {
  kind: CapsuleStatusKind;
  targetName?: string;
  detail?: string;
}

export function hasCapsuleExtension(path: string): boolean {
  return path.toLowerCase().endsWith(CAPSULE_EXTENSION);
}

export function ensureCapsuleExtension(path: string): string {
  const normalizedPath = path.trim();
  if (!normalizedPath) {
    throw new Error("Capsule path is required.");
  }

  return hasCapsuleExtension(normalizedPath)
    ? normalizedPath
    : `${normalizedPath}${CAPSULE_EXTENSION}`;
}

export function buildExportDefaultFileName(workspace: WorkspaceSummary): string {
  const safeName = workspace.name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);

  return `${safeName || "sofvary-app"}-${workspace.appId}${CAPSULE_EXTENSION}`;
}

export function buildExportCapsulePayload(
  workspace: WorkspaceSummary,
  outputPath: string,
  includePromptHistory = false,
): AppCapsuleExportPayload {
  return {
    appId: workspace.appId,
    includePromptHistory,
    outputPath: ensureCapsuleExtension(outputPath),
  };
}

export function buildImportCapsulePayload(inputPath: string): AppCapsuleImportPayload {
  const normalizedPath = inputPath.trim();
  if (!normalizedPath) {
    throw new Error("Select a .sfcapsule file to import.");
  }
  if (!hasCapsuleExtension(normalizedPath)) {
    throw new Error("Only .sfcapsule files can be imported.");
  }

  return { capsulePath: normalizedPath };
}

export function formatCapsuleStatus(state: CapsuleStatusState): string {
  const target = state.targetName ? ` ${state.targetName}` : "";

  switch (state.kind) {
    case "choosing-export":
      return `Choose where to export${target}.`;
    case "exporting":
      return `Exporting${target}...`;
    case "choosing-import":
      return "Choose a .sfcapsule file to import.";
    case "importing":
      return "Importing capsule...";
    case "previewing":
      return `Opening preview${target}...`;
    case "deleting":
      return `Deleting${target}...`;
    case "success":
      return state.detail ?? `Capsule operation completed${target}.`;
    case "error":
      return state.detail ?? "Capsule operation failed.";
    case "canceled":
      return state.detail ?? "Capsule operation canceled.";
    case "idle":
      return "Capsule export/import ready.";
  }
}

export function getCapsuleErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}
