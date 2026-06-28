import type { LlmProviderConfig, LlmProviderConfigState, LlmProviderKind } from "../../types";

export interface LlmProviderPreset {
  kind: LlmProviderKind;
  providerId: string;
  label: string;
  baseUrl: string | null;
  apiKeyPlaceholder: string;
  apiKeyRequired: boolean;
  modelOptions: string[];
}

export const llmProviderPresets: LlmProviderPreset[] = [
  {
    kind: "openai",
    providerId: "openai",
    label: "OpenAI",
    baseUrl: null,
    apiKeyPlaceholder: "OPENAI_API_KEY",
    apiKeyRequired: true,
    modelOptions: ["gpt-4.1", "gpt-4.1-mini", "gpt-4o", "o4-mini"],
  },
  {
    kind: "anthropic",
    providerId: "anthropic",
    label: "Anthropic",
    baseUrl: null,
    apiKeyPlaceholder: "ANTHROPIC_API_KEY",
    apiKeyRequired: true,
    modelOptions: ["claude-sonnet-4-20250514", "claude-3-7-sonnet-latest", "claude-3-5-haiku-latest"],
  },
  {
    kind: "openrouter",
    providerId: "openrouter",
    label: "OpenRouter",
    baseUrl: null,
    apiKeyPlaceholder: "OPENROUTER_API_KEY",
    apiKeyRequired: true,
    modelOptions: ["openai/gpt-4.1", "anthropic/claude-sonnet-4", "deepseek/deepseek-chat"],
  },
  {
    kind: "deepseek",
    providerId: "deepseek",
    label: "DeepSeek",
    baseUrl: null,
    apiKeyPlaceholder: "DEEPSEEK_API_KEY",
    apiKeyRequired: true,
    modelOptions: ["deepseek-chat", "deepseek-reasoner"],
  },
  {
    kind: "google",
    providerId: "google",
    label: "Google Gemini",
    baseUrl: null,
    apiKeyPlaceholder: "GEMINI_API_KEY",
    apiKeyRequired: true,
    modelOptions: ["gemini-2.5-pro", "gemini-2.5-flash", "gemini-2.0-flash"],
  },
  {
    kind: "groq",
    providerId: "groq",
    label: "Groq",
    baseUrl: null,
    apiKeyPlaceholder: "GROQ_API_KEY",
    apiKeyRequired: true,
    modelOptions: ["llama-3.3-70b-versatile", "openai/gpt-oss-120b"],
  },
  {
    kind: "xai",
    providerId: "xai",
    label: "xAI",
    baseUrl: null,
    apiKeyPlaceholder: "XAI_API_KEY",
    apiKeyRequired: true,
    modelOptions: ["grok-4", "grok-3"],
  },
  {
    kind: "kimi-coding",
    providerId: "kimi-coding",
    label: "Kimi Coding",
    baseUrl: "https://api.kimi.com/coding/v1",
    apiKeyPlaceholder: "KIMI_API_KEY",
    apiKeyRequired: true,
    modelOptions: ["kimi-for-coding"],
  },
  {
    kind: "ollama",
    providerId: "ollama",
    label: "Ollama",
    baseUrl: "http://localhost:11434/v1",
    apiKeyPlaceholder: "Ollama ignores API key",
    apiKeyRequired: false,
    modelOptions: ["llama3.1:8b", "qwen2.5-coder:7b", "deepseek-coder-v2:16b"],
  },
  {
    kind: "openai-compatible",
    providerId: "openai-compatible",
    label: "OpenAI Compatible",
    baseUrl: "http://localhost:1234/v1",
    apiKeyPlaceholder: "OPENAI_API_KEY or service key",
    apiKeyRequired: false,
    modelOptions: ["gpt-4.1", "local-model", "qwen2.5-coder:7b"],
  },
];

export function sortLlmProviders(
  providers: LlmProviderConfig[],
  defaultProviderId?: string | null,
): LlmProviderConfig[] {
  return [...providers].sort((left, right) => {
    const leftDefault = left.providerId === defaultProviderId ? 0 : 1;
    const rightDefault = right.providerId === defaultProviderId ? 0 : 1;
    if (leftDefault !== rightDefault) return leftDefault - rightDefault;
    const leftEnabled = left.enabled ? 0 : 1;
    const rightEnabled = right.enabled ? 0 : 1;
    if (leftEnabled !== rightEnabled) return leftEnabled - rightEnabled;
    return left.label.localeCompare(right.label);
  });
}

export function getDefaultLlmProvider(state: LlmProviderConfigState): LlmProviderConfig | null {
  return state.providers.find((provider) => provider.providerId === state.defaultProviderId) ?? null;
}

export function getLlmProviderStatusLine(provider: LlmProviderConfig | null): string {
  if (!provider) return "未配置 LLM Provider";
  if (!provider.enabled) return "Provider 未启用";
  if (!provider.lastTest) return provider.apiKeyRef ? "已保存密钥引用" : "未测试";
  return provider.lastTest.ok ? "LLM 配置正常" : "LLM 配置失败";
}

export function llmProviderRequiresApiKey(kind: LlmProviderKind): boolean {
  return getLlmProviderPreset(kind).apiKeyRequired;
}

export function isLlmProviderUsableForSofvaryAgent(provider: LlmProviderConfig): boolean {
  if (!provider.enabled) return false;
  if (!provider.model.trim()) return false;
  if (!llmProviderRequiresApiKey(provider.kind)) return true;
  return Boolean(provider.apiKeyRef || provider.lastTest?.ok);
}

export function getSelectableSofvaryAgentLlmProviders(
  state: LlmProviderConfigState,
): LlmProviderConfig[] {
  return sortLlmProviders(state.providers, state.defaultProviderId).filter(
    isLlmProviderUsableForSofvaryAgent,
  );
}

export function getSelectedSofvaryAgentLlmProvider(
  selectedProviderId: string | null | undefined,
  state: LlmProviderConfigState,
): LlmProviderConfig | null {
  const providers = getSelectableSofvaryAgentLlmProviders(state);
  if (selectedProviderId) {
    const selected = providers.find((provider) => provider.providerId === selectedProviderId);
    if (selected) return selected;
  }
  const defaultProvider = providers.find((provider) => provider.providerId === state.defaultProviderId);
  return defaultProvider ?? providers[0] ?? null;
}

export function defaultLlmProviderConfig(): LlmProviderConfig {
  return createLlmProviderConfigFromPreset(llmProviderPresets[0]);
}

export function createLlmProviderConfigFromPreset(preset: LlmProviderPreset): LlmProviderConfig {
  return {
    providerId: preset.providerId,
    label: preset.label,
    kind: preset.kind,
    baseUrl: preset.baseUrl,
    model: preset.modelOptions[0] ?? "",
    apiKeyRef: null,
    enabled: true,
    lastTest: null,
  };
}

export function getLlmProviderPreset(kind: LlmProviderKind): LlmProviderPreset {
  return llmProviderPresets.find((preset) => preset.kind === kind) ?? llmProviderPresets[0];
}

export function getLlmModelOptions(kind: LlmProviderKind): string[] {
  return getLlmProviderPreset(kind).modelOptions;
}

export function normalizeLlmProviderDraft(config: LlmProviderConfig): LlmProviderConfig {
  return {
    ...config,
    providerId: config.providerId.trim(),
    label: config.label.trim(),
    baseUrl: config.baseUrl?.trim() || null,
    model: config.model.trim(),
  };
}
