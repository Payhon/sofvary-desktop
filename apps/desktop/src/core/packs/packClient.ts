import { safeInvoke } from "../../platform/tauriClient";
import type {
  InstalledPackSummary,
  InstallRegistryPackPayload,
  InstallRegistryPackResult,
  PolicyApprovalSet,
  ResolveRegistryPackPayload,
} from "../../types";

export async function listInstalledPacks(): Promise<InstalledPackSummary[]> {
  return safeInvoke<InstalledPackSummary[]>("list_installed_packs");
}

export async function resolveRegistryPack(payload: ResolveRegistryPackPayload): Promise<unknown> {
  return safeInvoke<unknown>("resolve_registry_pack", { payload });
}

export async function installRegistryPack(
  payload: InstallRegistryPackPayload,
  policyApprovals?: PolicyApprovalSet,
): Promise<InstallRegistryPackResult> {
  return safeInvoke<InstallRegistryPackResult>("install_registry_pack", {
    payload: { ...payload, policyApprovals },
  });
}
