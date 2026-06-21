import type {
  AppReleaseCapability,
  AppReleasePayload,
  AppReleaseStealthUiSettings,
  AppReleaseTargetPlatform,
  PackagerToolchainStatus,
  PolicyApprovalSet,
  RuntimeKind,
  WorkspaceSummary,
} from "../../types";

export type ReleaseStatusKind =
  | "idle"
  | "choosing-output"
  | "checking-toolchain"
  | "installing-toolchain"
  | "ready"
  | "publishing"
  | "success"
  | "error"
  | "canceled";

export interface ReleaseStatusState {
  kind: ReleaseStatusKind;
  targetName?: string;
  detail?: string;
}

export const DEFAULT_RELEASE_PLATFORMS: AppReleaseTargetPlatform[] = [
  "windows",
  "macos",
  "linux",
];

export const DEFAULT_RELEASE_STEALTH_UI_SETTINGS: AppReleaseStealthUiSettings = {
  aiMenuLabel: "Optimize with AI",
  aiShortcut: "CmdOrCtrl+Shift+I",
  aiPanelTitle: "AI Optimize",
  providerSetupTitle: "Connect your AI provider",
  promptPlaceholder: "Describe what you want to improve in this app.",
};

export function buildReleaseDefaultName(workspace: WorkspaceSummary): string {
  const safeName = workspace.name
    .trim()
    .replace(/\s+/g, " ")
    .slice(0, 64);

  return safeName || "Sofvary App";
}

export function sanitizeReleaseFileStem(value: string): string {
  const safeName = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);

  return safeName || "sofvary-app";
}

export function getCurrentReleasePlatform(
  capabilities: AppReleaseCapability | null,
): AppReleaseTargetPlatform {
  return capabilities?.currentPlatform ?? platformFromNavigator();
}

export function platformFromNavigator(): AppReleaseTargetPlatform {
  if (typeof navigator === "undefined") return "linux";
  const platform = navigator.platform.toLowerCase();
  if (platform.includes("win")) return "windows";
  if (platform.includes("mac")) return "macos";
  return "linux";
}

export function isReleasePlatformEnabled(
  capabilities: AppReleaseCapability | null,
  platform: AppReleaseTargetPlatform,
): boolean {
  return capabilities?.targetPlatforms.find((item) => item.platform === platform)?.enabled ?? false;
}

export function getReleasePlatformReason(
  capabilities: AppReleaseCapability | null,
  platform: AppReleaseTargetPlatform,
): string {
  const capability = capabilities?.targetPlatforms.find((item) => item.platform === platform);
  return capability?.reason ?? "本机发布仅支持当前 OS";
}

export function runtimeReleaseCapability(
  capabilities: AppReleaseCapability | null,
  runtimeKind: RuntimeKind,
) {
  return capabilities?.runtimes.find((runtime) => runtime.runtimeKind === runtimeKind) ?? null;
}

export function buildAppReleasePayload(input: {
  workspace: WorkspaceSummary;
  appName: string;
  targetPlatform: AppReleaseTargetPlatform;
  outputDir: string;
  iconPath?: string | null;
  includeAiContinuation: boolean;
  stealthUi?: Partial<AppReleaseStealthUiSettings> | null;
  selectedRuntimePacks?: string[];
  selectedPluginPacks?: string[];
  policyApprovals?: PolicyApprovalSet;
}): AppReleasePayload {
  const appName = input.appName.trim();
  if (!appName) {
    throw new Error("Release app name is required.");
  }
  const outputDir = input.outputDir.trim();
  if (!outputDir) {
    throw new Error("Release output folder is required.");
  }

  return {
    appId: input.workspace.appId,
    appName,
    targetPlatform: input.targetPlatform,
    outputDir,
    iconPath: input.iconPath || null,
    includeAiContinuation: input.includeAiContinuation,
    stealthUi: normalizeReleaseStealthUiSettings(input.stealthUi),
    selectedRuntimePacks: input.selectedRuntimePacks ?? [],
    selectedPluginPacks: input.selectedPluginPacks ?? [],
    policyApprovals: input.policyApprovals,
  };
}

export function normalizeReleaseStealthUiSettings(
  settings?: Partial<AppReleaseStealthUiSettings> | null,
): AppReleaseStealthUiSettings {
  return {
    aiMenuLabel: textOrDefault(settings?.aiMenuLabel, DEFAULT_RELEASE_STEALTH_UI_SETTINGS.aiMenuLabel),
    aiShortcut: textOrDefault(settings?.aiShortcut, DEFAULT_RELEASE_STEALTH_UI_SETTINGS.aiShortcut),
    aiPanelTitle: textOrDefault(settings?.aiPanelTitle, DEFAULT_RELEASE_STEALTH_UI_SETTINGS.aiPanelTitle),
    providerSetupTitle: textOrDefault(
      settings?.providerSetupTitle,
      DEFAULT_RELEASE_STEALTH_UI_SETTINGS.providerSetupTitle,
    ),
    promptPlaceholder: textOrDefault(settings?.promptPlaceholder, DEFAULT_RELEASE_STEALTH_UI_SETTINGS.promptPlaceholder),
  };
}

function textOrDefault(value: string | undefined, fallback: string): string {
  const trimmed = value?.trim();
  return trimmed ? trimmed.slice(0, 96) : fallback;
}

export function canStartRelease(input: {
  appName: string;
  outputDir: string;
  targetPlatform: AppReleaseTargetPlatform;
  capabilities: AppReleaseCapability | null;
  toolchainStatus: PackagerToolchainStatus | null;
  busy: boolean;
}): boolean {
  if (input.busy) return false;
  if (!input.appName.trim() || !input.outputDir.trim()) return false;
  if (!isReleasePlatformEnabled(input.capabilities, input.targetPlatform)) return false;
  return input.toolchainStatus?.ready ?? false;
}

export function formatReleaseStatus(state: ReleaseStatusState): string {
  const target = state.targetName ? ` ${state.targetName}` : "";
  switch (state.kind) {
    case "choosing-output":
      return `Choose output folder${target}.`;
    case "checking-toolchain":
      return "Checking packager toolchain.";
    case "installing-toolchain":
      return state.detail ?? "Installing packager toolchain.";
    case "ready":
      return state.detail ?? "Release form is ready.";
    case "publishing":
      return `Publishing${target}...`;
    case "success":
      return state.detail ?? `Release completed${target}.`;
    case "error":
      return state.detail ?? "Release failed.";
    case "canceled":
      return state.detail ?? "Release canceled.";
    case "idle":
      return "Publishing ready.";
  }
}
