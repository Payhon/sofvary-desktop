import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ExternalLink, LifeBuoy, LogOut, Minus, Monitor, Moon, Pin, PinOff, RefreshCw, Square, Sun, User, X } from "lucide-react";
import {
  clearRefreshToken,
  getSavedRefreshToken,
  loginAccount,
  logoutAccount,
  openSofvaryWebsite,
  refreshAccount as refreshAccountSession,
  registerAccount,
  saveRefreshToken,
  type AccountUser,
  type AuthTokens,
} from "../../core/account/accountClient";
import { accountInitials, accountStatusLine, type AccountState } from "../../core/account/accountLogic";
import {
  exportAppCapsule,
  importAppCapsule,
  selectExportCapsulePath,
  selectImportCapsulePath,
} from "../../core/capsule/capsuleClient";
import {
  deleteAgentConfig,
  discoverAgents,
  listAgentConfigs,
  setDefaultAgent,
  testAgentConnection,
  upsertAgentConfig,
} from "../../core/agents/agentClient";
import {
  formatAgentInteractionMode,
  formatAgentTestRecord,
  getAgentInteractionModes,
  getAgentStatusLine,
  getDefaultAgentInteractionMode,
  getSelectableAgents,
  getSelectedAgentId,
  normalizeAgentInteractionMode,
} from "../../core/agents/agentLogic";
import {
  cancelAgentInstall,
  getAgentInstallStatuses,
  openAgentInstallPage,
  refreshAgentInstallStatuses,
  startAgentInstall,
} from "../../core/agentInstall/agentInstallClient";
import {
  formatAgentInstallDetail,
  formatAgentInstallDetailText,
  sortAgentInstallStatuses,
} from "../../core/agentInstall/agentInstallLogic";
import {
  analyzeBuildIntent,
  cancelBuildThread,
  continueBuildThread,
  copyHandoffPrompt,
  copyHandoffRepairPrompt,
  deleteBuildThread,
  getBuildThread,
  listBuildThreads,
  openHandoffAgent,
  openHandoffWorkspace,
  rescanHandoffWorkspace,
  retryBuildThreadPreview,
  startBuildThread,
} from "../../core/buildThreads/buildThreadClient";
import {
  BuildThreadEventBatcher,
  type BuildThreadEventBatch,
} from "../../core/buildThreads/buildThreadEventBatcher";
import {
  applyBuildThreadEventBatch,
  canContinueBuildThread,
  formatBuildThreadStatus,
  getWorkspaceBuildThread,
  mergeBuildThreadEntries,
  sortBuildThreads,
  summarizeBuildThreadError,
  upsertBuildThreadSummary,
} from "../../core/buildThreads/buildThreadLogic";
import {
  installAppFromDeepLink,
  prepareDeepLinkInstall,
} from "../../core/deep-link/deepLinkClient";
import {
  formatDeepLinkStatus,
  type DeepLinkStatusState,
} from "../../core/deep-link/deepLinkLogic";
import {
  formatCapsuleStatus,
  getCapsuleErrorMessage,
  type CapsuleStatusState,
} from "../../core/capsule/capsuleLogic";
import {
  getAppReleaseCapabilities,
  getPackagerToolchainStatus,
  openAppReleaseOutputFolder,
  selectReleaseIconPath,
  selectReleaseOutputFolder,
  startAppReleaseJob,
  startPackagerToolchainInstall,
} from "../../core/release/releaseClient";
import {
  buildAppReleasePayload,
  formatReleaseStatus,
  type ReleaseStatusState,
} from "../../core/release/releaseLogic";
import { previewPolicy } from "../../core/policy/policyClient";
import {
  buildPolicyApprovalSet,
  emptyPolicyApprovals,
  formatPolicyBlockMessage,
  summarizePolicyDecisions,
} from "../../core/policy/policyLogic";
import { listInstalledPacks } from "../../core/packs/packClient";
import {
  formatPackStatus,
  sortInstalledPacks,
  type PackStatusState,
} from "../../core/packs/packLogic";
import {
  deleteLlmProviderConfig,
  listLlmProviderConfigs,
  setDefaultLlmProvider,
  testLlmProviderConfig,
  upsertLlmProviderConfig,
} from "../../core/llmProviders/llmProviderClient";
import {
  getDefaultLlmProvider,
  getLlmProviderStatusLine,
} from "../../core/llmProviders/llmProviderLogic";
import {
  getRuntimeEnvironmentStatuses,
  setActiveRuntimeEnvironmentVersion,
  startRuntimeEnvironmentInstall,
} from "../../core/runtimeEnvironment/runtimeEnvironmentClient";
import {
  getDefaultRuntimeEnvironmentVersion,
  getRuntimeEnvironmentRequirementIssue,
  runtimeEnvironmentInstallKey,
  sortRuntimeEnvironmentStatuses,
} from "../../core/runtimeEnvironment/runtimeEnvironmentLogic";
import { useUiAppearance } from "../../core/uiSettings/uiSettingsClient";
import {
  formatUiThemePreference,
  getNextUiThemePreference,
} from "../../core/uiSettings/uiSettingsLogic";
import { deleteWorkspace, listWorkspaces, previewWorkspace } from "../../core/workspace/workspaceClient";
import { hideCommandWindow, minimizeCommandWindow, showMainWindow } from "../../platform/shellClient";
import { emitShellEvent, listenShellEvent, type ShellEventName } from "../../platform/eventClient";
import { useWindowDrag } from "../../platform/useWindowDrag";
import { setCurrentWindowShadow, toggleCurrentWindowMaximize } from "../../platform/windowClient";
import type {
  DeepLinkInstallPreflight,
  AgentConfig,
  AgentConfigState,
  AgentInteractionMode,
  AgentInstallStatus,
  AppReleaseCapability,
  AppReleaseTargetPlatform,
  BuildThreadDetail,
  BuildThreadEntry,
  BuildThreadSummary,
  DiscoveredAgent,
  InstalledPackSummary,
  LlmProviderConfig,
  LlmProviderConfigState,
  PolicyApprovalSet,
  PolicyDecision,
  PreviewPolicyPayload,
  PromptEnvelopeSummary,
  RuntimeChoice,
  RuntimeEnvironmentStatus,
  RuntimeEnvironmentVersionOption,
  RuntimeIntentSelection,
  RuntimeKind,
  RuntimePreview,
  PackagerToolchainStatus,
  ShellState,
  WorkspaceSummary,
} from "../../types";
import { SofvaryBrandMark } from "../brand/SofvaryBrandMark";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";
import { FloatingCommandMenu, type NavigationKey } from "./FloatingCommandMenu";
import { PermissionDialog } from "./PermissionDialog";
import { ReleaseWizard } from "./ReleaseWizard";

const activeStates: ShellState[] = ["Planning", "Building"];
const POLICY_APPROVAL_CANCELED = "Policy approval canceled.";

interface PolicyDialogState {
  title: string;
  decisions: PolicyDecision[];
  resolve: (approvals: PolicyApprovalSet) => void;
  reject: (error: Error) => void;
}

export function CommandWindowRoot() {
  const { t } = useDesktopLocale();
  const [shellState, setShellState] = useState<ShellState>("CommandMenuVisible");
  const [isPinned, setPinned] = useState(false);
  const [accountCardOpen, setAccountCardOpen] = useState(false);
  const [accountMode, setAccountMode] = useState<"login" | "register">("login");
  const [accountEmail, setAccountEmail] = useState("");
  const [accountPassword, setAccountPassword] = useState("");
  const [accountUsername, setAccountUsername] = useState("");
  const [accountState, setAccountState] = useState<AccountState>({
    kind: "signed-out",
    user: null,
    tokens: null,
  });
  const [createPrompt, setCreatePrompt] = useState("");
  const [continuePrompt, setContinuePrompt] = useState("");
  const [activeAction, setActiveAction] = useState<NavigationKey>("create");
  const [runtimeChoice, setRuntimeChoice] = useState<RuntimeChoice>("auto");
  const [intentSelection, setIntentSelection] = useState<RuntimeIntentSelection | null>(null);
  const [promptEnvelopeSummary, setPromptEnvelopeSummary] =
    useState<PromptEnvelopeSummary | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [installedPacks, setInstalledPacks] = useState<InstalledPackSummary[]>([]);
  const [agentState, setAgentState] = useState<AgentConfigState>({
    defaultAgentId: null,
    agents: [],
  });
  const [buildThreads, setBuildThreads] = useState<BuildThreadSummary[]>([]);
  const [activeThreadId, setActiveThreadId] = useState<string | null>(null);
  const [activeThreadDetail, setActiveThreadDetail] = useState<BuildThreadDetail | null>(null);
  const buildThreadsRef = useRef<BuildThreadSummary[]>([]);
  const activeThreadIdRef = useRef<string | null>(null);
  const activeThreadDetailRef = useRef<BuildThreadDetail | null>(null);
  const [discoveredAgents, setDiscoveredAgents] = useState<DiscoveredAgent[]>([]);
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [selectedAgentMode, setSelectedAgentMode] = useState<AgentInteractionMode>("pi-native");
  const [agentStatusOverride, setAgentStatusOverride] = useState<string | null>(null);
  const [agentInstallStatuses, setAgentInstallStatuses] = useState<AgentInstallStatus[]>([]);
  const [activeAgentInstallId, setActiveAgentInstallId] = useState<string | null>(null);
  const [runtimeEnvironmentStatuses, setRuntimeEnvironmentStatuses] = useState<
    RuntimeEnvironmentStatus[]
  >([]);
  const [activeRuntimeEnvironmentInstallKey, setActiveRuntimeEnvironmentInstallKey] =
    useState<string | null>(null);
  const [runtimeEnvironmentStatusOverride, setRuntimeEnvironmentStatusOverride] = useState<
    string | null
  >(null);
  const [runtimePreflightMessage, setRuntimePreflightMessage] = useState<string | null>(null);
  const [llmProviderState, setLlmProviderState] = useState<LlmProviderConfigState>({
    defaultProviderId: null,
    providers: [],
  });
  const [llmProviderStatusOverride, setLlmProviderStatusOverride] = useState<string | null>(null);
  const [packStatus, setPackStatus] = useState<PackStatusState>({ kind: "idle" });
  const [capsuleStatus, setCapsuleStatus] = useState<CapsuleStatusState>({ kind: "idle" });
  const [releaseStatus, setReleaseStatus] = useState<ReleaseStatusState>({ kind: "idle" });
  const [releaseCapabilities, setReleaseCapabilities] = useState<AppReleaseCapability | null>(null);
  const [packagerToolchainStatus, setPackagerToolchainStatus] =
    useState<PackagerToolchainStatus | null>(null);
  const [activeReleaseWorkspace, setActiveReleaseWorkspace] = useState<WorkspaceSummary | null>(
    null,
  );
  const [activeReleaseAppId, setActiveReleaseAppId] = useState<string | null>(null);
  const [deepLinkValue, setDeepLinkValue] = useState("");
  const [deepLinkStatus, setDeepLinkStatus] = useState<DeepLinkStatusState>({ kind: "idle" });
  const [deepLinkPreflight, setDeepLinkPreflight] = useState<DeepLinkInstallPreflight | null>(null);
  const [activeCapsuleAppId, setActiveCapsuleAppId] = useState<string | null>(null);
  const [activePreviewAppId, setActivePreviewAppId] = useState<string | null>(null);
  const [policyDialog, setPolicyDialog] = useState<PolicyDialogState | null>(null);
  const startWindowDrag = useWindowDrag("command");
  const uiAppearance = useUiAppearance();
  const nextThemePreference = getNextUiThemePreference(uiAppearance.preference);
  const ThemeIcon =
    uiAppearance.preference === "light"
      ? Sun
      : uiAppearance.preference === "dark"
        ? Moon
        : Monitor;
  const themeToggleTitle = `Theme: ${formatUiThemePreference(uiAppearance.preference)} -> ${formatUiThemePreference(nextThemePreference)}`;
  const accountTitle = accountStatusLine(accountState);

  const applyAccountSession = useCallback(async (user: AccountUser, tokens: AuthTokens) => {
    await saveRefreshToken(tokens.refreshToken);
    setAccountState({ kind: "signed-in", user, tokens });
  }, []);

  const syncAccount = useCallback(async () => {
    setAccountState((current) => ({ ...current, kind: "loading", detail: undefined }));
    try {
      const refreshToken = await getSavedRefreshToken();
      if (!refreshToken) {
        setAccountState({ kind: "signed-out", user: null, tokens: null });
        return;
      }
      const response = await refreshAccountSession(refreshToken);
      await applyAccountSession(response.user, response.tokens);
    } catch (error) {
      setAccountState({
        kind: "error",
        user: null,
        tokens: null,
        detail: error instanceof Error ? error.message : "Account sync failed.",
      });
    }
  }, [applyAccountSession]);

  useEffect(() => {
    void syncAccount();
  }, [syncAccount]);

  const handleAccountSubmit = useCallback(async () => {
    setAccountState((current) => ({ ...current, kind: "loading", detail: undefined }));
    try {
      const response =
        accountMode === "register"
          ? await registerAccount({
              email: accountEmail,
              password: accountPassword,
              username: accountUsername || undefined,
            })
          : await loginAccount(accountEmail, accountPassword);
      await applyAccountSession(response.user, response.tokens);
      setAccountPassword("");
      setAccountUsername("");
      setAccountCardOpen(false);
    } catch (error) {
      setAccountState({
        kind: "error",
        user: null,
        tokens: null,
        detail: error instanceof Error ? error.message : "Account sign-in failed.",
      });
    }
  }, [accountEmail, accountMode, accountPassword, accountUsername, applyAccountSession]);

  const handleAccountLogout = useCallback(async () => {
    const refreshToken = accountState.tokens?.refreshToken;
    setAccountState({ kind: "signed-out", user: null, tokens: null });
    await clearRefreshToken().catch(() => undefined);
    if (refreshToken) {
      await logoutAccount(refreshToken).catch(() => undefined);
    }
  }, [accountState.tokens?.refreshToken]);

  const isActive = activeStates.includes(shellState);
  const isPolicyBusy = policyDialog !== null;
  const isCapsuleBusy =
    capsuleStatus.kind === "choosing-export" ||
    capsuleStatus.kind === "exporting" ||
    capsuleStatus.kind === "choosing-import" ||
    capsuleStatus.kind === "importing";
  const isReleaseBusy =
    releaseStatus.kind === "publishing" ||
    releaseStatus.kind === "installing-toolchain" ||
    releaseStatus.kind === "checking-toolchain" ||
    releaseStatus.kind === "choosing-output";
  const isDeepLinkBusy = deepLinkStatus.kind === "reviewing" || deepLinkStatus.kind === "installing";
  const selectableAgents = useMemo(() => getSelectableAgents(agentState), [agentState]);
  const sortedAgentInstallStatuses = useMemo(
    () => sortAgentInstallStatuses(agentInstallStatuses, agentState),
    [agentInstallStatuses, agentState],
  );
  const activeAgentId = useMemo(
    () => getSelectedAgentId(selectedAgentId, agentState),
    [agentState, selectedAgentId],
  );
  const activeAgent = useMemo(
    () => agentState.agents.find((agent) => agent.id === activeAgentId) ?? null,
    [activeAgentId, agentState.agents],
  );
  const availableAgentModes = useMemo(
    () => getAgentInteractionModes(activeAgent),
    [activeAgent],
  );
  const activeAgentMode = useMemo(
    () => normalizeAgentInteractionMode(activeAgent, selectedAgentMode),
    [activeAgent, selectedAgentMode],
  );
  const agentStatusLine = agentStatusOverride ?? getAgentStatusLine(activeAgent, t);
  const defaultLlmProvider = useMemo(
    () => getDefaultLlmProvider(llmProviderState),
    [llmProviderState],
  );
  const llmProviderStatusLine =
    llmProviderStatusOverride ?? getLlmProviderStatusLine(defaultLlmProvider);
  const activeThread = useMemo(
    () => buildThreads.find((thread) => thread.id === activeThreadId) ?? null,
    [activeThreadId, buildThreads],
  );
  const statusLine = useMemo(() => {
    if (runtimePreflightMessage) return runtimePreflightMessage;
    if (policyDialog) return t("permission.title");
    if (releaseStatus.kind !== "idle") return formatReleaseStatus(releaseStatus);
    if (shellState === "Planning") return t("build.status.planning");
    if (shellState === "Building") return t("build.status.building");
    if (shellState === "Error") return t("status.failed");
    return activeThread ? formatBuildThreadStatus(activeThread, t) : t("status.ready");
  }, [activeThread, policyDialog, releaseStatus, runtimePreflightMessage, shellState, t]);

  const refreshWorkspaces = useCallback(() => {
    listWorkspaces()
      .then(setWorkspaces)
      .catch(() => setWorkspaces([]));
  }, []);

  const refreshInstalledPacks = useCallback(() => {
    setPackStatus({ kind: "loading" });
    listInstalledPacks()
      .then((packs) => {
        const sorted = sortInstalledPacks(packs);
        setInstalledPacks(sorted);
        setPackStatus({
          kind: "success",
          detail: `${sorted.length} installed pack${sorted.length === 1 ? "" : "s"}.`,
        });
      })
      .catch((error) => {
        setInstalledPacks([]);
        setPackStatus({
          kind: "error",
          detail: error instanceof Error ? error.message : String(error),
        });
      });
  }, []);

  const refreshAgents = useCallback(() => {
    setAgentStatusOverride(t("agent.status.discovering"));
    Promise.all([listAgentConfigs(), discoverAgents()])
      .then(([configs, discovered]) => {
        setAgentState(configs);
        setDiscoveredAgents(discovered);
        setAgentStatusOverride(null);
      })
      .catch((error) => {
        setAgentState({ defaultAgentId: null, agents: [] });
        setDiscoveredAgents([]);
        setAgentStatusOverride(error instanceof Error ? error.message : String(error));
      });
  }, [t]);

  const refreshAgentInstalls = useCallback(() => {
    getAgentInstallStatuses()
      .then(setAgentInstallStatuses)
      .catch((error) => {
        setAgentInstallStatuses([]);
        setAgentStatusOverride(error instanceof Error ? error.message : String(error));
      });
  }, []);

  const refreshRuntimeEnvironments = useCallback(() => {
    getRuntimeEnvironmentStatuses()
      .then((statuses) => {
        setRuntimeEnvironmentStatuses(sortRuntimeEnvironmentStatuses(statuses));
        setRuntimeEnvironmentStatusOverride(null);
      })
      .catch((error) => {
        setRuntimeEnvironmentStatuses([]);
        setRuntimeEnvironmentStatusOverride(error instanceof Error ? error.message : String(error));
      });
  }, []);

  const refreshReleaseCapabilities = useCallback(() => {
    getAppReleaseCapabilities()
      .then(setReleaseCapabilities)
      .catch((error) => {
        setReleaseCapabilities(null);
        setReleaseStatus({ kind: "error", detail: error instanceof Error ? error.message : String(error) });
      });
  }, []);

  const refreshPackagerToolchain = useCallback(async (showStatus = false) => {
    if (showStatus) {
      setReleaseStatus({ kind: "checking-toolchain" });
    }
    try {
      const status = await getPackagerToolchainStatus();
      setPackagerToolchainStatus(status);
      if (showStatus) {
        setReleaseStatus({ kind: "ready", detail: status.detail });
      }
    } catch (error) {
      setPackagerToolchainStatus(null);
      if (showStatus) {
        setReleaseStatus({ kind: "error", detail: error instanceof Error ? error.message : String(error) });
      }
    }
  }, []);

  const refreshBuildThreads = useCallback(() => {
    listBuildThreads()
      .then((threads) => setBuildThreads(sortBuildThreads(threads)))
      .catch(() => setBuildThreads([]));
  }, []);

  const refreshActiveThread = useCallback((threadId: string | null) => {
    if (!threadId) {
      setActiveThreadDetail(null);
      return;
    }
    getBuildThread(threadId)
      .then((detail) => {
        setActiveThreadDetail((current) => mergeFetchedThreadDetail(current, detail));
      })
      .catch(() => setActiveThreadDetail(null));
  }, []);

  const refreshLlmProviders = useCallback(() => {
    listLlmProviderConfigs()
      .then((state) => {
        setLlmProviderState(state);
        setLlmProviderStatusOverride(null);
      })
      .catch((error) => {
        setLlmProviderState({ defaultProviderId: null, providers: [] });
        setLlmProviderStatusOverride(error instanceof Error ? error.message : String(error));
      });
  }, []);

  const applyThreadBatch = useCallback((batch: BuildThreadEventBatch) => {
    const next = applyBuildThreadEventBatch(
      {
        threads: buildThreadsRef.current,
        activeThreadId: activeThreadIdRef.current,
        activeThreadDetail: activeThreadDetailRef.current,
      },
      batch,
      { selectFirstThread: true },
    );
    buildThreadsRef.current = next.threads;
    activeThreadIdRef.current = next.activeThreadId;
    activeThreadDetailRef.current = next.activeThreadDetail;
    setBuildThreads(next.threads);
    setActiveThreadId(next.activeThreadId);
    setActiveThreadDetail(next.activeThreadDetail);
  }, []);

  useEffect(() => {
    buildThreadsRef.current = buildThreads;
  }, [buildThreads]);

  useEffect(() => {
    activeThreadIdRef.current = activeThreadId;
  }, [activeThreadId]);

  useEffect(() => {
    activeThreadDetailRef.current = activeThreadDetail;
  }, [activeThreadDetail]);

  useEffect(() => {
    setSelectedAgentMode(getDefaultAgentInteractionMode(activeAgent));
  }, [activeAgent]);

  useEffect(() => {
    refreshWorkspaces();
    refreshInstalledPacks();
    refreshAgents();
    refreshAgentInstalls();
    refreshRuntimeEnvironments();
    refreshBuildThreads();
    refreshLlmProviders();
    refreshReleaseCapabilities();
    void refreshPackagerToolchain();
  }, [
    refreshAgentInstalls,
    refreshAgents,
    refreshBuildThreads,
    refreshInstalledPacks,
    refreshLlmProviders,
    refreshPackagerToolchain,
    refreshReleaseCapabilities,
    refreshRuntimeEnvironments,
    refreshWorkspaces,
  ]);

  useEffect(() => {
    refreshActiveThread(activeThread?.id ?? null);
  }, [activeThread?.id, refreshActiveThread]);

  useEffect(() => {
    const batcher = new BuildThreadEventBatcher(applyThreadBatch);
    const unlisteners = [
      listenShellEvent<BuildThreadSummary>("sofvary-build-thread-updated", (thread) => {
        batcher.pushSummary(thread);
        if (thread.status === "preview-blocked") {
          refreshWorkspaces();
        }
      }),
      listenShellEvent<BuildThreadEntry>("sofvary-build-thread-entry", (entry) => {
        batcher.pushEntry(entry);
      }),
      listenShellEvent<RuntimePreview>("sofvary-runtime-preview", (preview) => {
        setPromptEnvelopeSummary(preview.promptEnvelopeSummary);
        refreshWorkspaces();
        refreshInstalledPacks();
      }),
      listenShellEvent<{
        kind: string;
        version?: string | null;
        state?: string;
        detail?: string;
      }>("sofvary-runtime-environment-install-updated", (update) => {
        const version = update.version ?? "";
        setRuntimeEnvironmentStatusOverride(
          update.detail ?? `${update.kind}${version ? ` ${version}` : ""} updated.`,
        );
        if (update.state !== "installing") {
          setActiveRuntimeEnvironmentInstallKey(null);
          refreshAgentInstalls();
          refreshAgents();
        }
        refreshRuntimeEnvironments();
      }),
      listenShellEvent<{ agentId: string; state?: string; detail?: string }>(
        "sofvary-agent-install-updated",
        (update) => {
          setAgentStatusOverride(
            update.detail
              ? formatAgentInstallDetailText(update.detail, t)
              : t("agent.status.installUpdated", { agentId: update.agentId }),
          );
          if (update.state !== "installing") {
            setActiveAgentInstallId((current) => (current === update.agentId ? null : current));
          }
          refreshAgentInstalls();
          refreshAgents();
        },
      ),
      listenShellEvent<{ agentId: string; message: string }>("sofvary-agent-install-log", (log) => {
        setAgentStatusOverride(`${log.agentId}: ${log.message}`);
      }),
    ];

    return () => {
      batcher.dispose();
      void Promise.all(unlisteners).then((listeners) => listeners.forEach((unlisten) => unlisten()));
    };
  }, [
    applyThreadBatch,
    refreshAgentInstalls,
    refreshAgents,
    refreshInstalledPacks,
    refreshRuntimeEnvironments,
    refreshWorkspaces,
    t,
  ]);

  useEffect(() => {
    void setCurrentWindowShadow(false).catch(() => {
      // Some platforms ignore shadow changes for transparent or decorated windows.
    });
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "escape" && !isPinned && !isActive) {
        void hideCommandWindow();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isActive, isPinned]);

  const requestPolicyApprovals = useCallback(
    async (payload: PreviewPolicyPayload, title: string): Promise<PolicyApprovalSet> => {
      const preview = await previewPolicy(payload);
      const confirmation = summarizePolicyDecisions(preview.decisions);

      if (confirmation.kind === "clear") {
        return emptyPolicyApprovals();
      }
      if (confirmation.kind === "forbidden") {
        throw new Error(formatPolicyBlockMessage(confirmation.decisions, t));
      }

      return new Promise((resolve, reject) => {
        setPolicyDialog({
          title,
          decisions: confirmation.decisions,
          resolve,
          reject,
        });
      });
    },
    [t],
  );

  const approvePolicyDialog = useCallback(() => {
    if (!policyDialog) {
      return;
    }

    policyDialog.resolve(buildPolicyApprovalSet(policyDialog.decisions));
    setPolicyDialog(null);
  }, [policyDialog]);

  const cancelPolicyDialog = useCallback(() => {
    if (!policyDialog) {
      return;
    }

    policyDialog.reject(new Error(POLICY_APPROVAL_CANCELED));
    setPolicyDialog(null);
  }, [policyDialog]);

  const exportWorkspace = async (workspace: WorkspaceSummary) => {
    setActiveCapsuleAppId(workspace.appId);
    setCapsuleStatus({ kind: "choosing-export", targetName: workspace.name });

    try {
      const outputPath = await selectExportCapsulePath(workspace);
      if (!outputPath) {
        setCapsuleStatus({ kind: "canceled", detail: "Export canceled." });
        return;
      }

      setCapsuleStatus({ kind: "exporting", targetName: workspace.name });
      await exportAppCapsule(workspace, outputPath);
      setCapsuleStatus({
        kind: "success",
        detail: `${workspace.name} exported as .sfcapsule.`,
      });
    } catch (error) {
      setCapsuleStatus({ kind: "error", detail: getCapsuleErrorMessage(error) });
    } finally {
      setActiveCapsuleAppId(null);
    }
  };

  const openReleaseWizard = (workspace: WorkspaceSummary) => {
    setActiveReleaseWorkspace(workspace);
    setReleaseStatus({
      kind: "ready",
      targetName: workspace.name,
      detail: "Fill release form and confirm policy before packaging.",
    });
    refreshReleaseCapabilities();
    void refreshPackagerToolchain(true);
  };

  const closeReleaseWizard = () => {
    if (isReleaseBusy) {
      return;
    }
    setActiveReleaseWorkspace(null);
    setActiveReleaseAppId(null);
    setReleaseStatus({ kind: "idle" });
  };

  const chooseReleaseOutputFolder = async (): Promise<string | null> => {
    setReleaseStatus({ kind: "choosing-output" });
    const selected = await selectReleaseOutputFolder();
    setReleaseStatus(
      selected
        ? { kind: "ready", detail: "Output folder selected." }
        : { kind: "canceled", detail: "Release output selection canceled." },
    );
    return selected;
  };

  const chooseReleaseIcon = async (): Promise<string | null> => {
    const selected = await selectReleaseIconPath();
    setReleaseStatus(
      selected
        ? { kind: "ready", detail: "Custom icon selected." }
        : { kind: "ready", detail: "Default Sofvary icon will be used." },
    );
    return selected;
  };

  const installPackagerToolchain = async () => {
    const targetPlatform = releaseCapabilities?.currentPlatform ?? packagerToolchainStatus?.platform ?? "windows";
    setReleaseStatus({ kind: "checking-toolchain" });
    try {
      const statuses = sortRuntimeEnvironmentStatuses(await getRuntimeEnvironmentStatuses());
      setRuntimeEnvironmentStatuses(statuses);
      const nodeStatus = statuses.find((status) => status.catalog.kind === "nodejs");
      const version = nodeStatus ? getDefaultRuntimeEnvironmentVersion(nodeStatus) : null;
      if (!nodeStatus || !version || !version.supported) {
        throw new Error("Sofvary-managed Node/pnpm is not available for this platform yet.");
      }
      const installKey = runtimeEnvironmentInstallKey(nodeStatus, version);
      setActiveRuntimeEnvironmentInstallKey(installKey);
      const policyApprovals = await requestPolicyApprovals(
        {
          scope: "runtime-environment-install",
          runtimeEnvironmentKind: "nodejs",
          version: version.version,
        },
        t("policy.dialog.installNodeToolchain", { version: version.version }),
      );
      setReleaseStatus({
        kind: "installing-toolchain",
        detail: `Installing Sofvary-managed Node.js ${version.version} and pnpm support.`,
      });
      const status = await startPackagerToolchainInstall(targetPlatform, policyApprovals);
      setPackagerToolchainStatus(status);
      setReleaseStatus({ kind: "ready", detail: status.detail });
      refreshRuntimeEnvironments();
      refreshAgents();
      refreshAgentInstalls();
    } catch (error) {
      if (error instanceof Error && error.message === POLICY_APPROVAL_CANCELED) {
        setReleaseStatus({ kind: "ready", detail: "Packager toolchain install canceled." });
      } else {
        setReleaseStatus({ kind: "error", detail: error instanceof Error ? error.message : String(error) });
      }
    } finally {
      setActiveRuntimeEnvironmentInstallKey(null);
    }
  };

  const publishWorkspace = async (input: {
    appName: string;
    targetPlatform: AppReleaseTargetPlatform;
    outputDir: string;
    iconPath: string | null;
    includeAiContinuation: boolean;
    stealthUi: {
      aiMenuLabel: string;
      aiShortcut: string;
      aiPanelTitle: string;
      providerSetupTitle: string;
      promptPlaceholder: string;
    };
  }) => {
    if (!activeReleaseWorkspace) return;

    setActiveReleaseAppId(activeReleaseWorkspace.appId);
    setReleaseStatus({ kind: "publishing", targetName: input.appName });
    try {
      const policyApprovals = await requestPolicyApprovals(
        {
          scope: "app-release",
          appId: activeReleaseWorkspace.appId,
          appName: input.appName,
          targetPlatform: input.targetPlatform,
          outputDir: input.outputDir,
          includeAiContinuation: input.includeAiContinuation,
          selectedPluginPacks: [],
        },
        t("policy.dialog.publishApp", { name: input.appName }),
      );
      const job = await startAppReleaseJob(
        buildAppReleasePayload({
          workspace: activeReleaseWorkspace,
          appName: input.appName,
          targetPlatform: input.targetPlatform,
          outputDir: input.outputDir,
          iconPath: input.iconPath,
          includeAiContinuation: input.includeAiContinuation,
          stealthUi: input.stealthUi,
          policyApprovals,
        }),
      );
      setReleaseStatus({
        kind: "success",
        targetName: input.appName,
        detail: `${job.appName} published: ${job.nativeInstallerPath ?? job.nativeAppPath ?? job.artifactPath ?? job.outputDir}.`,
      });
      await openAppReleaseOutputFolder(job.outputDir).catch(() => undefined);
    } catch (error) {
      if (error instanceof Error && error.message === POLICY_APPROVAL_CANCELED) {
        setReleaseStatus({ kind: "canceled", detail: "Release canceled before packaging." });
        return;
      }
      setReleaseStatus({ kind: "error", detail: error instanceof Error ? error.message : String(error) });
    } finally {
      setActiveReleaseAppId(null);
    }
  };

  const previewExistingWorkspace = async (workspace: WorkspaceSummary) => {
    setActivePreviewAppId(workspace.appId);
    setCapsuleStatus({ kind: "previewing", targetName: workspace.name });

    try {
      const preview = await previewWorkspace(workspace, "dev");
      setPromptEnvelopeSummary(preview.promptEnvelopeSummary);
      setCapsuleStatus({ kind: "success", detail: `${workspace.name} 已打开预览。` });
      await showMainWindow().catch(() => {
        // Browser-only Vite sessions cannot open native Tauri windows.
      });
      await emitShellEvent("sofvary-runtime-preview", preview);
      if (!isPinned) {
        await hideCommandWindow().catch(() => {});
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setCapsuleStatus({ kind: "error", detail: message });
      emitShellEventSafely("sofvary-runtime-error", message);
    } finally {
      setActivePreviewAppId(null);
    }
  };

  const repairPreviewBlockedThread = async (
    thread: BuildThreadSummary,
    workspaceName?: string,
  ) => {
    const appId = thread.appId ?? thread.workspaceId;
    setActivePreviewAppId(appId ?? null);
    setActiveThreadId(thread.id);
    setCapsuleStatus({
      kind: "previewing",
      targetName: workspaceName ?? thread.title,
      detail: t("workspace.repairingPreview"),
    });

    try {
      setRuntimePreflightMessage(t("workspace.repairingPreview"));
      const runtimeEnvironment = await ensureRuntimeEnvironmentReady(thread.runtimeKind);
      if (!runtimeEnvironment.ready) {
        setCapsuleStatus({
          kind: "error",
          detail: runtimeEnvironment.message ?? t("workspace.previewRepairPending"),
        });
        await getBuildThread(thread.id)
          .then(setActiveThreadDetail)
          .catch(() => undefined);
        return;
      }

      const policyApprovals = await requestPolicyApprovals(
        {
          scope: "runtime-build",
          runtimeKind: thread.runtimeKind,
          mode: thread.runtimeMode,
          agentId: thread.agentId,
        },
        t("policy.dialog.runRuntime", { runtimeKind: thread.runtimeKind }),
      );
      const result = await retryBuildThreadPreview(thread.id, policyApprovals);
      setPromptEnvelopeSummary(result.preview.promptEnvelopeSummary);
      setBuildThreads((current) => upsertBuildThreadSummary(current, result.thread));
      setActiveThreadDetail((current) =>
        current?.summary.id === result.thread.id
          ? { ...current, summary: result.thread }
          : current,
      );
      setCapsuleStatus({
        kind: "success",
        detail: `${workspaceName ?? thread.title} 已打开预览。`,
      });
      await showMainWindow().catch(() => {
        // Browser-only Vite sessions cannot open native Tauri windows.
      });
      refreshWorkspaces();
      if (!isPinned) {
        await hideCommandWindow().catch(() => {});
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message === POLICY_APPROVAL_CANCELED) {
        setCapsuleStatus({ kind: "canceled", detail: "Preview retry canceled." });
        return;
      }
      setCapsuleStatus({ kind: "error", detail: message });
    } finally {
      setActivePreviewAppId((current) => (current === appId ? null : current));
    }
  };

  const repairPreviewBlockedWorkspace = async (workspace: WorkspaceSummary) => {
    const thread = getWorkspaceBuildThread(workspace, buildThreadsRef.current);
    if (!thread || thread.status !== "preview-blocked") {
      await previewExistingWorkspace(workspace);
      return;
    }
    await repairPreviewBlockedThread(thread, workspace.name);
  };

  const modifyExistingWorkspace = async (workspace: WorkspaceSummary) => {
    const thread = getWorkspaceBuildThread(workspace, buildThreadsRef.current);
    if (!thread || !canContinueBuildThread(thread)) {
      setCapsuleStatus({
        kind: "error",
        detail: `${workspace.name} does not have a completed generation thread to continue.`,
      });
      return;
    }

    setRuntimeChoice(thread.runtimeKind);
    setSelectedAgentId(thread.agentId);
    setCreatePrompt("");
    setContinuePrompt("");
    setIntentSelection(null);
    setRuntimePreflightMessage(null);
    setPromptEnvelopeSummary(thread.preview?.promptEnvelopeSummary ?? null);
    setActiveAction("create");
    await selectBuildThread(thread.id);
  };

  const deleteExistingWorkspace = async (workspace: WorkspaceSummary) => {
    const confirmed = window.confirm(
      `Delete "${workspace.name}" and all related local files and records? This cannot be undone.`,
    );
    if (!confirmed) {
      setCapsuleStatus({ kind: "canceled", detail: "Delete canceled." });
      return;
    }

    setActiveCapsuleAppId(workspace.appId);
    setCapsuleStatus({ kind: "deleting", targetName: workspace.name });
    try {
      await deleteWorkspace(workspace);
      setWorkspaces((current) => current.filter((item) => item.appId !== workspace.appId));
      setBuildThreads((current) =>
        sortBuildThreads(
          current.filter(
            (thread) => thread.appId !== workspace.appId && thread.workspaceId !== workspace.appId,
          ),
        ),
      );
      setActiveThreadId((threadId) => {
        const thread = buildThreads.find((item) => item.id === threadId);
        return thread?.appId === workspace.appId || thread?.workspaceId === workspace.appId
          ? null
          : threadId;
      });
      setActiveThreadDetail((current) =>
        current?.summary.appId === workspace.appId || current?.summary.workspaceId === workspace.appId
          ? null
          : current,
      );
      setCapsuleStatus({ kind: "success", detail: `${workspace.name} deleted.` });
      refreshWorkspaces();
      refreshBuildThreads();
    } catch (error) {
      setCapsuleStatus({
        kind: "error",
        detail: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setActiveCapsuleAppId(null);
    }
  };

  const importCapsule = async () => {
    setCapsuleStatus({ kind: "choosing-import" });
    setActiveCapsuleAppId(null);

    try {
      const inputPath = await selectImportCapsulePath();
      if (!inputPath) {
        setCapsuleStatus({ kind: "canceled", detail: "Import canceled." });
        return;
      }

      const policyApprovals = await requestPolicyApprovals(
        { scope: "capsule-import", capsulePath: inputPath },
        t("policy.dialog.importCapsule"),
      );
      setCapsuleStatus({ kind: "importing" });
      await importAppCapsule(inputPath, policyApprovals);
      refreshWorkspaces();
      refreshInstalledPacks();
      setCapsuleStatus({ kind: "success", detail: "Capsule imported into local workspaces." });
    } catch (error) {
      if (error instanceof Error && error.message === POLICY_APPROVAL_CANCELED) {
        setCapsuleStatus({ kind: "canceled", detail: "Import canceled." });
        return;
      }
      setCapsuleStatus({ kind: "error", detail: getCapsuleErrorMessage(error) });
    }
  };

  const reviewDeepLink = async () => {
    setDeepLinkStatus({ kind: "reviewing" });
    setDeepLinkPreflight(null);

    try {
      const preflight = await prepareDeepLinkInstall(deepLinkValue);
      setDeepLinkPreflight(preflight);
      setDeepLinkStatus({
        kind: "ready",
        detail: `${preflight.app.name} v${preflight.version.version} is ready for permission review.`,
      });
    } catch (error) {
      setDeepLinkStatus({
        kind: "error",
        detail: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const installDeepLink = async () => {
    if (!deepLinkPreflight) {
      return;
    }

    setDeepLinkStatus({ kind: "installing" });
    try {
      const policyApprovals = await requestPolicyApprovals(
        {
          scope: "deep-link-install",
          mode: "dev",
          capsuleName: `${deepLinkPreflight.app.name} v${deepLinkPreflight.version.version}`,
          permissionSummary: deepLinkPreflight.permissionSummary,
        },
        t("policy.dialog.installSharedCapsule"),
      );
      const result = await installAppFromDeepLink(deepLinkValue, "dev", policyApprovals);
      refreshWorkspaces();
      refreshInstalledPacks();
      setDeepLinkPreflight(null);
      setDeepLinkStatus({
        kind: "success",
        detail: `${result.app.name} installed into a new workspace.`,
      });
      await emitShellEvent("sofvary-runtime-preview", result.preview);

      if (!isPinned) {
        await hideCommandWindow();
      }
    } catch (error) {
      if (error instanceof Error && error.message === POLICY_APPROVAL_CANCELED) {
        setDeepLinkStatus({
          kind: "ready",
          detail: "Install canceled before local changes.",
        });
        return;
      }
      setDeepLinkStatus({
        kind: "error",
        detail: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const clearDeepLink = () => {
    setDeepLinkValue("");
    setDeepLinkPreflight(null);
    setDeepLinkStatus({ kind: "idle" });
  };

  const updateDeepLinkValue = (value: string) => {
    setDeepLinkValue(value);
    setDeepLinkPreflight(null);
    setDeepLinkStatus({ kind: "idle" });
  };

  const addDiscoveredAgent = async (discovered: DiscoveredAgent) => {
    setAgentStatusOverride(t("agent.status.adding", { label: discovered.config.label }));
    try {
      const state = await upsertAgentConfig(discovered.config);
      setAgentState(state);
      setSelectedAgentId(discovered.config.id);
      setAgentStatusOverride(t("agent.status.added", { label: discovered.config.label }));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const toggleAgentEnabled = async (agent: AgentConfig) => {
    setAgentStatusOverride(
      agent.enabled
        ? t("agent.status.disabling", { label: agent.label })
        : t("agent.status.enabling", { label: agent.label }),
    );
    try {
      const state = await upsertAgentConfig({ ...agent, enabled: !agent.enabled });
      setAgentState(state);
      setAgentStatusOverride(null);
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const makeDefaultAgent = async (agentId: string) => {
    setAgentStatusOverride(t("agent.status.settingDefault"));
    try {
      const state = await setDefaultAgent(agentId);
      setAgentState(state);
      setSelectedAgentId(agentId);
      setAgentStatusOverride(null);
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const removeAgent = async (agentId: string) => {
    setAgentStatusOverride(t("agent.status.deleting"));
    try {
      const state = await deleteAgentConfig(agentId);
      setAgentState(state);
      setSelectedAgentId((current) => (current === agentId ? null : current));
      setAgentStatusOverride(null);
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const changeAgentMode = async (mode: AgentInteractionMode) => {
    const normalizedMode = normalizeAgentInteractionMode(activeAgent, mode);
    setSelectedAgentMode(normalizedMode);
    if (!activeAgent || activeAgent.provider === "sofvary-pi") {
      return;
    }
    if (activeAgent.defaultInteractionMode === normalizedMode) {
      return;
    }

    try {
      const state = await upsertAgentConfig({
        ...activeAgent,
        defaultInteractionMode: normalizedMode,
      });
      setAgentState(state);
      setAgentStatusOverride(formatAgentInteractionMode(normalizedMode, t));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const testAgent = async (agentId: string) => {
    setAgentStatusOverride(t("agent.status.testing"));
    try {
      const record = await testAgentConnection(agentId);
      const configs = await listAgentConfigs();
      setAgentState(configs);
      await refreshAgentInstallStatuses().then(setAgentInstallStatuses).catch(() => {});
      setAgentStatusOverride(formatAgentTestRecord(record, t));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const installAgent = async (status: AgentInstallStatus) => {
    const agentId = status.catalog.id;
    setActiveAgentInstallId(agentId);
    setAgentStatusOverride(t("agent.status.preparingInstall", { label: status.catalog.label }));
    try {
      const policyApprovals = await requestPolicyApprovals(
        { scope: "agent-install", agentId },
        t("policy.dialog.installAgent", { label: status.catalog.label }),
      );
      const updated = await startAgentInstall(agentId, policyApprovals);
      setAgentInstallStatuses((current) =>
        sortAgentInstallStatuses(
          [updated, ...current.filter((item) => item.catalog.id !== updated.catalog.id)],
          agentState,
        ),
      );
      refreshAgents();
      refreshAgentInstalls();
      setAgentStatusOverride(`${updated.catalog.label}: ${formatAgentInstallDetail(updated, t)}`);
    } catch (error) {
      if (error instanceof Error && error.message === POLICY_APPROVAL_CANCELED) {
        setAgentStatusOverride(t("agent.status.installCanceled"));
      } else {
        setAgentStatusOverride(error instanceof Error ? error.message : String(error));
      }
    } finally {
      setActiveAgentInstallId(null);
    }
  };

  const installRuntimeEnvironment = async (
    status: RuntimeEnvironmentStatus,
    version: RuntimeEnvironmentVersionOption,
  ): Promise<{ installed: boolean; message?: string }> => {
    const installKey = runtimeEnvironmentInstallKey(status, version);
    setActiveRuntimeEnvironmentInstallKey(installKey);
    setRuntimeEnvironmentStatusOverride(`Preparing ${status.catalog.label} ${version.version}`);
    try {
      const policyApprovals = await requestPolicyApprovals(
        {
          scope: "runtime-environment-install",
          runtimeEnvironmentKind: status.catalog.kind,
          version: version.version,
        },
        t("policy.dialog.installRuntimeEnvironment", {
          label: status.catalog.label,
          version: version.version,
        }),
      );
      const updated = await startRuntimeEnvironmentInstall(
        status.catalog.kind,
        version.version,
        policyApprovals,
      );
      setRuntimeEnvironmentStatuses((current) =>
        sortRuntimeEnvironmentStatuses([
          updated,
          ...current.filter((item) => item.catalog.kind !== updated.catalog.kind),
        ]),
      );
      refreshRuntimeEnvironments();
      refreshAgents();
      refreshAgentInstalls();
      setRuntimeEnvironmentStatusOverride(updated.detail);
      return { installed: true, message: updated.detail };
    } catch (error) {
      let message: string;
      if (error instanceof Error && error.message === POLICY_APPROVAL_CANCELED) {
        message = "Runtime environment install canceled.";
      } else {
        message = error instanceof Error ? error.message : String(error);
      }
      setRuntimeEnvironmentStatusOverride(message);
      return { installed: false, message };
    } finally {
      setActiveRuntimeEnvironmentInstallKey((current) =>
        current === installKey ? null : current,
      );
    }
  };

  const activateRuntimeEnvironmentVersion = async (
    status: RuntimeEnvironmentStatus,
    version: RuntimeEnvironmentVersionOption,
  ) => {
    setRuntimeEnvironmentStatusOverride(
      `Switching ${status.catalog.label} to ${version.version}`,
    );
    try {
      const updated = await setActiveRuntimeEnvironmentVersion(
        status.catalog.kind,
        version.version,
      );
      setRuntimeEnvironmentStatuses((current) =>
        sortRuntimeEnvironmentStatuses([
          updated,
          ...current.filter((item) => item.catalog.kind !== updated.catalog.kind),
        ]),
      );
      refreshRuntimeEnvironments();
      refreshAgentInstalls();
      setRuntimeEnvironmentStatusOverride(updated.detail);
    } catch (error) {
      setRuntimeEnvironmentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const cancelAgentInstallById = async (agentId: string) => {
    try {
      const statuses = await cancelAgentInstall(agentId);
      setAgentInstallStatuses(statuses);
      setActiveAgentInstallId((current) => (current === agentId ? null : current));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const openAgentInstallDocs = async (agentId: string) => {
    try {
      await openAgentInstallPage(agentId);
      setAgentStatusOverride(t("agent.status.openedInstallPage"));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const saveLlmProvider = async (config: LlmProviderConfig, apiKey?: string) => {
    setLlmProviderStatusOverride("正在保存 LLM Provider");
    try {
      const state = await upsertLlmProviderConfig(config, apiKey || undefined);
      setLlmProviderState(state);
      setLlmProviderStatusOverride("LLM Provider 已保存");
    } catch (error) {
      setLlmProviderStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const toggleLlmProviderEnabled = async (provider: LlmProviderConfig) => {
    setLlmProviderStatusOverride(provider.enabled ? "正在停用 Provider" : "正在启用 Provider");
    try {
      const state = await upsertLlmProviderConfig({ ...provider, enabled: !provider.enabled });
      setLlmProviderState(state);
      setLlmProviderStatusOverride(null);
    } catch (error) {
      setLlmProviderStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const makeDefaultLlmProvider = async (providerId: string) => {
    setLlmProviderStatusOverride("正在设置默认 LLM Provider");
    try {
      const state = await setDefaultLlmProvider(providerId);
      setLlmProviderState(state);
      setLlmProviderStatusOverride(null);
    } catch (error) {
      setLlmProviderStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const removeLlmProvider = async (providerId: string) => {
    setLlmProviderStatusOverride("正在删除 LLM Provider");
    try {
      const state = await deleteLlmProviderConfig(providerId);
      setLlmProviderState(state);
      setLlmProviderStatusOverride(null);
    } catch (error) {
      setLlmProviderStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const testLlmProvider = async (providerId: string) => {
    setLlmProviderStatusOverride("正在测试 LLM Provider");
    try {
      const record = await testLlmProviderConfig(providerId);
      setLlmProviderStatusOverride(record.detail);
    } catch (error) {
      setLlmProviderStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const ensureRuntimeEnvironmentReady = async (
    runtimeKind: RuntimeKind,
  ): Promise<{ ready: boolean; message?: string }> => {
    try {
      const statuses = sortRuntimeEnvironmentStatuses(await getRuntimeEnvironmentStatuses());
      setRuntimeEnvironmentStatuses(statuses);
      const issue = getRuntimeEnvironmentRequirementIssue(runtimeKind, statuses);
      if (!issue) {
        setRuntimePreflightMessage(null);
        return { ready: true };
      }

      setRuntimePreflightMessage(issue.message);
      setRuntimeEnvironmentStatusOverride(issue.message);
      const nodeStatus = statuses.find((status) => status.catalog.kind === issue.runtimeEnvironmentKind);
      const version = nodeStatus ? getDefaultRuntimeEnvironmentVersion(nodeStatus) : null;
      if (!nodeStatus || !version || !version.supported) {
        const message = "Sofvary-managed Node.js Toolchain is not available for this platform yet.";
        setRuntimePreflightMessage(message);
        setRuntimeEnvironmentStatusOverride(message);
        return { ready: false, message };
      }
      const installResult = await installRuntimeEnvironment(nodeStatus, version);
      if (!installResult.installed) {
        const message = installResult.message ?? issue.message;
        setRuntimePreflightMessage(message);
        return { ready: false, message };
      }
      const refreshed = sortRuntimeEnvironmentStatuses(await getRuntimeEnvironmentStatuses());
      setRuntimeEnvironmentStatuses(refreshed);
      const remainingIssue = getRuntimeEnvironmentRequirementIssue(runtimeKind, refreshed);
      if (remainingIssue) {
        setRuntimePreflightMessage(remainingIssue.message);
        setRuntimeEnvironmentStatusOverride(remainingIssue.message);
        return { ready: false, message: remainingIssue.message };
      }
      setRuntimePreflightMessage(null);
      return { ready: true };
    } catch (error) {
      const message = `Runtime environment check failed: ${
        error instanceof Error ? error.message : String(error)
      }`;
      setRuntimePreflightMessage(message);
      setRuntimeEnvironmentStatusOverride(message);
      return { ready: false, message };
    }
  };

  const startBuilding = async () => {
    setPromptEnvelopeSummary(null);
    setRuntimePreflightMessage(null);

    try {
      const agentId = getSelectedAgentId(selectedAgentId, agentState);
      if (!agentId) {
        throw new Error(t("agent.error.noEnabledAgent"));
      }
      const selectedRuntime =
        runtimeChoice === "auto" ? await analyzeBuildIntent(createPrompt) : null;
      if (selectedRuntime) {
        setIntentSelection(selectedRuntime);
      }
      const runtimeKind = selectedRuntime?.runtimeKind ?? runtimeChoice;
      if (runtimeKind === "auto") {
        throw new Error("Sofvary could not resolve a runtime for this request.");
      }

      const policyApprovals = await requestPolicyApprovals(
        { scope: "runtime-build", runtimeKind, mode: "dev", agentId },
        t("policy.dialog.runRuntime", { runtimeKind }),
      );

      setShellState("Planning");
      await showMainWindow().catch(() => {
        // Browser-only Vite sessions cannot open native Tauri windows.
      });
      emitShellEventSafely("sofvary-build-state", "Planning");
      if (!isPinned) {
        void hideCommandWindow().catch(() => {
          // The build host is already visible; command hide failures should not block the run.
        });
      }

      const thread = await startBuildThread(
        createPrompt,
        runtimeChoice === "auto" ? null : runtimeChoice,
        "dev",
        policyApprovals,
        agentId,
        activeAgentMode,
      );
      setActiveThreadId(thread.id);
      setBuildThreads((current) => upsertBuildThreadSummary(current, thread));
      setActiveAction("create");
      setShellState("CommandMenuVisible");
    } catch (buildError) {
      const message = buildError instanceof Error ? buildError.message : String(buildError);
      if (message === POLICY_APPROVAL_CANCELED) {
        setShellState("CommandMenuVisible");
        return;
      }
      setShellState("Error");
      await showMainWindow().catch(() => {
        // Browser-only Vite sessions cannot open native Tauri windows.
      });
      emitShellEventSafely("sofvary-runtime-error", message);
    }
  };

  const selectBuildThread = async (threadId: string) => {
    setActiveThreadId(threadId);
    await getBuildThread(threadId)
      .then(setActiveThreadDetail)
      .catch(() => setActiveThreadDetail(null));
  };

  const startNewBuildThreadDraft = () => {
    setActiveThreadId(null);
    setActiveThreadDetail(null);
    setCreatePrompt("");
    setContinuePrompt("");
    setIntentSelection(null);
    setRuntimePreflightMessage(null);
    setPromptEnvelopeSummary(null);
  };

  const continueActiveThread = async () => {
    if (!activeThread || !continuePrompt.trim()) return;
    setRuntimePreflightMessage(null);
    try {
      const policyApprovals = await requestPolicyApprovals(
        { scope: "runtime-build", runtimeKind: activeThread.runtimeKind, mode: activeThread.runtimeMode, agentId: activeThread.agentId },
        t("policy.dialog.continueRuntime", { runtimeKind: activeThread.runtimeKind }),
      );
      const thread = await continueBuildThread(
        activeThread.id,
        continuePrompt,
        policyApprovals,
        activeThread.agentMode,
      );
      setBuildThreads((current) => upsertBuildThreadSummary(current, thread));
      setActiveThreadId(thread.id);
      setContinuePrompt("");
      if (!isPinned) {
        void hideCommandWindow().catch(() => {});
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message !== POLICY_APPROVAL_CANCELED) {
        emitShellEventSafely("sofvary-runtime-error", message);
      }
    }
  };

  const cancelActiveThread = async () => {
    if (!activeThread) return;
    try {
      const thread = await cancelBuildThread(activeThread.id);
      setBuildThreads((current) => upsertBuildThreadSummary(current, thread));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const syncHandoffThread = (thread: BuildThreadSummary) => {
    setBuildThreads((current) => upsertBuildThreadSummary(current, thread));
    setActiveThreadId(thread.id);
    refreshActiveThread(thread.id);
  };

  const copyTextToClipboard = async (value: string) => {
    if (!navigator.clipboard?.writeText) {
      throw new Error("Clipboard API is not available in this window.");
    }
    await navigator.clipboard.writeText(value);
  };

  const copyActiveHandoffPrompt = async () => {
    if (!activeThread) return;
    try {
      const result = await copyHandoffPrompt(activeThread.id);
      await copyTextToClipboard(result.prompt);
      syncHandoffThread(result.thread);
      setAgentStatusOverride(t("task.handoff.copyPrompt"));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const copyActiveHandoffRepairPrompt = async () => {
    if (!activeThread) return;
    try {
      const result = await copyHandoffRepairPrompt(activeThread.id);
      await copyTextToClipboard(result.prompt);
      syncHandoffThread(result.thread);
      setAgentStatusOverride(t("task.handoff.copyRepairPrompt"));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const openActiveHandoffWorkspace = async () => {
    if (!activeThread) return;
    try {
      const result = await openHandoffWorkspace(activeThread.id);
      syncHandoffThread(result.thread);
      setAgentStatusOverride(t("task.handoff.openWorkspace"));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const openActiveHandoffAgent = async () => {
    if (!activeThread) return;
    try {
      const policyApprovals = await requestPolicyApprovals(
        {
          scope: "runtime-build",
          runtimeKind: activeThread.runtimeKind,
          mode: activeThread.runtimeMode,
          agentId: activeThread.agentId,
        },
        t("task.handoff.openAgent"),
      );
      const result = await openHandoffAgent(activeThread.id, policyApprovals);
      syncHandoffThread(result.thread);
      setAgentStatusOverride(t("task.handoff.openAgent"));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message !== POLICY_APPROVAL_CANCELED) {
        setAgentStatusOverride(message);
      }
    }
  };

  const rescanActiveHandoffWorkspace = async () => {
    if (!activeThread) return;
    try {
      const result = await rescanHandoffWorkspace(activeThread.id);
      syncHandoffThread(result.thread);
      if (result.preview) {
        setPromptEnvelopeSummary(result.preview.promptEnvelopeSummary);
        await showMainWindow().catch(() => {
          // Browser-only Vite sessions cannot open native Tauri windows.
        });
        await emitShellEvent("sofvary-runtime-preview", result.preview);
        refreshWorkspaces();
        if (!isPinned) {
          await hideCommandWindow().catch(() => {});
        }
      }
      setAgentStatusOverride(t("task.handoff.rescan"));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  const deleteThread = async (threadId: string) => {
    try {
      await deleteBuildThread(threadId);
      setBuildThreads((current) => sortBuildThreads(current.filter((thread) => thread.id !== threadId)));
      setActiveThreadId((current) => (current === threadId ? null : current));
      setActiveThreadDetail((current) => (current?.summary.id === threadId ? null : current));
    } catch (error) {
      setAgentStatusOverride(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <main
      className="command-shell"
      data-state={shellState}
      data-theme={uiAppearance.resolvedTheme}
      data-theme-preference={uiAppearance.preference}
      data-tauri-drag-region
      style={uiAppearance.cssVariables}
      {...uiAppearance.dataAttributes}
      onPointerDownCapture={startWindowDrag}
    >
      <section className="command-window-frame" data-tauri-drag-region>
        <header className="command-titlebar" data-tauri-drag-region>
          <div className="command-titlebar__extensions" data-no-drag>
            <div className="command-titlebar__extension-group command-titlebar__theme-controls">
              <button
                className="command-titlebar__button command-titlebar__theme-button"
                type="button"
                aria-label={themeToggleTitle}
                title={themeToggleTitle}
                data-theme-mode={uiAppearance.preference}
                onClick={() => uiAppearance.setThemePreference(nextThemePreference)}
              >
                <ThemeIcon aria-hidden="true" />
              </button>
            </div>
            <div className="command-titlebar__extension-group command-titlebar__window-controls">
              <div className="command-titlebar__account" data-no-drag>
                <button
                  className="command-titlebar__button command-titlebar__account-button"
                  type="button"
                  aria-label="Sofvary account"
                  title={accountTitle}
                  aria-expanded={accountCardOpen}
                  onClick={() => setAccountCardOpen((current) => !current)}
                >
                  {accountState.user ? (
                    <span className="command-titlebar__account-initials">
                      {accountInitials(accountState.user)}
                    </span>
                  ) : (
                    <User aria-hidden="true" />
                  )}
                </button>
                {accountCardOpen ? (
                  <div className="command-account-card" data-no-drag>
                    {accountState.user ? (
                      <>
                        <div className="command-account-card__identity">
                          <span className="command-account-card__avatar" aria-hidden="true">
                            {accountInitials(accountState.user)}
                          </span>
                          <div>
                            <strong>{accountState.user.displayName || accountState.user.username}</strong>
                            <span>{accountState.user.email}</span>
                            <small>{accountState.user.plan} · {accountState.user.role}</small>
                          </div>
                        </div>
                        <div className="command-account-card__actions">
                          <button type="button" onClick={() => void syncAccount()}>
                            <RefreshCw aria-hidden="true" />
                            {t("action.refresh")}
                          </button>
                          <button type="button" onClick={() => void openSofvaryWebsite("/dashboard")}>
                            <ExternalLink aria-hidden="true" />
                            {t("agent.officialSite")}
                          </button>
                          <button type="button" onClick={() => void openSofvaryWebsite("/support")}>
                            <LifeBuoy aria-hidden="true" />
                            {t("action.support", {}, "Support")}
                          </button>
                          <button type="button" onClick={() => void handleAccountLogout()}>
                            <LogOut aria-hidden="true" />
                            {t("nav.signOut")}
                          </button>
                        </div>
                      </>
                    ) : (
                      <>
                        <div className="command-account-card__header">
                          <strong>{accountMode === "login" ? t("auth.loginTitle") : t("auth.registerTitle")}</strong>
                          <span>{accountTitle}</span>
                        </div>
                        <label>
                          {t("auth.email")}
                          <input
                            value={accountEmail}
                            onChange={(event) => setAccountEmail(event.target.value)}
                            type="email"
                          />
                        </label>
                        <label>
                          {t("auth.password")}
                          <input
                            value={accountPassword}
                            onChange={(event) => setAccountPassword(event.target.value)}
                            type="password"
                          />
                        </label>
                        {accountMode === "register" ? (
                          <label>
                            {t("auth.username")}
                            <input
                              value={accountUsername}
                              onChange={(event) => setAccountUsername(event.target.value)}
                            />
                          </label>
                        ) : null}
                        <div className="command-account-card__actions">
                          <button type="button" onClick={() => void handleAccountSubmit()}>
                            {accountMode === "login" ? t("nav.signIn") : t("nav.register")}
                          </button>
                          <button
                            type="button"
                            onClick={() => setAccountMode((current) => (current === "login" ? "register" : "login"))}
                          >
                            {accountMode === "login" ? t("auth.createAccount") : t("auth.already")}
                          </button>
                          <button type="button" onClick={() => void openSofvaryWebsite("/auth/login")}>
                            <ExternalLink aria-hidden="true" />
                            {t("agent.officialSite")}
                          </button>
                        </div>
                      </>
                    )}
                  </div>
                ) : null}
              </div>
              <button
                className="command-titlebar__button"
                type="button"
                aria-label={isPinned ? "Unpin Sofvary window" : "Pin Sofvary window"}
                title={isPinned ? "Unpin" : "Pin"}
                aria-pressed={isPinned}
                onClick={() => setPinned((current) => !current)}
              >
                {isPinned ? <PinOff aria-hidden="true" /> : <Pin aria-hidden="true" />}
              </button>
              <button
                className="command-titlebar__button"
                type="button"
                aria-label="Minimize Sofvary window"
                title="Minimize"
                onClick={() => void minimizeCommandWindow({ hasActiveTask: isActive })}
              >
                <Minus aria-hidden="true" />
              </button>
              <button
                className="command-titlebar__button"
                type="button"
                aria-label="Maximize or restore Sofvary window"
                title="Maximize"
                onClick={() => void toggleCurrentWindowMaximize("command")}
              >
                <Square aria-hidden="true" />
              </button>
              <button
                className="command-titlebar__button command-titlebar__button--close"
                type="button"
                aria-label="Close Sofvary window"
                title="Close"
                onClick={() => void hideCommandWindow()}
              >
                <X aria-hidden="true" />
              </button>
            </div>
          </div>
          <div className="command-titlebar__brand" data-tauri-drag-region>
            <span className="command-titlebar__logo" aria-hidden="true" data-tauri-drag-region>
              <SofvaryBrandMark className="command-titlebar__logo-mark" />
            </span>
            <strong data-tauri-drag-region>Sofvary</strong>
          </div>
          <div className="command-titlebar__title" data-tauri-drag-region>
            <span data-tauri-drag-region>{navigationTitle(activeAction, t)}</span>
            <small data-tauri-drag-region>{statusLine}</small>
          </div>
        </header>
      <FloatingCommandMenu
          createPrompt={createPrompt}
          continuePrompt={continuePrompt}
          isActive={isActive || isPolicyBusy}
          isPinned={isPinned}
          statusLine={statusLine}
          activeAction={activeAction}
          runtimeChoice={runtimeChoice}
          intentSelection={intentSelection}
          runtimePreflightMessage={runtimePreflightMessage}
          agentState={agentState}
          discoveredAgents={discoveredAgents}
          agentInstallStatuses={sortedAgentInstallStatuses}
          activeAgentInstallId={activeAgentInstallId}
          runtimeEnvironmentStatuses={runtimeEnvironmentStatuses}
          activeRuntimeEnvironmentInstallKey={activeRuntimeEnvironmentInstallKey}
          runtimeEnvironmentStatusLine={runtimeEnvironmentStatusOverride}
          selectableAgents={selectableAgents}
          selectedAgentId={activeAgentId}
          selectedAgentMode={activeAgentMode}
          availableAgentModes={availableAgentModes}
          agentStatusLine={agentStatusLine}
          buildThreads={buildThreads}
          activeThread={activeThread}
          activeThreadDetail={activeThreadDetail}
          promptEnvelopeSummary={promptEnvelopeSummary}
          workspaces={workspaces}
          installedPacks={installedPacks}
          llmProviderState={llmProviderState}
          llmProviderStatusLine={llmProviderStatusLine}
          appearancePreferences={uiAppearance.preferences}
          themePreference={uiAppearance.preference}
          resolvedTheme={uiAppearance.resolvedTheme}
          systemTheme={uiAppearance.systemTheme}
          deepLinkValue={deepLinkValue}
          deepLinkStatusLine={formatDeepLinkStatus(deepLinkStatus)}
          deepLinkPreflight={deepLinkPreflight}
          packStatusLine={formatPackStatus(packStatus)}
          capsuleStatusLine={formatCapsuleStatus(capsuleStatus)}
          releaseStatusLine={formatReleaseStatus(releaseStatus)}
          isCapsuleBusy={isCapsuleBusy}
          isReleaseBusy={isReleaseBusy}
          isDeepLinkBusy={isDeepLinkBusy}
          activeCapsuleAppId={activeCapsuleAppId}
          activeReleaseAppId={activeReleaseAppId}
          activePreviewAppId={activePreviewAppId}
          onCreatePromptChange={(value) => {
            setCreatePrompt(value);
            setIntentSelection(null);
            setRuntimePreflightMessage(null);
          }}
          onContinuePromptChange={(value) => {
            setContinuePrompt(value);
            setRuntimePreflightMessage(null);
          }}
          onStart={startBuilding}
          onTogglePin={() => setPinned((current) => !current)}
          onAction={setActiveAction}
          onRuntimeChoiceChange={(choice) => {
            setRuntimeChoice(choice);
            setIntentSelection(null);
            setRuntimePreflightMessage(null);
          }}
          onAgentChange={setSelectedAgentId}
          onAgentModeChange={(mode) => void changeAgentMode(mode)}
          onSelectBuildThread={(threadId) => void selectBuildThread(threadId)}
          onStartNewBuildThreadDraft={startNewBuildThreadDraft}
          onContinueBuildThread={() => void continueActiveThread()}
          onCancelBuildThread={() => void cancelActiveThread()}
          onDeleteBuildThread={(threadId) => void deleteThread(threadId)}
          onRepairPreviewBlockedThread={(thread) => void repairPreviewBlockedThread(thread)}
          onCopyHandoffPrompt={() => void copyActiveHandoffPrompt()}
          onOpenHandoffWorkspace={() => void openActiveHandoffWorkspace()}
          onOpenHandoffAgent={() => void openActiveHandoffAgent()}
          onRescanHandoffWorkspace={() => void rescanActiveHandoffWorkspace()}
          onCopyHandoffRepairPrompt={() => void copyActiveHandoffRepairPrompt()}
          onAddDiscoveredAgent={addDiscoveredAgent}
          onToggleAgentEnabled={toggleAgentEnabled}
          onSetDefaultAgent={makeDefaultAgent}
          onDeleteAgent={removeAgent}
          onTestAgent={testAgent}
          onRefreshAgents={refreshAgents}
          onInstallAgent={(status) => void installAgent(status)}
          onCancelAgentInstall={(agentId) => void cancelAgentInstallById(agentId)}
          onOpenAgentInstallPage={(agentId) => void openAgentInstallDocs(agentId)}
          onRefreshAgentInstallStatuses={refreshAgentInstalls}
          onInstallRuntimeEnvironment={(status, version) =>
            void installRuntimeEnvironment(status, version)
          }
          onActivateRuntimeEnvironmentVersion={(status, version) =>
            void activateRuntimeEnvironmentVersion(status, version)
          }
          onRefreshRuntimeEnvironments={refreshRuntimeEnvironments}
          onSaveLlmProvider={(config, apiKey) => void saveLlmProvider(config, apiKey)}
          onToggleLlmProviderEnabled={(provider) => void toggleLlmProviderEnabled(provider)}
          onSetDefaultLlmProvider={(providerId) => void makeDefaultLlmProvider(providerId)}
          onDeleteLlmProvider={(providerId) => void removeLlmProvider(providerId)}
          onTestLlmProvider={(providerId) => void testLlmProvider(providerId)}
          onRefreshLlmProviders={refreshLlmProviders}
          onAppearanceChange={uiAppearance.setPreferences}
          onThemePreferenceChange={uiAppearance.setThemePreference}
          onDeepLinkChange={updateDeepLinkValue}
          onReviewDeepLink={reviewDeepLink}
          onInstallDeepLink={installDeepLink}
          onClearDeepLink={clearDeepLink}
          onPreviewWorkspace={previewExistingWorkspace}
          onRepairWorkspacePreview={(workspace) => void repairPreviewBlockedWorkspace(workspace)}
          onModifyWorkspace={(workspace) => void modifyExistingWorkspace(workspace)}
          onExportWorkspace={exportWorkspace}
          onReleaseWorkspace={openReleaseWizard}
          onDeleteWorkspace={(workspace) => void deleteExistingWorkspace(workspace)}
          onImportCapsule={importCapsule}
          onRefreshPacks={refreshInstalledPacks}
        />
      </section>
      {activeReleaseWorkspace ? (
        <ReleaseWizard
          workspace={activeReleaseWorkspace}
          capabilities={releaseCapabilities}
          toolchainStatus={packagerToolchainStatus}
          releaseStatus={releaseStatus}
          statusLine={formatReleaseStatus(releaseStatus)}
          busy={isReleaseBusy}
          onClose={closeReleaseWizard}
          onRefreshToolchain={() => void refreshPackagerToolchain(true)}
          onInstallToolchain={() => void installPackagerToolchain()}
          onSelectOutputFolder={chooseReleaseOutputFolder}
          onSelectIcon={chooseReleaseIcon}
          onSubmit={(input) => void publishWorkspace(input)}
        />
      ) : null}
      {policyDialog ? (
        <PermissionDialog
          title={policyDialog.title}
          decisions={policyDialog.decisions}
          onApprove={approvePolicyDialog}
          onCancel={cancelPolicyDialog}
        />
      ) : null}
    </main>
  );
}

function emitShellEventSafely<T>(eventName: ShellEventName, payload: T) {
  void emitShellEvent(eventName, payload).catch(() => {
    // Cross-window shell events are best-effort; local command state must keep moving.
  });
}

function mergeFetchedThreadDetail(
  current: BuildThreadDetail | null,
  fetched: BuildThreadDetail,
): BuildThreadDetail {
  if (!current || current.summary.id !== fetched.summary.id) {
    return fetched;
  }
  return {
    summary: fetched.summary,
    entries: mergeBuildThreadEntries([...fetched.entries, ...current.entries]),
  };
}

function navigationTitle(
  key: NavigationKey,
  t: (key: string, params?: Record<string, string | number | boolean | null | undefined>, fallback?: string) => string,
): string {
  switch (key) {
    case "apps":
      return t("nav.apps");
    case "marketplace":
      return t("nav.marketplace");
    case "settings":
      return t("nav.settings");
    case "create":
    default:
      return t("nav.create");
  }
}
