import test from "node:test";
import assert from "node:assert/strict";
import type { LlmProviderConfig, LlmProviderConfigState } from "../../types";
import {
  createLlmProviderConfigFromPreset,
  defaultLlmProviderConfig,
  getLlmModelOptions,
  getLlmProviderPreset,
  getDefaultLlmProvider,
  getLlmProviderStatusLine,
  getSelectableSofvaryAgentLlmProviders,
  getSelectedSofvaryAgentLlmProvider,
  isLlmProviderUsableForSofvaryAgent,
  llmProviderPresets,
  llmProviderRequiresApiKey,
  normalizeLlmProviderDraft,
  sortLlmProviders,
} from "./llmProviderLogic";

const openai: LlmProviderConfig = {
  providerId: "openai",
  label: "OpenAI",
  kind: "openai",
  baseUrl: null,
  model: "gpt-4.1",
  apiKeyRef: "sofvary.llm-provider.openai.api-key",
  enabled: true,
  lastTest: null,
};

test("sortLlmProviders puts default and enabled providers first", () => {
  const providers: LlmProviderConfig[] = [
    { ...openai, providerId: "disabled", label: "Disabled", enabled: false },
    { ...openai, providerId: "ollama", label: "Ollama", kind: "ollama", apiKeyRef: null },
    openai,
  ];

  assert.deepEqual(
    sortLlmProviders(providers, "ollama").map((provider) => provider.providerId),
    ["ollama", "openai", "disabled"],
  );
});

test("getDefaultLlmProvider resolves configured default provider", () => {
  const state: LlmProviderConfigState = {
    defaultProviderId: "openai",
    providers: [openai],
  };

  assert.equal(getDefaultLlmProvider(state)?.providerId, "openai");
});

test("getLlmProviderStatusLine distinguishes key reference and test state", () => {
  assert.equal(getLlmProviderStatusLine(null), "未配置 LLM Provider");
  assert.equal(getLlmProviderStatusLine({ ...openai, enabled: false }), "Provider 未启用");
  assert.equal(getLlmProviderStatusLine(openai), "已保存密钥引用");
  assert.equal(
    getLlmProviderStatusLine({
      ...openai,
      lastTest: {
        ok: true,
        checkedAt: "2026-06-10T08:00:00Z",
        detail: "ok",
      },
    }),
    "LLM 配置正常",
  );
});

test("Sofvary Agent LLM selector only includes usable provider configs", () => {
  const noKey: LlmProviderConfig = {
    ...openai,
    providerId: "kimi-missing-key",
    label: "Kimi Missing Key",
    kind: "kimi-coding",
    apiKeyRef: null,
  };
  const testedByEnv: LlmProviderConfig = {
    ...noKey,
    providerId: "kimi-tested",
    label: "Kimi Tested",
    lastTest: {
      ok: true,
      checkedAt: "2026-06-28T08:00:00Z",
      detail: "credential is available",
    },
  };
  const ollama: LlmProviderConfig = {
    ...openai,
    providerId: "ollama",
    label: "Ollama",
    kind: "ollama",
    apiKeyRef: null,
    model: "qwen2.5-coder:7b",
  };
  const disabled = { ...openai, providerId: "disabled", enabled: false };
  const state: LlmProviderConfigState = {
    defaultProviderId: "ollama",
    providers: [noKey, openai, testedByEnv, ollama, disabled],
  };

  assert.equal(llmProviderRequiresApiKey("kimi-coding"), true);
  assert.equal(llmProviderRequiresApiKey("ollama"), false);
  assert.equal(isLlmProviderUsableForSofvaryAgent(noKey), false);
  assert.equal(isLlmProviderUsableForSofvaryAgent(testedByEnv), true);

  assert.deepEqual(
    getSelectableSofvaryAgentLlmProviders(state).map((provider) => provider.providerId),
    ["ollama", "kimi-tested", "openai"],
  );
  assert.equal(getSelectedSofvaryAgentLlmProvider(null, state)?.providerId, "ollama");
  assert.equal(getSelectedSofvaryAgentLlmProvider("openai", state)?.providerId, "openai");
  assert.equal(getSelectedSofvaryAgentLlmProvider("kimi-missing-key", state)?.providerId, "ollama");
});

test("defaultLlmProviderConfig creates the built-in OpenAI provider draft", () => {
  assert.deepEqual(defaultLlmProviderConfig(), {
    providerId: "openai",
    label: "OpenAI",
    kind: "openai",
    baseUrl: null,
    model: "gpt-4.1",
    apiKeyRef: null,
    enabled: true,
    lastTest: null,
  });
});

test("llmProviderPresets include Pi API-key providers and model choices", () => {
  const kinds = llmProviderPresets.map((preset) => preset.kind);

  for (const kind of ["openai", "anthropic", "openrouter", "deepseek", "google", "groq", "xai", "kimi-coding", "ollama", "openai-compatible"]) {
    assert.ok(kinds.includes(kind as typeof llmProviderPresets[number]["kind"]));
    assert.ok(getLlmModelOptions(kind as typeof llmProviderPresets[number]["kind"]).length > 0);
  }
});

test("createLlmProviderConfigFromPreset maps provider kind and first model", () => {
  const preset = getLlmProviderPreset("deepseek");
  const config = createLlmProviderConfigFromPreset(preset);

  assert.equal(config.providerId, "deepseek");
  assert.equal(config.kind, "deepseek");
  assert.equal(config.model, preset.modelOptions[0]);
});

test("normalizeLlmProviderDraft trims editable fields and normalizes empty baseUrl", () => {
  const config = normalizeLlmProviderDraft({
    ...openai,
    providerId: " custom ",
    label: " Custom ",
    baseUrl: "   ",
    model: " gpt-4.1-mini ",
  });

  assert.equal(config.providerId, "custom");
  assert.equal(config.label, "Custom");
  assert.equal(config.baseUrl, null);
  assert.equal(config.model, "gpt-4.1-mini");
});
