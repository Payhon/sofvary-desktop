import { safeInvoke } from "../../platform/tauriClient";

export interface InstallRegistrySkillPayload {
  id: string;
  version: string;
}

export interface InstalledSkillSummary {
  id: string;
  version: string;
  cachePath: string;
  sha256: string;
  executable: false;
}

export async function installRegistrySkill(payload: InstallRegistrySkillPayload): Promise<InstalledSkillSummary> {
  return safeInvoke<InstalledSkillSummary>("install_registry_skill", { payload });
}
