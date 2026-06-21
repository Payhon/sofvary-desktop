import { SOFVARY_API_BASE_URL } from "../cloudConfig";
import { installRegistryPack } from "../packs/packClient";
import type { InstallRegistryPackResult, PolicyApprovalSet } from "../../types";

interface PackReference {
  id: string;
  version: string;
}

export interface RegistrySoftwareSpecies {
  id: string;
  name: string;
  summary: string;
  description: string;
  runtimePack: PackReference;
  harnessPacks: PackReference[];
  tags: string[];
  status: string;
}

export interface InstallSoftwareSpeciesResult {
  species: RegistrySoftwareSpecies;
  installedPacks: InstallRegistryPackResult[];
}

export async function resolveSoftwareSpecies(id: string, accessToken?: string | null): Promise<RegistrySoftwareSpecies> {
  const headers = new Headers();
  if (accessToken) {
    headers.set("Authorization", `Bearer ${accessToken}`);
  }
  const response = await fetch(
    `${SOFVARY_API_BASE_URL}/v1/registry/software-species/resolve?id=${encodeURIComponent(id)}`,
    { headers },
  );
  if (!response.ok) {
    throw new Error(`software species resolve failed: ${response.status}`);
  }
  const payload = (await response.json()) as { species: RegistrySoftwareSpecies };
  return payload.species;
}

export async function installSoftwareSpecies(
  id: string,
  policyApprovals?: PolicyApprovalSet,
  accessToken?: string | null,
): Promise<InstallSoftwareSpeciesResult> {
  const species = await resolveSoftwareSpecies(id, accessToken);
  const refs = [species.runtimePack, ...species.harnessPacks];
  const installedPacks: InstallRegistryPackResult[] = [];
  for (const ref of refs) {
    installedPacks.push(await installRegistryPack({ id: ref.id, version: ref.version }, policyApprovals));
  }
  return { species, installedPacks };
}
