import { safeInvoke } from "../../platform/tauriClient";
import type {
  PolicyApprovalSet,
  RuntimeEnvironmentCatalogItem,
  RuntimeEnvironmentKind,
  RuntimeEnvironmentStatus,
} from "../../types";

export async function listRuntimeEnvironmentCatalog(): Promise<
  RuntimeEnvironmentCatalogItem[]
> {
  return safeInvoke<RuntimeEnvironmentCatalogItem[]>("list_runtime_environment_catalog");
}

export async function getRuntimeEnvironmentStatuses(): Promise<
  RuntimeEnvironmentStatus[]
> {
  return safeInvoke<RuntimeEnvironmentStatus[]>("get_runtime_environment_statuses");
}

export async function startRuntimeEnvironmentInstall(
  kind: RuntimeEnvironmentKind,
  version: string,
  policyApprovals: PolicyApprovalSet,
): Promise<RuntimeEnvironmentStatus> {
  return safeInvoke<RuntimeEnvironmentStatus>("start_runtime_environment_install", {
    payload: { kind, version, policyApprovals },
  });
}

export async function setActiveRuntimeEnvironmentVersion(
  kind: RuntimeEnvironmentKind,
  version: string,
): Promise<RuntimeEnvironmentStatus> {
  return safeInvoke<RuntimeEnvironmentStatus>("set_active_runtime_environment_version", {
    payload: { kind, version },
  });
}
