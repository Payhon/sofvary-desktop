import { safeInvoke } from "../../platform/tauriClient";
import type { PolicyPreview, PreviewPolicyPayload } from "../../types";

export async function previewPolicy(payload: PreviewPolicyPayload): Promise<PolicyPreview> {
  return safeInvoke<PolicyPreview>("preview_policy", { payload });
}
