export type PromptEnvelopeSchemaVersion = "1.0";

export interface PackReference {
  id: string;
  version: string;
}

export interface BoxRuntimeContext {
  runtimePack: PackReference;
  harnessPacks: PackReference[];
  runtimeKind:
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
}

export interface CurrentAppFileContext {
  relativePath: string;
  contents: string;
  byteSize: number;
  truncated: boolean;
}

export interface CurrentAppState {
  appId: string;
  workspaceName: string;
  mode: "create" | "update";
  existingFiles: string[];
  fileContext?: CurrentAppFileContext[];
  previewState: string;
}

export interface RuntimePolicy {
  runtimeKind:
    | "static-html"
    | "react-vite"
    | "react-sqlite"
    | "canvas2d"
    | "markdown-knowledge"
    | "data-table"
    | "file-processor"
    | "desktop-widget"
    | "ai-agent-app";
  allowedEntrypoints: string[];
  allowedServerBind: "127.0.0.1";
  network: "local-only";
  packageInstall: false;
}

export interface HarnessPolicy {
  systemInstructions: string[];
  fileSystemRules: string[];
  outputRules: string[];
  blockedCapabilities: string[];
}

export interface FileSystemPolicy {
  root: string;
  allowedFiles: string[];
  allowExternalFiles: false;
  allowPathTraversal: false;
}

export interface CommandPolicy {
  allowShell: false;
  allowPackageInstall: false;
  allowGlobalInstall: false;
  allowedCommands: string[];
}

export interface OutputContract {
  format:
    | "static-html-files"
    | "react-vite-project"
    | "react-sqlite-project"
    | "canvas2d-project"
    | "markdown-knowledge-project"
    | "data-table-project"
    | "file-processor-project"
    | "desktop-widget-project"
    | "ai-agent-app-project";
  files: string[];
  shellUiIncluded: false;
}

export interface PromptEnvelope {
  schemaVersion: PromptEnvelopeSchemaVersion;
  envelopeId: string;
  createdAt: string;
  boxRuntimeContext: BoxRuntimeContext;
  userIntent: string;
  currentAppState: CurrentAppState;
  runtimePolicy: RuntimePolicy;
  harnessPolicy: HarnessPolicy;
  fileSystemPolicy: FileSystemPolicy;
  commandPolicy: CommandPolicy;
  outputContract: OutputContract;
  acceptanceCriteria: string[];
}

export interface PromptEnvelopeSummary {
  runtime: string;
  harnesses: string[];
  allowedFiles: string[];
  blockedCapabilities: string[];
  outputContract: string[];
  acceptanceCriteriaCount: number;
}

export function summarizePromptEnvelope(envelope: PromptEnvelope): PromptEnvelopeSummary {
  const runtime = `${envelope.boxRuntimeContext.runtimePack.id}@${envelope.boxRuntimeContext.runtimePack.version}`;
  const harnesses = envelope.boxRuntimeContext.harnessPacks.map((pack) => `${pack.id}@${pack.version}`);

  return {
    runtime,
    harnesses,
    allowedFiles: [...envelope.fileSystemPolicy.allowedFiles],
    blockedCapabilities: [...envelope.harnessPolicy.blockedCapabilities],
    outputContract: [...envelope.outputContract.files],
    acceptanceCriteriaCount: envelope.acceptanceCriteria.length,
  };
}
