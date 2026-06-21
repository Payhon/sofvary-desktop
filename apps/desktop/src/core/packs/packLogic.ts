import type { InstalledPackSummary, PackKind } from "../../types";

export type PackStatusKind = "idle" | "loading" | "success" | "error";

export interface PackStatusState {
  kind: PackStatusKind;
  detail?: string;
}

const KIND_ORDER: Record<PackKind, number> = {
  runtime: 0,
  harness: 1,
  plugin: 2,
};

export function formatPackStatus(state: PackStatusState): string {
  switch (state.kind) {
    case "loading":
      return "Loading installed packs...";
    case "success":
      return state.detail ?? "Installed packs are current.";
    case "error":
      return state.detail ?? "Installed packs could not be loaded.";
    case "idle":
      return "Installed packs ready.";
  }
}

export function formatPackLabel(pack: Pick<InstalledPackSummary, "id" | "version">): string {
  return `${pack.id}@${pack.version}`;
}

export function sortInstalledPacks(packs: InstalledPackSummary[]): InstalledPackSummary[] {
  return [...packs].sort((left, right) => {
    const kind = KIND_ORDER[left.kind] - KIND_ORDER[right.kind];
    if (kind !== 0) return kind;
    const id = left.id.localeCompare(right.id);
    if (id !== 0) return id;
    return left.version.localeCompare(right.version);
  });
}
