import { safeInvoke } from "../../platform/tauriClient";
import type { LlmProviderConfigState, LlmProviderConfig, LlmProviderTestRecord } from "../../types";

export async function listLlmProviderConfigs(): Promise<LlmProviderConfigState> {
  return safeInvoke<LlmProviderConfigState>("list_llm_provider_configs");
}

export async function upsertLlmProviderConfig(
  config: LlmProviderConfig,
  apiKey?: string,
): Promise<LlmProviderConfigState> {
  return safeInvoke<LlmProviderConfigState>("upsert_llm_provider_config", {
    payload: { config, apiKey },
  });
}

export async function deleteLlmProviderConfig(providerId: string): Promise<LlmProviderConfigState> {
  return safeInvoke<LlmProviderConfigState>("delete_llm_provider_config", { providerId });
}

export async function setDefaultLlmProvider(providerId: string): Promise<LlmProviderConfigState> {
  return safeInvoke<LlmProviderConfigState>("set_default_llm_provider", { providerId });
}

export async function testLlmProviderConfig(providerId: string): Promise<LlmProviderTestRecord> {
  return safeInvoke<LlmProviderTestRecord>("test_llm_provider_config", { providerId });
}
