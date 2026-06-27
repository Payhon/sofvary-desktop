export type PromptEnvelopeSchemaVersion = "1.0";

export interface PackReference {
  id: string;
  version: string;
}

export interface BoxRuntimeContext {
  runtimePack: PackReference;
  harnessPacks: PackReference[];
  runtimeKind: string;
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
  runtimeKind: string;
  allowedEntrypoints: string[];
  allowedServerBind: "127.0.0.1";
  network: "local-only";
  packageInstall: boolean;
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
  allowShell: boolean;
  allowPackageInstall: boolean;
  allowGlobalInstall: boolean;
  allowedCommands: string[];
}

export interface OutputContract {
  format: string;
  files: string[];
  shellUiIncluded: boolean;
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
