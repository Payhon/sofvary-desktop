import { readFileSync, readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type {
  CommandPolicy,
  CurrentAppState,
  FileSystemPolicy,
  HarnessPolicy,
  OutputContract,
  PackReference,
  PromptEnvelope,
  RuntimePolicy,
} from "./promptEnvelope";

export interface RuntimePackInput {
  id: string;
  version: string;
  runtime: {
    kind: string;
    generatedRoot: string;
    entrypoint: string;
    bind: "127.0.0.1";
    network: "local-only";
  };
  executor: {
    kind: string;
    requiredToolchains?: string[];
    contextRoot?: string;
    allowedTopLevelDirs?: string[];
    clearRoots?: string[];
    preserveFiles?: string[];
  };
  promptEnvelope: string;
  defaultHarness: string[];
  selection?: {
    priority?: number;
  };
  builtin?: boolean;
  resourceRoot?: string;
}

export interface HarnessPackInput {
  id: string;
  version: string;
  runtime?: string;
  promptPolicy: string;
  builtin?: boolean;
  resourceRoot?: string;
}

export interface HarnessFragment {
  pack: PackReference;
  policy: HarnessPolicy;
  acceptanceCriteria: string[];
}

export interface CreatePromptEnvelopeInput {
  userIntent: string;
  currentAppState: CurrentAppState;
  runtimePack?: RuntimePackInput;
  harnessPacks?: HarnessPackInput[];
  envelopeId?: string;
  createdAt?: string;
}

interface RuntimePromptEnvelopeConfig {
  outputFormat: OutputContract["format"];
  allowedFiles: string[];
  fileSystemRoot: string;
  runtimePolicy: {
    allowedEntrypoints?: string[];
    packageInstall: boolean;
  };
  commandPolicy: CommandPolicy;
  harnessPolicy: HarnessPolicy;
  acceptanceCriteria: string[];
  updateMode?: {
    systemInstructions?: string[];
    outputRules?: string[];
    acceptanceCriteria?: string[];
  };
  shellUiIncluded?: boolean;
}

const sourceDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(sourceDir, "../../..");
export const builtinPackRoot = resolve(repoRoot, "apps/desktop/src-tauri/builtin-packs");

export function createPromptEnvelopeForRuntimeKind(
  runtimeKind: string,
  input: CreatePromptEnvelopeInput,
  packRoot = builtinPackRoot,
): PromptEnvelope {
  const runtimePack = input.runtimePack ?? readRuntimeManifestByKind(runtimeKind, packRoot);
  const harnessPacks = input.harnessPacks ?? [readDefaultHarnessManifest(runtimePack, packRoot)];
  return createPromptEnvelope({
    ...input,
    runtimePack,
    harnessPacks,
  });
}

export function listRuntimePackManifests(packRoot = builtinPackRoot): RuntimePackInput[] {
  return listPackManifests<RuntimePackInput>(packRoot, "runtimes").sort((left, right) => {
    const priority = leftSelectionPriority(left) - leftSelectionPriority(right);
    if (priority !== 0) return priority;
    return `${left.id}@${left.version}`.localeCompare(`${right.id}@${right.version}`);
  });
}

export function readRuntimeManifestByKind(
  runtimeKind: string,
  packRoot = builtinPackRoot,
): RuntimePackInput {
  const runtimePack = listRuntimePackManifests(packRoot).find(
    (manifest) => manifest.runtime.kind === runtimeKind,
  );
  if (!runtimePack) {
    throw new Error(`runtime kind '${runtimeKind}' is missing from pack catalog`);
  }
  return runtimePack;
}

export function createPromptEnvelope({
  userIntent,
  currentAppState,
  runtimePack,
  harnessPacks,
  envelopeId = createEnvelopeId(),
  createdAt = new Date().toISOString(),
}: CreatePromptEnvelopeInput & {
  runtimePack: RuntimePackInput;
  harnessPacks: HarnessPackInput[];
}): PromptEnvelope {
  assertHarnessPacks(runtimePack, harnessPacks);
  const envelopeConfig = readRuntimeEnvelopeConfig(runtimePack);
  const variables = templateVariables(userIntent, currentAppState, runtimePack, harnessPacks[0]);
  const allowedFiles = renderList(envelopeConfig.allowedFiles, variables);
  let harnessPolicy = renderPolicy(envelopeConfig.harnessPolicy, variables);

  for (const harnessPack of harnessPacks) {
    const policy = renderPolicy(readHarnessPolicy(harnessPack), variables);
    harnessPolicy = mergePolicy(harnessPolicy, policy);
  }

  const acceptanceCriteria = renderList(envelopeConfig.acceptanceCriteria, variables);
  if (currentAppState.mode === "update" && envelopeConfig.updateMode) {
    harnessPolicy = mergePolicy(harnessPolicy, {
      systemInstructions: renderList(envelopeConfig.updateMode.systemInstructions ?? [], variables),
      fileSystemRules: [],
      outputRules: renderList(envelopeConfig.updateMode.outputRules ?? [], variables),
      blockedCapabilities: [],
    });
    appendUnique(
      acceptanceCriteria,
      renderList(envelopeConfig.updateMode.acceptanceCriteria ?? [], variables),
    );
  }

  const runtimePolicy: RuntimePolicy = {
    runtimeKind: runtimePack.runtime.kind,
    allowedEntrypoints:
      envelopeConfig.runtimePolicy.allowedEntrypoints?.length
        ? renderList(envelopeConfig.runtimePolicy.allowedEntrypoints, variables)
        : [runtimePack.runtime.entrypoint],
    allowedServerBind: runtimePack.runtime.bind,
    network: runtimePack.runtime.network,
    packageInstall: envelopeConfig.runtimePolicy.packageInstall,
  };
  const fileSystemPolicy: FileSystemPolicy = {
    root: envelopeConfig.fileSystemRoot,
    allowedFiles,
    allowExternalFiles: false,
    allowPathTraversal: false,
  };
  const commandPolicy: CommandPolicy = {
    allowShell: envelopeConfig.commandPolicy.allowShell,
    allowPackageInstall: envelopeConfig.commandPolicy.allowPackageInstall,
    allowGlobalInstall: envelopeConfig.commandPolicy.allowGlobalInstall,
    allowedCommands: renderList(envelopeConfig.commandPolicy.allowedCommands, variables),
  };

  return {
    schemaVersion: "1.0",
    envelopeId,
    createdAt,
    boxRuntimeContext: {
      runtimePack: { id: runtimePack.id, version: runtimePack.version },
      harnessPacks: harnessPacks.map((pack) => ({ id: pack.id, version: pack.version })),
      runtimeKind: runtimePack.runtime.kind,
      generatedRoot: runtimePack.runtime.generatedRoot,
      entrypoint: runtimePack.runtime.entrypoint,
      bind: runtimePack.runtime.bind,
      network: runtimePack.runtime.network,
    },
    userIntent: userIntent.trim(),
    currentAppState,
    runtimePolicy,
    harnessPolicy,
    fileSystemPolicy,
    commandPolicy,
    outputContract: {
      format: envelopeConfig.outputFormat,
      files: allowedFiles,
      shellUiIncluded: envelopeConfig.shellUiIncluded ?? false,
    },
    acceptanceCriteria,
  };
}

export function mergeHarnessFragments(fragments: HarnessFragment[]): {
  policy: HarnessPolicy;
  acceptanceCriteria: string[];
} {
  const policy = fragments.reduce(
    (merged, fragment) => mergePolicy(merged, fragment.policy),
    emptyPolicy(),
  );
  return {
    policy,
    acceptanceCriteria: uniqueStable(fragments.flatMap((fragment) => fragment.acceptanceCriteria)),
  };
}

function readDefaultHarnessManifest(
  runtimePack: RuntimePackInput,
  packRoot = runtimePack.resourceRoot ?? builtinPackRoot,
): HarnessPackInput {
  const harnessId = runtimePack.defaultHarness[0];
  if (!harnessId) {
    throw new Error(`${runtimePack.id}@${runtimePack.version} must declare defaultHarness`);
  }
  const harness = listPackManifests<HarnessPackInput>(packRoot, "harness")
    .filter((manifest) => manifest.id === harnessId)
    .sort((left, right) => right.version.localeCompare(left.version))[0];
  if (!harness) {
    throw new Error(`default harness '${harnessId}' for ${runtimePack.id} is missing`);
  }
  return harness;
}

function readRuntimeEnvelopeConfig(runtimePack: RuntimePackInput): RuntimePromptEnvelopeConfig {
  const packRoot = runtimePack.resourceRoot ?? builtinPackRoot;
  return readJson(
    resolve(
      packRoot,
      "runtimes",
      runtimePack.id,
      runtimePack.version,
      runtimePack.promptEnvelope,
    ),
  );
}

function readHarnessPolicy(harnessPack: HarnessPackInput): HarnessPolicy {
  const packRoot = harnessPack.resourceRoot ?? builtinPackRoot;
  return readJson(
    resolve(
      packRoot,
      "harness",
      harnessPack.id,
      harnessPack.version,
      harnessPack.promptPolicy,
    ),
  );
}

function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

function listPackManifests<T extends { resourceRoot?: string }>(
  packRoot: string,
  kindDir: "runtimes" | "harness",
): T[] {
  const root = resolve(packRoot, kindDir);
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .flatMap((idEntry) => {
      const idRoot = resolve(root, idEntry.name);
      return readdirSync(idRoot, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((versionEntry) => {
          const manifest = readJson<T>(resolve(idRoot, versionEntry.name, "manifest.json"));
          manifest.resourceRoot = packRoot;
          return manifest;
        });
    });
}

function leftSelectionPriority(runtimePack: RuntimePackInput): number {
  return runtimePack.selection?.priority ?? 0;
}

function assertHarnessPacks(runtimePack: RuntimePackInput, harnessPacks: HarnessPackInput[]) {
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

function templateVariables(
  userIntent: string,
  currentAppState: CurrentAppState,
  runtimePack: RuntimePackInput,
  harnessPack: HarnessPackInput,
): Record<string, string> {
  return {
    "runtime.kind": runtimePack.runtime.kind,
    "runtime.id": runtimePack.id,
    "runtime.version": runtimePack.version,
    "runtime.generatedRoot": runtimePack.runtime.generatedRoot,
    "runtime.entrypoint": runtimePack.runtime.entrypoint,
    "runtime.bind": runtimePack.runtime.bind,
    "runtime.network": runtimePack.runtime.network,
    "executor.kind": runtimePack.executor.kind,
    "harness.id": harnessPack.id,
    "harness.version": harnessPack.version,
    "workspace.name": currentAppState.workspaceName,
    "user.intent": userIntent.trim(),
    "diagnostic.summary": "",
  };
}

function renderPolicy(policy: HarnessPolicy, variables: Record<string, string>): HarnessPolicy {
  return {
    systemInstructions: renderList(policy.systemInstructions, variables),
    fileSystemRules: renderList(policy.fileSystemRules, variables),
    outputRules: renderList(policy.outputRules, variables),
    blockedCapabilities: renderList(policy.blockedCapabilities, variables),
  };
}

function renderList(values: readonly string[], variables: Record<string, string>): string[] {
  return values.map((value) => renderTemplate(value, variables));
}

function renderTemplate(template: string, variables: Record<string, string>) {
  return template.replace(/\{\{\s*([^}]+?)\s*\}\}/g, (_, key: string) => {
    if (!(key in variables)) {
      throw new Error(`template variable '${key}' is missing`);
    }
    return variables[key] ?? "";
  });
}

function mergePolicy(left: HarnessPolicy, right: HarnessPolicy): HarnessPolicy {
  return {
    systemInstructions: uniqueStable([...left.systemInstructions, ...right.systemInstructions]),
    fileSystemRules: uniqueStable([...left.fileSystemRules, ...right.fileSystemRules]),
    outputRules: uniqueStable([...left.outputRules, ...right.outputRules]),
    blockedCapabilities: uniqueStable([
      ...left.blockedCapabilities,
      ...right.blockedCapabilities,
    ]),
  };
}

function emptyPolicy(): HarnessPolicy {
  return {
    systemInstructions: [],
    fileSystemRules: [],
    outputRules: [],
    blockedCapabilities: [],
  };
}

function appendUnique(target: string[], values: string[]) {
  for (const value of values) {
    if (value.trim() && !target.includes(value)) {
      target.push(value);
    }
  }
}

function uniqueStable(values: string[]): string[] {
  return values.filter((value, index, all) => value.trim() && all.indexOf(value) === index);
}

function createEnvelopeId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `penv_${crypto.randomUUID().replaceAll("-", "")}`;
  }

  return `penv_${Date.now().toString(36)}`;
}
