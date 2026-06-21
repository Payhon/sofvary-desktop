import test from "node:test";
import assert from "node:assert/strict";
import type { LlmProviderConfig, LlmProviderConfigState } from "../../types";
import {
  createAiProviderBindingFromProvider,
  getAiAgentProviderBindingState,
  getDefaultBindableAiProvider,
  getProviderBindingStateForSpecies,
  normalizeAiProviderBinding,
  providerRequiresApiKey,
} from "./aiProviderBindingLogic";

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

const providerState: LlmProviderConfigState = {
  defaultProviderId: "openai",
  providers: [openai],
};

test("AI Agent App without binding needs provider binding", () => {
  const state = getAiAgentProviderBindingState(null, providerState);

  assert.equal(state.kind, "needs-provider-binding");
  assert.equal(state.actionLabel, "Bind provider");
  assert.equal(state.candidateProvider?.providerId, "openai");
});

test("AI Agent App binding is ready when provider is enabled and credential is referenced", () => {
  const binding = createAiProviderBindingFromProvider(openai, "2026-06-16T08:00:00Z");
  const state = getAiAgentProviderBindingState(binding, providerState);

  assert.equal(state.kind, "bound");
  assert.equal(state.provider?.providerId, "openai");
  assert.equal(state.binding?.apiKeyRef, "sofvary.llm-provider.openai.api-key");
  assert.equal(state.binding && "apiKey" in state.binding, false);
});

test("missing or disabled bound provider asks for rebind", () => {
  const binding = createAiProviderBindingFromProvider(openai, "2026-06-16T08:00:00Z");

  assert.equal(
    getAiAgentProviderBindingState(binding, { defaultProviderId: null, providers: [] }).kind,
    "needs-rebind",
  );
  assert.equal(
    getAiAgentProviderBindingState(binding, {
      defaultProviderId: "openai",
      providers: [{ ...openai, enabled: false }],
    }).kind,
    "needs-rebind",
  );
});

test("provider requiring an API key must have a key reference, not a raw key", () => {
  const providerWithoutKey = { ...openai, apiKeyRef: null };
  const binding = createAiProviderBindingFromProvider(providerWithoutKey, "2026-06-16T08:00:00Z");
  const state = getAiAgentProviderBindingState(binding, {
    defaultProviderId: "openai",
    providers: [providerWithoutKey],
  });

  assert.equal(providerRequiresApiKey(openai), true);
  assert.equal(state.kind, "needs-provider-binding");
  assert.match(state.reason, /saved API key reference/);
});

test("local providers that do not require keys can be bound without key references", () => {
  const ollama: LlmProviderConfig = {
    ...openai,
    providerId: "ollama",
    label: "Ollama",
    kind: "ollama",
    baseUrl: "http://localhost:11434/v1",
    model: "llama3.1:8b",
    apiKeyRef: null,
  };
  const binding = createAiProviderBindingFromProvider(ollama, "2026-06-16T08:00:00Z");

  assert.equal(providerRequiresApiKey(ollama), false);
  assert.equal(
    getAiAgentProviderBindingState(binding, {
      defaultProviderId: "ollama",
      providers: [ollama],
    }).kind,
    "bound",
  );
});

test("default bindable provider skips disabled and uncredentialed providers", () => {
  const ollama: LlmProviderConfig = {
    ...openai,
    providerId: "ollama",
    label: "Ollama",
    kind: "ollama",
    model: "llama3.1:8b",
    apiKeyRef: null,
  };

  assert.equal(
    getDefaultBindableAiProvider({
      defaultProviderId: "openai",
      providers: [
        { ...openai, apiKeyRef: null },
        { ...openai, providerId: "disabled", label: "Disabled", enabled: false },
        ollama,
      ],
    })?.providerId,
    "ollama",
  );
});

test("binding normalization strips secret-like accidental fields by selecting safe fields only", () => {
  const binding = normalizeAiProviderBinding({
    speciesId: "ai-agent-app",
    providerId: " openai ",
    providerKind: "openai",
    providerLabel: " OpenAI ",
    model: " gpt-4.1 ",
    apiKeyRef: " key-ref ",
    apiKey: "sk-secret",
    token: "secret-token",
  } as Parameters<typeof normalizeAiProviderBinding>[0] & { apiKey: string; token: string });

  assert.deepEqual(Object.keys(binding).sort(), [
    "apiKeyRef",
    "createdAt",
    "lastVerifiedAt",
    "model",
    "providerId",
    "providerKind",
    "providerLabel",
    "speciesId",
    "updatedAt",
  ]);
  assert.equal(binding.providerId, "openai");
  assert.equal(binding.apiKeyRef, "key-ref");
  assert.equal("token" in binding, false);
});

test("non AI Agent species reports binding as not required", () => {
  const state = getProviderBindingStateForSpecies(
    {
      label: "Task board",
      requiresProviderBinding: false,
    },
    null,
    providerState,
  );

  assert.equal(state.kind, "not-required");
  assert.equal(state.canRebind, false);
});
