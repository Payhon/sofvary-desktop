import type {
  CommandPolicy,
  FileSystemPolicy,
  HarnessPolicy,
  OutputContract,
  RuntimePolicy,
} from "./promptEnvelope";

export const MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID = "sofvary.runtime.markdown-knowledge";
export const MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID = "sofvary.harness.markdown-knowledge";
export const MARKDOWN_KNOWLEDGE_PACK_VERSION = "0.1.0";
export const MARKDOWN_KNOWLEDGE_ALLOWED_FILES = [
  "markdown/index.json",
  "markdown/content/getting-started.md",
  "react/package.json",
  "react/index.html",
  "react/vite.config.ts",
  "react/tsconfig.json",
  "react/src/main.tsx",
  "react/src/App.tsx",
  "react/src/components/MarkdownKnowledgeApp.tsx",
  "react/src/styles/app.css",
] as const;

export const DATA_TABLE_RUNTIME_PACK_ID = "sofvary.runtime.data-table";
export const DATA_TABLE_HARNESS_PACK_ID = "sofvary.harness.data-table";
export const DATA_TABLE_PACK_VERSION = "0.1.0";
export const DATA_TABLE_ALLOWED_FILES = [
  "data/schema.json",
  "data/tables/inventory.json",
  "react/package.json",
  "react/index.html",
  "react/vite.config.ts",
  "react/tsconfig.json",
  "react/src/main.tsx",
  "react/src/App.tsx",
  "react/src/components/DataTableApp.tsx",
  "react/src/styles/app.css",
] as const;

export const FILE_PROCESSOR_RUNTIME_PACK_ID = "sofvary.runtime.file-processor";
export const FILE_PROCESSOR_HARNESS_PACK_ID = "sofvary.harness.file-processor";
export const FILE_PROCESSOR_PACK_VERSION = "0.1.0";
export const FILE_PROCESSOR_ALLOWED_FILES = [
  "file-processor/policy.json",
  "file-processor/dry-run-template.json",
  "react/package.json",
  "react/index.html",
  "react/vite.config.ts",
  "react/tsconfig.json",
  "react/src/main.tsx",
  "react/src/App.tsx",
  "react/src/components/FileProcessorApp.tsx",
  "react/src/styles/app.css",
] as const;

export const DESKTOP_WIDGET_RUNTIME_PACK_ID = "sofvary.runtime.desktop-widget";
export const DESKTOP_WIDGET_HARNESS_PACK_ID = "sofvary.harness.desktop-widget";
export const DESKTOP_WIDGET_PACK_VERSION = "0.1.0";
export const DESKTOP_WIDGET_ALLOWED_FILES = [
  "widget/manifest.json",
  "react/package.json",
  "react/index.html",
  "react/vite.config.ts",
  "react/tsconfig.json",
  "react/src/main.tsx",
  "react/src/App.tsx",
  "react/src/components/DesktopWidgetApp.tsx",
  "react/src/styles/app.css",
] as const;

export const AI_AGENT_APP_RUNTIME_PACK_ID = "sofvary.runtime.ai-agent-app";
export const MULTIMODAL_STUDIO_AGENT_HARNESS_PACK_ID = "sofvary.harness.multimodal-studio-agent";
export const AI_AGENT_APP_PACK_VERSION = "0.1.0";
export const AI_AGENT_APP_ALLOWED_FILES = [
  "ai/agents.json",
  "ai/provider-requirements.json",
  "ai/jobs.seed.json",
  "react/package.json",
  "react/index.html",
  "react/vite.config.ts",
  "react/tsconfig.json",
  "react/src/main.tsx",
  "react/src/App.tsx",
  "react/src/components/AiAgentApp.tsx",
  "react/src/components/ProviderSettings.tsx",
  "react/src/components/ArtifactGallery.tsx",
  "react/src/styles/app.css",
] as const;

export const GENERATED_REACT_ENTRYPOINT = "react/src/main.tsx";
export const GENERATED_PROJECT_ROOT = "generated";

export function generatedProjectRuntimePolicy(runtimeKind: RuntimePolicy["runtimeKind"]): RuntimePolicy {
  return {
    runtimeKind,
    allowedEntrypoints: [GENERATED_REACT_ENTRYPOINT],
    allowedServerBind: "127.0.0.1",
    network: "local-only",
    packageInstall: false,
  };
}

export function generatedProjectFileSystemPolicy(files: readonly string[]): FileSystemPolicy {
  return {
    root: GENERATED_PROJECT_ROOT,
    allowedFiles: [...files],
    allowExternalFiles: false,
    allowPathTraversal: false,
  };
}

export const GENERATED_PROJECT_COMMAND_POLICY: CommandPolicy = {
  allowShell: false,
  allowPackageInstall: false,
  allowGlobalInstall: false,
  allowedCommands: [],
};

export function generatedProjectOutputContract(
  format: OutputContract["format"],
  files: readonly string[],
): OutputContract {
  return {
    format,
    files: [...files],
    shellUiIncluded: false,
  };
}

export const MARKDOWN_KNOWLEDGE_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a React + Vite Markdown knowledge app with local notes, categories, tags, search, preview, and editing.",
    "Do not access arbitrary user notes, upload notes, use SQLite, call cloud services, or execute plugins.",
  ],
  fileSystemRules: [
    "Write Markdown content and index files only inside generated/markdown.",
    "Write React UI files only inside generated/react.",
    "Do not read or write paths outside the active Sofvary workspace.",
    "Do not create files outside the Markdown Knowledge output contract.",
  ],
  outputRules: [
    "Start with Markdown preview and editing.",
    "Search must use generated local content only.",
    "Keep Sofvary shell UI out of generated app source.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [
    "arbitrary-user-notes",
    "cloud-service",
    "external-ui-framework",
    "note-upload",
    "plugin-execution",
    "remote-network",
    "shell-command",
    "sofvary-shell-ui",
    "sqlite-runtime",
  ],
};

export const DATA_TABLE_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a React + Vite personal data table app using workspace-local JSON data.",
    "Do not upload table data, use remote databases, access arbitrary CSV files, or execute plugins.",
  ],
  fileSystemRules: [
    "Write table schema and data only inside generated/data.",
    "Write React UI files only inside generated/react.",
    "CSV import must require an explicit user-selected file.",
    "Do not create files outside the Data Table output contract.",
  ],
  outputRules: [
    "Support add, edit, delete, search, filter, and sort for table rows.",
    "CSV import remains a safe placeholder until a user-selected file is available.",
    "Keep Sofvary shell UI out of generated app source.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [
    "arbitrary-csv-access",
    "cloud-service",
    "data-upload",
    "plugin-execution",
    "remote-database",
    "remote-network",
    "shell-command",
    "sofvary-shell-ui",
    "sqlite-runtime",
  ],
};

export const FILE_PROCESSOR_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a React file processor app that starts read-only and requires explicit file or folder selection.",
    "Phase 14 must not modify files; confirmation records the dry-run plan only.",
  ],
  fileSystemRules: [
    "Write React UI files only inside generated/react.",
    "Write runtime-local policy metadata only inside generated/file-processor.",
    "Never access paths that were not selected by the user.",
    "Do not create files outside the File Processor output contract.",
  ],
  outputRules: [
    "Show a dry-run preview before any write-like operation.",
    "Confirmation records the plan in the operation log only.",
    "Do not rename, delete, move, or rewrite files in Phase 14.",
    "Log planned operations without executing them.",
    "Keep Sofvary shell UI out of generated app source.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [
    "arbitrary-file-access",
    "file-mutation",
    "permanent-delete",
    "global-install",
    "path-traversal",
    "plugin-execution",
    "remote-network",
    "shell-command",
    "sofvary-shell-ui",
    "write-without-dry-run",
  ],
};

export const DESKTOP_WIDGET_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a compact React desktop widget app that runs inside the main Sofvary preview.",
    "Do not create transparent windows, always-on-top windows, tray behavior, notifications, or system automation.",
  ],
  fileSystemRules: [
    "Write React UI files only inside generated/react.",
    "Write widget metadata only inside generated/widget.",
    "Do not read or write paths outside the active Sofvary workspace.",
    "Do not create files outside the Desktop Widget output contract.",
  ],
  outputRules: [
    "Keep the widget layout compact.",
    "Run inside the main Sofvary preview.",
    "Avoid unauthorized system APIs.",
    "Keep Sofvary shell UI out of generated app source.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [
    "always-on-top-window",
    "notification-plugin",
    "plugin-execution",
    "remote-network",
    "shell-command",
    "sofvary-shell-ui",
    "system-automation",
    "transparent-window",
    "tray-integration",
  ],
};

export const AI_AGENT_APP_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a React + Vite AI Agent App with provider binding settings and text, image, and video task flows through the Sofvary local AI Gateway.",
    "Generated app code must never store API keys, secure key references, local provider profile ids, or call external model providers directly.",
    "Expose provider requirements and binding state to the user, but leave actual provider selection and secrets to Sofvary.",
  ],
  fileSystemRules: [
    "Write agent metadata, provider requirements, and seeded job examples only inside generated/ai.",
    "Write React UI files only inside generated/react.",
    "Do not read or write paths outside the active Sofvary workspace.",
    "Do not create files outside the AI Agent App output contract.",
  ],
  outputRules: [
    "Start imported or newly generated apps in a needs-provider-binding state until Sofvary provides a local binding.",
    "Text, image, and video actions call only the Sofvary AI Gateway loopback endpoints such as /__sofvary/ai/text, /__sofvary/ai/image, /__sofvary/ai/video, job status, and artifact download.",
    "Provider requirements may be exported with a capsule, but secrets, secure key refs, and author-local provider ids must not be exported.",
    "Keep Sofvary shell UI out of generated app source.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [
    "0.0.0.0-bind",
    "arbitrary-model-endpoint",
    "capsule-secret-export",
    "coding-agent-gateway-access",
    "direct-provider-network",
    "local-provider-id-export",
    "plaintext-api-key",
    "plugin-execution",
    "remote-network",
    "secure-key-ref-export",
    "shell-command",
    "sofvary-shell-ui",
  ],
};
