import { safeInvoke } from "../../platform/tauriClient";
import type {
  DeepLinkInstallPreflight,
  DeepLinkInstallResult,
  PolicyApprovalSet,
  RuntimeMode,
} from "../../types";

export async function prepareDeepLinkInstall(url: string): Promise<DeepLinkInstallPreflight> {
  return safeInvoke<DeepLinkInstallPreflight>("prepare_deep_link_install", { payload: { url } });
}

export async function installAppFromDeepLink(
  url: string,
  mode: RuntimeMode = "dev",
  policyApprovals?: PolicyApprovalSet,
): Promise<DeepLinkInstallResult> {
  return safeInvoke<DeepLinkInstallResult>("install_app_from_deep_link", {
    payload: { url, mode, confirmed: true, policyApprovals },
  });
}
