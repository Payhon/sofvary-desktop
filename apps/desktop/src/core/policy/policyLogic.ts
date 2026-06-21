import type { PolicyActionKind, PolicyApprovalSet, PolicyDecision } from "../../types";

type Translator = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
  fallback?: string,
) => string;

export type PolicyConfirmationState =
  | { kind: "clear" }
  | { kind: "requires-confirmation"; decisions: PolicyDecision[] }
  | { kind: "forbidden"; decisions: PolicyDecision[] };

export function emptyPolicyApprovals(): PolicyApprovalSet {
  return { approved: [] };
}

export function summarizePolicyDecisions(decisions: PolicyDecision[]): PolicyConfirmationState {
  const forbidden = decisions.filter((decision) => decision.decision === "forbidden");
  if (forbidden.length > 0) {
    return { kind: "forbidden", decisions: forbidden };
  }

  const requiresConfirmation = decisions.filter(
    (decision) => decision.decision === "requires-confirmation",
  );
  if (requiresConfirmation.length > 0) {
    return { kind: "requires-confirmation", decisions: requiresConfirmation };
  }

  return { kind: "clear" };
}

export function buildPolicyApprovalSet(decisions: PolicyDecision[]): PolicyApprovalSet {
  return {
    approved: decisions
      .filter((decision) => decision.decision === "requires-confirmation")
      .map((decision) => ({
        action: decision.action,
        subject: decision.subject ?? null,
      })),
  };
}

export function formatPolicyActionLabel(
  action: PolicyActionKind,
  t: Translator = fallbackPolicyT,
): string {
  const key = policyActionKeys[action];
  return key ? t(key, {}, action) : action;
}

export function formatPolicyDialogTitle(
  title: string,
  t: Translator = fallbackPolicyT,
): string {
  if (title === "Import app capsule") {
    return t("policy.dialog.importCapsule");
  }
  if (title === "Install shared app capsule") {
    return t("policy.dialog.installSharedCapsule");
  }

  const installToolchainMatch = title.match(/^Install Node\.js Toolchain (.+)$/);
  if (installToolchainMatch) {
    return t("policy.dialog.installNodeToolchain", { version: installToolchainMatch[1] });
  }

  const publishMatch = title.match(/^Publish (.+)$/);
  if (publishMatch) {
    return t("policy.dialog.publishApp", { name: publishMatch[1] });
  }

  const continueMatch = title.match(/^Continue (.+)$/);
  if (continueMatch) {
    return t("policy.dialog.continueRuntime", { runtimeKind: continueMatch[1] });
  }

  const runMatch = title.match(/^Run (.+)$/);
  if (runMatch) {
    return t("policy.dialog.runRuntime", { runtimeKind: runMatch[1] });
  }

  const installMatch = title.match(/^Install (.+)$/);
  if (installMatch) {
    return t("policy.dialog.installAgent", { label: installMatch[1] });
  }

  return title;
}

export function formatPolicyDecisionTitle(
  decision: PolicyDecision,
  t: Translator = fallbackPolicyT,
): string {
  return formatPolicySourceText(decision.title, t);
}

export function formatPolicyDecisionSummary(
  decision: PolicyDecision,
  t: Translator = fallbackPolicyT,
): string {
  return formatPolicySourceText(decision.summary, t);
}

export function formatPolicyDecisionReasons(
  decision: PolicyDecision,
  t: Translator = fallbackPolicyT,
): string[] {
  return decision.reasons.map((reason) => formatPolicySourceText(reason, t));
}

export function formatPolicyDecisionRisks(
  decision: PolicyDecision,
  t: Translator = fallbackPolicyT,
): string[] {
  return decision.risks.map((risk) => formatPolicyRisk(risk, t));
}

export function formatPolicyBlockMessage(
  decisions: PolicyDecision[],
  t: Translator = fallbackPolicyT,
): string {
  const primary = decisions[0];
  if (!primary) {
    return t("policy.blocked.default");
  }

  const title = formatPolicyDecisionTitle(primary, t);
  const summary = ensureTerminalPunctuation(formatPolicyDecisionSummary(primary, t));
  const reason = formatPolicyDecisionReasons(primary, t)[0];
  if (!reason) {
    return t("policy.blocked.message", { title, summary }, `${title}: ${summary}`);
  }

  return t(
    "policy.blocked.withReason",
    { title, summary, reason: ensureTerminalPunctuation(reason) },
    `${title}: ${summary} ${ensureTerminalPunctuation(reason)}`,
  );
}

const policyActionKeys: Record<PolicyActionKind, string> = {
  "agent-file-write": "policy.action.agentFileWrite",
  "external-agent-process": "policy.action.externalAgentProcess",
  "command-execution": "policy.action.commandExecution",
  "dependency-install": "policy.action.dependencyInstall",
  "runtime-start": "policy.action.runtimeStart",
  "capsule-import": "policy.action.capsuleImport",
  "pack-install": "policy.action.packInstall",
  "agent-install": "policy.action.agentInstall",
  "runtime-environment-install": "policy.action.runtimeEnvironmentInstall",
  "workspace-lockfile-update": "policy.action.workspaceLockfileUpdate",
  "plugin-enablement": "policy.action.pluginEnablement",
  "ai-provider-key-store": "policy.action.aiProviderKeyStore",
  "ai-provider-rebind": "policy.action.aiProviderRebind",
  "ai-provider-call": "policy.action.aiProviderCall",
  "app-release": "policy.action.appRelease",
};

const policySourceTextKeys: Record<string, string> = {
  "File write blocked": "policy.decision.fileWriteBlocked.title",
  "File write allowed": "policy.decision.fileWriteAllowed.title",
  "Command blocked": "policy.decision.commandBlocked.title",
  "Network command requires approval": "policy.decision.networkCommandRequiresApproval.title",
  "Command allowed": "policy.decision.commandAllowed.title",
  "Command requires approval": "policy.decision.commandRequiresApproval.title",
  "External agent requires approval": "policy.decision.externalAgentRequiresApproval.title",
  "Agent install requires approval": "policy.decision.agentInstallRequiresApproval.title",
  "Runtime environment install requires approval":
    "policy.decision.runtimeEnvironmentInstallRequiresApproval.title",
  "Dependency install requires approval": "policy.decision.dependencyInstallRequiresApproval.title",
  "Runtime start blocked": "policy.decision.runtimeStartBlocked.title",
  "Runtime network access requires approval":
    "policy.decision.runtimeNetworkAccessRequiresApproval.title",
  "Runtime start allowed": "policy.decision.runtimeStartAllowed.title",
  "Pack install requires approval": "policy.decision.packInstallRequiresApproval.title",
  "Pack install allowed": "policy.decision.packInstallAllowed.title",
  "Plugin enablement requires approval": "policy.decision.pluginEnablementRequiresApproval.title",
  "App release blocked": "policy.decision.appReleaseBlocked.title",
  "App release requires approval": "policy.decision.appReleaseRequiresApproval.title",
  "Workspace lockfile update requires approval":
    "policy.decision.workspaceLockfileUpdateRequiresApproval.title",
  "AI provider key storage blocked": "policy.decision.aiProviderKeyStorageBlocked.title",
  "AI provider key storage requires approval":
    "policy.decision.aiProviderKeyStorageRequiresApproval.title",
  "AI provider binding blocked": "policy.decision.aiProviderBindingBlocked.title",
  "AI provider binding requires approval": "policy.decision.aiProviderBindingRequiresApproval.title",
  "AI provider call blocked": "policy.decision.aiProviderCallBlocked.title",
  "AI provider call requires approval": "policy.decision.aiProviderCallRequiresApproval.title",
  "Capsule import blocked": "policy.decision.capsuleImportBlocked.title",
  "Capsule import requires approval": "policy.decision.capsuleImportRequiresApproval.title",
  "The requested file write leaves the active workspace boundary.":
    "policy.decision.fileWriteBoundary.summary",
  "The target is inside the active workspace generated area or metadata lockfile.":
    "policy.decision.fileWriteAllowed.summary",
  "The target path is not allowed by the Phase 22 file policy.":
    "policy.decision.fileWritePhasePolicy.summary",
  "Global package installation is forbidden by the Phase 22 command policy.":
    "policy.decision.globalInstallForbidden.summary",
  "Commands that modify PATH are forbidden.": "policy.decision.pathModifyForbidden.summary",
  "Remote download scripts are forbidden.": "policy.decision.remoteDownloadForbidden.summary",
  "Binding generated app servers to non-loopback interfaces is forbidden.":
    "policy.decision.publicBindForbidden.summary",
  "Deleting system directories is forbidden.": "policy.decision.deleteSystemForbidden.summary",
  "This command requests network access.": "policy.decision.networkCommand.summary",
  "The command matches Sofvary's local runtime command allowlist.":
    "policy.decision.commandAllowed.summary",
  "This structured command is not on the default allowlist.":
    "policy.decision.commandRequiresApproval.summary",
  "Sofvary is about to start a configured coding agent process.":
    "policy.decision.externalAgent.summary",
  "Sofvary is about to install or open setup instructions for a coding agent.":
    "policy.decision.agentInstall.summary",
  "Sofvary is about to install a managed runtime environment into application data.":
    "policy.decision.runtimeEnvironmentInstall.summary",
  "The runtime wants to install workspace dependencies.":
    "policy.decision.dependencyInstall.summary",
  "Generated app runtimes must bind to 127.0.0.1.":
    "policy.decision.runtimeStartBlocked.summary",
  "The runtime requests non-local network access.":
    "policy.decision.runtimeNetworkAccess.summary",
  "The runtime starts inside the current workspace and binds locally.":
    "policy.decision.runtimeStartAllowed.summary",
  "Installing a registry or plugin pack requires explicit approval.":
    "policy.decision.packInstall.summary",
  "Built-in pack installation is allowed.": "policy.decision.packInstallAllowed.summary",
  "Enabling a plugin pack requires explicit approval.":
    "policy.decision.pluginEnablement.summary",
  "Publishing requires complete app, platform, and output metadata.":
    "policy.decision.appReleaseMetadataRequired.summary",
  "Publishing requires a concrete output folder.":
    "policy.decision.appReleaseOutputRequired.summary",
  "Sofvary is about to create a local beta release package for this generated app.":
    "policy.decision.appRelease.summary",
  "Installing a registry pack into a workspace changes that workspace's exact pack lockfile.":
    "policy.decision.workspaceLockfileUpdate.summary",
  "AI provider key storage requires complete app, profile, and provider metadata.":
    "policy.decision.aiProviderKeyStorageMetadataRequired.summary",
  "Secure credential storage is not available on this platform session.":
    "policy.decision.secureStoreUnavailable.summary",
  "Sofvary is about to store an AI provider key in platform secure storage.":
    "policy.decision.aiProviderKeyStorage.summary",
  "AI provider rebinding requires complete app, requirement, and profile metadata.":
    "policy.decision.aiProviderRebindMetadataRequired.summary",
  "Sofvary is about to bind this AI Agent App to a local provider profile.":
    "policy.decision.aiProviderBinding.summary",
  "AI provider calls require complete app, profile, provider, and capability metadata.":
    "policy.decision.aiProviderCallMetadataRequired.summary",
  "This AI Agent App does not have an approved provider binding.":
    "policy.decision.aiProviderBindingMissing.summary",
  "Generated AI Agent Apps may only call the Sofvary AI Gateway on 127.0.0.1.":
    "policy.decision.aiProviderGatewayBind.summary",
  "Secure credential storage is not available, so real AI provider calls are disabled.":
    "policy.decision.aiProviderCallSecureStoreUnavailable.summary",
  "The selected AI provider profile does not have a stored credential.":
    "policy.decision.aiProviderCredentialMissing.summary",
  "Sofvary is about to send this request through the local AI Gateway.":
    "policy.decision.aiProviderCall.summary",
  "Capsules requesting non-local network access are blocked in Phase 22.":
    "policy.decision.capsuleNetworkBlocked.summary",
  "Importing an App Capsule creates a new local workspace from external package metadata.":
    "policy.decision.capsuleImport.summary",
  "Importing an App Capsule creates a local workspace.":
    "policy.decision.capsuleImportLegacy.summary",
  "Sofvary is about to install Node.js.": "policy.decision.installNodeLegacy.summary",
  "pnpm install is about to run": "policy.decision.pnpmInstallLegacy.summary",
  "Global install is not allowed": "policy.decision.globalInstallLegacy.summary",
  "Package registry network access may download workspace dependencies.":
    "policy.risk.packageRegistryNetwork",
  "AI continuation metadata is included; raw provider secrets are not allowed.":
    "policy.risk.aiContinuationMetadata",
  "Workspace boundary must contain every generated file write.":
    "policy.reason.workspaceBoundary",
  "Current workspace generated files and required metadata are allowed.":
    "policy.reason.currentWorkspaceFilesAllowed",
  "Agent writes are limited to generated files and Sofvary metadata.":
    "policy.reason.agentWritesLimited",
  "Global installs can modify shared system or user tooling.":
    "policy.reason.globalInstallsModifyTooling",
  "Generated apps must not modify system PATH.": "policy.reason.noSystemPathModify",
  "Sofvary does not execute curl/wget or shell download pipelines.":
    "policy.reason.noRemoteDownloadPipelines",
  "Runtime servers must remain on 127.0.0.1.": "policy.reason.runtimeLoopbackOnly",
  "Delete operations cannot target system directories.":
    "policy.reason.noSystemDirectoryDeletes",
  "Network access is off by default.": "policy.reason.networkOffByDefault",
  "Current workspace local dev/build commands are allowed.":
    "policy.reason.localCommandsAllowed",
  "Unknown commands require explicit approval.": "policy.reason.unknownCommandsRequireApproval",
  "External coding agents may use their own model credentials, network access, and native tool policy.":
    "policy.reason.externalAgentOwnPolicy",
  "Sofvary will still stage file output and validate workspace writes before preview.":
    "policy.reason.sofvaryStagesOutput",
  "Sofvary-managed installs stay inside the application data directory.":
    "policy.reason.managedInstallsInsideData",
  "External agents remain external processes and must be discovered, configured, and tested before use.":
    "policy.reason.externalAgentsRemainExternal",
  "Managed runtime environments stay inside the Sofvary data directory.":
    "policy.reason.runtimeEnvInsideData",
  "Managed runtime environments stay inside Sofvary data.":
    "policy.reason.runtimeEnvInsideDataLegacy",
  "Sofvary will verify artifact hashes before activating sidecar executables.":
    "policy.reason.verifyHashes",
  "This does not modify the system PATH or use a global package manager.":
    "policy.reason.noPathOrGlobalPackageManager",
  "Dependency installation is a high-risk action even when offline.":
    "policy.reason.dependencyInstallHighRisk",
  "Network access is only used to hydrate the local dependency cache when offline install cannot proceed.":
    "policy.reason.networkHydratesDependencyCache",
  "Dependency changes can alter local app behavior.":
    "policy.reason.dependencyChangesBehavior",
  "Runtime previews must stay local-only.": "policy.reason.runtimePreviewsLocalOnly",
  "Network access is disabled by default.": "policy.reason.networkDisabledByDefault",
  "Local runtime preview is allowed.": "policy.reason.localRuntimeAllowed",
  "Network-distributed packs are not implicitly trusted in Phase 22.":
    "policy.reason.distributedPacksNotTrusted",
  "Built-in packs are bundled with the desktop client.": "policy.reason.builtinPacksBundled",
  "Plugin execution is still out of scope for Phase 22.": "policy.reason.pluginOutOfScope",
  "Release metadata must be explicit before files are written.":
    "policy.reason.releaseMetadataExplicit",
  "Release artifacts cannot be written to an implicit location.":
    "policy.reason.releaseOutputExplicit",
  "Publishing reads the generated workspace and writes a distributable release package.":
    "policy.reason.publishReadsWorkspace",
  "The release package stores a seed copy and runtime metadata, not Sofvary account or marketplace state.":
    "policy.reason.releaseStoresSeedOnly",
  "AI continuation is white-label and requires the installed app user to configure their own provider credential.":
    "policy.reason.aiContinuationWhiteLabel",
  "Workspace runtime, harness, and plugin versions are reproducibility-critical.":
    "policy.reason.workspaceVersionsCritical",
  "Provider credentials must be scoped to a specific app profile.":
    "policy.reason.providerCredentialsScoped",
  "API keys must only be written to the platform secure store.":
    "policy.reason.apiKeysSecureStoreOnly",
  "The key will not be written into generated app files, logs, capsules, or provider binding metadata.":
    "policy.reason.providerKeyNotWrittenToFiles",
  "Generated apps may receive binding status only, never local provider ids or key references.":
    "policy.reason.generatedAppsNoLocalProviderIds",
  "Capsule exports will keep only provider requirements and reset imported apps to needs-provider-binding.":
    "policy.reason.capsulesResetProviderBinding",
  "Provider calls must be attributable to a local app binding.":
    "policy.reason.providerCallsAttributable",
  "Imported AI Agent Apps start as needs-provider-binding.":
    "policy.reason.importedAppsNeedProviderBinding",
  "Direct provider network calls from generated apps are forbidden.":
    "policy.reason.directProviderCallsForbidden",
  "Provider keys must come from platform secure storage before a gateway adapter can call a remote provider.":
    "policy.reason.providerKeysFromSecureStorage",
  "No token or API key is available for this provider binding.":
    "policy.reason.noProviderCredential",
  "Generated code talks only to the loopback gateway; provider credentials remain in secure storage.":
    "policy.reason.loopbackGatewayOnly",
  "Imported app capsules must remain local-only.": "policy.reason.importedCapsulesLocalOnly",
  "Capsule imports require shell-owned permission review.":
    "policy.reason.capsuleImportsShellReview",
};

const policyRiskPrefixKeys: Array<[prefix: string, key: string]> = [
  ["Output: ", "policy.risk.output"],
  ["Runtime: ", "policy.risk.runtime"],
  ["Plugin metadata: ", "policy.risk.pluginMetadata"],
  ["Workspace write: ", "policy.risk.workspaceWrite"],
  ["Requested: ", "policy.risk.requested"],
  ["Plugins: ", "policy.risk.plugins"],
];

function formatPolicySourceText(text: string, t: Translator): string {
  const key = policySourceTextKeys[text];
  return key ? t(key, {}, text) : text;
}

function formatPolicyRisk(risk: string, t: Translator): string {
  const exactKey = policySourceTextKeys[risk];
  if (exactKey) {
    return t(exactKey, {}, risk);
  }

  for (const [prefix, key] of policyRiskPrefixKeys) {
    if (risk.startsWith(prefix)) {
      return t(key, { value: risk.slice(prefix.length) }, risk);
    }
  }

  const runtimeEnvironmentMatch = risk.match(/^([^ ]+) ([^ ]+) for ([^(]+) \(([^)]+)\)$/);
  if (runtimeEnvironmentMatch) {
    return t(
      "policy.risk.runtimeEnvironment",
      {
        kind: runtimeEnvironmentMatch[1],
        version: runtimeEnvironmentMatch[2],
        platform: runtimeEnvironmentMatch[3].trim(),
        sha256: runtimeEnvironmentMatch[4],
      },
      risk,
    );
  }

  const viaMatch = risk.match(/^(.+) via (.+) \((.+)\)$/);
  if (viaMatch) {
    return t(
      "policy.risk.via",
      { label: viaMatch[1], method: viaMatch[2], subject: viaMatch[3] },
      risk,
    );
  }

  return risk;
}

function ensureTerminalPunctuation(value: string): string {
  return /[.!?。！？]$/.test(value) ? value : `${value}.`;
}

function fallbackPolicyT(
  key: string,
  params: Record<string, string | number | boolean | null | undefined> = {},
  fallback = key,
): string {
  const template = fallbackPolicyMessages[key] ?? fallback;
  return template.replace(/\{([a-zA-Z0-9_.-]+)\}/g, (match, name) =>
    params[name] === undefined || params[name] === null ? match : String(params[name]),
  );
}

const fallbackPolicyMessages: Record<string, string> = {
  "policy.blocked.default": "Security policy blocked the requested action.",
  "policy.blocked.message": "{title}: {summary}",
  "policy.blocked.withReason": "{title}: {summary} {reason}",
  "policy.action.agentFileWrite": "File write",
  "policy.action.externalAgentProcess": "External agent process",
  "policy.action.commandExecution": "Command execution",
  "policy.action.dependencyInstall": "Dependency install",
  "policy.action.runtimeStart": "Runtime start",
  "policy.action.capsuleImport": "Capsule import",
  "policy.action.packInstall": "Pack install",
  "policy.action.agentInstall": "Agent install",
  "policy.action.runtimeEnvironmentInstall": "Runtime environment install",
  "policy.action.workspaceLockfileUpdate": "Workspace lockfile update",
  "policy.action.pluginEnablement": "Plugin enablement",
  "policy.action.aiProviderKeyStore": "AI provider key storage",
  "policy.action.aiProviderRebind": "AI provider binding",
  "policy.action.aiProviderCall": "AI provider call",
  "policy.action.appRelease": "App release",
  "policy.dialog.installNodeToolchain": "Install Node.js Toolchain {version}",
  "policy.dialog.publishApp": "Publish {name}",
  "policy.dialog.importCapsule": "Import app capsule",
  "policy.dialog.installSharedCapsule": "Install shared app capsule",
  "policy.dialog.installAgent": "Install {label}",
  "policy.dialog.installRuntimeEnvironment": "Install {label} {version}",
  "policy.dialog.runRuntime": "Run {runtimeKind}",
  "policy.dialog.continueRuntime": "Continue {runtimeKind}",
  "policy.risk.output": "Output: {value}",
  "policy.risk.runtime": "Runtime: {value}",
  "policy.risk.pluginMetadata": "Plugin metadata: {value}",
  "policy.risk.workspaceWrite": "Workspace write: {value}",
  "policy.risk.requested": "Requested: {value}",
  "policy.risk.plugins": "Plugins: {value}",
  "policy.risk.runtimeEnvironment": "{kind} {version} for {platform} ({sha256})",
  "policy.risk.via": "{label} via {method} ({subject})",
};
