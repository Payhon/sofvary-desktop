import { memo, useEffect, useMemo, useRef, useState } from "react";
import type { Locale } from "@sofvary/i18n";
import {
  Boxes,
  Download,
  Eye,
  FileStack,
  ListTodo,
  PencilLine,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  PackageCheck,
  Settings,
  Store,
  Trash2,
  Upload,
} from "lucide-react";
import type {
  AgentConfig,
  AgentConfigState,
  AgentInstallStatus,
  BuildThreadDetail,
  BuildThreadSummary,
  DeepLinkInstallPreflight,
  DiscoveredAgent,
  InstalledPackSummary,
  LlmProviderConfig,
  LlmProviderConfigState,
  LlmProviderKind,
  PromptEnvelopeSummary,
  ResolvedUiTheme,
  RuntimeChoice,
  RuntimeEnvironmentStatus,
  RuntimeEnvironmentVersionOption,
  RuntimeIntentSelection,
  RuntimeKind,
  UiAppearancePreferences,
  UiThemePreference,
  WorkspaceSummary,
} from "../../types";
import {
  discoverableAgentsToAdd,
  formatDiscoveredAgentStatus,
  getAgentStatusLine,
  sortAgents,
} from "../../core/agents/agentLogic";
import {
  canInstallAgent,
  formatAgentConnection,
  formatAgentInstallDetail,
  formatAgentInstallState,
  formatAgentSource,
  formatAgentTest,
  getAgentIconLabel,
  getAgentInstallActionLabel,
  summarizeAgentInstall,
} from "../../core/agentInstall/agentInstallLogic";
import {
  canContinueBuildThread,
  formatBuildThreadStatus,
  getWorkspaceBuildThread,
  summarizeBuildThreadError,
  visibleThreadEntries,
} from "../../core/buildThreads/buildThreadLogic";
import {
  mergeBuildThreadPresentationItems,
  presentBuildThreadEntry,
  type BuildThreadPresentationItem,
} from "../../core/buildThreads/buildThreadPresentation";
import {
  createLlmProviderConfigFromPreset,
  getLlmModelOptions,
  getLlmProviderPreset,
  llmProviderPresets,
  normalizeLlmProviderDraft,
  sortLlmProviders,
} from "../../core/llmProviders/llmProviderLogic";
import {
  formatUiAccentPreference,
  formatUiAccentPreferenceDetail,
  formatUiDensityPreference,
  formatUiDensityPreferenceDetail,
  formatUiGlassPreference,
  formatUiGlassPreferenceDetail,
  formatUiMotionPreference,
  formatUiMotionPreferenceDetail,
  formatUiRadiusPreference,
  formatUiRadiusPreferenceDetail,
  formatUiShadowPreference,
  formatUiShadowPreferenceDetail,
  formatUiThemePreference,
  formatUiThemePreferenceDetail,
  formatUiThemeStatus,
  uiAccentPreferences,
  uiDensityPreferences,
  uiGlassPreferences,
  uiMotionPreferences,
  uiRadiusPreferences,
  uiShadowPreferences,
  uiThemePreferences,
} from "../../core/uiSettings/uiSettingsLogic";
import {
  canActivateRuntimeEnvironmentVersion,
  canInstallRuntimeEnvironment,
  formatRuntimeEnvironmentState,
  formatRuntimeEnvironmentStatus,
  getDefaultRuntimeEnvironmentVersion,
  getRuntimeEnvironmentActionLabel,
  runtimeEnvironmentInstallKey,
  sortRuntimeEnvironmentVersions,
} from "../../core/runtimeEnvironment/runtimeEnvironmentLogic";
import { formatPackLabel } from "../../core/packs/packLogic";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";
import { DeepLinkInstallPanel } from "./DeepLinkInstallPanel";
import { PromptInput } from "./PromptInput";
import { QuickPromptExamples } from "./QuickPromptExamples";
import { SettingsSurface } from "./SettingsSurface";
import { defaultSettingsSection, type SettingsSectionKey } from "./settingsSectionLogic";

const navigationItems = [
  { key: "create", icon: Plus, labelKey: "nav.create" },
  { key: "apps", icon: Boxes, labelKey: "nav.apps" },
  { key: "marketplace", icon: Store, labelKey: "nav.marketplace" },
  { key: "settings", icon: Settings, labelKey: "nav.settings" },
] as const;

type RuntimeCategory = "page" | "app" | "data" | "graphics" | "knowledge" | "desktop";

interface RuntimeOption {
  kind: RuntimeChoice;
  label: string;
  detail: string;
  category: RuntimeCategory;
}

const runtimeOptions: RuntimeOption[] = [
  { kind: "auto", label: "AI auto-select", detail: "Infer from intent", category: "page" },
  { kind: "static-html", label: "Static page", detail: "Lightweight page", category: "page" },
  { kind: "react-vite", label: "React app", detail: "Interactive UI", category: "app" },
  { kind: "react-sqlite", label: "React + SQLite", detail: "Local data", category: "app" },
  { kind: "ai-agent-app", label: "AI Agent App", detail: "Provider binding", category: "app" },
  { kind: "canvas2d", label: "Canvas 2D", detail: "Graphics tool", category: "graphics" },
  { kind: "markdown-knowledge", label: "Markdown", detail: "Knowledge base", category: "knowledge" },
  { kind: "data-table", label: "Data table", detail: "Table processing", category: "data" },
  { kind: "file-processor", label: "File processor", detail: "Safe preview", category: "data" },
  { kind: "desktop-widget", label: "Desktop widget", detail: "Small widget", category: "desktop" },
];

const runtimeCategoryOrder: RuntimeCategory[] = ["page", "app", "data", "graphics", "knowledge", "desktop"];

const runtimeCategoryDescriptions: Record<RuntimeCategory, string> = {
  page: "Lightweight pages and static tools",
  app: "Interactive apps and local data",
  data: "Tables, files, and data processing",
  graphics: "Canvas and visual tools",
  knowledge: "Documents and knowledge bases",
  desktop: "Widgets and desktop experiences",
};

const runtimeOptionIcons: Record<RuntimeChoice, string> = {
  auto: "AI",
  "static-html": "HTML",
  "react-vite": "R",
  "react-sqlite": "DB",
  "ai-agent-app": "AI",
  canvas2d: "2D",
  "markdown-knowledge": "MD",
  "data-table": "T",
  "file-processor": "FX",
  "desktop-widget": "W",
};

type DesktopTranslator = ReturnType<typeof useDesktopLocale>["t"];

function runtimeOptionLabel(kind: RuntimeChoice, t: DesktopTranslator): string {
  switch (kind) {
    case "auto":
      return t("composer.autoRuntime");
    case "static-html":
      return t("runtime.option.staticHtml");
    case "react-vite":
      return t("runtime.option.reactVite");
    case "react-sqlite":
      return t("runtime.option.reactSqlite");
    case "ai-agent-app":
      return t("runtime.option.aiAgentApp");
    case "canvas2d":
      return t("runtime.option.canvas2d");
    case "markdown-knowledge":
      return t("runtime.option.markdownKnowledge");
    case "data-table":
      return t("runtime.option.dataTable");
    case "file-processor":
      return t("runtime.option.fileProcessor");
    case "desktop-widget":
      return t("runtime.option.desktopWidget");
  }
}

function runtimeOptionDetail(kind: RuntimeChoice, t: DesktopTranslator): string {
  switch (kind) {
    case "auto":
      return t("composer.autoRuntimeDetail");
    case "static-html":
      return t("runtime.option.staticHtmlDetail");
    case "react-vite":
      return t("runtime.option.reactViteDetail");
    case "react-sqlite":
      return t("runtime.option.reactSqliteDetail");
    case "ai-agent-app":
      return t("runtime.option.aiAgentAppDetail");
    case "canvas2d":
      return t("runtime.option.canvas2dDetail");
    case "markdown-knowledge":
      return t("runtime.option.markdownKnowledgeDetail");
    case "data-table":
      return t("runtime.option.dataTableDetail");
    case "file-processor":
      return t("runtime.option.fileProcessorDetail");
    case "desktop-widget":
      return t("runtime.option.desktopWidgetDetail");
  }
}

function runtimeCategoryLabel(category: RuntimeCategory, t: DesktopTranslator): string {
  return t(`runtime.category.${category}`);
}

function runtimeCategoryDetail(category: RuntimeCategory, t: DesktopTranslator): string {
  return t(`runtime.category.${category}Detail`);
}

function fallbackDesktopT(
  key: string,
  params: Record<string, string | number | boolean | null | undefined> = {},
  fallback?: string,
): string {
  const messages: Record<string, string> = {
    "composer.autoRuntime": "AI auto-select",
    "composer.autoRuntimeDetail": "Sofvary chooses the software type from your intent",
    "runtime.option.staticHtml": "Static page",
    "runtime.option.reactVite": "React app",
    "runtime.option.reactSqlite": "React + SQLite",
    "runtime.option.aiAgentApp": "AI Agent App",
    "runtime.option.canvas2d": "Canvas 2D",
    "runtime.option.markdownKnowledge": "Markdown",
    "runtime.option.dataTable": "Data table",
    "runtime.option.fileProcessor": "File processor",
    "runtime.option.desktopWidget": "Desktop widget",
    "workspace.updatedUnknown": "Updated time unknown",
    "workspace.updated": "Updated {value}",
  };
  return (messages[key] ?? fallback ?? key).replace(/\{([a-zA-Z0-9_.-]+)\}/g, (match, name) =>
    params[name] === undefined || params[name] === null ? match : String(params[name]),
  );
}

export type NavigationKey = (typeof navigationItems)[number]["key"];

interface FloatingCommandMenuProps {
  createPrompt: string;
  continuePrompt: string;
  isActive: boolean;
  isPinned: boolean;
  statusLine: string;
  activeAction: NavigationKey;
  runtimeChoice: RuntimeChoice;
  intentSelection: RuntimeIntentSelection | null;
  runtimePreflightMessage: string | null;
  agentState: AgentConfigState;
  discoveredAgents: DiscoveredAgent[];
  agentInstallStatuses: AgentInstallStatus[];
  activeAgentInstallId: string | null;
  runtimeEnvironmentStatuses: RuntimeEnvironmentStatus[];
  activeRuntimeEnvironmentInstallKey: string | null;
  runtimeEnvironmentStatusLine: string | null;
  selectableAgents: AgentConfig[];
  selectedAgentId: string | null;
  agentStatusLine: string;
  buildThreads: BuildThreadSummary[];
  activeThread: BuildThreadSummary | null;
  activeThreadDetail: BuildThreadDetail | null;
  promptEnvelopeSummary: PromptEnvelopeSummary | null;
  workspaces: WorkspaceSummary[];
  installedPacks: InstalledPackSummary[];
  llmProviderState: LlmProviderConfigState;
  llmProviderStatusLine: string;
  appearancePreferences: UiAppearancePreferences;
  themePreference: UiThemePreference;
  resolvedTheme: ResolvedUiTheme;
  systemTheme: ResolvedUiTheme;
  deepLinkValue: string;
  deepLinkStatusLine: string;
  deepLinkPreflight: DeepLinkInstallPreflight | null;
  packStatusLine: string;
  capsuleStatusLine: string;
  releaseStatusLine: string;
  isCapsuleBusy: boolean;
  isReleaseBusy: boolean;
  isDeepLinkBusy: boolean;
  activeCapsuleAppId: string | null;
  activeReleaseAppId: string | null;
  activePreviewAppId: string | null;
  onCreatePromptChange: (value: string) => void;
  onContinuePromptChange: (value: string) => void;
  onStart: () => void;
  onTogglePin: () => void;
  onAction: (action: NavigationKey) => void;
  onRuntimeChoiceChange: (runtimeChoice: RuntimeChoice) => void;
  onAgentChange: (agentId: string) => void;
  onSelectBuildThread: (threadId: string) => void;
  onStartNewBuildThreadDraft: () => void;
  onContinueBuildThread: () => void;
  onCancelBuildThread: () => void;
  onDeleteBuildThread: (threadId: string) => void;
  onAddDiscoveredAgent: (agent: DiscoveredAgent) => void;
  onToggleAgentEnabled: (agent: AgentConfig) => void;
  onSetDefaultAgent: (agentId: string) => void;
  onDeleteAgent: (agentId: string) => void;
  onTestAgent: (agentId: string) => void;
  onRefreshAgents: () => void;
  onInstallAgent: (status: AgentInstallStatus) => void;
  onCancelAgentInstall: (agentId: string) => void;
  onOpenAgentInstallPage: (agentId: string) => void;
  onRefreshAgentInstallStatuses: () => void;
  onInstallRuntimeEnvironment: (
    status: RuntimeEnvironmentStatus,
    version: RuntimeEnvironmentVersionOption,
  ) => void;
  onActivateRuntimeEnvironmentVersion: (
    status: RuntimeEnvironmentStatus,
    version: RuntimeEnvironmentVersionOption,
  ) => void;
  onRefreshRuntimeEnvironments: () => void;
  onSaveLlmProvider: (config: LlmProviderConfig, apiKey?: string) => void;
  onToggleLlmProviderEnabled: (provider: LlmProviderConfig) => void;
  onSetDefaultLlmProvider: (providerId: string) => void;
  onDeleteLlmProvider: (providerId: string) => void;
  onTestLlmProvider: (providerId: string) => void;
  onRefreshLlmProviders: () => void;
  onAppearanceChange: (preferences: UiAppearancePreferences) => void;
  onThemePreferenceChange: (preference: UiThemePreference) => void;
  onDeepLinkChange: (value: string) => void;
  onReviewDeepLink: () => void;
  onInstallDeepLink: () => void;
  onClearDeepLink: () => void;
  onPreviewWorkspace: (workspace: WorkspaceSummary) => void;
  onModifyWorkspace: (workspace: WorkspaceSummary) => void;
  onExportWorkspace: (workspace: WorkspaceSummary) => void;
  onReleaseWorkspace: (workspace: WorkspaceSummary) => void;
  onDeleteWorkspace: (workspace: WorkspaceSummary) => void;
  onImportCapsule: () => void;
  onRefreshPacks: () => void;
}

export function FloatingCommandMenu({
  createPrompt,
  continuePrompt,
  isActive,
  isPinned,
  activeAction,
  runtimeChoice,
  intentSelection,
  runtimePreflightMessage,
  agentState,
  discoveredAgents,
  agentInstallStatuses,
  activeAgentInstallId,
  runtimeEnvironmentStatuses,
  activeRuntimeEnvironmentInstallKey,
  runtimeEnvironmentStatusLine,
  selectableAgents,
  selectedAgentId,
  agentStatusLine,
  buildThreads,
  activeThread,
  activeThreadDetail,
  promptEnvelopeSummary,
  workspaces,
  installedPacks,
  llmProviderState,
  llmProviderStatusLine,
  appearancePreferences,
  themePreference,
  resolvedTheme,
  systemTheme,
  deepLinkValue,
  deepLinkStatusLine,
  deepLinkPreflight,
  packStatusLine,
  capsuleStatusLine,
  releaseStatusLine,
  isCapsuleBusy,
  isReleaseBusy,
  isDeepLinkBusy,
  activeCapsuleAppId,
  activeReleaseAppId,
  activePreviewAppId,
  onCreatePromptChange,
  onContinuePromptChange,
  onStart,
  onTogglePin,
  onAction,
  onRuntimeChoiceChange,
  onAgentChange,
  onSelectBuildThread,
  onStartNewBuildThreadDraft,
  onContinueBuildThread,
  onCancelBuildThread,
  onDeleteBuildThread,
  onAddDiscoveredAgent,
  onToggleAgentEnabled,
  onSetDefaultAgent,
  onDeleteAgent,
  onTestAgent,
  onRefreshAgents,
  onInstallAgent,
  onCancelAgentInstall,
  onOpenAgentInstallPage,
  onRefreshAgentInstallStatuses,
  onInstallRuntimeEnvironment,
  onActivateRuntimeEnvironmentVersion,
  onRefreshRuntimeEnvironments,
  onSaveLlmProvider,
  onToggleLlmProviderEnabled,
  onSetDefaultLlmProvider,
  onDeleteLlmProvider,
  onTestLlmProvider,
  onRefreshLlmProviders,
  onAppearanceChange,
  onThemePreferenceChange,
  onDeepLinkChange,
  onReviewDeepLink,
  onInstallDeepLink,
  onClearDeepLink,
  onPreviewWorkspace,
  onModifyWorkspace,
  onExportWorkspace,
  onReleaseWorkspace,
  onDeleteWorkspace,
  onImportCapsule,
  onRefreshPacks,
}: FloatingCommandMenuProps) {
  const { locale, setLocale, t } = useDesktopLocale();
  const isBusy =
    isActive || isCapsuleBusy || isReleaseBusy || isDeepLinkBusy || activePreviewAppId !== null;
  const activeNavigation = getActiveNavigation(activeAction);
  const currentRuntime = runtimeOptions.find((option) => option.kind === runtimeChoice);
  const securityState = isBusy ? t("menu.securityChecking") : t("status.ready");
  const visibleRuntimes = runtimeOptions;
  const selectedAgent = selectableAgents.find((agent) => agent.id === selectedAgentId) ?? null;
  const canStart = !isBusy && selectableAgents.length > 0 && createPrompt.trim().length > 0;
  const [isTaskRailOpen, setTaskRailOpen] = useState(false);
  const [activeSettingsSection, setActiveSettingsSection] =
    useState<SettingsSectionKey>(defaultSettingsSection);

  return (
    <aside
      className="floating-menu"
      aria-label={t("menu.aria", {}, "Sofvary floating command menu")}
      data-tauri-drag-region
    >
      <nav className="shell-nav" aria-label={t("nav.menu")} data-no-drag>
        {navigationItems.map((item) => {
          const NavigationIcon = item.icon;
          const label = t(item.labelKey);
          return (
            <button
              key={item.key}
              type="button"
              className={activeNavigation === item.key ? "is-active" : ""}
              title={label}
              data-tooltip={label}
              aria-label={label}
              aria-current={activeNavigation === item.key ? "page" : undefined}
              onClick={() => onAction(item.key)}
            >
              <span className="shell-nav__icon" aria-hidden="true">
                <NavigationIcon aria-hidden="true" />
              </span>
              <span className="shell-nav__label">{label}</span>
            </button>
          );
        })}
      </nav>

      <section className="command-feature" aria-label={t("nav.featureAria", { label: t(navigationItems.find((item) => item.key === activeNavigation)?.labelKey ?? "nav.create") })}>
        {activeNavigation === "create" ? (
          <CreateTaskSurface
            createPrompt={createPrompt}
            continuePrompt={continuePrompt}
            isBusy={isBusy}
            isPinned={isPinned}
            isTaskRailOpen={isTaskRailOpen}
            canStart={canStart}
            runtimeChoice={runtimeChoice}
            intentSelection={intentSelection}
            runtimePreflightMessage={runtimePreflightMessage}
            currentRuntimeLabel={currentRuntime?.label ?? runtimeChoice}
            securityState={securityState}
            selectedAgentLabel={selectedAgent?.label ?? t("menu.unconfigured")}
            selectableAgents={selectableAgents}
            selectedAgentId={selectedAgentId}
            agentStatusLine={agentStatusLine}
            visibleRuntimes={visibleRuntimes}
            threads={buildThreads}
            activeThread={activeThread}
            detail={activeThreadDetail}
            promptEnvelopeSummary={promptEnvelopeSummary}
            workspaceCount={workspaces.length}
            onCreatePromptChange={onCreatePromptChange}
            onContinuePromptChange={onContinuePromptChange}
            onPickPrompt={onCreatePromptChange}
            onStart={onStart}
            onToggleTaskRail={() => setTaskRailOpen((current) => !current)}
            onCloseTaskRail={() => setTaskRailOpen(false)}
            onRuntimeChoiceChange={onRuntimeChoiceChange}
            onAgentChange={onAgentChange}
            onSelect={onSelectBuildThread}
            onStartNew={onStartNewBuildThreadDraft}
            onContinue={onContinueBuildThread}
            onCancel={onCancelBuildThread}
            onDelete={onDeleteBuildThread}
          />
        ) : null}

        {activeNavigation === "apps" ? (
          <WorkspaceListPanel
            workspaces={workspaces}
            capsuleStatusLine={capsuleStatusLine}
            releaseStatusLine={releaseStatusLine}
            isBusy={isBusy}
            activeCapsuleAppId={activeCapsuleAppId}
            activeReleaseAppId={activeReleaseAppId}
            activePreviewAppId={activePreviewAppId}
            buildThreads={buildThreads}
            onImportCapsule={onImportCapsule}
            onPreviewWorkspace={onPreviewWorkspace}
            onModifyWorkspace={onModifyWorkspace}
            onExportWorkspace={onExportWorkspace}
            onReleaseWorkspace={onReleaseWorkspace}
            onDeleteWorkspace={onDeleteWorkspace}
          />
        ) : null}

        {activeNavigation === "marketplace" ? (
          <div className="panel-stack">
            <DeepLinkInstallPanel
              value={deepLinkValue}
              statusLine={deepLinkStatusLine}
              preflight={deepLinkPreflight}
              disabled={isBusy}
              canInstall={deepLinkPreflight !== null}
              onChange={onDeepLinkChange}
              onReview={onReviewDeepLink}
              onInstall={onInstallDeepLink}
              onClear={onClearDeepLink}
            />
            <PackListPanel
              installedPacks={installedPacks}
              packStatusLine={packStatusLine}
              isBusy={isBusy}
              onRefreshPacks={onRefreshPacks}
            />
          </div>
        ) : null}

        {activeNavigation === "settings" ? (
          <SettingsSurface activeSection={activeSettingsSection} onSectionChange={setActiveSettingsSection}>
            {{
              general: (
                <GeneralSettingsPanel
                  locale={locale}
                  isBusy={isBusy}
                  isPinned={isPinned}
                  onLocaleChange={setLocale}
                  onTogglePin={onTogglePin}
                />
              ),
              appearance: (
                <AppearanceSettingsPanel
                  preferences={appearancePreferences}
                  themePreference={themePreference}
                  resolvedTheme={resolvedTheme}
                  systemTheme={systemTheme}
                  isBusy={isBusy}
                  onAppearanceChange={onAppearanceChange}
                  onThemePreferenceChange={onThemePreferenceChange}
                />
              ),
              workspace: (
                <WorkspaceSettingsPanel
                  capsuleStatusLine={capsuleStatusLine}
                  isBusy={isBusy}
                  onImportCapsule={onImportCapsule}
                />
              ),
              runtime: (
                <div className="panel-stack">
                  <RuntimeEnvironmentSettingsPanel
                    statuses={runtimeEnvironmentStatuses}
                    statusLine={runtimeEnvironmentStatusLine}
                    activeInstallKey={activeRuntimeEnvironmentInstallKey}
                    isBusy={isBusy}
                    onInstall={onInstallRuntimeEnvironment}
                    onActivate={onActivateRuntimeEnvironmentVersion}
                    onRefresh={onRefreshRuntimeEnvironments}
                  />
                  <PackListPanel
                    installedPacks={installedPacks}
                    packStatusLine={packStatusLine}
                    isBusy={isBusy}
                    onRefreshPacks={onRefreshPacks}
                  />
                </div>
              ),
              ai: (
                <div className="panel-stack">
                  <AgentSettingsPanel
                    agentState={agentState}
                    discoveredAgents={discoveredAgents}
                    agentInstallStatuses={agentInstallStatuses}
                    activeAgentInstallId={activeAgentInstallId}
                    agentStatusLine={agentStatusLine}
                    isBusy={isBusy}
                    onAddDiscoveredAgent={onAddDiscoveredAgent}
                    onToggleAgentEnabled={onToggleAgentEnabled}
                    onSetDefaultAgent={onSetDefaultAgent}
                    onDeleteAgent={onDeleteAgent}
                    onTestAgent={onTestAgent}
                    onRefreshAgents={onRefreshAgents}
                    onInstallAgent={onInstallAgent}
                    onCancelAgentInstall={onCancelAgentInstall}
                    onOpenAgentInstallPage={onOpenAgentInstallPage}
                    onRefreshAgentInstallStatuses={onRefreshAgentInstallStatuses}
                  />
                  <LlmProviderSettingsPanel
                    providerState={llmProviderState}
                    statusLine={llmProviderStatusLine}
                    isBusy={isBusy}
                    onSaveProvider={onSaveLlmProvider}
                    onToggleProviderEnabled={onToggleLlmProviderEnabled}
                    onSetDefaultProvider={onSetDefaultLlmProvider}
                    onDeleteProvider={onDeleteLlmProvider}
                    onTestProvider={onTestLlmProvider}
                    onRefreshProviders={onRefreshLlmProviders}
                  />
                </div>
              ),
            }}
          </SettingsSurface>
        ) : null}
      </section>
    </aside>
  );
}

interface GeneralSettingsPanelProps {
  locale: Locale;
  isBusy: boolean;
  isPinned: boolean;
  onLocaleChange: (locale: Locale) => void;
  onTogglePin: () => void;
}

function GeneralSettingsPanel({
  locale,
  isBusy,
  isPinned,
  onLocaleChange,
  onTogglePin,
}: GeneralSettingsPanelProps) {
  const { t } = useDesktopLocale();
  return (
    <div className="panel-stack">
      <section className="settings-panel ui-settings" aria-label={t("locale.panelTitle")}>
        <div className="settings-row ui-settings__header">
          <div>
            <strong>{t("locale.panelTitle")}</strong>
            <small>{t("locale.panelDetail")}</small>
          </div>
        </div>
      <div className="theme-choice" role="radiogroup" aria-label={t("locale.panelTitle")}>
        <button
          type="button"
          className={locale === "en" ? "is-active" : ""}
          aria-checked={locale === "en"}
          role="radio"
          disabled={isBusy}
          onClick={() => onLocaleChange("en")}
          data-no-drag
        >
          <strong>{t("locale.english")}</strong>
          <small>{t("locale.panelDetail")}</small>
        </button>
        <button
          type="button"
          className={locale === "zh-CN" ? "is-active" : ""}
          aria-checked={locale === "zh-CN"}
          role="radio"
          disabled={isBusy}
          onClick={() => onLocaleChange("zh-CN")}
          data-no-drag
        >
          <strong>{t("locale.chinese")}</strong>
          <small>{t("locale.panelDetail")}</small>
        </button>
      </div>
      </section>
      <section className="settings-panel" aria-label={t("settings.title")}>
        <div className="settings-row">
          <div>
            <strong>{t("settings.pin")}</strong>
            <small>{isPinned ? t("settings.pinned") : t("settings.unpinned")}</small>
          </div>
          <button type="button" aria-pressed={isPinned} onClick={onTogglePin} data-no-drag>
            {isPinned ? t("action.disable") : t("action.enable")}
          </button>
        </div>
      </section>
    </div>
  );
}

interface AppearanceSettingsPanelProps {
  preferences: UiAppearancePreferences;
  themePreference: UiThemePreference;
  resolvedTheme: ResolvedUiTheme;
  systemTheme: ResolvedUiTheme;
  isBusy: boolean;
  onAppearanceChange: (preferences: UiAppearancePreferences) => void;
  onThemePreferenceChange: (preference: UiThemePreference) => void;
}

function AppearanceSettingsPanel({
  preferences,
  themePreference,
  resolvedTheme,
  systemTheme,
  isBusy,
  onAppearanceChange,
  onThemePreferenceChange,
}: AppearanceSettingsPanelProps) {
  const { t } = useDesktopLocale();
  const updateAppearance = (patch: Partial<UiAppearancePreferences>) => {
    onAppearanceChange({ ...preferences, ...patch });
  };

  return (
    <div className="panel-stack">
      <section className="settings-panel ui-settings" aria-label={t("settings.ui")}>
        <div className="settings-row ui-settings__header">
          <div>
            <strong>{t("settings.theme")}</strong>
            <small>{formatUiThemeStatus(themePreference, resolvedTheme, t)}</small>
          </div>
          <span>{t("settings.systemTheme", { theme: systemTheme === "light" ? t("theme.light") : t("theme.dark") })}</span>
        </div>
        <SettingsChoiceGrid
          ariaLabel={t("settings.theme")}
          options={uiThemePreferences.map((item) => ({
            preference: item.preference,
            label: formatUiThemePreference(item.preference, t),
            detail: formatUiThemePreferenceDetail(item.preference, t),
          }))}
          value={themePreference}
          disabled={isBusy}
          onChange={(preference) => {
            onThemePreferenceChange(preference);
            updateAppearance({ themePreference: preference });
          }}
        />
      </section>

      <section className="settings-panel appearance-settings" aria-label={t("appearance.title")}>
        <SettingsChoiceGrid
          title={t("appearance.accent")}
          ariaLabel={t("appearance.accent")}
          options={uiAccentPreferences.map((item) => ({
            preference: item.preference,
            label: formatUiAccentPreference(item.preference, t),
            detail: formatUiAccentPreferenceDetail(item.preference, t),
            swatch: item.preference,
          }))}
          value={preferences.accent}
          disabled={isBusy}
          onChange={(accent) => updateAppearance({ accent })}
        />
        <SettingsChoiceGrid
          title={t("appearance.glass")}
          ariaLabel={t("appearance.glass")}
          options={uiGlassPreferences.map((item) => ({
            preference: item.preference,
            label: formatUiGlassPreference(item.preference, t),
            detail: formatUiGlassPreferenceDetail(item.preference, t),
          }))}
          value={preferences.glass}
          disabled={isBusy}
          onChange={(glass) => updateAppearance({ glass })}
        />
        <SettingsChoiceGrid
          title={t("appearance.shadow")}
          ariaLabel={t("appearance.shadow")}
          options={uiShadowPreferences.map((item) => ({
            preference: item.preference,
            label: formatUiShadowPreference(item.preference, t),
            detail: formatUiShadowPreferenceDetail(item.preference, t),
          }))}
          value={preferences.shadow}
          disabled={isBusy}
          onChange={(shadow) => updateAppearance({ shadow })}
        />
        <SettingsChoiceGrid
          title={t("appearance.density")}
          ariaLabel={t("appearance.density")}
          options={uiDensityPreferences.map((item) => ({
            preference: item.preference,
            label: formatUiDensityPreference(item.preference, t),
            detail: formatUiDensityPreferenceDetail(item.preference, t),
          }))}
          value={preferences.density}
          disabled={isBusy}
          onChange={(density) => updateAppearance({ density })}
        />
        <SettingsChoiceGrid
          title={t("appearance.radius")}
          ariaLabel={t("appearance.radius")}
          options={uiRadiusPreferences.map((item) => ({
            preference: item.preference,
            label: formatUiRadiusPreference(item.preference, t),
            detail: formatUiRadiusPreferenceDetail(item.preference, t),
          }))}
          value={preferences.radius}
          disabled={isBusy}
          onChange={(radius) => updateAppearance({ radius })}
        />
        <SettingsChoiceGrid
          title={t("appearance.motion")}
          ariaLabel={t("appearance.motion")}
          options={uiMotionPreferences.map((item) => ({
            preference: item.preference,
            label: formatUiMotionPreference(item.preference, t),
            detail: formatUiMotionPreferenceDetail(item.preference, t),
          }))}
          value={preferences.motion}
          disabled={isBusy}
          onChange={(motion) => updateAppearance({ motion })}
        />
      </section>
    </div>
  );
}

interface WorkspaceSettingsPanelProps {
  capsuleStatusLine: string;
  isBusy: boolean;
  onImportCapsule: () => void;
}

function WorkspaceSettingsPanel({
  capsuleStatusLine,
  isBusy,
  onImportCapsule,
}: WorkspaceSettingsPanelProps) {
  const { t } = useDesktopLocale();
  return (
    <section className="settings-panel" aria-label={t("settings.workspace")}>
      <div className="settings-row">
        <div>
          <strong>{t("settings.workspace")}</strong>
          <small>{capsuleStatusLine}</small>
        </div>
        <button type="button" disabled={isBusy} onClick={onImportCapsule} data-no-drag>
          {t("action.import")}
        </button>
      </div>
    </section>
  );
}

interface SettingsChoiceGridProps<T extends string> {
  title?: string;
  ariaLabel: string;
  options: Array<{
    preference: T;
    label: string;
    detail: string;
    swatch?: string;
  }>;
  value: T;
  disabled: boolean;
  onChange: (value: T) => void;
}

function SettingsChoiceGrid<T extends string>({
  title,
  ariaLabel,
  options,
  value,
  disabled,
  onChange,
}: SettingsChoiceGridProps<T>) {
  return (
    <div className="settings-choice-group">
      {title ? <strong className="settings-choice-group__title">{title}</strong> : null}
      <div className="theme-choice settings-choice-grid" role="radiogroup" aria-label={ariaLabel}>
        {options.map((item) => (
          <button
            key={item.preference}
            type="button"
            className={item.preference === value ? "is-active" : ""}
            aria-checked={item.preference === value}
            role="radio"
            disabled={disabled}
            onClick={() => onChange(item.preference)}
            data-swatch={item.swatch}
            data-no-drag
          >
            {item.swatch ? <span className="appearance-swatch" aria-hidden="true" /> : null}
            <strong>{item.label}</strong>
            <small>{item.detail}</small>
          </button>
        ))}
      </div>
    </div>
  );
}

interface RuntimeEnvironmentSettingsPanelProps {
  statuses: RuntimeEnvironmentStatus[];
  statusLine: string | null;
  activeInstallKey: string | null;
  isBusy: boolean;
  onInstall: (
    status: RuntimeEnvironmentStatus,
    version: RuntimeEnvironmentVersionOption,
  ) => void;
  onActivate: (
    status: RuntimeEnvironmentStatus,
    version: RuntimeEnvironmentVersionOption,
  ) => void;
  onRefresh: () => void;
}

function RuntimeEnvironmentSettingsPanel({
  statuses,
  statusLine,
  activeInstallKey,
  isBusy,
  onInstall,
  onActivate,
  onRefresh,
}: RuntimeEnvironmentSettingsPanelProps) {
  const { t } = useDesktopLocale();
  const nodeStatus = statuses.find((status) => status.catalog.kind === "nodejs") ?? null;
  const versions = useMemo(
    () => (nodeStatus ? sortRuntimeEnvironmentVersions(nodeStatus.catalog.versions) : []),
    [nodeStatus],
  );
  const [selectedVersion, setSelectedVersion] = useState<string>("");

  useEffect(() => {
    if (!nodeStatus) return;
    const defaultVersion = getDefaultRuntimeEnvironmentVersion(nodeStatus);
    setSelectedVersion((current) => {
      if (current && versions.some((version) => version.version === current)) return current;
      return defaultVersion?.version ?? "";
    });
  }, [nodeStatus, versions]);

  const selected =
    versions.find((version) => version.version === selectedVersion) ??
    (nodeStatus ? getDefaultRuntimeEnvironmentVersion(nodeStatus) : null);
  const selectedInstallKey =
    nodeStatus && selected ? runtimeEnvironmentInstallKey(nodeStatus, selected) : null;
  const isInstalling = selectedInstallKey !== null && activeInstallKey === selectedInstallKey;

  return (
    <section className="agent-settings runtime-environment-settings" aria-label={t("runtimeEnvironment.title")}>
      <header className="agent-settings__header">
        <div>
          <strong>{t("runtimeEnvironment.title")}</strong>
          <small>
            {statusLine ??
              (nodeStatus ? formatRuntimeEnvironmentStatus(nodeStatus) : t("runtimeEnvironment.loading"))}
          </small>
        </div>
        <button type="button" disabled={isBusy || isInstalling} onClick={onRefresh} data-no-drag>
          {t("action.refresh")}
        </button>
      </header>

      {nodeStatus ? (
        <div className="runtime-environment-card">
          <div className="runtime-environment-card__summary">
            <div className="runtime-environment-card__icon" aria-hidden="true">
              JS
            </div>
            <div>
              <strong>{nodeStatus.catalog.label}</strong>
              <small>{nodeStatus.catalog.description}</small>
            </div>
            <span>{formatRuntimeEnvironmentState(nodeStatus.installState)}</span>
          </div>

          <div className="runtime-environment-tools">
            {[nodeStatus.node, nodeStatus.pnpm].filter(Boolean).map((tool) => (
              <article key={tool?.name}>
                <span>{tool?.name}</span>
                <strong>{tool?.version ?? "missing"}</strong>
                <small>{tool?.detail}</small>
              </article>
            ))}
          </div>

          <div className="runtime-environment-actions">
            <label>
              <span>{t("runtimeEnvironment.version")}</span>
              <select
                value={selected?.version ?? ""}
                disabled={isBusy || isInstalling || versions.length === 0}
                onChange={(event) => setSelectedVersion(event.currentTarget.value)}
                data-no-drag
              >
                {versions.map((version) => (
                  <option key={version.version} value={version.version}>
                    {version.label} · {version.channel}
                    {version.recommended ? ` · ${t("runtimeEnvironment.recommended")}` : ""}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              disabled={
                !nodeStatus ||
                !selected ||
                isBusy ||
                (!canInstallRuntimeEnvironment(nodeStatus, selected, activeInstallKey) &&
                  !canActivateRuntimeEnvironmentVersion(nodeStatus, selected, activeInstallKey))
              }
              onClick={() => {
                if (!nodeStatus || !selected) return;
                if (canActivateRuntimeEnvironmentVersion(nodeStatus, selected, activeInstallKey)) {
                  onActivate(nodeStatus, selected);
                } else {
                  onInstall(nodeStatus, selected);
                }
              }}
              data-no-drag
            >
              {nodeStatus && selected
                ? getRuntimeEnvironmentActionLabel(nodeStatus, selected, activeInstallKey)
                : t("runtimeEnvironment.unavailable")}
            </button>
          </div>
        </div>
      ) : (
        <p className="agent-settings__empty">{t("runtimeEnvironment.empty")}</p>
      )}
    </section>
  );
}

interface PromptEnvelopeSummaryPanelProps {
  summary: PromptEnvelopeSummary;
}

function PromptEnvelopeSummaryPanel({ summary }: PromptEnvelopeSummaryPanelProps) {
  return (
    <section className="prompt-envelope-summary" aria-label="Prompt envelope summary">
      <div>
        <span>Runtime</span>
        <strong>{summary.runtime}</strong>
      </div>
      <div>
        <span>Harness</span>
        <strong>{summary.harnesses.join(", ")}</strong>
      </div>
      <div>
        <span>Output</span>
        <strong>{summary.outputContract.join(", ")}</strong>
      </div>
      <div>
        <span>Blocked</span>
        <strong>{summary.blockedCapabilities.slice(0, 4).join(", ")}</strong>
      </div>
    </section>
  );
}

interface IntentSelectionPanelProps {
  selection: RuntimeIntentSelection | null;
}

function IntentSelectionPanel({ selection }: IntentSelectionPanelProps) {
  const { t } = useDesktopLocale();
  if (!selection) {
    return (
      <section className="intent-selection-summary" aria-label={t("composer.autoRuntime")}>
        <div>
          <span>{t("composer.autoRuntime")}</span>
          <strong>{t("composer.autoRuntimeDetail")}</strong>
        </div>
        <small>{t("composer.manualRuntimeHint")}</small>
      </section>
    );
  }

  return (
    <section className="intent-selection-summary" aria-label={t("composer.autoRuntime")}>
      <div>
        <span>{t("composer.autoRuntime")}</span>
        <strong>{formatSoftwareType(selection.softwareType)}</strong>
      </div>
      <div>
        <span>{t("composer.solution")}</span>
        <strong>{formatRuntimeChoice(selection.runtimeKind, t)}</strong>
      </div>
      <div>
        <span>{t("composer.confidence")}</span>
        <strong>{formatConfidence(selection.confidence)}</strong>
      </div>
      <small>{selection.reason}</small>
    </section>
  );
}

interface RuntimePreflightPanelProps {
  message: string;
}

function RuntimePreflightPanel({ message }: RuntimePreflightPanelProps) {
  const { t } = useDesktopLocale();
  return (
    <section className="runtime-preflight-warning" role="alert">
      <strong>{t("composer.environmentNotReady")}</strong>
      <small>{message}</small>
    </section>
  );
}

interface CreateTaskSurfaceProps {
  createPrompt: string;
  continuePrompt: string;
  isBusy: boolean;
  isPinned: boolean;
  isTaskRailOpen: boolean;
  canStart: boolean;
  runtimeChoice: RuntimeChoice;
  intentSelection: RuntimeIntentSelection | null;
  runtimePreflightMessage: string | null;
  currentRuntimeLabel: string;
  securityState: string;
  selectedAgentLabel: string;
  selectableAgents: AgentConfig[];
  selectedAgentId: string | null;
  agentStatusLine: string;
  visibleRuntimes: RuntimeOption[];
  threads: BuildThreadSummary[];
  activeThread: BuildThreadSummary | null;
  detail: BuildThreadDetail | null;
  promptEnvelopeSummary: PromptEnvelopeSummary | null;
  workspaceCount: number;
  onCreatePromptChange: (value: string) => void;
  onContinuePromptChange: (value: string) => void;
  onPickPrompt: (value: string) => void;
  onStart: () => void;
  onToggleTaskRail: () => void;
  onCloseTaskRail: () => void;
  onRuntimeChoiceChange: (runtimeChoice: RuntimeChoice) => void;
  onAgentChange: (agentId: string) => void;
  onSelect: (threadId: string) => void;
  onStartNew: () => void;
  onContinue: () => void;
  onCancel: () => void;
  onDelete: (threadId: string) => void;
}

function CreateTaskSurface({
  createPrompt,
  continuePrompt,
  isBusy,
  isPinned,
  isTaskRailOpen,
  canStart,
  runtimeChoice,
  intentSelection,
  runtimePreflightMessage,
  currentRuntimeLabel,
  securityState,
  selectedAgentLabel,
  selectableAgents,
  selectedAgentId,
  agentStatusLine,
  visibleRuntimes,
  threads,
  activeThread,
  detail,
  promptEnvelopeSummary,
  workspaceCount,
  onCreatePromptChange,
  onContinuePromptChange,
  onPickPrompt,
  onStart,
  onToggleTaskRail,
  onCloseTaskRail,
  onRuntimeChoiceChange,
  onAgentChange,
  onSelect,
  onStartNew,
  onContinue,
  onCancel,
  onDelete,
}: CreateTaskSurfaceProps) {
  const { t } = useDesktopLocale();
  const isContinuingTask = activeThread !== null;
  const composerValue = isContinuingTask ? continuePrompt : createPrompt;
  const onComposerChange = isContinuingTask ? onContinuePromptChange : onCreatePromptChange;
  const [openComposerMenu, setOpenComposerMenu] = useState<"agent" | "runtime" | null>(null);
  const composerPickerRootRef = useRef<HTMLDivElement | null>(null);
  const selectedRuntime =
    visibleRuntimes.find((option) => option.kind === runtimeChoice) ?? visibleRuntimes[0];
  const activeRuntimeOption = activeThread
    ? visibleRuntimes.find((option) => option.kind === activeThread.runtimeKind)
    : selectedRuntime;
  const activeRuntimeLabel =
    activeThread
      ? activeRuntimeOption ? runtimeOptionLabel(activeRuntimeOption.kind, t) : activeThread.runtimeKind
      : runtimeChoice === "auto" && intentSelection
        ? `AI: ${formatSoftwareType(intentSelection.softwareType)}`
        : activeRuntimeOption ? runtimeOptionLabel(activeRuntimeOption.kind, t) : currentRuntimeLabel;
  const activeAgentLabel = activeThread
    ? (selectableAgents.find((agent) => agent.id === activeThread.agentId)?.label ??
      activeThread.agentId)
    : selectedAgentLabel;
  const composerStatusItems = useMemo(
    () => [
      { icon: "▦", label: t("stats.workspaces"), value: String(workspaceCount) },
      { icon: "⚡", label: t("stats.currentRuntime"), value: activeRuntimeLabel },
      { icon: "◌", label: t("stats.security"), value: securityState },
      { icon: "AI", label: t("stats.currentAgent"), value: activeAgentLabel },
    ],
    [activeAgentLabel, activeRuntimeLabel, securityState, t, workspaceCount],
  );

  useEffect(() => {
    if (openComposerMenu === null) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (composerPickerRootRef.current?.contains(target)) return;
      setOpenComposerMenu(null);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
    };
  }, [openComposerMenu]);

  useEffect(() => {
    if (activeThread) {
      setOpenComposerMenu(null);
    }
  }, [activeThread?.id]);

  const startNew = () => {
    onStartNew();
    if (!isPinned) {
      onCloseTaskRail();
    }
  };
  const selectThread = (threadId: string) => {
    onSelect(threadId);
    if (!isPinned) {
      onCloseTaskRail();
    }
  };

  return (
    <div className={`create-task-surface ${isTaskRailOpen ? "is-rail-open" : ""}`}>
      <div className="create-task-toolbar">
        <button
          className="task-rail-toggle"
          type="button"
          aria-label={isTaskRailOpen ? t("task.hideRail") : t("task.showRail")}
          title={isTaskRailOpen ? t("task.hideRail") : t("task.showRail")}
          aria-pressed={isTaskRailOpen}
          onClick={onToggleTaskRail}
          data-no-drag
        >
          {isTaskRailOpen ? (
            <PanelLeftClose aria-hidden="true" />
          ) : (
            <PanelLeftOpen aria-hidden="true" />
          )}
        </button>
        <div>
          <span>{t("task.create")}</span>
          <strong>{activeThread ? activeThread.title : t("task.describeNew")}</strong>
        </div>
        <button
          className={`task-new-button ${activeThread ? "" : "is-active"}`}
          type="button"
          disabled={isBusy}
          aria-pressed={!activeThread}
          onClick={startNew}
          data-no-drag
        >
          {t("task.new")}
        </button>
      </div>

      <div className="create-task-layout">
        <TaskRail
          threads={threads}
          activeThread={activeThread}
          isBusy={isBusy}
          isOpen={isTaskRailOpen}
          onStartNew={startNew}
          onSelect={selectThread}
          onDelete={onDelete}
        />

        <main className="create-task-main">
          {activeThread ? (
            <TaskConversationPanel
              activeThread={activeThread}
              detail={detail}
              isBusy={isBusy}
              onCancel={onCancel}
            />
          ) : null}

          <section
            className={`create-panel ${
              activeThread ? "create-panel--compact create-panel--continue" : ""
            }`}
            aria-label={t("nav.create")}
          >
            <PromptInput
              value={composerValue}
              disabled={isBusy}
              rows={4}
              placeholder={activeThread ? t("prompt.placeholderContinue") : t("prompt.placeholderCreate")}
              onChange={onComposerChange}
            />
            <div className="create-composer-toolbar">
              <div className="create-composer-tools" ref={composerPickerRootRef}>
                {activeThread ? (
                  <ComposerLockedContext
                    agentLabel={activeAgentLabel}
                    runtimeLabel={activeRuntimeLabel}
                  />
                ) : (
                  <>
                    <AgentSelectorPanel
                      agents={selectableAgents}
                      selectedAgentId={selectedAgentId}
                      agentStatusLine={agentStatusLine}
                      disabled={isBusy}
                      isOpen={openComposerMenu === "agent"}
                      onToggle={() =>
                        setOpenComposerMenu((current) => (current === "agent" ? null : "agent"))
                      }
                      onChange={(agentId) => {
                        onAgentChange(agentId);
                        setOpenComposerMenu(null);
                      }}
                    />

                    <RuntimeSelectorPanel
                      runtimes={visibleRuntimes}
                      selectedRuntime={selectedRuntime}
                      runtimeChoice={runtimeChoice}
                      disabled={isBusy}
                      isOpen={openComposerMenu === "runtime"}
                      onToggle={() =>
                        setOpenComposerMenu((current) =>
                          current === "runtime" ? null : "runtime",
                        )
                      }
                      onChange={(nextRuntimeChoice) => {
                        onRuntimeChoiceChange(nextRuntimeChoice);
                        setOpenComposerMenu(null);
                      }}
                    />
                  </>
                )}
              </div>

              <div className="create-composer-right">
                <section className="create-composer-status" aria-label={t("composer.status")}>
                  <ComposerStatusSummary statuses={composerStatusItems} />
                </section>
                {activeThread ? (
                  <button
                    className="composer-submit-button"
                    type="button"
                    disabled={
                      isBusy || !continuePrompt.trim() || !canContinueBuildThread(activeThread)
                    }
                    aria-label={t("task.continue")}
                    title={t("task.continue")}
                    onClick={onContinue}
                    data-no-drag
                  >
                    ↵
                  </button>
                ) : (
                  <button
                    className="composer-submit-button"
                    type="button"
                    disabled={!canStart}
                    aria-label={t("task.start")}
                    title={t("task.start")}
                    onClick={onStart}
                    data-no-drag
                  >
                    ↑
                  </button>
                )}
              </div>
            </div>
          </section>

          {!activeThread ? (
            <div className="create-task-examples">
              <QuickPromptExamples disabled={isBusy} onPick={onPickPrompt} />
            </div>
          ) : null}

          {promptEnvelopeSummary ? (
            <PromptEnvelopeSummaryPanel summary={promptEnvelopeSummary} />
          ) : null}

          {!activeThread && runtimeChoice === "auto" ? (
            <IntentSelectionPanel selection={intentSelection} />
          ) : null}

          {runtimePreflightMessage ? (
            <RuntimePreflightPanel message={runtimePreflightMessage} />
          ) : null}

          {!activeThread ? (
            <TaskConversationPanel
              activeThread={activeThread}
              detail={detail}
              isBusy={isBusy}
              onCancel={onCancel}
            />
          ) : null}
        </main>
      </div>
    </div>
  );
}

interface TaskRailProps {
  threads: BuildThreadSummary[];
  activeThread: BuildThreadSummary | null;
  isBusy: boolean;
  isOpen: boolean;
  onStartNew: () => void;
  onSelect: (threadId: string) => void;
  onDelete: (threadId: string) => void;
}

function TaskRail({
  threads,
  activeThread,
  isBusy,
  isOpen,
  onStartNew,
  onSelect,
  onDelete,
}: TaskRailProps) {
  const { t } = useDesktopLocale();
  return (
    <aside className="task-rail" aria-label={t("task.session")} aria-hidden={!isOpen}>
      {isOpen ? (
        <div className="thread-list">
          <button
            type="button"
            className={`task-rail__new ${activeThread ? "" : "is-active"}`}
            disabled={isBusy}
            aria-pressed={!activeThread}
            onClick={onStartNew}
            data-no-drag
          >
            <span className="task-rail__new-icon" aria-hidden="true">
              <Plus aria-hidden="true" />
            </span>
            <span className="task-rail__new-copy">
              <strong>{t("task.new")}</strong>
              <small>{t("task.newDetail")}</small>
            </span>
          </button>
          {threads.length === 0 ? (
            <p className="workspace-list__empty">{t("task.empty")}</p>
          ) : null}
          {threads.map((thread) => (
            <div key={thread.id} className="thread-list__item">
              <button
                type="button"
                className={`thread-list__select ${
                  thread.id === activeThread?.id ? "is-active" : ""
                }`}
                onClick={() => onSelect(thread.id)}
              >
                <span className="thread-list__select-icon" aria-hidden="true">
                  {thread.id === activeThread?.id ? (
                    <ListTodo aria-hidden="true" />
                  ) : (
                    <FileStack aria-hidden="true" />
                  )}
                </span>
                <span className="thread-list__select-copy">
                  <strong>{thread.title}</strong>
                  <small>
                    {formatBuildThreadStatus(thread, t)} · {thread.runtimeKind}
                  </small>
                </span>
              </button>
              <button
                type="button"
                className="thread-list__delete"
                disabled={isBusy}
                aria-label={`${t("task.delete")} ${thread.title}`}
                title={t("task.delete")}
                onClick={() => onDelete(thread.id)}
                data-no-drag
              >
                <Trash2 aria-hidden="true" />
              </button>
            </div>
          ))}
        </div>
      ) : null}
    </aside>
  );
}

interface TaskConversationPanelProps {
  activeThread: BuildThreadSummary | null;
  detail: BuildThreadDetail | null;
  isBusy: boolean;
  onCancel: () => void;
}

function TaskConversationPanel({
  activeThread,
  detail,
  isBusy,
  onCancel,
}: TaskConversationPanelProps) {
  const { t } = useDesktopLocale();
  const entries = useMemo(() => visibleThreadEntries(detail), [detail]);
  const timelineItems = useMemo(
    () => mergeBuildThreadPresentationItems(entries.map((entry) => presentBuildThreadEntry(entry, t))),
    [entries, t],
  );
  const errorSummary = summarizeBuildThreadError(activeThread);

  return (
    <section className="build-thread-panel create-task-transcript" aria-label={t("task.session")}>
      <article className="thread-detail">
        {activeThread ? (
          <>
            <header className="thread-detail__header">
              <div>
                <strong>{activeThread.title}</strong>
                <small>{formatBuildThreadStatus(activeThread, t)}</small>
              </div>
              <div className="thread-detail__actions">
                <button type="button" disabled={isBusy} onClick={onCancel}>
                  {t("task.cancel")}
                </button>
              </div>
            </header>
            {errorSummary ? <p className="thread-error">{errorSummary}</p> : null}
            <div className="thread-entry-list task-timeline">
              {timelineItems.map((item) => (
                <TaskTimelineItem key={item.id} item={item} />
              ))}
            </div>
          </>
        ) : (
          <div className="thread-detail__empty">
            <strong>{t("task.readyTitle")}</strong>
            <p>{t("task.readyCopy")}</p>
          </div>
        )}
      </article>
    </section>
  );
}

interface TaskTimelineItemProps {
  item: BuildThreadPresentationItem;
}

const TaskTimelineItem = memo(function TaskTimelineItem({ item }: TaskTimelineItemProps) {
  const { locale, t } = useDesktopLocale();
  const [detailOpen, setDetailOpen] = useState(false);
  const hasTechnicalDetail = Boolean(item.hidesTechnicalDetail && item.technicalDetail);

  return (
    <div
      className={[
        `task-timeline-row task-timeline-row--${item.kind} task-timeline-row--${item.tone}`,
        item.isActive ? "task-timeline-row--active" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div className="task-timeline-row__icon" aria-hidden="true">
        {item.icon}
      </div>
      <article className="task-timeline-card">
        <header className="task-timeline-card__header">
          <span>{item.label}</span>
          <time dateTime={item.timestamp}>{formatTimelineTime(item.timestamp, locale)}</time>
        </header>
        <div className="task-timeline-card__title-row">
          <strong>{item.title}</strong>
          {item.isActive ? (
            <span className="task-timeline-card__activity" aria-hidden="true">
              <span />
              <span />
              <span />
            </span>
          ) : null}
        </div>
        {item.description ? <p>{item.description}</p> : null}
        {item.details.length > 0 ? (
          <div className="task-timeline-card__details">
            {item.details.map((detail, index) => (
              <code key={`${detail.label}-${detail.value}-${index}`}>
                {detail.label}: {detail.value}
              </code>
            ))}
          </div>
        ) : null}
        {item.hidesTechnicalDetail ? (
          <div className="task-timeline-card__privacy-row">
            <small className="task-timeline-card__privacy">{t("task.hiddenDetail")}</small>
            {hasTechnicalDetail ? (
              <button
                type="button"
                className="task-timeline-card__detail-toggle"
                aria-expanded={detailOpen}
                onClick={() => setDetailOpen((open) => !open)}
              >
                {detailOpen ? t("task.hideDetail") : t("task.showDetail")}
              </button>
            ) : null}
          </div>
        ) : null}
        {hasTechnicalDetail && detailOpen ? (
          <pre className="task-timeline-card__technical-detail">
            <code>{item.technicalDetail}</code>
          </pre>
        ) : null}
      </article>
    </div>
  );
});

function formatTimelineTime(timestamp: string, locale: string): string {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return date.toLocaleTimeString(locale, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

interface AgentSelectorPanelProps {
  agents: AgentConfig[];
  selectedAgentId: string | null;
  agentStatusLine: string;
  disabled: boolean;
  isOpen: boolean;
  onToggle: () => void;
  onChange: (agentId: string) => void;
}

function AgentSelectorPanel({
  agents,
  selectedAgentId,
  agentStatusLine,
  disabled,
  isOpen,
  onToggle,
  onChange,
}: AgentSelectorPanelProps) {
  const { t } = useDesktopLocale();
  const selectedAgent = agents.find((agent) => agent.id === selectedAgentId) ?? null;
  const selectorLabel = selectedAgent?.label ?? t("menu.unconfigured");

  return (
    <div className={`composer-picker ${isOpen ? "is-open" : ""}`}>
      <button
        className="composer-picker__trigger composer-picker__trigger--agent"
        type="button"
        disabled={disabled}
        aria-label={t("composer.agentSelect", { label: selectorLabel })}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        title={agentStatusLine}
        onClick={onToggle}
        data-no-drag
      >
        <span className="composer-control-icon" aria-hidden="true">
          AI
        </span>
        <strong>{selectorLabel}</strong>
        <span className="composer-control-chevron" aria-hidden="true">
          ⌄
        </span>
      </button>
      {isOpen ? (
        <div className="composer-picker__menu" role="listbox" aria-label={t("composer.agentSelect", { label: selectorLabel })}>
          {agents.length === 0 ? (
            <div className="composer-picker__empty">
              <strong>{t("agent.notConfigured")}</strong>
              <small>{agentStatusLine}</small>
            </div>
          ) : (
            agents.map((agent) => (
              <button
                key={agent.id}
                type="button"
                role="option"
                aria-selected={agent.id === selectedAgentId}
                className={agent.id === selectedAgentId ? "is-selected" : ""}
                onClick={() => onChange(agent.id)}
                data-no-drag
              >
                <span aria-hidden="true">AI</span>
                <strong>{agent.label}</strong>
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}

interface RuntimeSelectorPanelProps {
  runtimes: RuntimeOption[];
  selectedRuntime: RuntimeOption;
  runtimeChoice: RuntimeChoice;
  disabled: boolean;
  isOpen: boolean;
  onToggle: () => void;
  onChange: (runtimeChoice: RuntimeChoice) => void;
}

function RuntimeSelectorPanel({
  runtimes,
  selectedRuntime,
  runtimeChoice,
  disabled,
  isOpen,
  onToggle,
  onChange,
}: RuntimeSelectorPanelProps) {
  const { t } = useDesktopLocale();
  const runtimeGroups = useMemo(
    () =>
      runtimeCategoryOrder
        .map((category) => ({
          category,
          runtimes: runtimes.filter((runtime) => runtime.category === category),
        }))
        .filter((group) => group.runtimes.length > 0),
    [runtimes],
  );
  const selectedRuntimeLabel = runtimeOptionLabel(selectedRuntime.kind, t);
  const selectedRuntimeDetail = runtimeOptionDetail(selectedRuntime.kind, t);

  return (
    <div className={`composer-picker composer-picker--runtime ${isOpen ? "is-open" : ""}`}>
      <button
        className="composer-picker__trigger"
        type="button"
        disabled={disabled}
        aria-label={t("composer.runtimeSelect", { label: selectedRuntimeLabel })}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        title={`${selectedRuntimeLabel} · ${selectedRuntimeDetail}`}
        onClick={onToggle}
        data-no-drag
      >
        <span className="composer-runtime-icon" aria-hidden="true">
          {runtimeOptionIcons[selectedRuntime.kind]}
        </span>
        <strong>{selectedRuntimeLabel}</strong>
        <span className="composer-control-chevron" aria-hidden="true">
          ⌄
        </span>
      </button>
      {isOpen ? (
        <div
          className="composer-picker__menu composer-picker__menu--runtime"
          role="listbox"
          aria-label={t("composer.runtimeMenu")}
        >
          <header className="composer-picker__menu-header">
            <strong>{t("composer.chooseRuntime")}</strong>
            <small>{t("composer.chooseRuntimeDetail")}</small>
          </header>
          <div className="composer-runtime-groups">
            {runtimeGroups.map((group) => (
              <section key={group.category} className="composer-runtime-group">
                <header>
                  <span>{runtimeCategoryLabel(group.category, t)}</span>
                  <small>{runtimeCategoryDetail(group.category, t)}</small>
                </header>
                {group.runtimes.map((runtime) => (
                  <button
                    key={runtime.kind}
                    type="button"
                    role="option"
                    aria-selected={runtime.kind === runtimeChoice}
                    className={runtime.kind === runtimeChoice ? "is-selected" : ""}
                    onClick={() => onChange(runtime.kind)}
                    data-no-drag
                  >
                    <span aria-hidden="true">{runtimeOptionIcons[runtime.kind]}</span>
                    <strong>{runtimeOptionLabel(runtime.kind, t)}</strong>
                    <small>{runtimeOptionDetail(runtime.kind, t)}</small>
                  </button>
                ))}
              </section>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

interface ComposerLockedContextProps {
  agentLabel: string;
  runtimeLabel: string;
}

function ComposerLockedContext({ agentLabel, runtimeLabel }: ComposerLockedContextProps) {
  const { t } = useDesktopLocale();
  return (
    <div className="composer-locked-context" aria-label={t("composer.context")}>
      <span className="composer-lock-chip composer-lock-chip--agent" title={t("composer.useAgent", { label: agentLabel })}>
        <span className="composer-lock-chip__badge" aria-hidden="true">AI</span>
        <strong>{agentLabel}</strong>
        <span className="composer-lock-indicator" aria-hidden="true" />
        <small>{t("composer.locked")}</small>
      </span>
      <span className="composer-lock-chip" title={t("composer.useRuntime", { label: runtimeLabel })}>
        <span className="composer-lock-chip__badge" aria-hidden="true">RT</span>
        <strong>{runtimeLabel}</strong>
        <span className="composer-lock-indicator" aria-hidden="true" />
        <small>{t("composer.locked")}</small>
      </span>
      <small className="composer-locked-context__note">{t("composer.lockedContext")}</small>
    </div>
  );
}

interface ComposerStatusItem {
  icon: string;
  label: string;
  value: string;
}

interface ComposerStatusSummaryProps {
  statuses: ComposerStatusItem[];
}

function ComposerStatusSummary({ statuses }: ComposerStatusSummaryProps) {
  const { t } = useDesktopLocale();
  const [isOpen, setOpen] = useState(false);

  return (
    <div
      className={`composer-status-summary ${isOpen ? "is-open" : ""}`}
      tabIndex={0}
      aria-label={t("composer.status")}
      onBlur={() => setOpen(false)}
      onClick={() => setOpen((current) => !current)}
      onFocus={() => setOpen(true)}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      data-no-drag
    >
      <span aria-hidden="true">◌</span>
      <div className="composer-status-card composer-status-card--summary" role="tooltip">
        <header>
          <strong>{t("composer.status")}</strong>
          <small>{t("composer.context")}</small>
        </header>
        {statuses.map((status) => (
          <div key={status.label} className="composer-status-row">
            <span aria-hidden="true">{status.icon}</span>
            <small>{status.label}</small>
            <strong>{status.value}</strong>
          </div>
        ))}
      </div>
    </div>
  );
}

interface AgentSettingsPanelProps {
  agentState: AgentConfigState;
  discoveredAgents: DiscoveredAgent[];
  agentInstallStatuses: AgentInstallStatus[];
  activeAgentInstallId: string | null;
  agentStatusLine: string;
  isBusy: boolean;
  onAddDiscoveredAgent: (agent: DiscoveredAgent) => void;
  onToggleAgentEnabled: (agent: AgentConfig) => void;
  onSetDefaultAgent: (agentId: string) => void;
  onDeleteAgent: (agentId: string) => void;
  onTestAgent: (agentId: string) => void;
  onRefreshAgents: () => void;
  onInstallAgent: (status: AgentInstallStatus) => void;
  onCancelAgentInstall: (agentId: string) => void;
  onOpenAgentInstallPage: (agentId: string) => void;
  onRefreshAgentInstallStatuses: () => void;
}

function AgentSettingsPanel({
  agentState,
  discoveredAgents,
  agentInstallStatuses,
  activeAgentInstallId,
  agentStatusLine,
  isBusy,
  onAddDiscoveredAgent,
  onToggleAgentEnabled,
  onSetDefaultAgent,
  onDeleteAgent,
  onTestAgent,
  onRefreshAgents,
  onInstallAgent,
  onCancelAgentInstall,
  onOpenAgentInstallPage,
  onRefreshAgentInstallStatuses,
}: AgentSettingsPanelProps) {
  const { t } = useDesktopLocale();
  const configuredAgents = sortAgents(agentState.agents, agentState.defaultAgentId);
  const addableAgents = discoverableAgentsToAdd(discoveredAgents, agentState.agents);
  const discoveredById = new Map(discoveredAgents.map((agent) => [agent.config.id, agent]));

  return (
    <section className="agent-settings agent-install-settings" aria-label={t("agent.title")}>
      <header className="agent-settings__header">
        <div>
          <strong>{t("agent.title")}</strong>
          <small>{agentStatusLine}</small>
        </div>
        <button
          type="button"
          disabled={isBusy}
          onClick={() => {
            onRefreshAgents();
            onRefreshAgentInstallStatuses();
          }}
          data-no-drag
        >
          {t("action.refresh")}
        </button>
      </header>

      {agentInstallStatuses.length > 0 ? (
        <div className="agent-card-grid">
          {agentInstallStatuses.map((status) => {
            const configured = status.configured ?? agentState.agents.find((agent) => agent.id === status.catalog.id) ?? null;
            const discovered = discoveredById.get(status.catalog.id);
            const addable = discovered && !configured && discovered.detected;
            const isDefault = status.catalog.id === agentState.defaultAgentId;
            const isInstalling = activeAgentInstallId === status.catalog.id;

            return (
              <article key={status.catalog.id} className="agent-card">
                <div className="agent-card__icon" data-icon={status.catalog.iconKey} aria-hidden="true">
                  {getAgentIconLabel(status.catalog.iconKey)}
                </div>
                <div className="agent-card__body">
                  <div className="agent-card__title">
                    <strong>{status.catalog.label}</strong>
                    {status.catalog.recommended ? <span>{t("agent.builtInRecommended")}</span> : null}
                    {isDefault ? <span>{t("action.default")}</span> : null}
                  </div>
                  <small>{summarizeAgentInstall({ ...status, configured }, t)}</small>
                  <div className="agent-card__meta">
                    <span>{formatAgentInstallState({ ...status, configured }, t)}</span>
                    <span>{formatAgentConnection({ ...status, configured }, t)}</span>
                    <span>{formatAgentSource(status, t)}</span>
                    <span>{formatAgentTest({ ...status, configured }, t)}</span>
                  </div>
                  <p>{formatAgentInstallDetail({ ...status, configured }, t)}</p>
                </div>
                <div className="agent-card__actions">
                  {isInstalling ? (
                    <button
                      type="button"
                      disabled={isBusy}
                      onClick={() => onCancelAgentInstall(status.catalog.id)}
                    >
                      {t("action.cancel")}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    disabled={isBusy || !canInstallAgent(status, activeAgentInstallId)}
                    onClick={() => onInstallAgent(status)}
                  >
                    {isInstalling ? t("agent.installing") : getAgentInstallActionLabel(status, t)}
                  </button>
                  <button
                    type="button"
                    disabled={isBusy}
                    onClick={() => onOpenAgentInstallPage(status.catalog.id)}
                  >
                    {t("agent.officialSite")}
                  </button>
                  {addable ? (
                    <button type="button" disabled={isBusy} onClick={() => onAddDiscoveredAgent(discovered)}>
                      {t("action.add")}
                    </button>
                  ) : null}
                  {configured ? (
                    <>
                      <button type="button" disabled={isBusy} onClick={() => onTestAgent(configured.id)}>
                        {t("action.test")}
                      </button>
                      <button
                        type="button"
                        disabled={isBusy || !configured.enabled || isDefault}
                        onClick={() => onSetDefaultAgent(configured.id)}
                      >
                        {t("action.default")}
                      </button>
                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={() => onToggleAgentEnabled(configured)}
                      >
                        {configured.enabled ? t("action.disable") : t("action.enable")}
                      </button>
                      <button type="button" disabled={isBusy} onClick={() => onDeleteAgent(configured.id)}>
                        {t("action.delete")}
                      </button>
                    </>
                  ) : null}
                </div>
              </article>
            );
          })}
        </div>
      ) : (
        <>
          <div className="agent-settings__list">
            {configuredAgents.length === 0 ? (
              <p className="agent-settings__empty">{t("agent.notConfigured")}</p>
            ) : null}
            {configuredAgents.map((agent) => (
              <article key={agent.id} className="agent-row">
                <div className="agent-row__meta">
                  <strong>
                    {agent.label}
                    {agent.id === agentState.defaultAgentId ? <span>{t("action.default")}</span> : null}
                  </strong>
                  <small>{agent.acp ? "ACP" : agent.provider === "sofvary-pi" && agent.cli ? "Pi RPC" : agent.allowCliFallback && agent.cli ? "CLI" : t("agent.disconnected")} · {getAgentStatusLine(agent, t)}</small>
                </div>
                <div className="agent-row__actions">
                  <button type="button" disabled={isBusy} onClick={() => onTestAgent(agent.id)}>
                    {t("action.test")}
                  </button>
                  <button
                    type="button"
                    disabled={isBusy || !agent.enabled}
                    onClick={() => onSetDefaultAgent(agent.id)}
                  >
                    {t("action.default")}
                  </button>
                  <button type="button" disabled={isBusy} onClick={() => onToggleAgentEnabled(agent)}>
                    {agent.enabled ? t("action.disable") : t("action.enable")}
                  </button>
                  <button type="button" disabled={isBusy} onClick={() => onDeleteAgent(agent.id)}>
                    {t("action.delete")}
                  </button>
                </div>
              </article>
            ))}
          </div>

          <div className="agent-settings__list">
            {addableAgents.length === 0 ? (
              <p className="agent-settings__empty">{t("agent.noNewLocal")}</p>
            ) : null}
            {addableAgents.map((agent) => (
              <article key={agent.config.id} className="agent-row">
                <div className="agent-row__meta">
                  <strong>{agent.config.label}</strong>
                  <small>{formatDiscoveredAgentStatus(agent, t)}</small>
                </div>
                <button type="button" disabled={isBusy} onClick={() => onAddDiscoveredAgent(agent)}>
                  {t("action.add")}
                </button>
              </article>
            ))}
          </div>
        </>
      )}
    </section>
  );
}

interface LlmProviderSettingsPanelProps {
  providerState: LlmProviderConfigState;
  statusLine: string;
  isBusy: boolean;
  onSaveProvider: (config: LlmProviderConfig, apiKey?: string) => void;
  onToggleProviderEnabled: (provider: LlmProviderConfig) => void;
  onSetDefaultProvider: (providerId: string) => void;
  onDeleteProvider: (providerId: string) => void;
  onTestProvider: (providerId: string) => void;
  onRefreshProviders: () => void;
}

function LlmProviderSettingsPanel({
  providerState,
  statusLine,
  isBusy,
  onSaveProvider,
  onToggleProviderEnabled,
  onSetDefaultProvider,
  onDeleteProvider,
  onTestProvider,
  onRefreshProviders,
}: LlmProviderSettingsPanelProps) {
  const { t } = useDesktopLocale();
  const providers = sortLlmProviders(providerState.providers, providerState.defaultProviderId);
  const [draft, setDraft] = useState<LlmProviderConfig>(() =>
    providerState.providers.find((provider) => provider.providerId === providerState.defaultProviderId) ??
    createLlmProviderConfigFromPreset(llmProviderPresets[0]),
  );
  const [draftApiKey, setDraftApiKey] = useState("");
  const preset = getLlmProviderPreset(draft.kind);
  const modelOptions = useMemo(() => getLlmModelOptions(draft.kind), [draft.kind]);
  const datalistId = `llm-model-options-${draft.kind}`;

  useEffect(() => {
    if (!providerState.defaultProviderId) return;
    const defaultProvider = providerState.providers.find(
      (provider) => provider.providerId === providerState.defaultProviderId,
    );
    if (defaultProvider) {
      setDraft(defaultProvider);
      setDraftApiKey("");
    }
  }, [providerState.defaultProviderId, providerState.providers]);

  const updateDraft = (patch: Partial<LlmProviderConfig>) => {
    setDraft((current) => ({ ...current, ...patch }));
  };

  const selectPreset = (kind: LlmProviderKind) => {
    const nextPreset = getLlmProviderPreset(kind);
    setDraft(createLlmProviderConfigFromPreset(nextPreset));
    setDraftApiKey("");
  };

  const editProvider = (provider: LlmProviderConfig) => {
    setDraft(provider);
    setDraftApiKey("");
  };

  const saveProvider = () => {
    onSaveProvider(normalizeLlmProviderDraft(draft), draftApiKey || undefined);
    setDraftApiKey("");
  };

  return (
    <section className="agent-settings llm-provider-settings" aria-label={t("llm.title")}>
      <header className="agent-settings__header">
        <div>
          <strong>{t("llm.title")}</strong>
          <small>{statusLine}</small>
        </div>
        <button type="button" disabled={isBusy} onClick={onRefreshProviders} data-no-drag>
          {t("action.refresh")}
        </button>
      </header>

      <div className="llm-provider-form">
        <label>
          <span>{t("llm.provider")}</span>
          <select
            value={draft.kind}
            disabled={isBusy}
            onChange={(event) => selectPreset(event.currentTarget.value as LlmProviderKind)}
          >
            {llmProviderPresets.map((option) => (
              <option key={option.kind} value={option.kind}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("llm.configId")}</span>
          <input
            value={draft.providerId}
            disabled={isBusy}
            onChange={(event) => updateDraft({ providerId: event.currentTarget.value })}
          />
        </label>
        <label>
          <span>{t("llm.name")}</span>
          <input
            value={draft.label}
            disabled={isBusy}
            onChange={(event) => updateDraft({ label: event.currentTarget.value })}
          />
        </label>
        <label>
          <span>{t("llm.baseUrl")}</span>
          <input
            value={draft.baseUrl ?? ""}
            disabled={isBusy}
            placeholder={preset.baseUrl ?? t("llm.defaultEndpoint")}
            onChange={(event) => updateDraft({ baseUrl: event.currentTarget.value || null })}
          />
        </label>
        <label>
          <span>{t("llm.model")}</span>
          <input
            value={draft.model}
            list={datalistId}
            disabled={isBusy}
            onChange={(event) => updateDraft({ model: event.currentTarget.value })}
          />
          <datalist id={datalistId}>
            {modelOptions.map((model) => (
              <option key={model} value={model} />
            ))}
          </datalist>
        </label>
        <label>
          <span>{t("llm.apiKey")}</span>
          <input
            value={draftApiKey}
            type="password"
            placeholder={draft.apiKeyRef ? t("llm.savedSecret") : preset.apiKeyPlaceholder}
            disabled={isBusy}
            onChange={(event) => setDraftApiKey(event.currentTarget.value)}
          />
        </label>
        <div className="llm-provider-form__actions">
          <small>
            {t("llm.piCommand", { provider: draft.kind, model: draft.model || t("status.none") })}
            {preset.apiKeyRequired ? t("llm.apiKeyRequired") : t("llm.apiKeyOptional")}
          </small>
          <button
            type="button"
            disabled={isBusy || !draft.providerId.trim() || !draft.label.trim() || !draft.model.trim()}
            onClick={saveProvider}
            data-no-drag
          >
            {t("action.save")}
          </button>
        </div>
      </div>

      <div className="agent-settings__list">
        {providers.length === 0 ? (
          <p className="agent-settings__empty">{t("llm.notConfigured")}</p>
        ) : null}
        {providers.map((provider) => (
          <article key={provider.providerId} className="agent-row">
            <div className="agent-row__meta">
              <strong>
                {provider.label}
                {provider.providerId === providerState.defaultProviderId ? <span>{t("action.default")}</span> : null}
              </strong>
              <small>
                {provider.kind} · {provider.model}
                {provider.baseUrl ? ` · ${provider.baseUrl}` : ""}
              </small>
            </div>
            <div className="agent-row__actions">
              <button type="button" disabled={isBusy} onClick={() => editProvider(provider)}>
                {t("action.edit")}
              </button>
              <button type="button" disabled={isBusy} onClick={() => onTestProvider(provider.providerId)}>
                {t("action.test")}
              </button>
              <button
                type="button"
                disabled={isBusy || !provider.enabled}
                onClick={() => onSetDefaultProvider(provider.providerId)}
              >
                {t("action.default")}
              </button>
              <button type="button" disabled={isBusy} onClick={() => onToggleProviderEnabled(provider)}>
                {provider.enabled ? t("action.disable") : t("action.enable")}
              </button>
              <button type="button" disabled={isBusy} onClick={() => onDeleteProvider(provider.providerId)}>
                {t("action.delete")}
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

interface WorkspaceListPanelProps {
  workspaces: WorkspaceSummary[];
  capsuleStatusLine: string;
  releaseStatusLine: string;
  isBusy: boolean;
  activeCapsuleAppId: string | null;
  activeReleaseAppId: string | null;
  activePreviewAppId: string | null;
  buildThreads: BuildThreadSummary[];
  onImportCapsule: () => void;
  onPreviewWorkspace: (workspace: WorkspaceSummary) => void;
  onModifyWorkspace: (workspace: WorkspaceSummary) => void;
  onExportWorkspace: (workspace: WorkspaceSummary) => void;
  onReleaseWorkspace: (workspace: WorkspaceSummary) => void;
  onDeleteWorkspace: (workspace: WorkspaceSummary) => void;
}

function WorkspaceListPanel({
  workspaces,
  capsuleStatusLine,
  releaseStatusLine,
  isBusy,
  activeCapsuleAppId,
  activeReleaseAppId,
  activePreviewAppId,
  buildThreads,
  onImportCapsule,
  onPreviewWorkspace,
  onModifyWorkspace,
  onExportWorkspace,
  onReleaseWorkspace,
  onDeleteWorkspace,
}: WorkspaceListPanelProps) {
  const { locale, t } = useDesktopLocale();
  return (
    <section className="workspace-list" aria-label={t("workspace.aria")}>
      <div className="workspace-list__header">
        <div>
          <strong>{t("workspace.title")}</strong>
          <small>{releaseStatusLine !== "Publishing ready." ? releaseStatusLine : capsuleStatusLine}</small>
        </div>
        <button
          className="workspace-icon-button workspace-list__import-button"
          type="button"
          disabled={isBusy}
          title={t("action.import")}
          aria-label={t("workspace.importCapsule")}
          onClick={onImportCapsule}
          data-no-drag
        >
          <Upload aria-hidden="true" />
        </button>
      </div>
      {workspaces.length > 0 ? (
        <div className="workspace-list__rows">
          {workspaces.map((workspace) => {
            const isExporting = activeCapsuleAppId === workspace.appId;
            const isReleasing = activeReleaseAppId === workspace.appId;
            const isPreviewing = activePreviewAppId === workspace.appId;
            const buildThread = getWorkspaceBuildThread(workspace, buildThreads);
            const canModify = canContinueBuildThread(buildThread);

            return (
              <div key={workspace.appId} className="workspace-row">
                <div className="workspace-row__identity">
                  <span
                    className="workspace-row__runtime"
                    title={formatWorkspaceRuntime(workspace.mode, t)}
                    aria-hidden="true"
                  >
                    {formatWorkspaceRuntimeBadge(workspace.mode)}
                  </span>
                  <div className="workspace-row__meta">
                    <span>{workspace.name}</span>
                    <small>
                      {formatWorkspaceRuntime(workspace.mode, t)} / {formatWorkspaceUpdatedAt(workspace.updatedAt, locale, t)}
                    </small>
                  </div>
                </div>
                <div className="workspace-row__actions">
                  <button
                    className={`workspace-icon-button workspace-row__preview ${
                      isPreviewing ? "is-active" : ""
                    }`}
                    type="button"
                    disabled={isBusy}
                    title={isPreviewing ? t("workspace.opening") : t("action.preview")}
                    aria-label={`${isPreviewing ? t("workspace.opening") : t("action.preview")} ${workspace.name}`}
                    onClick={() => onPreviewWorkspace(workspace)}
                    data-no-drag
                  >
                    <Eye aria-hidden="true" />
                  </button>
                  <button
                    className="workspace-icon-button workspace-row__modify"
                    type="button"
                    disabled={isBusy || !canModify}
                    title={canModify ? t("task.continue") : t("workspace.noThread")}
                    aria-label={`${t("task.continue")} ${workspace.name}`}
                    onClick={() => onModifyWorkspace(workspace)}
                    data-no-drag
                  >
                    <PencilLine aria-hidden="true" />
                  </button>
                  <button
                    className={`workspace-icon-button workspace-row__export ${
                      isExporting ? "is-active" : ""
                    }`}
                    type="button"
                    disabled={isBusy}
                    title={isExporting ? t("workspace.exporting") : t("action.export")}
                    aria-label={`${isExporting ? t("workspace.exporting") : t("action.export")} ${workspace.name}`}
                    onClick={() => onExportWorkspace(workspace)}
                    data-no-drag
                  >
                    <Download aria-hidden="true" />
                  </button>
                  <button
                    className={`workspace-icon-button workspace-row__release ${
                      isReleasing ? "is-active" : ""
                    }`}
                    type="button"
                    disabled={isBusy}
                    title={isReleasing ? t("release.publishing", {}, "Publishing") : t("release.start", {}, "Publish")}
                    aria-label={`${isReleasing ? t("release.publishing", {}, "Publishing") : t("release.start", {}, "Publish")} ${workspace.name}`}
                    onClick={() => onReleaseWorkspace(workspace)}
                    data-no-drag
                  >
                    <PackageCheck aria-hidden="true" />
                    <span>{t("release.start", {}, "Publish")}</span>
                  </button>
                  <button
                    className="workspace-icon-button workspace-row__delete"
                    type="button"
                    disabled={isBusy}
                    title={t("action.delete")}
                    aria-label={`${t("action.delete")} ${workspace.name}`}
                    onClick={() => onDeleteWorkspace(workspace)}
                    data-no-drag
                  >
                    <Trash2 aria-hidden="true" />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <p className="workspace-list__empty">{t("workspace.empty")}</p>
      )}
    </section>
  );
}

function formatWorkspaceRuntimeBadge(kind: RuntimeKind): string {
  return runtimeOptionIcons[kind] ?? kind;
}

function formatWorkspaceRuntime(kind: RuntimeKind, t: DesktopTranslator = fallbackDesktopT): string {
  return runtimeOptionLabel(kind, t);
}

function formatRuntimeChoice(choice: RuntimeChoice, t: DesktopTranslator = fallbackDesktopT): string {
  return runtimeOptionLabel(choice, t);
}

function formatSoftwareType(value: string): string {
  const labels: Record<string, string> = {
    "File Processor": "文件处理工具",
    "Local Data App": "本地数据应用",
    "Data Table Tool": "表格数据工具",
    "Knowledge App": "知识库应用",
    "Interactive Visual Tool": "图形互动工具",
    "Desktop Widget": "桌面小组件",
    "Interactive App": "完整交互应用",
    "Lightweight Page Tool": "轻量单页工具",
  };
  return labels[value] ?? value;
}

function formatConfidence(confidence: number): string {
  return `${Math.round(confidence * 100)}%`;
}

function formatWorkspaceUpdatedAt(updatedAt: string, locale = "en", t: DesktopTranslator = fallbackDesktopT): string {
  const timestamp = new Date(updatedAt);
  if (Number.isNaN(timestamp.getTime())) {
    return t("workspace.updatedUnknown");
  }

  const value = new Intl.DateTimeFormat(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(timestamp);
  return t("workspace.updated", { value });
}

interface PackListPanelProps {
  installedPacks: InstalledPackSummary[];
  packStatusLine: string;
  isBusy: boolean;
  onRefreshPacks: () => void;
}

function PackListPanel({
  installedPacks,
  packStatusLine,
  isBusy,
  onRefreshPacks,
}: PackListPanelProps) {
  const { t } = useDesktopLocale();
  return (
    <section className="pack-list" aria-label={t("pack.aria")}>
      <div className="workspace-list__header">
        <div>
          <strong>{t("pack.title")}</strong>
          <small>{packStatusLine}</small>
        </div>
        <button type="button" disabled={isBusy} onClick={onRefreshPacks} data-no-drag>
          {t("action.refresh")}
        </button>
      </div>
      {installedPacks.length > 0 ? (
        <div className="pack-list__rows">
          {installedPacks.slice(0, 8).map((pack) => (
            <div key={`${pack.kind}:${pack.id}@${pack.version}`} className="pack-row">
              <div className="workspace-row__meta">
                <span>{formatPackLabel(pack)}</span>
                <small>
                  {pack.kind} / {pack.source}
                </small>
              </div>
              <span className="pack-row__hash">{pack.sha256 ? "sha256" : "local"}</span>
            </div>
          ))}
        </div>
      ) : (
        <p className="workspace-list__empty">{t("pack.empty")}</p>
      )}
    </section>
  );
}

function getActiveNavigation(activeAction: NavigationKey): NavigationKey {
  return navigationItems.some((item) => item.key === activeAction)
    ? activeAction
    : "create";
}
