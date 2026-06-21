import type {
  CurrentAppState,
  HarnessPolicy,
  PackReference,
  PromptEnvelope,
} from "./promptEnvelope";
import {
  STATIC_HTML_COMMAND_POLICY,
  STATIC_HTML_ENTRYPOINT,
  STATIC_HTML_FILE_SYSTEM_POLICY,
  STATIC_HTML_GENERATED_ROOT,
  STATIC_HTML_HARNESS_PACK_ID,
  STATIC_HTML_HARNESS_POLICY,
  STATIC_HTML_OUTPUT_CONTRACT,
  STATIC_HTML_PACK_VERSION,
  STATIC_HTML_RUNTIME_PACK_ID,
  STATIC_HTML_RUNTIME_POLICY,
} from "./staticHtmlHarness";
import {
  REACT_VITE_COMMAND_POLICY,
  REACT_VITE_ENTRYPOINT,
  REACT_VITE_FILE_SYSTEM_POLICY,
  REACT_VITE_GENERATED_ROOT,
  REACT_VITE_HARNESS_PACK_ID,
  REACT_VITE_HARNESS_POLICY,
  REACT_VITE_OUTPUT_CONTRACT,
  REACT_VITE_PACK_VERSION,
  REACT_VITE_RUNTIME_PACK_ID,
  REACT_VITE_RUNTIME_POLICY,
} from "./reactViteHarness";
import {
  REACT_SQLITE_COMMAND_POLICY,
  REACT_SQLITE_ENTRYPOINT,
  REACT_SQLITE_FILE_SYSTEM_POLICY,
  REACT_SQLITE_GENERATED_ROOT,
  REACT_SQLITE_HARNESS_PACK_ID,
  REACT_SQLITE_HARNESS_POLICY,
  REACT_SQLITE_OUTPUT_CONTRACT,
  REACT_SQLITE_PACK_VERSION,
  REACT_SQLITE_RUNTIME_PACK_ID,
  REACT_SQLITE_RUNTIME_POLICY,
} from "./reactSqliteHarness";
import {
  CANVAS2D_COMMAND_POLICY,
  CANVAS2D_ENTRYPOINT,
  CANVAS2D_FILE_SYSTEM_POLICY,
  CANVAS2D_GENERATED_ROOT,
  CANVAS2D_HARNESS_PACK_ID,
  CANVAS2D_HARNESS_POLICY,
  CANVAS2D_OUTPUT_CONTRACT,
  CANVAS2D_PACK_VERSION,
  CANVAS2D_RUNTIME_PACK_ID,
  CANVAS2D_RUNTIME_POLICY,
} from "./canvas2dHarness";
import {
  AI_AGENT_APP_ALLOWED_FILES,
  AI_AGENT_APP_HARNESS_POLICY,
  AI_AGENT_APP_PACK_VERSION,
  AI_AGENT_APP_RUNTIME_PACK_ID,
  DATA_TABLE_ALLOWED_FILES,
  DATA_TABLE_HARNESS_PACK_ID,
  DATA_TABLE_HARNESS_POLICY,
  DATA_TABLE_PACK_VERSION,
  DATA_TABLE_RUNTIME_PACK_ID,
  DESKTOP_WIDGET_ALLOWED_FILES,
  DESKTOP_WIDGET_HARNESS_PACK_ID,
  DESKTOP_WIDGET_HARNESS_POLICY,
  DESKTOP_WIDGET_PACK_VERSION,
  DESKTOP_WIDGET_RUNTIME_PACK_ID,
  FILE_PROCESSOR_ALLOWED_FILES,
  FILE_PROCESSOR_HARNESS_PACK_ID,
  FILE_PROCESSOR_HARNESS_POLICY,
  FILE_PROCESSOR_PACK_VERSION,
  FILE_PROCESSOR_RUNTIME_PACK_ID,
  GENERATED_PROJECT_COMMAND_POLICY,
  GENERATED_PROJECT_ROOT,
  GENERATED_REACT_ENTRYPOINT,
  MARKDOWN_KNOWLEDGE_ALLOWED_FILES,
  MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID,
  MARKDOWN_KNOWLEDGE_HARNESS_POLICY,
  MARKDOWN_KNOWLEDGE_PACK_VERSION,
  MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID,
  MULTIMODAL_STUDIO_AGENT_HARNESS_PACK_ID,
  generatedProjectFileSystemPolicy,
  generatedProjectOutputContract,
  generatedProjectRuntimePolicy,
} from "./projectRuntimeHarnesses";

export interface RuntimePackInput {
  id: string;
  version: string;
  runtime: {
    kind:
      | "static-html"
      | "react-vite"
      | "react-sqlite"
      | "canvas2d"
      | "markdown-knowledge"
      | "data-table"
      | "file-processor"
      | "desktop-widget"
      | "ai-agent-app";
    generatedRoot: string;
    entrypoint: string;
    bind: "127.0.0.1";
    network: "local-only";
  };
}

export interface HarnessPackInput {
  id: string;
  version: string;
  runtime?: string;
  instructions: {
    system: string[];
    fileSystemPolicy: string[];
    outputRules: string[];
  };
}

export interface HarnessFragment {
  pack: PackReference;
  policy: HarnessPolicy;
  acceptanceCriteria: string[];
}

export interface CreateStaticHtmlPromptEnvelopeInput {
  userIntent: string;
  currentAppState: CurrentAppState;
  runtimePack?: RuntimePackInput;
  harnessPacks?: HarnessPackInput[];
  envelopeId?: string;
  createdAt?: string;
}

export interface CreateReactVitePromptEnvelopeInput {
  userIntent: string;
  currentAppState: CurrentAppState;
  runtimePack?: RuntimePackInput;
  harnessPacks?: HarnessPackInput[];
  envelopeId?: string;
  createdAt?: string;
}

export interface CreateReactSqlitePromptEnvelopeInput {
  userIntent: string;
  currentAppState: CurrentAppState;
  runtimePack?: RuntimePackInput;
  harnessPacks?: HarnessPackInput[];
  envelopeId?: string;
  createdAt?: string;
}

export interface CreateCanvas2dPromptEnvelopeInput {
  userIntent: string;
  currentAppState: CurrentAppState;
  runtimePack?: RuntimePackInput;
  harnessPacks?: HarnessPackInput[];
  envelopeId?: string;
  createdAt?: string;
}

export interface CreateGeneratedProjectPromptEnvelopeInput {
  userIntent: string;
  currentAppState: CurrentAppState;
  runtimePack?: RuntimePackInput;
  harnessPacks?: HarnessPackInput[];
  envelopeId?: string;
  createdAt?: string;
}

const DEFAULT_RUNTIME_PACK: RuntimePackInput = {
  id: STATIC_HTML_RUNTIME_PACK_ID,
  version: STATIC_HTML_PACK_VERSION,
  runtime: {
    kind: "static-html",
    generatedRoot: STATIC_HTML_GENERATED_ROOT,
    entrypoint: STATIC_HTML_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_REACT_VITE_RUNTIME_PACK: RuntimePackInput = {
  id: REACT_VITE_RUNTIME_PACK_ID,
  version: REACT_VITE_PACK_VERSION,
  runtime: {
    kind: "react-vite",
    generatedRoot: REACT_VITE_GENERATED_ROOT,
    entrypoint: REACT_VITE_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_REACT_SQLITE_RUNTIME_PACK: RuntimePackInput = {
  id: REACT_SQLITE_RUNTIME_PACK_ID,
  version: REACT_SQLITE_PACK_VERSION,
  runtime: {
    kind: "react-sqlite",
    generatedRoot: REACT_SQLITE_GENERATED_ROOT,
    entrypoint: REACT_SQLITE_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_CANVAS2D_RUNTIME_PACK: RuntimePackInput = {
  id: CANVAS2D_RUNTIME_PACK_ID,
  version: CANVAS2D_PACK_VERSION,
  runtime: {
    kind: "canvas2d",
    generatedRoot: CANVAS2D_GENERATED_ROOT,
    entrypoint: CANVAS2D_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_MARKDOWN_KNOWLEDGE_RUNTIME_PACK: RuntimePackInput = {
  id: MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID,
  version: MARKDOWN_KNOWLEDGE_PACK_VERSION,
  runtime: {
    kind: "markdown-knowledge",
    generatedRoot: GENERATED_PROJECT_ROOT,
    entrypoint: GENERATED_REACT_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_DATA_TABLE_RUNTIME_PACK: RuntimePackInput = {
  id: DATA_TABLE_RUNTIME_PACK_ID,
  version: DATA_TABLE_PACK_VERSION,
  runtime: {
    kind: "data-table",
    generatedRoot: GENERATED_PROJECT_ROOT,
    entrypoint: GENERATED_REACT_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_FILE_PROCESSOR_RUNTIME_PACK: RuntimePackInput = {
  id: FILE_PROCESSOR_RUNTIME_PACK_ID,
  version: FILE_PROCESSOR_PACK_VERSION,
  runtime: {
    kind: "file-processor",
    generatedRoot: GENERATED_PROJECT_ROOT,
    entrypoint: GENERATED_REACT_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_DESKTOP_WIDGET_RUNTIME_PACK: RuntimePackInput = {
  id: DESKTOP_WIDGET_RUNTIME_PACK_ID,
  version: DESKTOP_WIDGET_PACK_VERSION,
  runtime: {
    kind: "desktop-widget",
    generatedRoot: GENERATED_PROJECT_ROOT,
    entrypoint: GENERATED_REACT_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_AI_AGENT_APP_RUNTIME_PACK: RuntimePackInput = {
  id: AI_AGENT_APP_RUNTIME_PACK_ID,
  version: AI_AGENT_APP_PACK_VERSION,
  runtime: {
    kind: "ai-agent-app",
    generatedRoot: GENERATED_PROJECT_ROOT,
    entrypoint: GENERATED_REACT_ENTRYPOINT,
    bind: "127.0.0.1",
    network: "local-only",
  },
};

const DEFAULT_HARNESS_PACK: HarnessPackInput = {
  id: STATIC_HTML_HARNESS_PACK_ID,
  version: STATIC_HTML_PACK_VERSION,
  runtime: STATIC_HTML_RUNTIME_PACK_ID,
  instructions: {
    system: STATIC_HTML_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: STATIC_HTML_HARNESS_POLICY.fileSystemRules,
    outputRules: STATIC_HTML_HARNESS_POLICY.outputRules,
  },
};

const DEFAULT_REACT_VITE_HARNESS_PACK: HarnessPackInput = {
  id: REACT_VITE_HARNESS_PACK_ID,
  version: REACT_VITE_PACK_VERSION,
  runtime: REACT_VITE_RUNTIME_PACK_ID,
  instructions: {
    system: REACT_VITE_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: REACT_VITE_HARNESS_POLICY.fileSystemRules,
    outputRules: REACT_VITE_HARNESS_POLICY.outputRules,
  },
};

const DEFAULT_REACT_SQLITE_HARNESS_PACK: HarnessPackInput = {
  id: REACT_SQLITE_HARNESS_PACK_ID,
  version: REACT_SQLITE_PACK_VERSION,
  runtime: REACT_SQLITE_RUNTIME_PACK_ID,
  instructions: {
    system: REACT_SQLITE_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: REACT_SQLITE_HARNESS_POLICY.fileSystemRules,
    outputRules: REACT_SQLITE_HARNESS_POLICY.outputRules,
  },
};

const DEFAULT_CANVAS2D_HARNESS_PACK: HarnessPackInput = {
  id: CANVAS2D_HARNESS_PACK_ID,
  version: CANVAS2D_PACK_VERSION,
  runtime: CANVAS2D_RUNTIME_PACK_ID,
  instructions: {
    system: CANVAS2D_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: CANVAS2D_HARNESS_POLICY.fileSystemRules,
    outputRules: CANVAS2D_HARNESS_POLICY.outputRules,
  },
};

const DEFAULT_MARKDOWN_KNOWLEDGE_HARNESS_PACK: HarnessPackInput = {
  id: MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID,
  version: MARKDOWN_KNOWLEDGE_PACK_VERSION,
  runtime: MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID,
  instructions: {
    system: MARKDOWN_KNOWLEDGE_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: MARKDOWN_KNOWLEDGE_HARNESS_POLICY.fileSystemRules,
    outputRules: MARKDOWN_KNOWLEDGE_HARNESS_POLICY.outputRules,
  },
};

const DEFAULT_DATA_TABLE_HARNESS_PACK: HarnessPackInput = {
  id: DATA_TABLE_HARNESS_PACK_ID,
  version: DATA_TABLE_PACK_VERSION,
  runtime: DATA_TABLE_RUNTIME_PACK_ID,
  instructions: {
    system: DATA_TABLE_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: DATA_TABLE_HARNESS_POLICY.fileSystemRules,
    outputRules: DATA_TABLE_HARNESS_POLICY.outputRules,
  },
};

const DEFAULT_FILE_PROCESSOR_HARNESS_PACK: HarnessPackInput = {
  id: FILE_PROCESSOR_HARNESS_PACK_ID,
  version: FILE_PROCESSOR_PACK_VERSION,
  runtime: FILE_PROCESSOR_RUNTIME_PACK_ID,
  instructions: {
    system: FILE_PROCESSOR_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: FILE_PROCESSOR_HARNESS_POLICY.fileSystemRules,
    outputRules: FILE_PROCESSOR_HARNESS_POLICY.outputRules,
  },
};

const DEFAULT_DESKTOP_WIDGET_HARNESS_PACK: HarnessPackInput = {
  id: DESKTOP_WIDGET_HARNESS_PACK_ID,
  version: DESKTOP_WIDGET_PACK_VERSION,
  runtime: DESKTOP_WIDGET_RUNTIME_PACK_ID,
  instructions: {
    system: DESKTOP_WIDGET_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: DESKTOP_WIDGET_HARNESS_POLICY.fileSystemRules,
    outputRules: DESKTOP_WIDGET_HARNESS_POLICY.outputRules,
  },
};

const DEFAULT_AI_AGENT_APP_HARNESS_PACK: HarnessPackInput = {
  id: MULTIMODAL_STUDIO_AGENT_HARNESS_PACK_ID,
  version: AI_AGENT_APP_PACK_VERSION,
  runtime: AI_AGENT_APP_RUNTIME_PACK_ID,
  instructions: {
    system: AI_AGENT_APP_HARNESS_POLICY.systemInstructions,
    fileSystemPolicy: AI_AGENT_APP_HARNESS_POLICY.fileSystemRules,
    outputRules: AI_AGENT_APP_HARNESS_POLICY.outputRules,
  },
};

export function createStaticHtmlPromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_RUNTIME_PACK,
  harnessPacks = [DEFAULT_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateStaticHtmlPromptEnvelopeInput): PromptEnvelope {
  assertStaticHtmlRuntime(runtimePack);
  assertStaticHtmlHarnessPacks(runtimePack, harnessPacks);

  const fragments = harnessPacks.map(toStaticHtmlFragment);
  const merged = mergeHarnessFragments(fragments);

  return {
    schemaVersion: "1.0",
    envelopeId,
    createdAt,
    boxRuntimeContext: {
      runtimePack: { id: runtimePack.id, version: runtimePack.version },
      harnessPacks: fragments.map((fragment) => fragment.pack),
      runtimeKind: "static-html",
      generatedRoot: runtimePack.runtime.generatedRoot,
      entrypoint: runtimePack.runtime.entrypoint,
      bind: runtimePack.runtime.bind,
      network: runtimePack.runtime.network,
    },
    userIntent: userIntent.trim(),
    currentAppState,
    runtimePolicy: { ...STATIC_HTML_RUNTIME_POLICY },
    harnessPolicy: merged.policy,
    fileSystemPolicy: { ...STATIC_HTML_FILE_SYSTEM_POLICY },
    commandPolicy: { ...STATIC_HTML_COMMAND_POLICY },
    outputContract: { ...STATIC_HTML_OUTPUT_CONTRACT },
    acceptanceCriteria: merged.acceptanceCriteria,
  };
}

export function createReactVitePromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_REACT_VITE_RUNTIME_PACK,
  harnessPacks = [DEFAULT_REACT_VITE_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateReactVitePromptEnvelopeInput): PromptEnvelope {
  assertReactViteRuntime(runtimePack);
  assertReactViteHarnessPacks(runtimePack, harnessPacks);

  const fragments = harnessPacks.map(toReactViteFragment);
  const merged = mergeReactViteHarnessFragments(fragments);

  return {
    schemaVersion: "1.0",
    envelopeId,
    createdAt,
    boxRuntimeContext: {
      runtimePack: { id: runtimePack.id, version: runtimePack.version },
      harnessPacks: fragments.map((fragment) => fragment.pack),
      runtimeKind: "react-vite",
      generatedRoot: runtimePack.runtime.generatedRoot,
      entrypoint: runtimePack.runtime.entrypoint,
      bind: runtimePack.runtime.bind,
      network: runtimePack.runtime.network,
    },
    userIntent: userIntent.trim(),
    currentAppState,
    runtimePolicy: { ...REACT_VITE_RUNTIME_POLICY },
    harnessPolicy: merged.policy,
    fileSystemPolicy: { ...REACT_VITE_FILE_SYSTEM_POLICY },
    commandPolicy: { ...REACT_VITE_COMMAND_POLICY },
    outputContract: { ...REACT_VITE_OUTPUT_CONTRACT },
    acceptanceCriteria: merged.acceptanceCriteria,
  };
}

export function createReactSqlitePromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_REACT_SQLITE_RUNTIME_PACK,
  harnessPacks = [DEFAULT_REACT_SQLITE_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateReactSqlitePromptEnvelopeInput): PromptEnvelope {
  assertReactSqliteRuntime(runtimePack);
  assertReactSqliteHarnessPacks(runtimePack, harnessPacks);

  const fragments = harnessPacks.map(toReactSqliteFragment);
  const merged = mergeReactSqliteHarnessFragments(fragments);

  return {
    schemaVersion: "1.0",
    envelopeId,
    createdAt,
    boxRuntimeContext: {
      runtimePack: { id: runtimePack.id, version: runtimePack.version },
      harnessPacks: fragments.map((fragment) => fragment.pack),
      runtimeKind: "react-sqlite",
      generatedRoot: runtimePack.runtime.generatedRoot,
      entrypoint: runtimePack.runtime.entrypoint,
      bind: runtimePack.runtime.bind,
      network: runtimePack.runtime.network,
    },
    userIntent: userIntent.trim(),
    currentAppState,
    runtimePolicy: { ...REACT_SQLITE_RUNTIME_POLICY },
    harnessPolicy: merged.policy,
    fileSystemPolicy: { ...REACT_SQLITE_FILE_SYSTEM_POLICY },
    commandPolicy: { ...REACT_SQLITE_COMMAND_POLICY },
    outputContract: { ...REACT_SQLITE_OUTPUT_CONTRACT },
    acceptanceCriteria: merged.acceptanceCriteria,
  };
}

export function createCanvas2dPromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_CANVAS2D_RUNTIME_PACK,
  harnessPacks = [DEFAULT_CANVAS2D_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateCanvas2dPromptEnvelopeInput): PromptEnvelope {
  assertCanvas2dRuntime(runtimePack);
  assertCanvas2dHarnessPacks(runtimePack, harnessPacks);

  const fragments = harnessPacks.map(toCanvas2dFragment);
  const merged = mergeCanvas2dHarnessFragments(fragments);

  return {
    schemaVersion: "1.0",
    envelopeId,
    createdAt,
    boxRuntimeContext: {
      runtimePack: { id: runtimePack.id, version: runtimePack.version },
      harnessPacks: fragments.map((fragment) => fragment.pack),
      runtimeKind: "canvas2d",
      generatedRoot: runtimePack.runtime.generatedRoot,
      entrypoint: runtimePack.runtime.entrypoint,
      bind: runtimePack.runtime.bind,
      network: runtimePack.runtime.network,
    },
    userIntent: userIntent.trim(),
    currentAppState,
    runtimePolicy: { ...CANVAS2D_RUNTIME_POLICY },
    harnessPolicy: merged.policy,
    fileSystemPolicy: { ...CANVAS2D_FILE_SYSTEM_POLICY },
    commandPolicy: { ...CANVAS2D_COMMAND_POLICY },
    outputContract: { ...CANVAS2D_OUTPUT_CONTRACT },
    acceptanceCriteria: merged.acceptanceCriteria,
  };
}

export function createMarkdownKnowledgePromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_MARKDOWN_KNOWLEDGE_RUNTIME_PACK,
  harnessPacks = [DEFAULT_MARKDOWN_KNOWLEDGE_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateGeneratedProjectPromptEnvelopeInput): PromptEnvelope {
  return createGeneratedProjectPromptEnvelope({
    userIntent,
    currentAppState,
    runtimePack,
    harnessPacks,
    envelopeId,
    createdAt,
    defaultPolicy: MARKDOWN_KNOWLEDGE_HARNESS_POLICY,
    allowedFiles: MARKDOWN_KNOWLEDGE_ALLOWED_FILES,
    outputFormat: "markdown-knowledge-project",
    expectedKind: "markdown-knowledge",
  });
}

export function createDataTablePromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_DATA_TABLE_RUNTIME_PACK,
  harnessPacks = [DEFAULT_DATA_TABLE_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateGeneratedProjectPromptEnvelopeInput): PromptEnvelope {
  return createGeneratedProjectPromptEnvelope({
    userIntent,
    currentAppState,
    runtimePack,
    harnessPacks,
    envelopeId,
    createdAt,
    defaultPolicy: DATA_TABLE_HARNESS_POLICY,
    allowedFiles: DATA_TABLE_ALLOWED_FILES,
    outputFormat: "data-table-project",
    expectedKind: "data-table",
  });
}

export function createFileProcessorPromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_FILE_PROCESSOR_RUNTIME_PACK,
  harnessPacks = [DEFAULT_FILE_PROCESSOR_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateGeneratedProjectPromptEnvelopeInput): PromptEnvelope {
  return createGeneratedProjectPromptEnvelope({
    userIntent,
    currentAppState,
    runtimePack,
    harnessPacks,
    envelopeId,
    createdAt,
    defaultPolicy: FILE_PROCESSOR_HARNESS_POLICY,
    allowedFiles: FILE_PROCESSOR_ALLOWED_FILES,
    outputFormat: "file-processor-project",
    expectedKind: "file-processor",
  });
}

export function createDesktopWidgetPromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_DESKTOP_WIDGET_RUNTIME_PACK,
  harnessPacks = [DEFAULT_DESKTOP_WIDGET_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateGeneratedProjectPromptEnvelopeInput): PromptEnvelope {
  return createGeneratedProjectPromptEnvelope({
    userIntent,
    currentAppState,
    runtimePack,
    harnessPacks,
    envelopeId,
    createdAt,
    defaultPolicy: DESKTOP_WIDGET_HARNESS_POLICY,
    allowedFiles: DESKTOP_WIDGET_ALLOWED_FILES,
    outputFormat: "desktop-widget-project",
    expectedKind: "desktop-widget",
  });
}

export function createAiAgentAppPromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack = DEFAULT_AI_AGENT_APP_RUNTIME_PACK,
  harnessPacks = [DEFAULT_AI_AGENT_APP_HARNESS_PACK],
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreateGeneratedProjectPromptEnvelopeInput): PromptEnvelope {
  return createGeneratedProjectPromptEnvelope({
    userIntent,
    currentAppState,
    runtimePack,
    harnessPacks,
    envelopeId,
    createdAt,
    defaultPolicy: AI_AGENT_APP_HARNESS_POLICY,
    allowedFiles: AI_AGENT_APP_ALLOWED_FILES,
    outputFormat: "ai-agent-app-project",
    expectedKind: "ai-agent-app",
  });
}

interface CreateGeneratedProjectPromptEnvelopeArgs {
  userIntent: string;
  currentAppState: CurrentAppState;
  runtimePack: RuntimePackInput;
  harnessPacks: HarnessPackInput[];
  envelopeId: string;
  createdAt: string;
  defaultPolicy: HarnessPolicy;
  allowedFiles: readonly string[];
  outputFormat: PromptEnvelope["outputContract"]["format"];
  expectedKind: RuntimePackInput["runtime"]["kind"];
}

function createGeneratedProjectPromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack,
  harnessPacks,
  envelopeId,
  createdAt,
  defaultPolicy,
  allowedFiles,
  outputFormat,
  expectedKind,
}: CreateGeneratedProjectPromptEnvelopeArgs): PromptEnvelope {
  assertGeneratedProjectRuntime(runtimePack, expectedKind);
  assertGeneratedProjectHarnessPacks(runtimePack, harnessPacks);

  const fragments = harnessPacks.map((pack) => toGeneratedProjectFragment(pack, defaultPolicy));
  const merged = mergeGeneratedProjectHarnessFragments(fragments, defaultPolicy);

  return {
    schemaVersion: "1.0",
    envelopeId,
    createdAt,
    boxRuntimeContext: {
      runtimePack: { id: runtimePack.id, version: runtimePack.version },
      harnessPacks: fragments.map((fragment) => fragment.pack),
      runtimeKind: runtimePack.runtime.kind,
      generatedRoot: runtimePack.runtime.generatedRoot,
      entrypoint: runtimePack.runtime.entrypoint,
      bind: runtimePack.runtime.bind,
      network: runtimePack.runtime.network,
    },
    userIntent: userIntent.trim(),
    currentAppState,
    runtimePolicy: generatedProjectRuntimePolicy(runtimePack.runtime.kind),
    harnessPolicy: merged.policy,
    fileSystemPolicy: generatedProjectFileSystemPolicy(allowedFiles),
    commandPolicy: { ...GENERATED_PROJECT_COMMAND_POLICY },
    outputContract: generatedProjectOutputContract(outputFormat, allowedFiles),
    acceptanceCriteria: merged.acceptanceCriteria,
  };
}

export function mergeHarnessFragments(fragments: HarnessFragment[]): {
  policy: HarnessPolicy;
  acceptanceCriteria: string[];
} {
  const ordered = [...fragments].sort((a, b) => packKey(a.pack).localeCompare(packKey(b.pack)));

  const systemInstructions = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.systemInstructions),
  );
  const fileSystemRules = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.fileSystemRules),
  );
  const outputRules = uniqueStable(ordered.flatMap((fragment) => fragment.policy.outputRules));
  const blockedCapabilities = uniqueStable([
    ...STATIC_HTML_HARNESS_POLICY.blockedCapabilities,
    ...ordered.flatMap((fragment) => fragment.policy.blockedCapabilities),
  ]).sort();
  const acceptanceCriteria = uniqueStable([
    "Generated output contains exactly index.html, style.css, and app.js.",
    "Generated app does not include Sofvary shell UI, floating menu, build overlay, or host controls.",
    "Generated app runs through the local static preview server without network downloads.",
    ...ordered.flatMap((fragment) => fragment.acceptanceCriteria),
  ]);

  return {
    policy: {
      systemInstructions,
      fileSystemRules,
      outputRules,
      blockedCapabilities,
    },
    acceptanceCriteria,
  };
}

function mergeReactViteHarnessFragments(fragments: HarnessFragment[]): {
  policy: HarnessPolicy;
  acceptanceCriteria: string[];
} {
  const ordered = [...fragments].sort((a, b) => packKey(a.pack).localeCompare(packKey(b.pack)));

  const systemInstructions = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.systemInstructions),
  );
  const fileSystemRules = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.fileSystemRules),
  );
  const outputRules = uniqueStable(ordered.flatMap((fragment) => fragment.policy.outputRules));
  const blockedCapabilities = uniqueStable([
    ...REACT_VITE_HARNESS_POLICY.blockedCapabilities,
    ...ordered.flatMap((fragment) => fragment.policy.blockedCapabilities),
  ]).sort();
  const acceptanceCriteria = uniqueStable([
    "Generated output contains exactly the React + Vite project file set.",
    "Generated app uses React function components, TypeScript, and Vite.",
    "Generated app does not include Sofvary shell UI, floating menu, build overlay, or host controls.",
    "Generated app uses no Next.js, Electron, external CDN, remote assets, SQLite, or default UI framework.",
    "Generated app can run through the local Vite dev server bound to 127.0.0.1.",
    ...ordered.flatMap((fragment) => fragment.acceptanceCriteria),
  ]);

  return {
    policy: {
      systemInstructions,
      fileSystemRules,
      outputRules,
      blockedCapabilities,
    },
    acceptanceCriteria,
  };
}

function mergeReactSqliteHarnessFragments(fragments: HarnessFragment[]): {
  policy: HarnessPolicy;
  acceptanceCriteria: string[];
} {
  const ordered = [...fragments].sort((a, b) => packKey(a.pack).localeCompare(packKey(b.pack)));

  const systemInstructions = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.systemInstructions),
  );
  const fileSystemRules = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.fileSystemRules),
  );
  const outputRules = uniqueStable(ordered.flatMap((fragment) => fragment.policy.outputRules));
  const blockedCapabilities = uniqueStable([
    ...REACT_SQLITE_HARNESS_POLICY.blockedCapabilities,
    ...ordered.flatMap((fragment) => fragment.policy.blockedCapabilities),
  ]).sort();
  const acceptanceCriteria = uniqueStable([
    "Generated output contains exactly the React + SQLite project file set.",
    "Frontend code calls /api/* endpoints and does not import or open SQLite directly.",
    "Node local API owns SQLite access and binds to 127.0.0.1 only.",
    "SQLite database file is stored inside generated/data/app.sqlite.",
    "Every user-controlled SQL value is passed through parameterized statements.",
    "Generated app uses no remote database, cloud service, sensitive credentials, external CDN, or Sofvary shell UI.",
    ...ordered.flatMap((fragment) => fragment.acceptanceCriteria),
  ]);

  return {
    policy: {
      systemInstructions,
      fileSystemRules,
      outputRules,
      blockedCapabilities,
    },
    acceptanceCriteria,
  };
}

function mergeCanvas2dHarnessFragments(fragments: HarnessFragment[]): {
  policy: HarnessPolicy;
  acceptanceCriteria: string[];
} {
  const ordered = [...fragments].sort((a, b) => packKey(a.pack).localeCompare(packKey(b.pack)));

  const systemInstructions = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.systemInstructions),
  );
  const fileSystemRules = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.fileSystemRules),
  );
  const outputRules = uniqueStable(ordered.flatMap((fragment) => fragment.policy.outputRules));
  const blockedCapabilities = uniqueStable([
    ...CANVAS2D_HARNESS_POLICY.blockedCapabilities,
    ...ordered.flatMap((fragment) => fragment.policy.blockedCapabilities),
  ]).sort();
  const acceptanceCriteria = uniqueStable([
    "Generated output contains exactly the Canvas 2D project file set.",
    "Generated app uses the Canvas 2D API and requestAnimationFrame.",
    "Generated app does not use React, external CDN, remote assets, npm packages, or Sofvary shell UI.",
    "Update, render, input, and state are split across declared engine and game files.",
    "Level data is configurable and pause/restart behavior is included where reasonable.",
    ...ordered.flatMap((fragment) => fragment.acceptanceCriteria),
  ]);

  return {
    policy: {
      systemInstructions,
      fileSystemRules,
      outputRules,
      blockedCapabilities,
    },
    acceptanceCriteria,
  };
}

function mergeGeneratedProjectHarnessFragments(
  fragments: HarnessFragment[],
  defaultPolicy: HarnessPolicy,
): {
  policy: HarnessPolicy;
  acceptanceCriteria: string[];
} {
  const ordered = [...fragments].sort((a, b) => packKey(a.pack).localeCompare(packKey(b.pack)));

  const systemInstructions = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.systemInstructions),
  );
  const fileSystemRules = uniqueStable(
    ordered.flatMap((fragment) => fragment.policy.fileSystemRules),
  );
  const outputRules = uniqueStable(ordered.flatMap((fragment) => fragment.policy.outputRules));
  const blockedCapabilities = uniqueStable([
    ...defaultPolicy.blockedCapabilities,
    ...ordered.flatMap((fragment) => fragment.policy.blockedCapabilities),
  ]).sort();
  const acceptanceCriteria = uniqueStable([
    ...defaultPolicy.outputRules,
    ...ordered.flatMap((fragment) => fragment.acceptanceCriteria),
  ]);

  return {
    policy: {
      systemInstructions,
      fileSystemRules,
      outputRules,
      blockedCapabilities,
    },
    acceptanceCriteria,
  };
}

function toStaticHtmlFragment(pack: HarnessPackInput): HarnessFragment {
  return {
    pack: { id: pack.id, version: pack.version },
    policy: {
      systemInstructions: [...STATIC_HTML_HARNESS_POLICY.systemInstructions, ...pack.instructions.system],
      fileSystemRules: [
        ...STATIC_HTML_HARNESS_POLICY.fileSystemRules,
        ...pack.instructions.fileSystemPolicy,
      ],
      outputRules: [...STATIC_HTML_HARNESS_POLICY.outputRules, ...pack.instructions.outputRules],
      blockedCapabilities: [...STATIC_HTML_HARNESS_POLICY.blockedCapabilities],
    },
    acceptanceCriteria: [
      `Harness ${pack.id}@${pack.version} constraints are represented in the prompt envelope.`,
    ],
  };
}

function toReactViteFragment(pack: HarnessPackInput): HarnessFragment {
  return {
    pack: { id: pack.id, version: pack.version },
    policy: {
      systemInstructions: [...REACT_VITE_HARNESS_POLICY.systemInstructions, ...pack.instructions.system],
      fileSystemRules: [
        ...REACT_VITE_HARNESS_POLICY.fileSystemRules,
        ...pack.instructions.fileSystemPolicy,
      ],
      outputRules: [...REACT_VITE_HARNESS_POLICY.outputRules, ...pack.instructions.outputRules],
      blockedCapabilities: [...REACT_VITE_HARNESS_POLICY.blockedCapabilities],
    },
    acceptanceCriteria: [
      `Harness ${pack.id}@${pack.version} constraints are represented in the prompt envelope.`,
    ],
  };
}

function toReactSqliteFragment(pack: HarnessPackInput): HarnessFragment {
  return {
    pack: { id: pack.id, version: pack.version },
    policy: {
      systemInstructions: [...REACT_SQLITE_HARNESS_POLICY.systemInstructions, ...pack.instructions.system],
      fileSystemRules: [
        ...REACT_SQLITE_HARNESS_POLICY.fileSystemRules,
        ...pack.instructions.fileSystemPolicy,
      ],
      outputRules: [...REACT_SQLITE_HARNESS_POLICY.outputRules, ...pack.instructions.outputRules],
      blockedCapabilities: [...REACT_SQLITE_HARNESS_POLICY.blockedCapabilities],
    },
    acceptanceCriteria: [
      `Harness ${pack.id}@${pack.version} constraints are represented in the prompt envelope.`,
    ],
  };
}

function toCanvas2dFragment(pack: HarnessPackInput): HarnessFragment {
  return {
    pack: { id: pack.id, version: pack.version },
    policy: {
      systemInstructions: [...CANVAS2D_HARNESS_POLICY.systemInstructions, ...pack.instructions.system],
      fileSystemRules: [
        ...CANVAS2D_HARNESS_POLICY.fileSystemRules,
        ...pack.instructions.fileSystemPolicy,
      ],
      outputRules: [...CANVAS2D_HARNESS_POLICY.outputRules, ...pack.instructions.outputRules],
      blockedCapabilities: [...CANVAS2D_HARNESS_POLICY.blockedCapabilities],
    },
    acceptanceCriteria: [
      `Harness ${pack.id}@${pack.version} constraints are represented in the prompt envelope.`,
    ],
  };
}

function toGeneratedProjectFragment(
  pack: HarnessPackInput,
  defaultPolicy: HarnessPolicy,
): HarnessFragment {
  return {
    pack: { id: pack.id, version: pack.version },
    policy: {
      systemInstructions: [...defaultPolicy.systemInstructions, ...pack.instructions.system],
      fileSystemRules: [...defaultPolicy.fileSystemRules, ...pack.instructions.fileSystemPolicy],
      outputRules: [...defaultPolicy.outputRules, ...pack.instructions.outputRules],
      blockedCapabilities: [...defaultPolicy.blockedCapabilities],
    },
    acceptanceCriteria: [
      `Harness ${pack.id}@${pack.version} constraints are represented in the prompt envelope.`,
    ],
  };
}

function assertStaticHtmlRuntime(runtimePack: RuntimePackInput) {
  const runtime = runtimePack.runtime;
  if (
    runtime.kind !== "static-html" ||
    runtime.generatedRoot !== STATIC_HTML_GENERATED_ROOT ||
    runtime.entrypoint !== STATIC_HTML_ENTRYPOINT ||
    runtime.bind !== "127.0.0.1" ||
    runtime.network !== "local-only"
  ) {
    throw new Error("runtime pack is not compatible with the static-html harness compiler");
  }
}

function assertReactViteRuntime(runtimePack: RuntimePackInput) {
  const runtime = runtimePack.runtime;
  if (
    runtime.kind !== "react-vite" ||
    runtime.generatedRoot !== REACT_VITE_GENERATED_ROOT ||
    runtime.entrypoint !== REACT_VITE_ENTRYPOINT ||
    runtime.bind !== "127.0.0.1" ||
    runtime.network !== "local-only"
  ) {
    throw new Error("runtime pack is not compatible with the react-vite harness compiler");
  }
}

function assertReactSqliteRuntime(runtimePack: RuntimePackInput) {
  const runtime = runtimePack.runtime;
  if (
    runtime.kind !== "react-sqlite" ||
    runtime.generatedRoot !== REACT_SQLITE_GENERATED_ROOT ||
    runtime.entrypoint !== REACT_SQLITE_ENTRYPOINT ||
    runtime.bind !== "127.0.0.1" ||
    runtime.network !== "local-only"
  ) {
    throw new Error("runtime pack is not compatible with the react-sqlite harness compiler");
  }
}

function assertCanvas2dRuntime(runtimePack: RuntimePackInput) {
  const runtime = runtimePack.runtime;
  if (
    runtime.kind !== "canvas2d" ||
    runtime.generatedRoot !== CANVAS2D_GENERATED_ROOT ||
    runtime.entrypoint !== CANVAS2D_ENTRYPOINT ||
    runtime.bind !== "127.0.0.1" ||
    runtime.network !== "local-only"
  ) {
    throw new Error("runtime pack is not compatible with the canvas2d harness compiler");
  }
}

function assertGeneratedProjectRuntime(runtimePack: RuntimePackInput, expectedKind: RuntimePackInput["runtime"]["kind"]) {
  const runtime = runtimePack.runtime;
  if (
    runtime.kind !== expectedKind ||
    runtime.generatedRoot !== GENERATED_PROJECT_ROOT ||
    runtime.entrypoint !== GENERATED_REACT_ENTRYPOINT ||
    runtime.bind !== "127.0.0.1" ||
    runtime.network !== "local-only"
  ) {
    throw new Error("runtime pack is not compatible with the generated-project harness compiler");
  }
}

function assertStaticHtmlHarnessPacks(runtimePack: RuntimePackInput, harnessPacks: HarnessPackInput[]) {
  if (harnessPacks.length === 0) {
    throw new Error("static-html prompt envelope requires at least one harness pack");
  }

  for (const harnessPack of harnessPacks) {
    if (harnessPack.runtime && harnessPack.runtime !== runtimePack.id) {
      throw new Error(
        `harness pack ${harnessPack.id}@${harnessPack.version} is not compatible with runtime ${runtimePack.id}`,
      );
    }
  }
}

function assertReactViteHarnessPacks(runtimePack: RuntimePackInput, harnessPacks: HarnessPackInput[]) {
  if (harnessPacks.length === 0) {
    throw new Error("react-vite prompt envelope requires at least one harness pack");
  }

  for (const harnessPack of harnessPacks) {
    if (harnessPack.runtime && harnessPack.runtime !== runtimePack.id) {
      throw new Error(
        `harness pack ${harnessPack.id}@${harnessPack.version} is not compatible with runtime ${runtimePack.id}`,
      );
    }
  }
}

function assertReactSqliteHarnessPacks(runtimePack: RuntimePackInput, harnessPacks: HarnessPackInput[]) {
  if (harnessPacks.length === 0) {
    throw new Error("react-sqlite prompt envelope requires at least one harness pack");
  }

  for (const harnessPack of harnessPacks) {
    if (harnessPack.runtime && harnessPack.runtime !== runtimePack.id) {
      throw new Error(
        `harness pack ${harnessPack.id}@${harnessPack.version} is not compatible with runtime ${runtimePack.id}`,
      );
    }
  }
}

function assertCanvas2dHarnessPacks(runtimePack: RuntimePackInput, harnessPacks: HarnessPackInput[]) {
  if (harnessPacks.length === 0) {
    throw new Error("canvas2d prompt envelope requires at least one harness pack");
  }

  for (const harnessPack of harnessPacks) {
    if (harnessPack.runtime && harnessPack.runtime !== runtimePack.id) {
      throw new Error(
        `harness pack ${harnessPack.id}@${harnessPack.version} is not compatible with runtime ${runtimePack.id}`,
      );
    }
  }
}

function assertGeneratedProjectHarnessPacks(runtimePack: RuntimePackInput, harnessPacks: HarnessPackInput[]) {
  if (harnessPacks.length === 0) {
    throw new Error(`${runtimePack.runtime.kind} prompt envelope requires at least one harness pack`);
  }

  for (const harnessPack of harnessPacks) {
    if (harnessPack.runtime && harnessPack.runtime !== runtimePack.id) {
      throw new Error(
        `harness pack ${harnessPack.id}@${harnessPack.version} is not compatible with runtime ${runtimePack.id}`,
      );
    }
  }
}

function createEnvelopeId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `penv_${crypto.randomUUID().replaceAll("-", "")}`;
  }

  return `penv_${Date.now().toString(36)}`;
}

function uniqueStable(values: string[]): string[] {
  return values.filter((value, index, all) => value.trim() && all.indexOf(value) === index);
}

function packKey(pack: PackReference) {
  return `${pack.id}@${pack.version}`;
}
