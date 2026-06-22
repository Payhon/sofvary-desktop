export type RuntimeKind =
  | "static-html"
  | "react-vite"
  | "react-sqlite"
  | "ai-agent-app"
  | "canvas2d"
  | "markdown-knowledge"
  | "data-table"
  | "file-processor"
  | "desktop-widget";
export type RuntimeChoice = "auto" | RuntimeKind;
export type RuntimeMode = "dev" | "prod";
export type PackKind = "runtime" | "harness" | "plugin";
export type PackSource = "builtin" | "cache" | "registry";
export type UiThemePreference = "system" | "dark" | "light";
export type ResolvedUiTheme = "dark" | "light";
export type UiAccentPreference = "blue" | "teal" | "violet" | "amber" | "rose";
export type UiDensityPreference = "compact" | "comfortable" | "spacious";
export type UiGlassPreference = "solid" | "balanced" | "transparent";
export type UiRadiusPreference = "sharp" | "soft" | "rounded";
export type UiShadowPreference = "flat" | "soft" | "deep";
export type UiMotionPreference = "reduced" | "balanced" | "expressive";
export interface UiAppearancePreferences {
  themePreference: UiThemePreference;
  accent: UiAccentPreference;
  density: UiDensityPreference;
  glass: UiGlassPreference;
  radius: UiRadiusPreference;
  shadow: UiShadowPreference;
  motion: UiMotionPreference;
}
export type AgentProvider =
  | "codex"
  | "claude-code"
  | "cursor"
  | "opencode"
  | "kimi-code"
  | "qoder"
  | "deepseek-tui"
  | "sofvary-pi"
  | "custom";
export type AgentTransportKind = "acp" | "cli" | "pi-rpc";
export type AgentInstallSource =
  | "bundled"
  | "dev-override"
  | "external-path"
  | "missing"
  | "manual";
export type RuntimeEnvironmentKind = "nodejs" | "python";
export type RuntimeEnvironmentInstallState =
  | "installed"
  | "not-installed"
  | "installing"
  | "failed"
  | "unsupported";
export type RuntimeEnvironmentSource = "managed" | "external-path" | "missing";

export type ShellState =
  | "BackgroundIdle"
  | "CommandMenuVisible"
  | "Planning"
  | "Building"
  | "Previewing"
  | "Error";

export interface WorkspaceSummary {
  appId: string;
  name: string;
  mode: RuntimeKind;
  updatedAt: string;
  root: string;
}

export interface InstalledPackSummary {
  id: string;
  version: string;
  kind: PackKind;
  name: string;
  description: string;
  source: PackSource;
  sha256?: string | null;
  signature?: string | null;
}

export interface ResolveRegistryPackPayload {
  id: string;
  version: string;
}

export interface InstallRegistryPackPayload {
  id: string;
  version: string;
  appId?: string;
  policyApprovals?: PolicyApprovalSet;
}

export interface InstallRegistryPackResult {
  pack: InstalledPackSummary;
  installed: boolean;
  lockfileUpdated: boolean;
}

export interface AppCapsuleExportPayload {
  appId: string;
  includePromptHistory: boolean;
  outputPath: string;
}

export interface AppCapsuleImportPayload {
  capsulePath: string;
  policyApprovals?: PolicyApprovalSet;
}

export interface AppCapsuleOperationResult {
  appId?: string;
  name?: string;
  capsulePath?: string;
  workspace?: WorkspaceSummary;
}

export type AppReleaseTargetPlatform = "windows" | "macos" | "linux";
export type AppReleaseJobStatus = "completed" | "failed" | "canceled";
export type PackagerToolchainRequirementKind =
  | "node"
  | "pnpm"
  | "rustc"
  | "cargo"
  | "tauri-cli";

export interface AppReleaseStealthUiSettings {
  aiMenuLabel: string;
  aiShortcut: string;
  aiPanelTitle: string;
  providerSetupTitle: string;
  promptPlaceholder: string;
}

export interface AppReleasePayload {
  appId: string;
  appName: string;
  targetPlatform: AppReleaseTargetPlatform;
  outputDir: string;
  iconPath?: string | null;
  includeAiContinuation: boolean;
  stealthUi: AppReleaseStealthUiSettings;
  selectedRuntimePacks: string[];
  selectedPluginPacks: string[];
  policyApprovals?: PolicyApprovalSet;
}

export interface AppReleasePlatformCapability {
  platform: AppReleaseTargetPlatform;
  label: string;
  enabled: boolean;
  current: boolean;
  reason?: string | null;
  outputKind: string;
}

export interface AppReleaseRuntimeCapability {
  runtimeKind: RuntimeKind;
  label: string;
  supported: boolean;
  releaseStrategy: string;
  aiContinuationSupported: boolean;
  notes: string[];
}

export interface AppReleaseCapability {
  currentPlatform: AppReleaseTargetPlatform;
  beta: boolean;
  targetPlatforms: AppReleasePlatformCapability[];
  runtimes: AppReleaseRuntimeCapability[];
}

export interface AppReleaseJob {
  jobId: string;
  status: AppReleaseJobStatus;
  appId: string;
  appName: string;
  targetPlatform: AppReleaseTargetPlatform | "";
  outputDir: string;
  stagingDir?: string | null;
  artifactPath?: string | null;
  nativeAppPath?: string | null;
  nativeInstallerPath?: string | null;
  manifestPath?: string | null;
  detail: string;
}

export interface PackagerToolchainRequirementStatus {
  kind: PackagerToolchainRequirementKind;
  label: string;
  installed: boolean;
  required: boolean;
  installable: boolean;
  version?: string | null;
  detail: string;
}

export interface PackagerToolchainStatus {
  platform: AppReleaseTargetPlatform;
  ready: boolean;
  beta: boolean;
  installActionAvailable: boolean;
  requirements: PackagerToolchainRequirementStatus[];
  detail: string;
}

export interface AppBoxManifest {
  appId: string;
  name: string;
  mode: RuntimeKind;
  createdAt: string;
  updatedAt: string;
  paths: {
    root: string;
    generated: string;
    generatedStatic: string;
    runtime: string;
    snapshots: string;
  };
  preview: {
    state: string;
    url: string | null;
  };
}

export interface RuntimePreview {
  appId: string;
  runtimeKind: RuntimeKind;
  runtimeMode: RuntimeMode;
  previewUrl: string;
  logs: string[];
  manifest: AppBoxManifest;
  promptEnvelopeSummary: PromptEnvelopeSummary;
}

export interface RuntimePreviewIssue {
  kind: string;
  runtimeKind: RuntimeKind;
  summary: string;
  diagnostic?: Record<string, unknown> | null;
  sourceDetail?: string | null;
  repairAction?: "install-runtime-environment" | "retry-preview" | string;
}

export interface RuntimeIntentSelection {
  runtimeKind: RuntimeKind;
  softwareType: string;
  confidence: number;
  reason: string;
  matchedSignals: string[];
  alternatives: RuntimeKind[];
  source: "automatic" | "manual";
}

export type BuildThreadStatus =
  | "queued"
  | "planning"
  | "building"
  | "repairing"
  | "previewing"
  | "preview-blocked"
  | "completed"
  | "failed"
  | "canceled";

export type BuildThreadEntryKind =
  | "user"
  | "assistant"
  | "agent-event"
  | "tool"
  | "file"
  | "system"
  | "error";

export interface BuildThreadSummary {
  id: string;
  title: string;
  status: BuildThreadStatus;
  workspaceId?: string | null;
  appId?: string | null;
  runtimeKind: RuntimeKind;
  runtimeMode: RuntimeMode;
  agentId: string;
  createdAt: string;
  updatedAt: string;
  preview?: RuntimePreview | null;
  previewIssue?: RuntimePreviewIssue | null;
  error?: string | null;
}

export interface BuildThreadPreviewRetryResult {
  thread: BuildThreadSummary;
  preview: RuntimePreview;
}

export interface BuildThreadEntry {
  id: string;
  threadId: string;
  timestamp: string;
  kind: BuildThreadEntryKind;
  content: string;
  metadata?: Record<string, unknown>;
}

export interface BuildThreadDetail {
  summary: BuildThreadSummary;
  entries: BuildThreadEntry[];
}

export type GatewayUniEventType =
  | "session.started"
  | "turn.started"
  | "message.delta"
  | "reasoning.delta"
  | "tool.started"
  | "tool.delta"
  | "tool.completed"
  | "approval.requested"
  | "approval.resolved"
  | "terminal.output"
  | "file.write.requested"
  | "file.written"
  | "status.changed"
  | "turn.completed"
  | "error";

export interface GatewayUniEvent {
  eventId: string;
  threadId: string;
  timestamp: string;
  agentId: string;
  transport: AgentTransportKind;
  sequence: number;
  type: GatewayUniEventType;
  payload: Record<string, unknown>;
}

export type LlmProviderKind =
  | "openai"
  | "anthropic"
  | "openrouter"
  | "deepseek"
  | "google"
  | "groq"
  | "xai"
  | "kimi-coding"
  | "ollama"
  | "openai-compatible";

export interface LlmProviderTestRecord {
  ok: boolean;
  checkedAt: string;
  detail: string;
}

export interface LlmProviderConfig {
  providerId: string;
  label: string;
  kind: LlmProviderKind;
  baseUrl?: string | null;
  model: string;
  apiKeyRef?: string | null;
  enabled: boolean;
  lastTest?: LlmProviderTestRecord | null;
}

export interface LlmProviderConfigState {
  defaultProviderId?: string | null;
  providers: LlmProviderConfig[];
}

export interface RegistryArtifactMetadata {
  id: string;
  kind: string;
  fileName: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
  storageKey: string;
  status: string;
  createdAt: string;
  signature?: string | null;
}

export interface RegistryAppMetadata {
  id: string;
  name: string;
  summary: string;
  visibility: string;
}

export interface RegistryAppVersionMetadata {
  id: string;
  appId: string;
  version: string;
  artifactId: string;
  artifact: RegistryArtifactMetadata;
  notes: string;
  createdAt: string;
}

export interface InstallAppDeepLink {
  appId: string;
  version: string;
}

export interface InstallPermissionSummary {
  workspaceRead: string[];
  workspaceWrite: string[];
  localDatabase: string;
  network: string;
  deviceAccess: string;
  systemAccess: string;
  requested: string[];
  pluginPacks: string[];
}

export interface DeepLinkInstallPreflight {
  request: InstallAppDeepLink;
  app: RegistryAppMetadata;
  version: RegistryAppVersionMetadata;
  artifact: RegistryArtifactMetadata;
  permissionSummary: InstallPermissionSummary;
}

export interface DeepLinkInstallResult extends DeepLinkInstallPreflight {
  importResult: AppCapsuleOperationResult;
  preview: RuntimePreview;
}

export interface AgentCommandConfig {
  executable: string;
  args: string[];
  env: Record<string, string>;
  source: AgentInstallSource;
}

export interface AgentTestRecord {
  ok: boolean;
  transport: AgentTransportKind;
  checkedAt: string;
  detail: string;
}

export interface AgentConfig {
  id: string;
  provider: AgentProvider;
  label: string;
  enabled: boolean;
  acp?: AgentCommandConfig | null;
  cli?: AgentCommandConfig | null;
  allowCliFallback: boolean;
  lastTest?: AgentTestRecord | null;
}

export interface AgentConfigState {
  defaultAgentId?: string | null;
  agents: AgentConfig[];
}

export interface DiscoveredAgent {
  config: AgentConfig;
  detected: boolean;
  status: string;
}

export type AgentInstallCapability = "managed" | "manual-download" | "unavailable";

export type AgentInstallStateKind =
  | "installed"
  | "not-installed"
  | "installing"
  | "failed"
  | "manual"
  | "needs-runtime"
  | "unsupported";

export interface AgentInstallCommandTemplate {
  executable: string;
  args: string[];
}

export interface AgentInstallCatalogItem {
  id: string;
  label: string;
  iconKey: string;
  provider: AgentProvider;
  docsUrl: string;
  installCapability: AgentInstallCapability;
  recommended: boolean;
  managed: boolean;
  supported: boolean;
  detectCommands: string[];
  acp?: AgentInstallCommandTemplate | null;
  cli?: AgentInstallCommandTemplate | null;
  versionCommand?: AgentInstallCommandTemplate | null;
}

export interface AgentInstallRecord {
  agentId: string;
  state: AgentInstallStateKind;
  detail: string;
  checkedAt: string;
  installMethod?: string | null;
  version?: string | null;
  executable?: string | null;
}

export interface AgentInstallStatus {
  catalog: AgentInstallCatalogItem;
  configured?: AgentConfig | null;
  detected: boolean;
  source?: AgentInstallSource | null;
  executable?: string | null;
  version?: string | null;
  installState: AgentInstallStateKind;
  detail: string;
  lastTest?: AgentTestRecord | null;
  lastInstall?: AgentInstallRecord | null;
}

export interface RuntimeEnvironmentVersionOption {
  version: string;
  label: string;
  channel: string;
  recommended: boolean;
  supported: boolean;
  platform: string;
  artifactUrl: string;
  sha256: string;
  pnpmVersion: string;
  pnpmArtifactUrl: string;
  pnpmIntegrity: string;
}

export interface RuntimeEnvironmentCatalogItem {
  kind: RuntimeEnvironmentKind;
  label: string;
  description: string;
  requiredTools: string[];
  supported: boolean;
  versions: RuntimeEnvironmentVersionOption[];
}

export interface RuntimeEnvironmentToolStatus {
  name: string;
  ok: boolean;
  version?: string | null;
  executable?: string | null;
  source: RuntimeEnvironmentSource;
  detail: string;
}

export interface RuntimeEnvironmentInstallRecord {
  kind: RuntimeEnvironmentKind;
  version: string;
  state: RuntimeEnvironmentInstallState;
  detail: string;
  checkedAt: string;
  platform: string;
  sha256: string;
  installPath?: string | null;
}

export interface RuntimeEnvironmentStatus {
  catalog: RuntimeEnvironmentCatalogItem;
  activeVersion?: string | null;
  installState: RuntimeEnvironmentInstallState;
  detail: string;
  source: RuntimeEnvironmentSource;
  supported: boolean;
  node?: RuntimeEnvironmentToolStatus | null;
  pnpm?: RuntimeEnvironmentToolStatus | null;
  lastInstall?: RuntimeEnvironmentInstallRecord | null;
}

export interface StartRuntimeEnvironmentInstallPayload {
  kind: RuntimeEnvironmentKind;
  version: string;
  policyApprovals: PolicyApprovalSet;
}

export type PolicyActionKind =
  | "agent-file-write"
  | "external-agent-process"
  | "command-execution"
  | "dependency-install"
  | "runtime-start"
  | "capsule-import"
  | "pack-install"
  | "agent-install"
  | "runtime-environment-install"
  | "workspace-lockfile-update"
  | "plugin-enablement"
  | "ai-provider-key-store"
  | "ai-provider-rebind"
  | "ai-provider-call"
  | "app-release";

export type PolicyDecisionKind = "allowed" | "requires-confirmation" | "forbidden";

export interface PolicyDecision {
  action: PolicyActionKind;
  decision: PolicyDecisionKind;
  title: string;
  summary: string;
  reasons: string[];
  risks: string[];
  subject?: string | null;
}

export interface PolicyApprovalGrant {
  action: PolicyActionKind;
  subject?: string | null;
}

export interface PolicyApprovalSet {
  approved: PolicyApprovalGrant[];
}

export type PolicyPreviewScope =
  | "runtime-build"
  | "capsule-import"
  | "deep-link-install"
  | "pack-install"
  | "agent-install"
  | "runtime-environment-install"
  | "app-release";

export interface PreviewPolicyPayload {
  scope: PolicyPreviewScope;
  runtimeKind?: RuntimeKind;
  mode?: RuntimeMode;
  agentId?: string;
  packKind?: PackKind;
  packId?: string;
  runtimeEnvironmentKind?: RuntimeEnvironmentKind;
  version?: string;
  appId?: string;
  appName?: string;
  targetPlatform?: AppReleaseTargetPlatform;
  outputDir?: string;
  includeAiContinuation?: boolean;
  selectedPluginPacks?: string[];
  capsulePath?: string;
  capsuleName?: string;
  permissionSummary?: InstallPermissionSummary;
}

export interface PolicyPreview {
  decisions: PolicyDecision[];
}

export interface PromptEnvelopeSummary {
  runtime: string;
  harnesses: string[];
  allowedFiles: string[];
  blockedCapabilities: string[];
  outputContract: string[];
  acceptanceCriteriaCount: number;
}

export interface PlatformBootstrap {
  os: "windows" | "macos" | "linux";
  arch: "x64" | "arm64" | "unknown";
  shortcut: string;
  trayOrMenuBarAvailable: boolean;
}
