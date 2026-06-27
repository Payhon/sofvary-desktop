import type { RuntimeChoice, RuntimeIntentSelection, RuntimeKind } from "../../types";

export const AI_AGENT_APP_SPECIES_ID = "ai-agent-app" as const;

export type SoftwareSpeciesId = string;
export type SoftwareSpeciesCategory = "runtime" | "productivity" | "ai-agent-app";

export interface SoftwareSpeciesCatalogItem {
  id: SoftwareSpeciesId;
  label: string;
  summary: string;
  category: SoftwareSpeciesCategory;
  runtimeKind: RuntimeKind;
  requiresProviderBinding: boolean;
  tags: readonly string[];
  intentSignals: readonly string[];
}

export interface SoftwareSpeciesCatalogFilter {
  categories?: readonly SoftwareSpeciesCategory[];
  includeRuntimeSpecies?: boolean;
  includeProductivitySpecies?: boolean;
  includeAiAgentSpecies?: boolean;
}

export interface SelectSoftwareSpeciesInput {
  requirement?: string | null;
  selectedSpeciesId?: string | null;
  selectedRuntimeKind?: RuntimeChoice | null;
  catalog?: readonly SoftwareSpeciesCatalogItem[];
}

export interface SoftwareSpeciesRank {
  species: SoftwareSpeciesCatalogItem;
  score: number;
  matchedSignals: string[];
}

export interface SoftwareSpeciesSelection {
  species: SoftwareSpeciesCatalogItem;
  speciesId: SoftwareSpeciesId;
  runtimeKind: RuntimeKind;
  confidence: number;
  reason: string;
  matchedSignals: string[];
  alternatives: SoftwareSpeciesCatalogItem[];
  source: RuntimeIntentSelection["source"];
  requiresProviderBinding: boolean;
}

const categoryPriority: Record<SoftwareSpeciesCategory, number> = {
  "ai-agent-app": 0,
  productivity: 1,
  runtime: 2,
};

export function getSoftwareSpeciesCatalog(
  filter: SoftwareSpeciesCatalogFilter = {},
  catalog: readonly SoftwareSpeciesCatalogItem[] = [],
): SoftwareSpeciesCatalogItem[] {
  const categories = filter.categories ? new Set(filter.categories) : null;
  return catalog.filter((species) => {
    if (categories && !categories.has(species.category)) return false;
    if (filter.includeRuntimeSpecies === false && species.category === "runtime") return false;
    if (filter.includeProductivitySpecies === false && species.category === "productivity") return false;
    if (filter.includeAiAgentSpecies === false && species.category === "ai-agent-app") return false;
    return true;
  });
}

export function findSoftwareSpecies(
  speciesId: string | null | undefined,
  catalog: readonly SoftwareSpeciesCatalogItem[] = [],
): SoftwareSpeciesCatalogItem | null {
  if (!speciesId) return null;
  return catalog.find((species) => species.id === speciesId) ?? null;
}

export function getRuntimeSoftwareSpecies(
  runtimeKind: RuntimeKind,
  catalog: readonly SoftwareSpeciesCatalogItem[] = [],
): SoftwareSpeciesCatalogItem {
  return (
    catalog.find((species) => species.category === "runtime" && species.runtimeKind === runtimeKind) ??
    catalog.find((species) => species.runtimeKind === runtimeKind) ??
    createRuntimeSpecies(runtimeKind)
  );
}

export function rankSoftwareSpecies(
  requirement: string | null | undefined,
  catalog: readonly SoftwareSpeciesCatalogItem[] = [],
): SoftwareSpeciesRank[] {
  const normalizedRequirement = normalizeIntentText(requirement ?? "");
  return catalog
    .map((species) => scoreSoftwareSpecies(species, normalizedRequirement))
    .sort((left, right) => {
      const scoreDiff = right.score - left.score;
      if (scoreDiff !== 0) return scoreDiff;
      const categoryDiff = categoryPriority[left.species.category] - categoryPriority[right.species.category];
      if (categoryDiff !== 0) return categoryDiff;
      return left.species.label.localeCompare(right.species.label);
    });
}

export function selectSoftwareSpecies(
  input: SelectSoftwareSpeciesInput = {},
): SoftwareSpeciesSelection {
  const catalog = input.catalog ?? [];
  const selectedSpecies = findSoftwareSpecies(input.selectedSpeciesId, catalog);
  if (selectedSpecies) {
    return createSelection(selectedSpecies, [], "manual", 1, `Selected ${selectedSpecies.label}.`);
  }

  if (input.selectedRuntimeKind && input.selectedRuntimeKind !== "auto") {
    const runtimeSpecies = getRuntimeSoftwareSpecies(input.selectedRuntimeKind, catalog);
    return createSelection(runtimeSpecies, [], "manual", 1, `Selected ${runtimeSpecies.label}.`);
  }

  const ranked = rankSoftwareSpecies(input.requirement, catalog);
  const best = ranked.find((rank) => rank.score > 0);
  if (!best) {
    const fallback = catalog.find((species) => species.category === "runtime") ?? catalog[0] ?? createRuntimeSpecies("runtime");
    return createSelection(
      fallback,
      ranked.slice(0, 3).map((rank) => rank.species).filter((species) => species.id !== fallback.id),
      "automatic",
      0.35,
      `No specific species signals matched; using ${fallback.label}.`,
    );
  }

  return createSelection(
    best.species,
    ranked
      .filter((rank) => rank.species.id !== best.species.id && rank.score > 0)
      .slice(0, 3)
      .map((rank) => rank.species),
    "automatic",
    Math.min(0.95, 0.45 + best.score / 12),
    `Matched ${best.matchedSignals.join(", ")} for ${best.species.label}.`,
    best.matchedSignals,
  );
}

export function toRuntimeIntentSelection(
  selection: SoftwareSpeciesSelection,
): RuntimeIntentSelection {
  return {
    runtimeKind: selection.runtimeKind,
    softwareType: selection.species.id,
    confidence: selection.confidence,
    reason: selection.reason,
    matchedSignals: selection.matchedSignals,
    alternatives: uniqueRuntimeKinds(
      selection.alternatives
        .map((species) => species.runtimeKind)
        .filter((runtimeKind) => runtimeKind !== selection.runtimeKind),
    ),
    source: selection.source,
  };
}

function createSelection(
  species: SoftwareSpeciesCatalogItem,
  alternatives: SoftwareSpeciesCatalogItem[],
  source: RuntimeIntentSelection["source"],
  confidence: number,
  reason: string,
  matchedSignals: string[] = [],
): SoftwareSpeciesSelection {
  return {
    species,
    speciesId: species.id,
    runtimeKind: species.runtimeKind,
    confidence,
    reason,
    matchedSignals,
    alternatives,
    source,
    requiresProviderBinding: species.requiresProviderBinding,
  };
}

function createRuntimeSpecies(runtimeKind: RuntimeKind): SoftwareSpeciesCatalogItem {
  return {
    id: runtimeKind,
    label: runtimeKind,
    summary: `${runtimeKind} runtime`,
    category: "runtime",
    runtimeKind,
    requiresProviderBinding: false,
    tags: [],
    intentSignals: [],
  };
}

function scoreSoftwareSpecies(
  species: SoftwareSpeciesCatalogItem,
  normalizedRequirement: string,
): SoftwareSpeciesRank {
  if (!normalizedRequirement) {
    return { species, score: 0, matchedSignals: [] };
  }

  const matchedSignals: string[] = [];
  const score = species.intentSignals.reduce((total, signal) => {
    const normalizedSignal = normalizeIntentText(signal);
    if (!normalizedSignal || !containsIntentSignal(normalizedRequirement, normalizedSignal)) {
      return total;
    }

    matchedSignals.push(signal);
    return total + (normalizedSignal.includes(" ") ? 3 : 1);
  }, 0);

  const categoryBoost = species.category === "runtime" || score === 0 ? 0 : 0.25;
  return { species, score: score + categoryBoost, matchedSignals };
}

function normalizeIntentText(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^\p{L}\p{N}+#]+/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function containsIntentSignal(text: string, signal: string): boolean {
  if (signal.includes(" ")) {
    return text.includes(signal);
  }
  return new RegExp(`(^|\\s)${escapeRegExp(signal)}(\\s|$)`).test(text);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function uniqueRuntimeKinds(runtimeKinds: RuntimeKind[]): RuntimeKind[] {
  return [...new Set(runtimeKinds)];
}
