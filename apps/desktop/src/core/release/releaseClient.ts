import { openDirectoryPath, openSingleFilePath } from "../../platform/dialogClient";
import { safeInvoke } from "../../platform/tauriClient";
import type {
  AppReleaseCapability,
  AppReleaseJob,
  AppReleasePayload,
  AppReleaseTargetPlatform,
  PackagerToolchainStatus,
  PolicyApprovalSet,
} from "../../types";

const ICON_FILTERS = [
  { name: "App icons", extensions: ["png", "ico", "icns"] },
];

export async function getAppReleaseCapabilities(): Promise<AppReleaseCapability> {
  return safeInvoke<AppReleaseCapability>("get_app_release_capabilities");
}

export async function getPackagerToolchainStatus(): Promise<PackagerToolchainStatus> {
  return safeInvoke<PackagerToolchainStatus>("get_packager_toolchain_status");
}

export async function startPackagerToolchainInstall(
  targetPlatform: AppReleaseTargetPlatform,
  policyApprovals: PolicyApprovalSet,
): Promise<PackagerToolchainStatus> {
  return safeInvoke<PackagerToolchainStatus>("start_packager_toolchain_install", {
    payload: { targetPlatform, policyApprovals },
  });
}

export async function startAppReleaseJob(payload: AppReleasePayload): Promise<AppReleaseJob> {
  return safeInvoke<AppReleaseJob>("start_app_release_job", { payload });
}

export async function cancelAppReleaseJob(jobId: string): Promise<AppReleaseJob> {
  return safeInvoke<AppReleaseJob>("cancel_app_release_job", {
    payload: { jobId },
  });
}

export async function openAppReleaseOutputFolder(path: string): Promise<void> {
  return safeInvoke<void>("open_app_release_output_folder", {
    payload: { path },
  });
}

export async function selectReleaseOutputFolder(): Promise<string | null> {
  return openDirectoryPath({ title: "Select published app output folder" });
}

export async function selectReleaseIconPath(): Promise<string | null> {
  return openSingleFilePath({
    title: "Select app icon",
    filters: ICON_FILTERS,
  });
}
