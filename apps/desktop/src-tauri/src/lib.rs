mod core;
mod platform;

use crate::core::acp_client::test_acp_connection;
use crate::core::agent_catalog::{discover_agents as discover_agent_catalog, DiscoveredAgent};
use crate::core::agent_cli_bridge::test_cli_agent;
use crate::core::agent_config::{
    fresh_test_record, AgentCommandConfig, AgentConfig, AgentConfigState, AgentConfigStore,
    AgentInteractionMode, AgentProvider, AgentTestRecord, AgentTransportKind,
};
use crate::core::agent_gateway::{AgentEvent, AgentEventSink};
use crate::core::agent_install::{
    agent_install_subject_for_id, get_agent_install_statuses as get_agent_install_statuses_core,
    list_agent_install_catalog as list_agent_install_catalog_core,
    open_agent_install_page as open_agent_install_page_core,
    start_agent_install as start_agent_install_core, AgentInstallCatalogItem, AgentInstallStatus,
    StartAgentInstallPayload,
};
use crate::core::app_capsule::{
    capsule_policy_request, export_app_capsule as export_capsule_core,
    import_app_capsule as import_capsule_core, inspect_app_capsule_bytes_with_adapter,
    ExportAppCapsulePayload, ExportAppCapsuleResult, ImportAppCapsulePayload,
    ImportAppCapsuleResult,
};
use crate::core::app_release::{
    cancel_app_release_job as cancel_app_release_job_core,
    get_app_release_capabilities as get_app_release_capabilities_core,
    policy_request_for_preview as app_release_policy_request_for_preview,
    start_app_release_job as start_app_release_job_core, AppReleaseCapability, AppReleaseJob,
    AppReleasePayload, CancelAppReleaseJobPayload, OpenAppReleaseOutputFolderPayload,
};
use crate::core::build_thread::{
    build_thread_title, BuildThreadDetail, BuildThreadEntry, BuildThreadEntryKind,
    BuildThreadStatus, BuildThreadStore, BuildThreadSummary, BuildThreadUpdate,
    CreateBuildThreadRequest,
};
use crate::core::cloud_config::sofvary_web_base_url_from_env;
use crate::core::deep_link_install::{
    install_app_from_deep_link as install_deep_link_core,
    prepare_deep_link_install as prepare_deep_link_core, ConfirmDeepLinkInstallPayload,
    DeepLinkInstallPayload, DeepLinkInstallPreflight, DeepLinkInstallResult,
    InstallPermissionSummary,
};
use crate::core::file_processor_runtime::{
    confirm_dry_run_plan as confirm_file_processor_plan,
    record_selected_files as record_file_processor_selected_files_log,
    FileProcessorDryRunOperation, FileProcessorSelectedFileMetadata,
};
use crate::core::gateway_uni_event::{
    gateway_uni_event_summary, GatewayUniEvent, GatewayUniEventEmitter, GatewayUniEventSink,
    GatewayUniEventType,
};
use crate::core::llm_provider_config::{
    fresh_llm_test_record, LlmProviderConfig, LlmProviderConfigError, LlmProviderConfigState,
    LlmProviderConfigStore, LlmProviderKind, LlmProviderTestRecord, UpsertLlmProviderPayload,
};
use crate::core::pack_manager::{InstalledPackSummary, PackManager};
use crate::core::pack_registry::{
    InstallRegistryPackPayload, InstallRegistryPackResult, PackRegistryInstaller,
    RegistryResolveResponse, ResolveRegistryPackPayload,
};
use crate::core::packager_toolchain::{
    get_packager_toolchain_status as get_packager_toolchain_status_core,
    start_packager_toolchain_install as start_packager_toolchain_install_core,
    PackagerToolchainStatus, StartPackagerToolchainInstallPayload,
};
use crate::core::pi_agent::test_pi_agent;
use crate::core::policy_engine::PolicyEngine;
use crate::core::policy_types::{
    PolicyActionKind, PolicyAgentInstallRequest, PolicyApprovalSet, PolicyCapsuleImportRequest,
    PolicyCommandRequest, PolicyDecision, PolicyExternalAgentProcessRequest,
    PolicyPackInstallRequest, PolicyRuntimeEnvironmentInstallRequest,
    PolicyWorkspaceLockfileUpdateRequest,
};
use crate::core::runtime_dependency_install::dependency_install_policy_preview_specs;
use crate::core::runtime_environment::{
    get_runtime_environment_statuses as get_runtime_environment_statuses_core,
    list_runtime_environment_catalog as list_runtime_environment_catalog_core,
    resolve_node_toolchain_with_adapter, runtime_environment_install_subject_for,
    set_active_runtime_environment_version as set_active_runtime_environment_version_core,
    start_runtime_environment_install as start_runtime_environment_install_core,
    RuntimeEnvironmentCatalogItem, RuntimeEnvironmentKind, RuntimeEnvironmentStatus,
    SetActiveRuntimeEnvironmentPayload, StartRuntimeEnvironmentInstallPayload,
};
use crate::core::runtime_manager::{
    runtime_preview_issue_from_diagnostic, RuntimeManager, RuntimeManagerError, RuntimePreview,
};
use crate::core::runtime_selector::{
    manual_runtime_selection, select_runtime_for_intent, RuntimeIntentSelection,
};
use crate::core::skill_registry::{
    InstallRegistrySkillPayload, InstalledSkillSummary, SkillRegistryInstaller,
};
use crate::core::workspace_handoff::{
    append_handoff_request_consumed, prepare_existing_handoff_workspace,
    prepare_new_handoff_workspace, read_handoff_envelope, read_handoff_prompt,
    read_handoff_repair_prompt, write_handoff_diagnostics, WorkspaceHandoffPreparation,
};
use crate::core::workspace_handoff_watcher::{
    scan_handoff_workspace, wait_for_handoff_assets, HandoffScanResult,
};
use crate::core::workspace_manager::WorkspaceManager;
use crate::core::workspace_types::{
    AppBoxManifest, RuntimeKind, RuntimeMode, SnapshotSummary, WorkspaceSummary,
};
use crate::platform::host_shell::{
    command_window_position, command_window_size, fit_window_size_to_work_area,
    global_shortcut_sequence, glyph_window_position, snap_command_window_position,
    tray_or_menu_bar_available, ShortcutKey, ShortcutModifier, WindowPosition, WindowSize,
    GLYPH_WINDOW,
};
use crate::platform::{
    current_adapter, ArchKind, CommandSpec, OsKind, PlatformDirs, WebviewProfile,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::image::Image;
use tauri::menu::{IconMenuItem, Menu, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri::{PhysicalPosition, PhysicalSize, Position, Size};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

const ACCOUNT_REFRESH_TOKEN_KEY: &str = "sofvary.account.refresh_token";

pub struct AppState {
    workspace_manager: WorkspaceManager,
    runtime_manager: Arc<RuntimeManager>,
    pack_manager: PackManager,
    agent_store: AgentConfigStore,
    build_thread_store: BuildThreadStore,
    llm_provider_store: LlmProviderConfigStore,
    last_shortcut_start_at: Mutex<Option<Instant>>,
    deep_link_install_lock: Mutex<()>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            workspace_manager: WorkspaceManager::new(),
            runtime_manager: Arc::new(RuntimeManager::new()),
            pack_manager: PackManager::new().expect("failed to initialize Sofvary pack manager"),
            agent_store: AgentConfigStore::new(),
            build_thread_store: BuildThreadStore::new(),
            llm_provider_store: LlmProviderConfigStore::new(),
            last_shortcut_start_at: Mutex::new(None),
            deep_link_install_lock: Mutex::new(()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewWorkspacePayload {
    app_id: String,
    mode: Option<RuntimeMode>,
    #[serde(default)]
    policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetryBuildThreadPreviewPayload {
    thread_id: String,
    #[serde(default)]
    policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RetryBuildThreadPreviewResult {
    thread: BuildThreadSummary,
    preview: RuntimePreview,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountRefreshTokenPayload {
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunGeneratedAppPayload {
    requirement: String,
    #[serde(default)]
    runtime_kind: Option<RuntimeKind>,
    mode: Option<RuntimeMode>,
    agent_id: Option<String>,
    #[serde(default)]
    policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartBuildThreadPayload {
    requirement: String,
    #[serde(default)]
    runtime_kind: Option<RuntimeKind>,
    mode: Option<RuntimeMode>,
    agent_id: Option<String>,
    #[serde(default)]
    agent_mode: Option<AgentInteractionMode>,
    #[serde(default)]
    policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalyzeBuildIntentPayload {
    requirement: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinueBuildThreadPayload {
    thread_id: String,
    prompt: String,
    #[serde(default)]
    agent_mode: Option<AgentInteractionMode>,
    #[serde(default)]
    policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HandoffPromptCopyResult {
    thread: BuildThreadSummary,
    prompt: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HandoffActionResult {
    thread: BuildThreadSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HandoffRescanResult {
    thread: BuildThreadSummary,
    scan: HandoffScanResult,
    #[serde(default)]
    preview: Option<RuntimePreview>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PolicyPreviewScope {
    RuntimeBuild,
    CapsuleImport,
    DeepLinkInstall,
    PackInstall,
    AgentInstall,
    RuntimeEnvironmentInstall,
    AppRelease,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewPolicyPayload {
    scope: PolicyPreviewScope,
    runtime_kind: Option<RuntimeKind>,
    mode: Option<RuntimeMode>,
    agent_id: Option<String>,
    pack_kind: Option<String>,
    pack_id: Option<String>,
    runtime_environment_kind: Option<RuntimeEnvironmentKind>,
    version: Option<String>,
    app_id: Option<String>,
    app_name: Option<String>,
    target_platform: Option<String>,
    output_dir: Option<PathBuf>,
    #[serde(default)]
    include_ai_continuation: bool,
    #[serde(default)]
    selected_plugin_packs: Vec<String>,
    capsule_path: Option<PathBuf>,
    capsule_name: Option<String>,
    permission_summary: Option<InstallPermissionSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyPreview {
    decisions: Vec<PolicyDecision>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformBootstrap {
    os: OsKind,
    arch: ArchKind,
    dirs: PlatformDirs,
    webview: WebviewProfile,
    shortcut: String,
    tray_or_menu_bar_available: bool,
}

#[tauri::command]
fn bootstrap_platform() -> Result<PlatformBootstrap, String> {
    let adapter = current_adapter();
    let shortcut = global_shortcut_sequence(adapter.os());

    Ok(PlatformBootstrap {
        os: adapter.os(),
        arch: adapter.arch(),
        dirs: adapter.dirs().map_err(|error| error.to_string())?,
        webview: adapter.current_webview_profile(),
        shortcut: shortcut.display.to_string(),
        tray_or_menu_bar_available: tray_or_menu_bar_available(adapter.os()),
    })
}

#[tauri::command]
fn simulate_global_shortcut() -> bool {
    true
}

#[tauri::command]
fn show_command_window(app: tauri::AppHandle) -> Result<(), String> {
    show_command_window_for_app(&app)
}

#[tauri::command]
fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    show_main_window_for_app(&app)
}

#[tauri::command]
fn hide_command_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("command")
        .ok_or_else(|| "Sofvary command window is not available".to_string())?;
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn minimize_command_window(app: tauri::AppHandle) -> Result<(), String> {
    hide_window(&app, "command")?;
    show_glyph_window_for_app(&app)
}

#[tauri::command]
fn start_window_drag(app: tauri::AppHandle, label: String) -> Result<(), String> {
    let window = app
        .get_webview_window(resolve_shell_window_label(&label)?)
        .ok_or_else(|| format!("Sofvary window '{label}' is not available"))?;
    let result = window.start_dragging().map_err(|error| error.to_string());
    if label == "command" {
        let _ = snap_command_window_to_right_if_near(&app);
    }
    result
}

#[tauri::command]
fn minimize_shell_window(app: tauri::AppHandle, label: String) -> Result<(), String> {
    let label = resolve_shell_window_label(&label)?;
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Sofvary window '{label}' is not available"))?;
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_shell_window(app: tauri::AppHandle, label: String) -> Result<(), String> {
    hide_window(&app, resolve_shell_window_label(&label)?)
}

#[tauri::command]
fn toggle_shell_window_maximize(app: tauri::AppHandle, label: String) -> Result<(), String> {
    let label = resolve_shell_window_label(&label)?;
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Sofvary window '{label}' is not available"))?;
    if window.is_maximized().map_err(|error| error.to_string())? {
        window.unmaximize().map_err(|error| error.to_string())
    } else {
        window.maximize().map_err(|error| error.to_string())
    }
}

#[tauri::command]
fn snap_command_window(app: tauri::AppHandle) -> Result<(), String> {
    snap_command_window_to_right_if_near(&app)
}

#[tauri::command]
fn create_workspace(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<AppBoxManifest, String> {
    state
        .workspace_manager
        .create_workspace(name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_workspaces(state: tauri::State<'_, AppState>) -> Result<Vec<WorkspaceSummary>, String> {
    state
        .workspace_manager
        .list_workspaces()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_workspace(
    state: tauri::State<'_, AppState>,
    app_id: String,
) -> Result<AppBoxManifest, String> {
    state
        .workspace_manager
        .get_workspace(app_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_workspace(
    state: tauri::State<'_, AppState>,
    app_id: String,
    name: String,
) -> Result<AppBoxManifest, String> {
    state
        .workspace_manager
        .rename_workspace(app_id, name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_workspace(
    state: tauri::State<'_, AppState>,
    app_id: String,
) -> Result<AppBoxManifest, String> {
    let _ = state
        .runtime_manager
        .stop_active_app(&app_id)
        .map_err(|error| error.to_string())?;
    let manifest = state
        .workspace_manager
        .delete_workspace(app_id.clone())
        .map_err(|error| error.to_string())?;
    state
        .build_thread_store
        .delete_for_app(&app_id)
        .map_err(|error| error.to_string())?;
    Ok(manifest)
}

#[tauri::command]
fn create_snapshot(
    state: tauri::State<'_, AppState>,
    app_id: String,
) -> Result<SnapshotSummary, String> {
    state
        .workspace_manager
        .create_snapshot(app_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_snapshots(
    state: tauri::State<'_, AppState>,
    app_id: String,
) -> Result<Vec<SnapshotSummary>, String> {
    state
        .workspace_manager
        .list_snapshots(app_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rollback_snapshot(
    state: tauri::State<'_, AppState>,
    app_id: String,
    snapshot_id: String,
) -> Result<AppBoxManifest, String> {
    state
        .workspace_manager
        .rollback_snapshot(app_id, snapshot_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn run_fake_static_app(
    state: tauri::State<'_, AppState>,
    requirement: String,
) -> Result<RuntimePreview, String> {
    state
        .runtime_manager
        .build_and_preview_static_app(requirement, &state.workspace_manager)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn run_generated_app(
    state: tauri::State<'_, AppState>,
    payload: RunGeneratedAppPayload,
) -> Result<RuntimePreview, String> {
    let runtime_manager = state.runtime_manager.clone();
    let workspace_manager = state.workspace_manager;
    let agent_store = state.agent_store.clone();
    let llm_provider_store = state.llm_provider_store;

    tauri::async_runtime::spawn_blocking(move || {
        let runtime_selection =
            runtime_selection_for_payload(&payload.requirement, payload.runtime_kind);
        ensure_runtime_environment_for_runtime(&runtime_selection.runtime_kind)?;
        let agent_config = agent_store
            .resolve_agent(payload.agent_id.as_deref())
            .map_err(|error| error.to_string())?;
        let agent_config = hydrate_agent_config_for_runtime(agent_config, &llm_provider_store)
            .map_err(|error| error.to_string())?;
        runtime_manager
            .build_and_preview_app_with_agent_policy(
                payload.requirement,
                runtime_selection.runtime_kind,
                payload.mode.unwrap_or_default(),
                &workspace_manager,
                &payload.policy_approvals,
                &agent_config,
            )
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("runtime build task failed: {error}"))?
}

#[tauri::command]
fn analyze_build_intent(payload: AnalyzeBuildIntentPayload) -> RuntimeIntentSelection {
    select_runtime_for_intent(&payload.requirement)
}

#[tauri::command]
fn start_build_thread(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    payload: StartBuildThreadPayload,
) -> Result<BuildThreadSummary, String> {
    let agent_config = state
        .agent_store
        .resolve_agent(payload.agent_id.as_deref())
        .map_err(|error| error.to_string())?;
    let agent_config = hydrate_agent_config_for_runtime(agent_config, &state.llm_provider_store)
        .map_err(|error| error.to_string())?;
    let agent_mode = resolve_agent_interaction_mode(payload.agent_mode, &agent_config);
    let mode = payload.mode.unwrap_or_default();
    let runtime_selection =
        runtime_selection_for_payload(&payload.requirement, payload.runtime_kind);
    let runtime_kind = runtime_selection.runtime_kind.clone();
    let thread = state
        .build_thread_store
        .create(CreateBuildThreadRequest {
            title: build_thread_title(&payload.requirement),
            prompt: payload.requirement.clone(),
            runtime_kind: runtime_kind.clone(),
            runtime_mode: mode,
            agent_id: agent_config.id.clone(),
            agent_mode,
        })
        .map_err(|error| error.to_string())?;
    emit_build_thread_updated(&app, &thread.summary);
    if let Some(entry) = thread.entries.last() {
        emit_build_thread_entry(&app, entry);
    }
    let selection_entry = state
        .build_thread_store
        .append_entry(
            &thread.summary.id,
            BuildThreadEntryKind::System,
            runtime_selection_message(&runtime_selection),
            serde_json::json!({
                "source": "runtime-selection",
                "runtimeSelection": runtime_selection
            }),
        )
        .map_err(|error| error.to_string())?;
    emit_build_thread_entry(&app, &selection_entry);

    let runtime_manager = state.runtime_manager.clone();
    let workspace_manager = state.workspace_manager;
    let build_thread_store = state.build_thread_store;
    let thread_id = thread.summary.id.clone();
    let requirement = payload.requirement;
    let policy_approvals = payload.policy_approvals;
    tauri::async_runtime::spawn_blocking(move || {
        run_build_thread_task(
            app,
            build_thread_store,
            runtime_manager,
            workspace_manager,
            thread_id,
            requirement,
            runtime_kind,
            mode,
            policy_approvals,
            agent_config,
            agent_mode,
            BuildThreadRunContext::NewApp,
        );
    });

    Ok(thread.summary)
}

#[tauri::command]
fn list_build_threads(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BuildThreadSummary>, String> {
    state
        .build_thread_store
        .list()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_build_thread(
    state: tauri::State<'_, AppState>,
    thread_id: String,
) -> Result<BuildThreadDetail, String> {
    state
        .build_thread_store
        .get(&thread_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_build_thread(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    thread_id: String,
    title: String,
) -> Result<BuildThreadSummary, String> {
    let summary = state
        .build_thread_store
        .update(
            &thread_id,
            BuildThreadUpdate {
                title: Some(title),
                ..BuildThreadUpdate::default()
            },
        )
        .map_err(|error| error.to_string())?;
    emit_build_thread_updated(&app, &summary);
    Ok(summary)
}

#[tauri::command]
fn delete_build_thread(state: tauri::State<'_, AppState>, thread_id: String) -> Result<(), String> {
    state
        .build_thread_store
        .delete(&thread_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn continue_build_thread(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    payload: ContinueBuildThreadPayload,
) -> Result<BuildThreadSummary, String> {
    let detail = state
        .build_thread_store
        .get(&payload.thread_id)
        .map_err(|error| error.to_string())?;
    let continuation_app_id = detail
        .summary
        .app_id
        .clone()
        .or_else(|| detail.summary.workspace_id.clone())
        .ok_or_else(|| "当前任务还没有可继续修改的本地软件工程".to_string())?;
    state
        .workspace_manager
        .get_workspace(continuation_app_id.clone())
        .map_err(|error| error.to_string())?;
    let runtime_kind = detail.summary.runtime_kind;
    let agent_config = state
        .agent_store
        .resolve_agent(Some(&detail.summary.agent_id))
        .map_err(|error| error.to_string())?;
    let agent_config = hydrate_agent_config_for_runtime(agent_config, &state.llm_provider_store)
        .map_err(|error| error.to_string())?;
    let agent_mode = resolve_agent_interaction_mode(
        payload.agent_mode.or(Some(detail.summary.agent_mode)),
        &agent_config,
    );
    let entry = state
        .build_thread_store
        .append_entry(
            &payload.thread_id,
            BuildThreadEntryKind::User,
            payload.prompt.clone(),
            serde_json::json!({ "source": "continue", "appId": continuation_app_id.clone() }),
        )
        .map_err(|error| error.to_string())?;
    emit_build_thread_entry(&app, &entry);
    let summary = state
        .build_thread_store
        .update(
            &payload.thread_id,
            BuildThreadUpdate {
                status: Some(BuildThreadStatus::Queued),
                error: Some(None),
                ..BuildThreadUpdate::default()
            },
        )
        .map_err(|error| error.to_string())?;
    emit_build_thread_updated(&app, &summary);

    let runtime_manager = state.runtime_manager.clone();
    let workspace_manager = state.workspace_manager;
    let build_thread_store = state.build_thread_store;
    let thread_id = payload.thread_id;
    let runtime_mode = detail.summary.runtime_mode;
    tauri::async_runtime::spawn_blocking(move || {
        run_build_thread_task(
            app,
            build_thread_store,
            runtime_manager,
            workspace_manager,
            thread_id,
            payload.prompt,
            runtime_kind,
            runtime_mode,
            payload.policy_approvals,
            agent_config,
            agent_mode,
            BuildThreadRunContext::ExistingApp {
                app_id: continuation_app_id,
            },
        );
    });
    Ok(summary)
}

#[tauri::command]
fn cancel_build_thread(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    thread_id: String,
) -> Result<BuildThreadSummary, String> {
    // Phase 1 cancellation marks the task as canceled and prevents later preview handoff.
    // Process-level Agent/runtime interruption is handled by the future BuildTaskRunner.
    let entry = state
        .build_thread_store
        .append_entry(
            &thread_id,
            BuildThreadEntryKind::System,
            "Build thread cancellation requested.",
            serde_json::json!({}),
        )
        .map_err(|error| error.to_string())?;
    emit_build_thread_entry(&app, &entry);
    let summary = state
        .build_thread_store
        .update(
            &thread_id,
            BuildThreadUpdate {
                status: Some(BuildThreadStatus::Canceled),
                ..BuildThreadUpdate::default()
            },
        )
        .map_err(|error| error.to_string())?;
    emit_build_thread_updated(&app, &summary);
    let _ = app.emit("sofvary-build-state", "BackgroundIdle");
    Ok(summary)
}

#[tauri::command]
fn copy_handoff_prompt(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    thread_id: String,
) -> Result<HandoffPromptCopyResult, String> {
    let (thread, manifest) = handoff_thread_manifest(&state, &thread_id)?;
    let prompt = read_handoff_prompt(&manifest).map_err(|error| error.to_string())?;
    if let Ok(entry) = state.build_thread_store.append_entry(
        &thread_id,
        BuildThreadEntryKind::System,
        "Workspace Handoff prompt copied.",
        serde_json::json!({ "kind": "workspace-handoff-prompt-copied" }),
    ) {
        emit_build_thread_entry(&app, &entry);
    }
    Ok(HandoffPromptCopyResult { thread, prompt })
}

#[tauri::command]
fn copy_handoff_repair_prompt(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    thread_id: String,
) -> Result<HandoffPromptCopyResult, String> {
    let (thread, manifest) = handoff_thread_manifest(&state, &thread_id)?;
    let prompt = read_handoff_repair_prompt(&manifest).map_err(|error| error.to_string())?;
    if let Ok(entry) = state.build_thread_store.append_entry(
        &thread_id,
        BuildThreadEntryKind::System,
        "Workspace Handoff repair prompt copied.",
        serde_json::json!({ "kind": "workspace-handoff-repair-prompt-copied" }),
    ) {
        emit_build_thread_entry(&app, &entry);
    }
    Ok(HandoffPromptCopyResult { thread, prompt })
}

#[tauri::command]
fn open_handoff_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    thread_id: String,
) -> Result<HandoffActionResult, String> {
    let (thread, manifest) = handoff_thread_manifest(&state, &thread_id)?;
    current_adapter()
        .reveal_path(&manifest.paths.root)
        .map_err(|error| error.to_string())?;
    if let Ok(entry) = state.build_thread_store.append_entry(
        &thread_id,
        BuildThreadEntryKind::System,
        "Workspace Handoff folder opened.",
        serde_json::json!({
            "kind": "workspace-handoff-workspace-opened",
            "workspaceRoot": manifest.paths.root,
        }),
    ) {
        emit_build_thread_entry(&app, &entry);
    }
    Ok(HandoffActionResult { thread })
}

#[tauri::command]
fn open_handoff_agent(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    thread_id: String,
    policy_approvals: Option<PolicyApprovalSet>,
) -> Result<HandoffActionResult, String> {
    let (thread, manifest) = handoff_thread_manifest(&state, &thread_id)?;
    let agent = state
        .agent_store
        .resolve_agent(Some(&thread.agent_id))
        .map_err(|error| error.to_string())?;
    let (transport, command) = if let Some(acp) = agent.acp.clone() {
        ("acp", acp)
    } else if agent.provider == AgentProvider::SofvaryPi {
        let command = agent
            .cli
            .clone()
            .ok_or_else(|| "selected Agent has no launch command".to_string())?;
        ("pi-rpc", command)
    } else if agent.allow_cli_fallback
        && agent
            .last_test
            .as_ref()
            .is_some_and(|record| record.ok && matches!(record.transport, AgentTransportKind::Cli))
    {
        let command = agent
            .cli
            .clone()
            .ok_or_else(|| "selected Agent has no launch command".to_string())?;
        ("cli", command)
    } else {
        return Err("selected Agent has no policy-approved launch command".to_string());
    };
    let subject = format!(
        "{}:{}:{}",
        agent.id,
        transport,
        command.executable.display()
    );
    if !policy_approvals
        .unwrap_or_default()
        .permits(PolicyActionKind::ExternalAgentProcess, Some(&subject))
    {
        return Err("Policy approval required to open the Workspace Handoff Agent.".to_string());
    }
    current_adapter()
        .spawn_process(CommandSpec {
            executable: command.executable.clone(),
            args: command.args.clone(),
            cwd: manifest.paths.root.clone(),
            env: command.env.clone(),
            allowed_network: false,
            timeout_ms: None,
            kill_on_drop: false,
        })
        .map_err(|error| error.to_string())?;
    if let Ok(entry) = state.build_thread_store.append_entry(
        &thread_id,
        BuildThreadEntryKind::System,
        format!("Workspace Handoff Agent launched: {}", agent.label),
        serde_json::json!({
            "kind": "workspace-handoff-agent-opened",
            "agentId": agent.id,
            "transport": transport,
            "workspaceRoot": manifest.paths.root,
        }),
    ) {
        emit_build_thread_entry(&app, &entry);
    }
    Ok(HandoffActionResult { thread })
}

#[tauri::command]
fn rescan_handoff_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    thread_id: String,
) -> Result<HandoffRescanResult, String> {
    let (thread, manifest) = handoff_thread_manifest(&state, &thread_id)?;
    let envelope = read_handoff_envelope(&manifest).map_err(|error| error.to_string())?;
    let scan = scan_handoff_workspace(&manifest, &envelope).map_err(|error| error.to_string())?;
    append_handoff_request_consumed(&manifest, "validate.json")
        .map_err(|error| error.to_string())?;
    append_handoff_request_consumed(&manifest, "preview.json")
        .map_err(|error| error.to_string())?;
    append_handoff_scan_entries(&app, &state.build_thread_store, &thread_id, &scan);

    let preview = if scan.complete {
        preview_handoff_workspace(
            &app,
            &state.build_thread_store,
            state.runtime_manager.clone(),
            state.workspace_manager,
            &thread_id,
            thread.runtime_mode,
            PolicyApprovalSet::default(),
            &manifest,
            &envelope,
        )
        .ok()
    } else {
        None
    };
    let thread = state
        .build_thread_store
        .get(&thread_id)
        .map_err(|error| error.to_string())?
        .summary;
    Ok(HandoffRescanResult {
        thread,
        scan,
        preview,
    })
}

enum BuildThreadRunContext {
    NewApp,
    ExistingApp { app_id: String },
}

fn run_build_thread_task(
    app: tauri::AppHandle,
    build_thread_store: BuildThreadStore,
    runtime_manager: Arc<RuntimeManager>,
    workspace_manager: WorkspaceManager,
    thread_id: String,
    requirement: String,
    runtime_kind: RuntimeKind,
    runtime_mode: RuntimeMode,
    policy_approvals: PolicyApprovalSet,
    agent_config: AgentConfig,
    agent_mode: AgentInteractionMode,
    run_context: BuildThreadRunContext,
) {
    if build_thread_is_canceled(&build_thread_store, &thread_id) {
        emit_build_thread_canceled_state(&app);
        return;
    }

    if agent_mode == AgentInteractionMode::WorkspaceHandoff {
        run_workspace_handoff_build_thread_task(
            app,
            build_thread_store,
            runtime_manager,
            workspace_manager,
            thread_id,
            requirement,
            runtime_kind,
            runtime_mode,
            policy_approvals,
            agent_config,
            run_context,
        );
        return;
    }

    if let Some(summary) = update_build_thread_unless_canceled(
        &app,
        &build_thread_store,
        &thread_id,
        BuildThreadUpdate {
            status: Some(BuildThreadStatus::Planning),
            error: Some(None),
            ..BuildThreadUpdate::default()
        },
    ) {
        emit_build_thread_updated(&app, &summary);
    }
    if build_thread_is_canceled(&build_thread_store, &thread_id) {
        emit_build_thread_canceled_state(&app);
        return;
    }
    let (start_message, start_metadata) = match &run_context {
        BuildThreadRunContext::NewApp => (
            format!("Starting build with {}.", agent_config.label),
            serde_json::json!({
                "agentId": agent_config.id.clone(),
                "runtimeKind": runtime_kind,
                "runtimeMode": runtime_mode,
                "source": "new-app"
            }),
        ),
        BuildThreadRunContext::ExistingApp { app_id } => (
            format!(
                "Continuing existing app {app_id} with {}.",
                agent_config.label
            ),
            serde_json::json!({
                "agentId": agent_config.id.clone(),
                "runtimeKind": runtime_kind,
                "runtimeMode": runtime_mode,
                "source": "continue-app",
                "appId": app_id
            }),
        ),
    };
    if let Ok(entry) = build_thread_store.append_entry(
        &thread_id,
        BuildThreadEntryKind::System,
        start_message,
        start_metadata,
    ) {
        emit_build_thread_entry(&app, &entry);
    }
    let _ = app.emit("sofvary-build-state", "Planning");

    if build_thread_is_canceled(&build_thread_store, &thread_id) {
        emit_build_thread_canceled_state(&app);
        return;
    }

    if let Some(summary) = update_build_thread_unless_canceled(
        &app,
        &build_thread_store,
        &thread_id,
        BuildThreadUpdate {
            status: Some(BuildThreadStatus::Building),
            ..BuildThreadUpdate::default()
        },
    ) {
        emit_build_thread_updated(&app, &summary);
    }
    let _ = app.emit("sofvary-build-state", "Building");

    let agent_event_sink: AgentEventSink = {
        let app = app.clone();
        let build_thread_store = build_thread_store;
        let thread_id = thread_id.clone();
        Arc::new(move |event| {
            if build_thread_is_canceled(&build_thread_store, &thread_id) {
                return;
            }
            append_live_agent_event(&app, &build_thread_store, &thread_id, event);
        })
    };
    let gateway_event_sink: GatewayUniEventSink = {
        let app = app.clone();
        let build_thread_store = build_thread_store;
        let thread_id = thread_id.clone();
        Arc::new(move |event| {
            if build_thread_is_canceled(&build_thread_store, &thread_id) {
                return;
            }
            append_gateway_uni_event(&app, &build_thread_store, &thread_id, event);
        })
    };
    let live_agent_event_sink = if agent_config.provider == AgentProvider::SofvaryPi {
        None
    } else {
        Some(agent_event_sink.clone())
    };

    let build_result = match run_context {
        BuildThreadRunContext::NewApp => runtime_manager
            .build_and_preview_app_with_agent_policy_and_events(
                requirement,
                runtime_kind,
                runtime_mode,
                &workspace_manager,
                &policy_approvals,
                &agent_config,
                live_agent_event_sink.clone(),
                Some(thread_id.clone()),
                Some(gateway_event_sink),
            ),
        BuildThreadRunContext::ExistingApp { app_id } => runtime_manager
            .continue_existing_app_with_agent_policy_and_events(
                requirement,
                app_id,
                runtime_kind,
                runtime_mode,
                &workspace_manager,
                &policy_approvals,
                &agent_config,
                live_agent_event_sink,
                Some(thread_id.clone()),
                Some(gateway_event_sink),
            ),
    };

    match build_result {
        Ok(preview) => {
            if build_thread_is_canceled(&build_thread_store, &thread_id) {
                emit_build_thread_canceled_state(&app);
                return;
            }
            append_preview_logs(&app, &build_thread_store, &thread_id, &preview);
            let summary = build_thread_store.update(
                &thread_id,
                BuildThreadUpdate {
                    status: Some(BuildThreadStatus::Completed),
                    workspace_id: Some(Some(preview.app_id.clone())),
                    app_id: Some(Some(preview.app_id.clone())),
                    preview: Some(Some(preview.clone())),
                    preview_issue: Some(None),
                    error: Some(None),
                    ..BuildThreadUpdate::default()
                },
            );
            if let Ok(summary) = summary {
                emit_build_thread_updated(&app, &summary);
            }
            let _ = app.emit("sofvary-runtime-preview", preview);
        }
        Err(error) => {
            if build_thread_is_canceled(&build_thread_store, &thread_id) {
                emit_build_thread_canceled_state(&app);
                return;
            }
            if mark_build_thread_preview_blocked(&app, &build_thread_store, &thread_id, &error) {
                return;
            }
            let (message, metadata) = build_thread_failure_summary(&error);
            if let Ok(entry) = build_thread_store.append_entry(
                &thread_id,
                BuildThreadEntryKind::Error,
                message.clone(),
                metadata,
            ) {
                emit_build_thread_entry(&app, &entry);
            }
            let summary = build_thread_store.update(
                &thread_id,
                BuildThreadUpdate {
                    status: Some(BuildThreadStatus::Failed),
                    error: Some(Some(message.clone())),
                    ..BuildThreadUpdate::default()
                },
            );
            if let Ok(summary) = summary {
                emit_build_thread_updated(&app, &summary);
            }
            let _ = app.emit("sofvary-runtime-error", message);
        }
    }
}

fn build_thread_failure_summary(error: &RuntimeManagerError) -> (String, serde_json::Value) {
    match error {
        RuntimeManagerError::RuntimeRepairExhausted {
            attempts,
            summary,
            diagnostic,
        } => (
            format!("Sofvary 已自动尝试修复 {attempts} 次，但运行问题仍未解决。{summary}"),
            serde_json::json!({
                "kind": "runtime-repair-exhausted",
                "attempts": attempts,
                "summary": summary,
                "diagnostic": diagnostic
            }),
        ),
        RuntimeManagerError::RuntimeDiagnosticBlocked {
            summary,
            diagnostic,
            source_detail,
            assets: _,
        } => (
            format!("Sofvary 已完成运行诊断：{summary}"),
            serde_json::json!({
                "kind": "runtime-diagnostic-blocked",
                "summary": summary,
                "diagnostic": diagnostic,
                "source": source_detail
            }),
        ),
        _ => (error.to_string(), serde_json::json!({})),
    }
}

fn run_workspace_handoff_build_thread_task(
    app: tauri::AppHandle,
    build_thread_store: BuildThreadStore,
    runtime_manager: Arc<RuntimeManager>,
    workspace_manager: WorkspaceManager,
    thread_id: String,
    requirement: String,
    runtime_kind: RuntimeKind,
    runtime_mode: RuntimeMode,
    policy_approvals: PolicyApprovalSet,
    agent_config: AgentConfig,
    run_context: BuildThreadRunContext,
) {
    let emitter = handoff_event_emitter(&app, build_thread_store, &thread_id, &agent_config.id);
    emitter.session_started(&agent_config.label);
    emitter.status("workspace-preparing", "Preparing Workspace Handoff files.");

    if let Some(summary) = update_build_thread_unless_canceled(
        &app,
        &build_thread_store,
        &thread_id,
        BuildThreadUpdate {
            status: Some(BuildThreadStatus::Planning),
            error: Some(None),
            ..BuildThreadUpdate::default()
        },
    ) {
        emit_build_thread_updated(&app, &summary);
    }
    let _ = app.emit("sofvary-build-state", "Planning");

    let preparation = match run_context {
        BuildThreadRunContext::NewApp => prepare_new_handoff_workspace(
            &requirement,
            runtime_kind,
            &workspace_manager,
            &agent_config,
        ),
        BuildThreadRunContext::ExistingApp { app_id } => workspace_manager
            .get_workspace(app_id)
            .map_err(Into::into)
            .and_then(|manifest| {
                prepare_existing_handoff_workspace(
                    &requirement,
                    manifest,
                    &workspace_manager,
                    &agent_config,
                )
            }),
    };

    let preparation = match preparation {
        Ok(preparation) => preparation,
        Err(error) => {
            fail_build_thread(
                &app,
                &build_thread_store,
                &thread_id,
                format!("Workspace Handoff preparation failed: {error}"),
                serde_json::json!({ "kind": "workspace-handoff-prepare-failed" }),
            );
            emitter.error(error.to_string());
            emitter.turn_completed("error");
            return;
        }
    };

    append_handoff_prepared_entry(&app, &build_thread_store, &thread_id, &preparation);
    let summary = build_thread_store.update(
        &thread_id,
        BuildThreadUpdate {
            status: Some(BuildThreadStatus::Building),
            workspace_id: Some(Some(preparation.manifest.app_id.clone())),
            app_id: Some(Some(preparation.manifest.app_id.clone())),
            error: Some(None),
            ..BuildThreadUpdate::default()
        },
    );
    if let Ok(summary) = summary {
        emit_build_thread_updated(&app, &summary);
    }
    let _ = app.emit("sofvary-build-state", "Building");
    emitter.status(
        "handoff-ready",
        "Workspace is ready. Paste SOFVARY_AGENT_PROMPT.md into the selected external Agent.",
    );

    let scan = wait_for_handoff_assets(
        &preparation.manifest,
        &preparation.prompt_envelope,
        Duration::from_secs(300),
        Duration::from_secs(2),
    );
    let scan = match scan {
        Ok(scan) => scan,
        Err(error) => {
            fail_build_thread(
                &app,
                &build_thread_store,
                &thread_id,
                format!("Workspace Handoff scan failed: {error}"),
                serde_json::json!({ "kind": "workspace-handoff-scan-failed" }),
            );
            emitter.error(error.to_string());
            emitter.turn_completed("error");
            return;
        }
    };

    append_handoff_scan_entries(&app, &build_thread_store, &thread_id, &scan);
    if !scan.complete {
        emitter.status(
            "waiting-for-files",
            format!(
                "Waiting for external Agent output. Missing {} files.",
                scan.missing_files.len()
            ),
        );
        return;
    }

    let _ = preview_handoff_workspace(
        &app,
        &build_thread_store,
        runtime_manager,
        workspace_manager,
        &thread_id,
        runtime_mode,
        policy_approvals,
        &preparation.manifest,
        &preparation.prompt_envelope,
    );
    emitter.turn_completed("ok");
}

fn handoff_event_emitter(
    app: &tauri::AppHandle,
    build_thread_store: BuildThreadStore,
    thread_id: &str,
    agent_id: &str,
) -> GatewayUniEventEmitter {
    let app = app.clone();
    let thread_id_for_sink = thread_id.to_string();
    let sink: GatewayUniEventSink = Arc::new(move |event| {
        if build_thread_is_canceled(&build_thread_store, &thread_id_for_sink) {
            return;
        }
        append_gateway_uni_event(&app, &build_thread_store, &thread_id_for_sink, event);
    });
    GatewayUniEventEmitter::new(
        thread_id.to_string(),
        agent_id.to_string(),
        AgentTransportKind::WorkspaceHandoff,
        sink,
    )
}

fn append_handoff_prepared_entry(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    preparation: &WorkspaceHandoffPreparation,
) {
    if let Ok(entry) = build_thread_store.append_entry(
        thread_id,
        BuildThreadEntryKind::System,
        "Workspace Handoff prepared. Prompt file is ready.",
        serde_json::json!({
            "kind": "workspace-handoff-prepared",
            "appId": preparation.manifest.app_id,
            "workspaceRoot": preparation.manifest.paths.root,
            "promptPath": preparation.prompt_path,
            "promptEnvelopeSummary": preparation.prompt_envelope_summary,
        }),
    ) {
        emit_build_thread_entry(app, &entry);
    }
}

fn append_handoff_scan_entries(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    scan: &HandoffScanResult,
) {
    for status in &scan.status_entries {
        if let Ok(entry) = build_thread_store.append_entry(
            thread_id,
            BuildThreadEntryKind::AgentEvent,
            format!("Workspace Handoff Agent status: {status}"),
            serde_json::json!({ "kind": "workspace-handoff-agent-status" }),
        ) {
            emit_build_thread_entry(app, &entry);
        }
    }

    let existing_files = scan
        .files
        .iter()
        .filter(|file| file.exists)
        .map(|file| file.relative_path.clone())
        .collect::<Vec<_>>();
    if !existing_files.is_empty() {
        if let Ok(entry) = build_thread_store.append_entry(
            thread_id,
            BuildThreadEntryKind::File,
            format!(
                "Workspace Handoff detected {} generated files.",
                existing_files.len()
            ),
            serde_json::json!({
                "kind": "workspace-handoff-files-changed",
                "generatedRoot": scan.generated_root,
                "files": existing_files,
                "missingFiles": scan.missing_files,
                "validateRequested": scan.validate_requested,
                "previewRequested": scan.preview_requested,
            }),
        ) {
            emit_build_thread_entry(app, &entry);
        }
    } else if let Ok(entry) = build_thread_store.append_entry(
        thread_id,
        BuildThreadEntryKind::System,
        "Workspace Handoff is waiting for generated files.",
        serde_json::json!({
            "kind": "workspace-handoff-waiting",
            "missingFiles": scan.missing_files,
        }),
    ) {
        emit_build_thread_entry(app, &entry);
    }
}

fn preview_handoff_workspace(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    runtime_manager: Arc<RuntimeManager>,
    workspace_manager: WorkspaceManager,
    thread_id: &str,
    runtime_mode: RuntimeMode,
    policy_approvals: PolicyApprovalSet,
    manifest: &AppBoxManifest,
    envelope: &crate::core::harness_engine::PromptEnvelope,
) -> Result<RuntimePreview, RuntimeManagerError> {
    if let Ok(entry) = build_thread_store.append_entry(
        thread_id,
        BuildThreadEntryKind::System,
        "Workspace Handoff contract is complete. Starting preview.",
        serde_json::json!({ "kind": "workspace-handoff-preview-starting" }),
    ) {
        emit_build_thread_entry(app, &entry);
    }
    if let Ok(summary) = build_thread_store.update(
        thread_id,
        BuildThreadUpdate {
            status: Some(BuildThreadStatus::Previewing),
            ..BuildThreadUpdate::default()
        },
    ) {
        emit_build_thread_updated(app, &summary);
    }

    match runtime_manager.preview_existing_workspace_with_policy(
        manifest.app_id.clone(),
        runtime_mode,
        &workspace_manager,
        &policy_approvals,
    ) {
        Ok(preview) => {
            append_preview_logs(app, build_thread_store, thread_id, &preview);
            let summary = build_thread_store.update(
                thread_id,
                BuildThreadUpdate {
                    status: Some(BuildThreadStatus::Completed),
                    workspace_id: Some(Some(preview.app_id.clone())),
                    app_id: Some(Some(preview.app_id.clone())),
                    preview: Some(Some(preview.clone())),
                    preview_issue: Some(None),
                    error: Some(None),
                    ..BuildThreadUpdate::default()
                },
            );
            if let Ok(summary) = summary {
                emit_build_thread_updated(app, &summary);
            }
            let _ = app.emit("sofvary-runtime-preview", preview.clone());
            Ok(preview)
        }
        Err(error) => {
            if let RuntimeManagerError::RuntimeDiagnosticBlocked {
                diagnostic,
                source_detail,
                ..
            }
            | RuntimeManagerError::RuntimeRepairExhausted {
                diagnostic,
                summary: source_detail,
                ..
            } = &error
            {
                let _ = write_handoff_diagnostics(manifest, envelope, diagnostic, source_detail);
            }
            if mark_build_thread_preview_blocked(app, build_thread_store, thread_id, &error) {
                return Err(error);
            }
            let (message, metadata) = build_thread_failure_summary(&error);
            fail_build_thread(app, build_thread_store, thread_id, message, metadata);
            Err(error)
        }
    }
}

fn fail_build_thread(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    message: String,
    metadata: serde_json::Value,
) {
    if let Ok(entry) = build_thread_store.append_entry(
        thread_id,
        BuildThreadEntryKind::Error,
        message.clone(),
        metadata,
    ) {
        emit_build_thread_entry(app, &entry);
    }
    if let Ok(summary) = build_thread_store.update(
        thread_id,
        BuildThreadUpdate {
            status: Some(BuildThreadStatus::Failed),
            error: Some(Some(message.clone())),
            ..BuildThreadUpdate::default()
        },
    ) {
        emit_build_thread_updated(app, &summary);
    }
    let _ = app.emit("sofvary-runtime-error", message);
}

fn mark_build_thread_preview_blocked(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    error: &RuntimeManagerError,
) -> bool {
    let RuntimeManagerError::RuntimeDiagnosticBlocked {
        summary,
        diagnostic,
        source_detail,
        assets: Some(assets),
    } = error
    else {
        return false;
    };

    let issue = runtime_preview_issue_from_diagnostic(
        assets.runtime_kind.clone(),
        summary.clone(),
        diagnostic.clone(),
        source_detail.clone(),
    );
    let message = format!("Sofvary 已生成软件资产，但预览环境未就绪：{summary}");
    if let Ok(entry) = build_thread_store.append_entry(
        thread_id,
        BuildThreadEntryKind::System,
        message,
        serde_json::json!({
            "kind": "runtime-preview-blocked",
            "summary": summary,
            "diagnostic": diagnostic,
            "source": source_detail,
            "previewIssue": issue.clone(),
            "assets": {
                "appId": assets.app_id.clone(),
                "runtimeKind": assets.runtime_kind,
                "runtimeMode": assets.runtime_mode,
                "manifest": assets.manifest.clone(),
                "promptEnvelopeSummary": assets.prompt_envelope_summary.clone()
            }
        }),
    ) {
        emit_build_thread_entry(app, &entry);
    }

    let summary = build_thread_store.update(
        thread_id,
        BuildThreadUpdate {
            status: Some(BuildThreadStatus::PreviewBlocked),
            workspace_id: Some(Some(assets.app_id.clone())),
            app_id: Some(Some(assets.app_id.clone())),
            preview: Some(None),
            preview_issue: Some(Some(issue)),
            error: Some(None),
            ..BuildThreadUpdate::default()
        },
    );
    if let Ok(summary) = summary {
        emit_build_thread_updated(app, &summary);
    }
    true
}

fn update_build_thread_unless_canceled(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    update: BuildThreadUpdate,
) -> Option<BuildThreadSummary> {
    if build_thread_is_canceled(build_thread_store, thread_id) {
        emit_build_thread_canceled_state(app);
        return None;
    }
    build_thread_store.update(thread_id, update).ok()
}

fn emit_build_thread_canceled_state(app: &tauri::AppHandle) {
    let _ = app.emit("sofvary-build-state", "BackgroundIdle");
}

fn build_thread_is_canceled(build_thread_store: &BuildThreadStore, thread_id: &str) -> bool {
    build_thread_store
        .get(thread_id)
        .map(|detail| detail.summary.status == BuildThreadStatus::Canceled)
        .unwrap_or(false)
}

fn resolve_agent_interaction_mode(
    requested: Option<AgentInteractionMode>,
    agent_config: &AgentConfig,
) -> AgentInteractionMode {
    let mode = requested.unwrap_or_else(|| agent_config.effective_interaction_mode());
    match (agent_config.provider, mode) {
        (AgentProvider::SofvaryPi, AgentInteractionMode::ThirdPartyManaged)
        | (AgentProvider::SofvaryPi, AgentInteractionMode::WorkspaceHandoff) => {
            AgentInteractionMode::PiNative
        }
        (_, AgentInteractionMode::PiNative) => AgentInteractionMode::ThirdPartyManaged,
        _ => mode,
    }
}

fn handoff_thread_manifest(
    state: &tauri::State<'_, AppState>,
    thread_id: &str,
) -> Result<(BuildThreadSummary, AppBoxManifest), String> {
    let thread = state
        .build_thread_store
        .get(thread_id)
        .map_err(|error| error.to_string())?
        .summary;
    if thread.agent_mode != AgentInteractionMode::WorkspaceHandoff {
        return Err("build thread is not a Workspace Handoff thread".to_string());
    }
    let app_id = thread
        .app_id
        .clone()
        .or_else(|| thread.workspace_id.clone())
        .ok_or_else(|| "Workspace Handoff thread has no workspace id".to_string())?;
    let manifest = state
        .workspace_manager
        .get_workspace(app_id)
        .map_err(|error| error.to_string())?;
    Ok((thread, manifest))
}

fn append_live_agent_event(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    event: AgentEvent,
) {
    let metadata_event = serde_json::to_value(&event).unwrap_or_else(|_| serde_json::json!({}));
    let (kind, content) = match &event {
        AgentEvent::SessionStarted { adapter, .. } => (
            BuildThreadEntryKind::AgentEvent,
            format!("Agent session started with {:?} adapter", adapter),
        ),
        AgentEvent::Planning { message } => (BuildThreadEntryKind::AgentEvent, message.clone()),
        AgentEvent::TextDelta { text } => (BuildThreadEntryKind::Assistant, text.clone()),
        AgentEvent::FileWriteRequested { relative_path } => (
            BuildThreadEntryKind::File,
            format!("Agent requested file write: {relative_path}"),
        ),
        AgentEvent::FileWritten { relative_path } => (
            BuildThreadEntryKind::File,
            format!("Workspace wrote generated file: {relative_path}"),
        ),
        AgentEvent::CommandRequested { executable } => (
            BuildThreadEntryKind::Tool,
            format!("Agent requested command: {executable}"),
        ),
        AgentEvent::CommandApproved { executable } => (
            BuildThreadEntryKind::Tool,
            format!("Command approved: {executable}"),
        ),
        AgentEvent::CommandRejected { executable, reason } => (
            BuildThreadEntryKind::Tool,
            format!("Command rejected: {executable}: {reason}"),
        ),
        AgentEvent::BuildStarted { target } => (
            BuildThreadEntryKind::AgentEvent,
            format!("Build started: {target}"),
        ),
        AgentEvent::BuildFinished { target } => (
            BuildThreadEntryKind::AgentEvent,
            format!("Build finished: {target}"),
        ),
        AgentEvent::RuntimeDiagnostic { diagnostic } => (
            BuildThreadEntryKind::System,
            format!("Runtime diagnostic: {}", diagnostic.summary()),
        ),
        AgentEvent::RepairStarted {
            attempt,
            max_attempts,
            summary,
        } => {
            if let Some(summary_update) = update_build_thread_unless_canceled(
                app,
                build_thread_store,
                thread_id,
                BuildThreadUpdate {
                    status: Some(BuildThreadStatus::Repairing),
                    ..BuildThreadUpdate::default()
                },
            ) {
                emit_build_thread_updated(app, &summary_update);
            }
            (
                BuildThreadEntryKind::AgentEvent,
                format!("Runtime repair attempt {attempt}/{max_attempts}: {summary}"),
            )
        }
        AgentEvent::RepairFinished { attempt, summary } => (
            BuildThreadEntryKind::AgentEvent,
            format!("Runtime repair attempt {attempt} finished: {summary}"),
        ),
        AgentEvent::Error { message } => (BuildThreadEntryKind::Error, message.clone()),
        AgentEvent::Completed => (
            BuildThreadEntryKind::AgentEvent,
            "Agent session completed".to_string(),
        ),
    };

    if content.trim().is_empty() {
        return;
    }

    if let Ok(entry) = build_thread_store.append_entry(
        thread_id,
        kind,
        content,
        serde_json::json!({ "source": "agent-live-stream", "event": metadata_event }),
    ) {
        emit_build_thread_entry(app, &entry);
    }
}

fn append_gateway_uni_event(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    event: GatewayUniEvent,
) {
    let kind = match event.event_type {
        GatewayUniEventType::MessageDelta => BuildThreadEntryKind::Assistant,
        GatewayUniEventType::ToolStarted
        | GatewayUniEventType::ToolDelta
        | GatewayUniEventType::ToolCompleted
        | GatewayUniEventType::TerminalOutput
        | GatewayUniEventType::ApprovalRequested
        | GatewayUniEventType::ApprovalResolved => BuildThreadEntryKind::Tool,
        GatewayUniEventType::FileWriteRequested | GatewayUniEventType::FileWritten => {
            BuildThreadEntryKind::File
        }
        GatewayUniEventType::Error => BuildThreadEntryKind::Error,
        GatewayUniEventType::SessionStarted
        | GatewayUniEventType::TurnStarted
        | GatewayUniEventType::ReasoningDelta
        | GatewayUniEventType::StatusChanged
        | GatewayUniEventType::TurnCompleted => BuildThreadEntryKind::AgentEvent,
    };
    let content = gateway_uni_event_summary(&event);
    if content.trim().is_empty() {
        return;
    }
    if let Ok(entry) = build_thread_store.append_entry(
        thread_id,
        kind,
        content,
        serde_json::json!({ "source": "gateway-uni-event", "gatewayUniEvent": event }),
    ) {
        emit_build_thread_entry(app, &entry);
    }
}

fn append_preview_logs(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    preview: &RuntimePreview,
) {
    let mut pending_assistant = String::new();
    for log in &preview.logs {
        if let Some(text) = log.strip_prefix("Agent message:") {
            append_agent_message_text(&mut pending_assistant, text);
            continue;
        }

        flush_pending_assistant(app, build_thread_store, thread_id, &mut pending_assistant);

        let kind = if log.starts_with("Agent requested file write:")
            || log.starts_with("Workspace wrote generated file:")
        {
            BuildThreadEntryKind::File
        } else if log.starts_with("Agent") {
            BuildThreadEntryKind::AgentEvent
        } else {
            BuildThreadEntryKind::System
        };
        if let Ok(entry) =
            build_thread_store.append_entry(thread_id, kind, log.clone(), serde_json::json!({}))
        {
            emit_build_thread_entry(app, &entry);
        }
    }
    flush_pending_assistant(app, build_thread_store, thread_id, &mut pending_assistant);
}

fn flush_pending_assistant(
    app: &tauri::AppHandle,
    build_thread_store: &BuildThreadStore,
    thread_id: &str,
    pending_assistant: &mut String,
) {
    if pending_assistant.trim().is_empty() {
        pending_assistant.clear();
        return;
    }
    if let Ok(entry) = build_thread_store.append_entry(
        thread_id,
        BuildThreadEntryKind::Assistant,
        pending_assistant.trim().to_string(),
        serde_json::json!({ "source": "agent-message-stream", "merged": true }),
    ) {
        emit_build_thread_entry(app, &entry);
    }
    pending_assistant.clear();
}

fn append_agent_message_text(output: &mut String, text: &str) {
    let next = text.trim();
    if next.is_empty() {
        return;
    }
    if output.is_empty()
        || output.chars().last().is_some_and(char::is_whitespace)
        || starts_with_join_punctuation(next)
    {
        output.push_str(next);
    } else {
        output.push(' ');
        output.push_str(next);
    }
}

fn starts_with_join_punctuation(value: &str) -> bool {
    value.chars().next().is_some_and(|ch| {
        matches!(
            ch,
            '.' | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | ')'
                | ']'
                | '}'
                | '。'
                | '，'
                | '；'
                | '：'
                | '！'
                | '？'
        )
    })
}

fn emit_build_thread_updated(app: &tauri::AppHandle, summary: &BuildThreadSummary) {
    let _ = app.emit("sofvary-build-thread-updated", summary);
}

fn emit_build_thread_entry(app: &tauri::AppHandle, entry: &BuildThreadEntry) {
    let _ = app.emit("sofvary-build-thread-entry", entry);
}

fn emit_agent_install_updated(app: &tauri::AppHandle, payload: serde_json::Value) {
    let _ = app.emit("sofvary-agent-install-updated", payload);
}

fn emit_agent_install_log(app: &tauri::AppHandle, payload: serde_json::Value) {
    let _ = app.emit("sofvary-agent-install-log", payload);
}

fn emit_runtime_environment_install_updated(app: &tauri::AppHandle, payload: serde_json::Value) {
    let _ = app.emit("sofvary-runtime-environment-install-updated", payload);
}

fn hydrate_agent_config_for_runtime(
    mut config: AgentConfig,
    llm_provider_store: &LlmProviderConfigStore,
) -> Result<AgentConfig, crate::core::llm_provider_config::LlmProviderConfigError> {
    let Some(provider) = llm_provider_store.resolve_default()? else {
        return Ok(config);
    };

    match config.provider {
        AgentProvider::SofvaryPi => {
            if let Some(command) = &mut config.cli {
                command.args.push("--provider".to_string());
                command
                    .args
                    .push(provider.kind.as_pi_provider().to_string());
                command.args.push("--model".to_string());
                command.args.push(provider.model.clone());
                apply_llm_provider_env(command, &provider)?;
            }
        }
        AgentProvider::KimiCode if provider.kind == LlmProviderKind::KimiCoding => {
            if let Some(command) = &mut config.acp {
                apply_kimi_code_env(command, &provider)?;
            }
            if let Some(command) = &mut config.cli {
                apply_kimi_code_env(command, &provider)?;
            }
        }
        _ => {}
    }

    Ok(config)
}

fn apply_llm_provider_env(
    command: &mut AgentCommandConfig,
    provider: &LlmProviderConfig,
) -> Result<(), LlmProviderConfigError> {
    if let Some(base_url_env) = llm_base_url_env(provider.kind) {
        if let Some(base_url) = provider.base_url.clone() {
            command.env.insert(base_url_env.to_string(), base_url);
        }
    }
    if let Some(api_key) = resolve_llm_api_key(provider)? {
        command
            .env
            .insert(llm_api_key_env(provider.kind).to_string(), api_key);
    }
    Ok(())
}

fn apply_kimi_code_env(
    command: &mut AgentCommandConfig,
    provider: &LlmProviderConfig,
) -> Result<(), LlmProviderConfigError> {
    command
        .env
        .insert("KIMI_MODEL_NAME".to_string(), provider.model.clone());
    command.env.insert(
        "KIMI_MODEL_BASE_URL".to_string(),
        provider
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.kimi.com/coding/v1".to_string()),
    );
    if let Some(api_key) = resolve_llm_api_key(provider)? {
        command
            .env
            .insert("KIMI_MODEL_API_KEY".to_string(), api_key);
    }
    Ok(())
}

fn resolve_llm_api_key(
    provider: &LlmProviderConfig,
) -> Result<Option<String>, LlmProviderConfigError> {
    if let Some(api_key_ref) = provider.api_key_ref.as_deref() {
        if let Some(env_name) = api_key_ref.strip_prefix("env:") {
            return Ok(std::env::var(env_name)
                .ok()
                .filter(|api_key| !api_key.trim().is_empty()));
        }
        if let Some(api_key) = current_adapter().secure_store_get(api_key_ref)? {
            if !api_key.trim().is_empty() {
                return Ok(Some(api_key));
            }
        }
    }

    Ok(std::env::var(llm_api_key_env(provider.kind))
        .ok()
        .filter(|api_key| !api_key.trim().is_empty()))
}

fn llm_base_url_env(kind: LlmProviderKind) -> Option<&'static str> {
    match kind {
        LlmProviderKind::Openai | LlmProviderKind::OpenaiCompatible => Some("OPENAI_BASE_URL"),
        LlmProviderKind::Anthropic => Some("ANTHROPIC_BASE_URL"),
        LlmProviderKind::Openrouter => Some("OPENROUTER_BASE_URL"),
        LlmProviderKind::Deepseek => Some("DEEPSEEK_BASE_URL"),
        LlmProviderKind::Google => None,
        LlmProviderKind::Groq => Some("GROQ_BASE_URL"),
        LlmProviderKind::Xai => Some("XAI_BASE_URL"),
        LlmProviderKind::KimiCoding => Some("KIMI_BASE_URL"),
        LlmProviderKind::Ollama => Some("OLLAMA_BASE_URL"),
    }
}

fn llm_api_key_env(kind: LlmProviderKind) -> &'static str {
    match kind {
        LlmProviderKind::Anthropic => "ANTHROPIC_API_KEY",
        LlmProviderKind::Openrouter => "OPENROUTER_API_KEY",
        LlmProviderKind::Deepseek => "DEEPSEEK_API_KEY",
        LlmProviderKind::Google => "GEMINI_API_KEY",
        LlmProviderKind::Groq => "GROQ_API_KEY",
        LlmProviderKind::Xai => "XAI_API_KEY",
        LlmProviderKind::KimiCoding => "KIMI_API_KEY",
        LlmProviderKind::Ollama => "OLLAMA_API_KEY",
        LlmProviderKind::Openai | LlmProviderKind::OpenaiCompatible => "OPENAI_API_KEY",
    }
}

fn llm_provider_requires_api_key(kind: LlmProviderKind) -> bool {
    !matches!(
        kind,
        LlmProviderKind::Ollama | LlmProviderKind::OpenaiCompatible
    )
}

fn llm_provider_test_outcome(
    provider: &LlmProviderConfig,
    api_key_available: bool,
) -> (bool, String) {
    if !llm_provider_requires_api_key(provider.kind) {
        return (
            true,
            "LLM provider config is syntactically valid; API key is optional for this provider."
                .to_string(),
        );
    }
    if api_key_available {
        return (
            true,
            "LLM provider credential is available from secure storage or environment.".to_string(),
        );
    }

    (
        false,
        format!(
            "LLM provider config has no available API key. Save the key again or set {}.",
            llm_api_key_env(provider.kind)
        ),
    )
}

#[tauri::command]
fn discover_agents() -> Result<Vec<DiscoveredAgent>, String> {
    discover_agent_catalog().map_err(|error| error.to_string())
}

#[tauri::command]
fn list_agent_configs(state: tauri::State<'_, AppState>) -> Result<AgentConfigState, String> {
    state.agent_store.load().map_err(|error| error.to_string())
}

#[tauri::command]
fn upsert_agent_config(
    state: tauri::State<'_, AppState>,
    config: AgentConfig,
) -> Result<AgentConfigState, String> {
    state
        .agent_store
        .upsert(config)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_agent_config(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<AgentConfigState, String> {
    state
        .agent_store
        .delete(&agent_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_default_agent(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<AgentConfigState, String> {
    state
        .agent_store
        .set_default(&agent_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn test_agent_connection(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<AgentTestRecord, String> {
    let config = state
        .agent_store
        .resolve_agent(Some(&agent_id))
        .map_err(|error| error.to_string())?;
    let record = match test_configured_agent(&config) {
        Ok(record) => record,
        Err((transport, detail)) => fresh_test_record(false, transport, detail),
    };
    let _ = state
        .agent_store
        .record_test(&agent_id, record.clone())
        .map_err(|error| error.to_string())?;
    Ok(record)
}

#[tauri::command]
fn list_agent_install_catalog() -> Vec<AgentInstallCatalogItem> {
    list_agent_install_catalog_core()
}

#[tauri::command]
fn get_agent_install_statuses(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AgentInstallStatus>, String> {
    get_agent_install_statuses_core(&state.agent_store).map_err(|error| error.to_string())
}

#[tauri::command]
async fn start_agent_install(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    payload: StartAgentInstallPayload,
) -> Result<AgentInstallStatus, String> {
    let agent_store = state.agent_store.clone();
    let agent_id = payload.agent_id.clone();
    emit_agent_install_updated(
        &app,
        serde_json::json!({
            "agentId": agent_id,
            "state": "installing",
            "detail": "Agent install started."
        }),
    );
    emit_agent_install_log(
        &app,
        serde_json::json!({
            "agentId": agent_id,
            "message": "Preparing agent install."
        }),
    );

    let result = tauri::async_runtime::spawn_blocking(move || {
        start_agent_install_core(&agent_store, payload).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("agent install task failed: {error}"))?;

    match &result {
        Ok(status) => emit_agent_install_updated(
            &app,
            serde_json::json!({
                "agentId": status.catalog.id,
                "state": status.install_state,
                "detail": status.detail
            }),
        ),
        Err(error) => emit_agent_install_updated(
            &app,
            serde_json::json!({
                "agentId": agent_id,
                "state": "failed",
                "detail": error
            }),
        ),
    }

    result
}

#[tauri::command]
fn cancel_agent_install(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<AgentInstallStatus>, String> {
    let _ = agent_id;
    get_agent_install_statuses_core(&state.agent_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn open_agent_install_page(agent_id: String) -> Result<(), String> {
    open_agent_install_page_core(&agent_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn refresh_agent_install_statuses(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AgentInstallStatus>, String> {
    get_agent_install_statuses_core(&state.agent_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_runtime_environment_catalog() -> Vec<RuntimeEnvironmentCatalogItem> {
    list_runtime_environment_catalog_core()
}

#[tauri::command]
fn get_runtime_environment_statuses() -> Result<Vec<RuntimeEnvironmentStatus>, String> {
    get_runtime_environment_statuses_core().map_err(|error| error.to_string())
}

#[tauri::command]
async fn start_runtime_environment_install(
    app: tauri::AppHandle,
    payload: StartRuntimeEnvironmentInstallPayload,
) -> Result<RuntimeEnvironmentStatus, String> {
    let kind = payload.kind;
    let version = payload.version.clone();
    emit_runtime_environment_install_updated(
        &app,
        serde_json::json!({
            "kind": kind,
            "version": version,
            "state": "installing",
            "detail": "Runtime environment install started."
        }),
    );

    let result = tauri::async_runtime::spawn_blocking(move || {
        start_runtime_environment_install_core(payload).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("runtime environment install task failed: {error}"))?;

    match &result {
        Ok(status) => emit_runtime_environment_install_updated(
            &app,
            serde_json::json!({
                "kind": status.catalog.kind,
                "version": status.active_version,
                "state": status.install_state,
                "detail": status.detail
            }),
        ),
        Err(error) => emit_runtime_environment_install_updated(
            &app,
            serde_json::json!({
                "kind": kind,
                "version": version,
                "state": "failed",
                "detail": error
            }),
        ),
    }

    result
}

#[tauri::command]
fn set_active_runtime_environment_version(
    payload: SetActiveRuntimeEnvironmentPayload,
) -> Result<RuntimeEnvironmentStatus, String> {
    set_active_runtime_environment_version_core(payload).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_llm_provider_configs(
    state: tauri::State<'_, AppState>,
) -> Result<LlmProviderConfigState, String> {
    state
        .llm_provider_store
        .load()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn upsert_llm_provider_config(
    state: tauri::State<'_, AppState>,
    payload: UpsertLlmProviderPayload,
) -> Result<LlmProviderConfigState, String> {
    state
        .llm_provider_store
        .upsert(payload)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_llm_provider_config(
    state: tauri::State<'_, AppState>,
    provider_id: String,
) -> Result<LlmProviderConfigState, String> {
    state
        .llm_provider_store
        .delete(&provider_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_default_llm_provider(
    state: tauri::State<'_, AppState>,
    provider_id: String,
) -> Result<LlmProviderConfigState, String> {
    state
        .llm_provider_store
        .set_default(&provider_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn test_llm_provider_config(
    state: tauri::State<'_, AppState>,
    provider_id: String,
) -> Result<LlmProviderTestRecord, String> {
    let state_value = state
        .llm_provider_store
        .load()
        .map_err(|error| error.to_string())?;
    let provider = state_value
        .providers
        .into_iter()
        .find(|provider| provider.provider_id == provider_id)
        .ok_or_else(|| format!("llm provider config not found: {provider_id}"))?;
    let credential_check = resolve_llm_api_key(&provider);
    let (ok, detail) = match credential_check {
        Ok(api_key) => llm_provider_test_outcome(&provider, api_key.is_some()),
        Err(error) => (
            false,
            format!("LLM provider credential check failed: {error}"),
        ),
    };
    let record = fresh_llm_test_record(ok, detail);
    let _ = state
        .llm_provider_store
        .record_test(&provider_id, record.clone())
        .map_err(|error| error.to_string())?;
    Ok(record)
}

#[tauri::command]
fn preview_workspace(
    state: tauri::State<'_, AppState>,
    payload: PreviewWorkspacePayload,
) -> Result<RuntimePreview, String> {
    state
        .runtime_manager
        .preview_existing_workspace_with_policy(
            payload.app_id,
            payload.mode.unwrap_or_default(),
            &state.workspace_manager,
            &payload.policy_approvals,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn retry_build_thread_preview(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    payload: RetryBuildThreadPreviewPayload,
) -> Result<RetryBuildThreadPreviewResult, String> {
    let detail = state
        .build_thread_store
        .get(&payload.thread_id)
        .map_err(|error| error.to_string())?;
    if detail.summary.status != BuildThreadStatus::PreviewBlocked {
        return Err("build thread is not blocked by preview environment".to_string());
    }
    let app_id = detail
        .summary
        .app_id
        .clone()
        .or_else(|| detail.summary.workspace_id.clone())
        .ok_or_else(|| "preview-blocked build thread has no workspace id".to_string())?;

    match state
        .runtime_manager
        .preview_existing_workspace_with_policy(
            app_id,
            detail.summary.runtime_mode,
            &state.workspace_manager,
            &payload.policy_approvals,
        ) {
        Ok(preview) => {
            append_preview_logs(
                &app,
                &state.build_thread_store,
                &payload.thread_id,
                &preview,
            );
            let thread = state
                .build_thread_store
                .update(
                    &payload.thread_id,
                    BuildThreadUpdate {
                        status: Some(BuildThreadStatus::Completed),
                        workspace_id: Some(Some(preview.app_id.clone())),
                        app_id: Some(Some(preview.app_id.clone())),
                        preview: Some(Some(preview.clone())),
                        preview_issue: Some(None),
                        error: Some(None),
                        ..BuildThreadUpdate::default()
                    },
                )
                .map_err(|error| error.to_string())?;
            emit_build_thread_updated(&app, &thread);
            let _ = app.emit("sofvary-runtime-preview", preview.clone());
            Ok(RetryBuildThreadPreviewResult { thread, preview })
        }
        Err(error) => {
            let message = format!("预览环境修复后重试失败：{error}");
            if let Ok(entry) = state.build_thread_store.append_entry(
                &payload.thread_id,
                BuildThreadEntryKind::Error,
                message.clone(),
                serde_json::json!({
                    "kind": "runtime-preview-retry-failed"
                }),
            ) {
                emit_build_thread_entry(&app, &entry);
            }
            if let Ok(thread) = state.build_thread_store.update(
                &payload.thread_id,
                BuildThreadUpdate {
                    status: Some(BuildThreadStatus::PreviewBlocked),
                    error: Some(Some(message.clone())),
                    ..BuildThreadUpdate::default()
                },
            ) {
                emit_build_thread_updated(&app, &thread);
            }
            Err(message)
        }
    }
}

#[tauri::command]
fn preview_policy(
    state: tauri::State<'_, AppState>,
    payload: PreviewPolicyPayload,
) -> Result<PolicyPreview, String> {
    let engine = PolicyEngine::new();
    let decisions = match payload.scope {
        PolicyPreviewScope::RuntimeBuild => {
            let runtime_kind = payload
                .runtime_kind
                .ok_or_else(|| "runtimeKind is required for runtime policy preview".to_string())?;
            let runtime_mode = payload.mode.unwrap_or_default();
            let mut decisions = vec![engine.evaluate_runtime_start(
                crate::core::policy_types::PolicyRuntimeStartRequest {
                    workspace_root: PathBuf::from("."),
                    runtime_kind: format!(
                        "{}:{}",
                        runtime_kind_label(&runtime_kind),
                        runtime_mode_label(runtime_mode)
                    ),
                    bind: "127.0.0.1".to_string(),
                    network: "local-only".to_string(),
                },
            )];
            if runtime_requires_dependency_install(&runtime_kind) {
                decisions.extend(dependency_install_policy_preview_specs().into_iter().map(
                    |command| {
                        engine.evaluate_dependency_install(PolicyCommandRequest {
                            name: "install".to_string(),
                            command,
                        })
                    },
                ));
            }
            let agent_config = state
                .agent_store
                .resolve_agent(payload.agent_id.as_deref())
                .map_err(|error| error.to_string())?;
            if let Some(acp) = &agent_config.acp {
                decisions.push(engine.evaluate_external_agent_process(
                    PolicyExternalAgentProcessRequest {
                        agent_id: agent_config.id.clone(),
                        provider: agent_config.provider.as_str().to_string(),
                        transport: "acp".to_string(),
                        executable: acp.executable.display().to_string(),
                    },
                ));
            }
            if agent_config.provider == AgentProvider::SofvaryPi {
                if let Some(command) = &agent_config.cli {
                    decisions.push(engine.evaluate_external_agent_process(
                        PolicyExternalAgentProcessRequest {
                            agent_id: agent_config.id.clone(),
                            provider: agent_config.provider.as_str().to_string(),
                            transport: "pi-rpc".to_string(),
                            executable: command.executable.display().to_string(),
                        },
                    ));
                }
            }
            if agent_config.allow_cli_fallback
                && agent_config.last_test.as_ref().is_some_and(|record| {
                    record.ok && matches!(record.transport, AgentTransportKind::Cli)
                })
            {
                if let Some(cli) = &agent_config.cli {
                    decisions.push(engine.evaluate_external_agent_process(
                        PolicyExternalAgentProcessRequest {
                            agent_id: agent_config.id.clone(),
                            provider: agent_config.provider.as_str().to_string(),
                            transport: "cli".to_string(),
                            executable: cli.executable.display().to_string(),
                        },
                    ));
                }
            }
            decisions
        }
        PolicyPreviewScope::CapsuleImport => {
            let capsule_path = payload
                .capsule_path
                .ok_or_else(|| "capsulePath is required for capsule import policy".to_string())?;
            let bytes = fs::read(&capsule_path).map_err(|error| error.to_string())?;
            let adapter = current_adapter();
            let manifest = inspect_app_capsule_bytes_with_adapter(&bytes, adapter.as_ref())
                .map_err(|error| error.to_string())?;
            vec![engine.evaluate_capsule_import(capsule_policy_request(&manifest))]
        }
        PolicyPreviewScope::DeepLinkInstall => {
            let summary = payload.permission_summary.ok_or_else(|| {
                "permissionSummary is required for deep link policy preview".to_string()
            })?;
            vec![
                engine.evaluate_capsule_import(capsule_policy_request_from_summary(
                    payload
                        .capsule_name
                        .unwrap_or_else(|| "Sofvary App Capsule".to_string()),
                    summary,
                )),
            ]
        }
        PolicyPreviewScope::PackInstall => {
            let kind = payload.pack_kind.unwrap_or_else(|| "runtime".to_string());
            let id = payload
                .pack_id
                .unwrap_or_else(|| "unknown-pack".to_string());
            let version = payload.version.unwrap_or_else(|| "0.0.0".to_string());
            let app_id = payload.app_id;
            let mut decisions = vec![engine.evaluate_pack_install(PolicyPackInstallRequest {
                app_id: app_id.clone(),
                kind: kind.clone(),
                id: id.clone(),
                version: version.clone(),
                trust_level: "registry".to_string(),
            })];
            if let Some(app_id) = app_id {
                decisions.push(engine.evaluate_workspace_lockfile_update(
                    PolicyWorkspaceLockfileUpdateRequest {
                        app_id,
                        kind: kind.clone(),
                        id: id.clone(),
                        version: version.clone(),
                    },
                ));
            }
            if kind == "plugin" {
                decisions.push(engine.evaluate_plugin_enablement(&id));
            }
            decisions
        }
        PolicyPreviewScope::AgentInstall => {
            let agent_id = payload.agent_id.ok_or_else(|| {
                "agentId is required for agent install policy preview".to_string()
            })?;
            let subject =
                agent_install_subject_for_id(&agent_id).map_err(|error| error.to_string())?;
            let catalog = list_agent_install_catalog_core();
            let item = catalog
                .iter()
                .find(|item| item.id == agent_id)
                .ok_or_else(|| format!("agent install catalog item not found: {agent_id}"))?;
            let install_method = if item.managed {
                "managed-npm"
            } else {
                "manual-download"
            };
            vec![engine.evaluate_agent_install(PolicyAgentInstallRequest {
                agent_id,
                label: item.label.clone(),
                install_method: install_method.to_string(),
                subject,
            })]
        }
        PolicyPreviewScope::RuntimeEnvironmentInstall => {
            let kind = payload.runtime_environment_kind.ok_or_else(|| {
                "runtimeEnvironmentKind is required for runtime environment policy preview"
                    .to_string()
            })?;
            let version = payload.version.ok_or_else(|| {
                "version is required for runtime environment policy preview".to_string()
            })?;
            let subject = runtime_environment_install_subject_for(kind, &version)
                .map_err(|error| error.to_string())?;
            let parts = subject.split(':').collect::<Vec<_>>();
            let platform = parts.get(3).copied().unwrap_or("unknown").to_string();
            let sha256 = parts.get(4).copied().unwrap_or("unknown").to_string();
            vec![engine.evaluate_runtime_environment_install(
                PolicyRuntimeEnvironmentInstallRequest {
                    kind: format!("{kind:?}").to_ascii_lowercase(),
                    version,
                    platform,
                    sha256,
                    subject,
                },
            )]
        }
        PolicyPreviewScope::AppRelease => {
            let app_id = payload
                .app_id
                .ok_or_else(|| "appId is required for app release policy preview".to_string())?;
            let target_platform = payload.target_platform.ok_or_else(|| {
                "targetPlatform is required for app release policy preview".to_string()
            })?;
            let output_dir = payload.output_dir.ok_or_else(|| {
                "outputDir is required for app release policy preview".to_string()
            })?;
            let manifest = state
                .workspace_manager
                .get_workspace(app_id.clone())
                .map_err(|error| error.to_string())?;
            let lockfile = state
                .workspace_manager
                .read_lockfile_for_manifest(&manifest)
                .map_err(|error| error.to_string())?;
            let plugin_packs = if payload.selected_plugin_packs.is_empty() {
                lockfile.plugin_packs.keys().cloned().collect::<Vec<_>>()
            } else {
                payload.selected_plugin_packs
            };
            vec![
                engine.evaluate_app_release(app_release_policy_request_for_preview(
                    app_id,
                    payload.app_name.unwrap_or(manifest.name),
                    target_platform,
                    output_dir,
                    payload.include_ai_continuation,
                    manifest.mode,
                    plugin_packs,
                )),
            ]
        }
    };

    Ok(PolicyPreview { decisions })
}

fn capsule_policy_request_from_summary(
    name: String,
    summary: InstallPermissionSummary,
) -> PolicyCapsuleImportRequest {
    PolicyCapsuleImportRequest {
        name,
        network: summary.network,
        workspace_read: summary.workspace_read,
        workspace_write: summary.workspace_write,
        requested: summary.requested,
        plugin_packs: summary.plugin_packs,
    }
}

fn test_configured_agent(
    config: &AgentConfig,
) -> Result<AgentTestRecord, (AgentTransportKind, String)> {
    if config.provider == AgentProvider::SofvaryPi {
        return config
            .cli
            .as_ref()
            .ok_or_else(|| {
                (
                    AgentTransportKind::PiRpc,
                    "Sofvary Pi has no RPC command configured.".to_string(),
                )
            })
            .and_then(|command| {
                test_pi_agent(command)
                    .map(|detail| fresh_test_record(true, AgentTransportKind::PiRpc, detail))
                    .map_err(|error| (AgentTransportKind::PiRpc, error.to_string()))
            });
    }

    if let Some(acp) = &config.acp {
        return test_acp_connection(acp)
            .map(|detail| fresh_test_record(true, AgentTransportKind::Acp, detail))
            .map_err(|error| (AgentTransportKind::Acp, error.to_string()));
    }

    if config.allow_cli_fallback && config.cli.is_some() {
        return test_cli_agent(config)
            .map(|detail| fresh_test_record(true, AgentTransportKind::Cli, detail))
            .map_err(|error| (AgentTransportKind::Cli, error.to_string()));
    }

    Err((
        AgentTransportKind::Acp,
        "No ACP command or enabled CLI fallback is configured.".to_string(),
    ))
}

fn runtime_kind_label(runtime_kind: &str) -> &str {
    runtime_kind
}

fn runtime_selection_for_payload(
    requirement: &str,
    runtime_kind: Option<RuntimeKind>,
) -> RuntimeIntentSelection {
    match runtime_kind {
        Some(runtime_kind) => manual_runtime_selection(runtime_kind),
        None => select_runtime_for_intent(requirement),
    }
}

fn runtime_selection_message(selection: &RuntimeIntentSelection) -> String {
    match selection.source {
        crate::core::runtime_selector::RuntimeSelectionSource::Automatic => format!(
            "Sofvary selected {} ({}) with {:.0}% confidence. {}",
            selection.software_type,
            runtime_kind_label(&selection.runtime_kind),
            selection.confidence * 100.0,
            selection.reason
        ),
        crate::core::runtime_selector::RuntimeSelectionSource::Manual => format!(
            "Using manually selected runtime: {} ({}).",
            selection.software_type,
            runtime_kind_label(&selection.runtime_kind)
        ),
    }
}

fn runtime_mode_label(runtime_mode: RuntimeMode) -> &'static str {
    match runtime_mode {
        RuntimeMode::Dev => "dev",
        RuntimeMode::Prod => "prod",
    }
}

fn runtime_requires_dependency_install(runtime_kind: &str) -> bool {
    PackManager::new()
        .and_then(|manager| manager.resolve_runtime_packs_by_kind(runtime_kind))
        .map(|packs| {
            packs
                .runtime
                .manifest
                .executor
                .required_toolchains
                .iter()
                .any(|toolchain| toolchain == "nodejs")
        })
        .unwrap_or(false)
}

fn ensure_runtime_environment_for_runtime(runtime_kind: &str) -> Result<(), String> {
    if !runtime_requires_dependency_install(runtime_kind) {
        return Ok(());
    }

    let adapter = current_adapter();
    resolve_node_toolchain_with_adapter(adapter.as_ref()).map(|_| ()).map_err(|error| {
        format!(
            "{} requires the Node.js Toolchain before building. Install it from Settings > Runtime Environment, then start again. {error}",
            runtime_kind_label(runtime_kind)
        )
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileProcessorSelectedFilesPayload {
    app_id: String,
    selected_files: Vec<FileProcessorSelectedFileMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileProcessorDryRunPayload {
    app_id: String,
    selected_files: Vec<FileProcessorSelectedFileMetadata>,
    operations: Vec<FileProcessorDryRunOperation>,
}

#[tauri::command]
fn record_file_processor_selected_files(
    state: tauri::State<'_, AppState>,
    payload: FileProcessorSelectedFilesPayload,
) -> Result<(), String> {
    let manifest = state
        .workspace_manager
        .get_workspace(payload.app_id)
        .map_err(|error| error.to_string())?;
    record_file_processor_selected_files_log(&manifest, &payload.selected_files)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn confirm_file_processor_dry_run_plan(
    state: tauri::State<'_, AppState>,
    payload: FileProcessorDryRunPayload,
) -> Result<(), String> {
    let manifest = state
        .workspace_manager
        .get_workspace(payload.app_id)
        .map_err(|error| error.to_string())?;
    confirm_file_processor_plan(&manifest, &payload.selected_files, &payload.operations)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn export_app_capsule(
    state: tauri::State<'_, AppState>,
    payload: ExportAppCapsulePayload,
) -> Result<ExportAppCapsuleResult, String> {
    export_capsule_core(&state.workspace_manager, payload).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_app_release_capabilities() -> Result<AppReleaseCapability, String> {
    Ok(get_app_release_capabilities_core())
}

#[tauri::command]
fn get_packager_toolchain_status() -> Result<PackagerToolchainStatus, String> {
    get_packager_toolchain_status_core().map_err(|error| error.to_string())
}

#[tauri::command]
fn start_packager_toolchain_install(
    payload: StartPackagerToolchainInstallPayload,
) -> Result<PackagerToolchainStatus, String> {
    start_packager_toolchain_install_core(payload).map_err(|error| error.to_string())
}

#[tauri::command]
async fn start_app_release_job(
    state: tauri::State<'_, AppState>,
    payload: AppReleasePayload,
) -> Result<AppReleaseJob, String> {
    let manager = state.workspace_manager;
    tauri::async_runtime::spawn_blocking(move || {
        start_app_release_job_core(&manager, payload).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("app release task failed: {error}"))?
}

#[tauri::command]
fn cancel_app_release_job(payload: CancelAppReleaseJobPayload) -> Result<AppReleaseJob, String> {
    Ok(cancel_app_release_job_core(payload))
}

#[tauri::command]
fn open_app_release_output_folder(
    payload: OpenAppReleaseOutputFolderPayload,
) -> Result<(), String> {
    current_adapter()
        .reveal_path(&payload.path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn import_app_capsule(
    state: tauri::State<'_, AppState>,
    payload: ImportAppCapsulePayload,
) -> Result<ImportAppCapsuleResult, String> {
    import_capsule_core(&state.workspace_manager, payload).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_account_refresh_token(payload: AccountRefreshTokenPayload) -> Result<(), String> {
    current_adapter()
        .secure_store_set(ACCOUNT_REFRESH_TOKEN_KEY, &payload.refresh_token)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_account_refresh_token() -> Result<Option<String>, String> {
    let token = current_adapter()
        .secure_store_get(ACCOUNT_REFRESH_TOKEN_KEY)
        .map_err(|error| error.to_string())?;
    Ok(token.filter(|value| !value.trim().is_empty()))
}

#[tauri::command]
fn clear_account_refresh_token() -> Result<(), String> {
    current_adapter()
        .secure_store_set(ACCOUNT_REFRESH_TOKEN_KEY, "")
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_sofvary_website(path: Option<String>) -> Result<(), String> {
    let base_url = sofvary_web_base_url_from_env();
    let normalized_path = path
        .filter(|value| value.starts_with('/') && !value.starts_with("//"))
        .unwrap_or_else(|| "/".to_string());
    let url = format!("{}{}", base_url.trim_end_matches('/'), normalized_path);
    current_adapter()
        .open_external(&url)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_installed_packs(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<InstalledPackSummary>, String> {
    state
        .pack_manager
        .list_installed_packs()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn resolve_registry_pack(
    payload: ResolveRegistryPackPayload,
) -> Result<RegistryResolveResponse, String> {
    PackRegistryInstaller::from_env()
        .resolve_pack(&payload.id, &payload.version)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn install_registry_pack(
    state: tauri::State<'_, AppState>,
    payload: InstallRegistryPackPayload,
) -> Result<InstallRegistryPackResult, String> {
    PackRegistryInstaller::from_env()
        .install_pack(&state.pack_manager, &state.workspace_manager, payload)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn install_registry_skill(
    payload: InstallRegistrySkillPayload,
) -> Result<InstalledSkillSummary, String> {
    SkillRegistryInstaller::from_env()
        .install_skill(payload)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn prepare_deep_link_install(
    payload: DeepLinkInstallPayload,
) -> Result<DeepLinkInstallPreflight, String> {
    prepare_deep_link_core(payload).map_err(|error| error.to_string())
}

#[tauri::command]
fn install_app_from_deep_link(
    state: tauri::State<'_, AppState>,
    payload: ConfirmDeepLinkInstallPayload,
) -> Result<DeepLinkInstallResult, String> {
    let _guard = state
        .deep_link_install_lock
        .lock()
        .map_err(|_| "deep link install lock poisoned".to_string())?;
    install_deep_link_core(&state.workspace_manager, &state.runtime_manager, payload)
        .map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            if !shell_integration_disabled() {
                setup_tray(app)?;
                setup_global_shortcuts(app)?;
            }
            setup_protocol_handler()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap_platform,
            simulate_global_shortcut,
            show_main_window,
            show_command_window,
            hide_command_window,
            minimize_command_window,
            start_window_drag,
            minimize_shell_window,
            hide_shell_window,
            toggle_shell_window_maximize,
            snap_command_window,
            create_workspace,
            list_workspaces,
            get_workspace,
            rename_workspace,
            delete_workspace,
            create_snapshot,
            list_snapshots,
            rollback_snapshot,
            run_fake_static_app,
            run_generated_app,
            analyze_build_intent,
            start_build_thread,
            list_build_threads,
            get_build_thread,
            rename_build_thread,
            delete_build_thread,
            continue_build_thread,
            cancel_build_thread,
            copy_handoff_prompt,
            open_handoff_workspace,
            open_handoff_agent,
            rescan_handoff_workspace,
            copy_handoff_repair_prompt,
            discover_agents,
            list_agent_configs,
            upsert_agent_config,
            delete_agent_config,
            set_default_agent,
            test_agent_connection,
            list_agent_install_catalog,
            get_agent_install_statuses,
            start_agent_install,
            cancel_agent_install,
            open_agent_install_page,
            refresh_agent_install_statuses,
            list_runtime_environment_catalog,
            get_runtime_environment_statuses,
            start_runtime_environment_install,
            set_active_runtime_environment_version,
            list_llm_provider_configs,
            upsert_llm_provider_config,
            delete_llm_provider_config,
            set_default_llm_provider,
            test_llm_provider_config,
            preview_workspace,
            retry_build_thread_preview,
            record_file_processor_selected_files,
            confirm_file_processor_dry_run_plan,
            preview_policy,
            export_app_capsule,
            get_app_release_capabilities,
            get_packager_toolchain_status,
            start_packager_toolchain_install,
            start_app_release_job,
            cancel_app_release_job,
            open_app_release_output_folder,
            import_app_capsule,
            set_account_refresh_token,
            get_account_refresh_token,
            clear_account_refresh_token,
            open_sofvary_website,
            list_installed_packs,
            resolve_registry_pack,
            install_registry_pack,
            install_registry_skill,
            prepare_deep_link_install,
            install_app_from_deep_link
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sofvary desktop app");
}

type SetupResult = Result<(), Box<dyn std::error::Error>>;

fn shell_integration_disabled() -> bool {
    env::var("SOFVARY_SAFE_SHELL")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn setup_tray(app: &mut tauri::App) -> SetupResult {
    let adapter = current_adapter();
    if !tray_or_menu_bar_available(adapter.os()) {
        return Ok(());
    }
    adapter.show_tray_or_menu_bar_item()?;

    let icon = app.default_window_icon().cloned().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "missing Sofvary tray icon")
    })?;
    let show_item = IconMenuItem::with_id(
        app,
        "show_sofvary",
        "Sofvary窗口",
        true,
        Some(icon.clone()),
        None::<&str>,
    )?;
    let quit_item = IconMenuItem::with_id(
        app,
        "quit_sofvary",
        "退出",
        true,
        Some(tray_quit_menu_icon()),
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show_item, &separator, &quit_item])?;

    TrayIconBuilder::with_id("sofvary")
        .tooltip("Sofvary")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show_sofvary" => {
                let _ = show_command_window_for_app(app);
            }
            "quit_sofvary" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_command_window_for_app(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn tray_quit_menu_icon() -> Image<'static> {
    const SIZE: usize = 16;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    let color = [248_u8, 113_u8, 113_u8, 255_u8];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 8.2;
            let distance = (dx * dx + dy * dy).sqrt();
            let in_ring = (4.5..=5.8).contains(&distance) && !(y <= 5 && (6..=9).contains(&x));
            let in_stem = (7..=8).contains(&x) && (2..=8).contains(&y);

            if in_ring || in_stem {
                let offset = (y * SIZE + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&color);
            }
        }
    }

    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}

fn setup_global_shortcuts(app: &mut tauri::App) -> SetupResult {
    let adapter = current_adapter();
    let shortcut_sequence = global_shortcut_sequence(adapter.os());
    adapter.register_global_shortcut(shortcut_sequence.display)?;

    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_shortcuts([
            shortcut_sequence.first_accelerator,
            shortcut_sequence.second_accelerator,
        ])?
        .with_handler(move |app, shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }

            if shortcut_matches_sequence_key(
                shortcut,
                shortcut_sequence.modifier,
                shortcut_sequence.first_key,
            ) {
                if let Ok(mut last_shortcut_start_at) =
                    app.state::<AppState>().last_shortcut_start_at.lock()
                {
                    last_shortcut_start_at.replace(Instant::now());
                }
                return;
            }

            if shortcut_matches_sequence_key(
                shortcut,
                shortcut_sequence.modifier,
                shortcut_sequence.second_key,
            ) {
                let should_summon = app
                    .state::<AppState>()
                    .last_shortcut_start_at
                    .lock()
                    .map(|mut last_shortcut_start_at| {
                        let should_summon = last_shortcut_start_at
                            .take()
                            .map(|pressed_at| {
                                pressed_at.elapsed()
                                    <= Duration::from_millis(shortcut_sequence.window_ms)
                            })
                            .unwrap_or(false);
                        should_summon
                    })
                    .unwrap_or(false);

                if should_summon {
                    let _ = show_command_window_for_app(app);
                }
            }
        })
        .build();

    if let Err(error) = app.handle().plugin(plugin) {
        let message = error.to_string();
        if is_registered_hotkey_conflict(&message) {
            eprintln!(
                "Sofvary global shortcut {} is unavailable because another process already registered it. Continuing without the OS-level shortcut.",
                shortcut_sequence.display
            );
            return Ok(());
        }

        return Err(Box::new(error));
    }

    Ok(())
}

fn is_registered_hotkey_conflict(message: &str) -> bool {
    message.contains("HotKey already registered")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_registered_hotkey_conflicts() {
        assert!(is_registered_hotkey_conflict(
            "failed to initialize plugin `global-shortcut`: HotKey already registered: HotKey { mods: Modifiers(ALT), key: KeyA, id: 65555 }"
        ));
        assert!(!is_registered_hotkey_conflict(
            "failed to initialize plugin `global-shortcut`: permission denied"
        ));
    }

    #[test]
    fn llm_provider_test_fails_when_required_key_is_missing() {
        let provider = llm_provider_for_test(LlmProviderKind::KimiCoding);
        let (ok, detail) = llm_provider_test_outcome(&provider, false);

        assert!(!ok);
        assert!(detail.contains("KIMI_API_KEY"));
    }

    #[test]
    fn llm_provider_test_passes_when_required_key_is_available() {
        let provider = llm_provider_for_test(LlmProviderKind::KimiCoding);
        let (ok, detail) = llm_provider_test_outcome(&provider, true);

        assert!(ok);
        assert!(detail.contains("credential is available"));
    }

    #[test]
    fn llm_provider_test_allows_keyless_local_providers() {
        for kind in [LlmProviderKind::Ollama, LlmProviderKind::OpenaiCompatible] {
            let provider = llm_provider_for_test(kind);
            let (ok, detail) = llm_provider_test_outcome(&provider, false);

            assert!(ok);
            assert!(detail.contains("API key is optional"));
        }
    }

    fn llm_provider_for_test(kind: LlmProviderKind) -> LlmProviderConfig {
        LlmProviderConfig {
            provider_id: kind.as_pi_provider().to_string(),
            label: kind.as_pi_provider().to_string(),
            kind,
            base_url: None,
            model: "model".to_string(),
            api_key_ref: Some("sofvary.llm-provider.test.api-key".to_string()),
            enabled: true,
            last_test: None,
        }
    }
}

fn shortcut_matches_sequence_key(
    shortcut: &Shortcut,
    modifier: ShortcutModifier,
    key: ShortcutKey,
) -> bool {
    shortcut.matches(tauri_shortcut_modifier(modifier), tauri_shortcut_code(key))
}

fn tauri_shortcut_modifier(modifier: ShortcutModifier) -> Modifiers {
    match modifier {
        ShortcutModifier::Alt => Modifiers::ALT,
    }
}

fn tauri_shortcut_code(key: ShortcutKey) -> Code {
    match key {
        ShortcutKey::A => Code::KeyA,
        ShortcutKey::I => Code::KeyI,
    }
}

fn setup_protocol_handler() -> SetupResult {
    current_adapter().register_protocol_handler("sofvary")?;
    Ok(())
}

fn show_command_window_for_app(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("command")
        .ok_or_else(|| "Sofvary command window is not available".to_string())?;
    let adapter = current_adapter();
    let work_area = active_work_area_for_window(&window)?;
    let size = fit_window_size_to_work_area(work_area, command_window_size(adapter.os()));

    hide_window(app, "glyph")?;
    window
        .set_size(physical_size(size))
        .map_err(|error| error.to_string())?;
    position_command_window(app)?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

fn show_main_window_for_app(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Sofvary main window is not available".to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

fn show_glyph_window_for_app(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("glyph")
        .ok_or_else(|| "Sofvary glyph window is not available".to_string())?;

    window
        .set_size(physical_size(GLYPH_WINDOW))
        .map_err(|error| error.to_string())?;
    position_glyph_bottom_right(app)?;
    window.show().map_err(|error| error.to_string())
}

fn hide_window(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(label) {
        window.hide().map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn resolve_shell_window_label(label: &str) -> Result<&'static str, String> {
    match label {
        "main" => Ok("main"),
        "command" => Ok("command"),
        "glyph" => Ok("glyph"),
        _ => Err(format!("unsupported Sofvary shell window label: {label}")),
    }
}

fn position_command_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("command")
        .ok_or_else(|| "Sofvary command window is not available".to_string())?;
    let work_area = active_work_area_for_window(&window)?;
    let position = command_window_position(work_area, window_size(&window)?);

    set_window_position(&window, position)
}

fn position_glyph_bottom_right(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("glyph")
        .ok_or_else(|| "Sofvary glyph window is not available".to_string())?;
    let monitor = window
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| window.current_monitor().ok().flatten())
        .ok_or_else(|| "Unable to resolve primary monitor".to_string())?;
    let work_area = work_area_from_monitor(&monitor);
    let position = glyph_window_position(work_area, window_size(&window)?);

    set_window_position(&window, position)
}

fn snap_command_window_to_right_if_near(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("command")
        .ok_or_else(|| "Sofvary command window is not available".to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| "Unable to resolve active monitor".to_string())?;
    let work_area = work_area_from_monitor(&monitor);
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let current_position = WindowPosition {
        x: position.x,
        y: position.y,
    };

    if let Some(position) =
        snap_command_window_position(work_area, window_size(&window)?, current_position)
    {
        set_window_position(&window, position)?;
    }

    Ok(())
}

fn physical_size(size: WindowSize) -> Size {
    Size::Physical(PhysicalSize::new(size.width, size.height))
}

fn window_size(window: &tauri::WebviewWindow) -> Result<WindowSize, String> {
    let size = window.outer_size().map_err(|error| error.to_string())?;
    Ok(WindowSize {
        width: size.width,
        height: size.height,
    })
}

fn work_area_from_monitor(monitor: &tauri::Monitor) -> crate::platform::WorkArea {
    let work_area = monitor.work_area();
    crate::platform::WorkArea {
        x: work_area.position.x,
        y: work_area.position.y,
        width: work_area.size.width,
        height: work_area.size.height,
    }
}

fn set_window_position(
    window: &tauri::WebviewWindow,
    position: WindowPosition,
) -> Result<(), String> {
    window
        .set_position(Position::Physical(PhysicalPosition::new(
            position.x, position.y,
        )))
        .map_err(|error| error.to_string())
}

fn active_work_area_for_window(
    window: &tauri::WebviewWindow,
) -> Result<crate::platform::WorkArea, String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| "Unable to resolve active monitor".to_string())?;
    Ok(work_area_from_monitor(&monitor))
}
