import type { LlmProviderConfig, LlmProviderConfigState, LlmProviderKind } from "../../types";
import { getLlmProviderPreset } from "../llmProviders/llmProviderLogic";
import {
  AI_AGENT_APP_SPECIES_ID,
  type SoftwareSpeciesCatalogItem,
} from "../softwareSpecies/softwareSpeciesLogic";

export type AiProviderBindingStateKind =
  | "not-required"
  | "bound"
  | "needs-provider-binding"
  | "needs-rebind";

export interface AiProviderBindingRecord {
  speciesId: typeof AI_AGENT_APP_SPECIES_ID | string;
  providerId: string | null;
  providerKind?: LlmProviderKind | null;
  providerLabel?: string | null;
  model?: string | null;
  apiKeyRef?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
  lastVerifiedAt?: string | null;
}

export interface AiProviderBindingState {
  kind: AiProviderBindingStateKind;
  binding: AiProviderBindingRecord | null;
  provider: LlmProviderConfig | null;
  candidateProvider: LlmProviderConfig | null;
  reason: string;
  actionLabel: string;
  canRebind: boolean;
}

export function getProviderBindingStateForSpecies(
  species: Pick<SoftwareSpeciesCatalogItem, "label" | "requiresProviderBinding">,
  binding: AiProviderBindingRecord | null | undefined,
  providerState: LlmProviderConfigState,
): AiProviderBindingState {
  if (!species.requiresProviderBinding) {
    return {
      kind: "not-required",
      binding: null,
      provider: null,
      candidateProvider: null,
      reason: `${species.label} does not require an AI provider binding.`,
      actionLabel: "Not required",
      canRebind: false,
    };
  }

  return getAiAgentProviderBindingState(binding, providerState);
}

export function getAiAgentProviderBindingState(
  binding: AiProviderBindingRecord | null | undefined,
  providerState: LlmProviderConfigState,
): AiProviderBindingState {
  const normalizedBinding = binding ? normalizeAiProviderBinding(binding) : null;
  const candidateProvider = getDefaultBindableAiProvider(providerState);

  if (!normalizedBinding?.providerId) {
    return {
      kind: "needs-provider-binding",
      binding: normalizedBinding,
      provider: null,
      candidateProvider,
      reason: "AI Agent App needs an AI provider binding before it can run.",
      actionLabel: "Bind provider",
      canRebind: true,
    };
  }

  const provider = providerState.providers.find(
    (item) => item.providerId === normalizedBinding.providerId,
  ) ?? null;

  if (!provider) {
    return {
      kind: "needs-rebind",
      binding: normalizedBinding,
      provider: null,
      candidateProvider,
      reason: `The saved provider binding points to ${normalizedBinding.providerId}, but that provider is no longer configured.`,
      actionLabel: "Rebind provider",
      canRebind: true,
    };
  }

  if (!provider.enabled) {
    return {
      kind: "needs-rebind",
      binding: normalizedBinding,
      provider,
      candidateProvider,
      reason: `${provider.label} is disabled. Choose an enabled provider for this AI Agent App.`,
      actionLabel: "Rebind provider",
      canRebind: true,
    };
  }

  if (!provider.model.trim()) {
    return {
      kind: "needs-rebind",
      binding: normalizedBinding,
      provider,
      candidateProvider,
      reason: `${provider.label} does not have a model selected.`,
      actionLabel: "Rebind provider",
      canRebind: true,
    };
  }

  if (providerRequiresApiKey(provider) && !provider.apiKeyRef) {
    return {
      kind: "needs-provider-binding",
      binding: normalizedBinding,
      provider,
      candidateProvider,
      reason: `${provider.label} needs a saved API key reference before this AI Agent App can run.`,
      actionLabel: "Bind provider",
      canRebind: true,
    };
  }

  return {
    kind: "bound",
    binding: createAiProviderBindingFromProvider(provider, normalizedBinding.updatedAt ?? undefined, normalizedBinding),
    provider,
    candidateProvider,
    reason: `${provider.label} is bound for AI Agent App generation.`,
    actionLabel: "Provider ready",
    canRebind: true,
  };
}

export function getDefaultBindableAiProvider(
  state: LlmProviderConfigState,
): LlmProviderConfig | null {
  return (
    [...state.providers]
      .filter((provider) => provider.enabled && provider.model.trim() && providerHasRequiredKey(provider))
      .sort((left, right) => {
        const leftDefault = left.providerId === state.defaultProviderId ? 0 : 1;
        const rightDefault = right.providerId === state.defaultProviderId ? 0 : 1;
        if (leftDefault !== rightDefault) return leftDefault - rightDefault;
        return left.label.localeCompare(right.label);
      })[0] ?? null
  );
}

export function createAiProviderBindingFromProvider(
  provider: LlmProviderConfig,
  timestamp?: string,
  existing?: AiProviderBindingRecord | null,
): AiProviderBindingRecord {
  const now = timestamp ?? new Date(0).toISOString();
  return normalizeAiProviderBinding({
    speciesId: AI_AGENT_APP_SPECIES_ID,
    providerId: provider.providerId,
    providerKind: provider.kind,
    providerLabel: provider.label,
    model: provider.model,
    apiKeyRef: provider.apiKeyRef ?? null,
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
    lastVerifiedAt: provider.lastTest?.ok ? provider.lastTest.checkedAt : existing?.lastVerifiedAt ?? null,
  });
}

export function normalizeAiProviderBinding(
  binding: Partial<AiProviderBindingRecord>,
): AiProviderBindingRecord {
  return {
    speciesId: normalizeText(binding.speciesId) ?? AI_AGENT_APP_SPECIES_ID,
    providerId: normalizeText(binding.providerId),
    providerKind: binding.providerKind ?? null,
    providerLabel: normalizeText(binding.providerLabel),
    model: normalizeText(binding.model),
    apiKeyRef: normalizeText(binding.apiKeyRef),
    createdAt: normalizeText(binding.createdAt),
    updatedAt: normalizeText(binding.updatedAt),
    lastVerifiedAt: normalizeText(binding.lastVerifiedAt),
  };
}

export function providerRequiresApiKey(provider: Pick<LlmProviderConfig, "kind">): boolean {
  return getLlmProviderPreset(provider.kind).apiKeyRequired;
}

function providerHasRequiredKey(provider: LlmProviderConfig): boolean {
  return !providerRequiresApiKey(provider) || Boolean(provider.apiKeyRef);
}

function normalizeText(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}
