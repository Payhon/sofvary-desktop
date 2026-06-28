use crate::core::agent_config::AgentConfig;
use crate::core::agent_gateway::{
    summarize_agent_events, AgentEvent, AgentEventSink, AgentGateway, AgentGatewayError,
    AgentRunContext, AgentSession, ConfiguredAgentAdapter, MockAgentAdapter,
};
use crate::core::builtin_resources::get_builtin_resource;
use crate::core::canvas2d_runtime::{Canvas2dRuntime, Canvas2dRuntimeError, Canvas2dRuntimeServer};
use crate::core::gateway_uni_event::{GatewayUniEventEmitter, GatewayUniEventSink};
use crate::core::harness_engine::{
    summarize_prompt_envelope, HarnessEngine, HarnessEngineError, PromptEnvelope,
    PromptEnvelopeSummary,
};
use crate::core::pack_manager::{PackError, PackManager, RuntimePackResolution};
use crate::core::pack_types::{HarnessPackManifest, RuntimePackManifest};
use crate::core::policy_engine::{PolicyEngine, PolicyError};
use crate::core::policy_types::{PolicyApprovalSet, PolicyRuntimeStartRequest};
use crate::core::prompt_template::render_template;
use crate::core::react_project_runtime::{
    ReactProjectRuntime, ReactProjectRuntimeError, ReactProjectRuntimeServer,
};
use crate::core::react_sqlite_runtime::{
    ReactSqliteRuntime, ReactSqliteRuntimeError, ReactSqliteRuntimeServer,
};
use crate::core::react_vite_runtime::{
    ReactViteRuntime, ReactViteRuntimeError, ReactViteRuntimeServer,
};
use crate::core::runtime_diagnostic::{
    diagnostic_from_react_project_error, diagnostic_from_react_sqlite_error,
    diagnostic_from_react_vite_error, RuntimeDiagnostic, RuntimeDiagnosticCategory,
    RuntimeDiagnosticRepairTarget,
};
use crate::core::software_naming::suggest_software_name;
use crate::core::static_html_runtime::{
    StaticHtmlRuntime, StaticRuntimeError, StaticRuntimeServer,
};
use crate::core::workspace_manager::{WorkspaceError, WorkspaceManager};
use crate::core::workspace_types::{AppBoxManifest, RuntimeKind, RuntimeMode};
use crate::platform::{current_adapter, PlatformAdapter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;

const MAX_RUNTIME_REPAIR_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePreview {
    pub app_id: String,
    pub runtime_kind: RuntimeKind,
    pub runtime_mode: RuntimeMode,
    pub preview_url: String,
    pub logs: Vec<String>,
    pub manifest: AppBoxManifest,
    pub prompt_envelope_summary: PromptEnvelopeSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePreviewIssue {
    pub kind: String,
    pub runtime_kind: RuntimeKind,
    pub summary: String,
    pub diagnostic: RuntimeDiagnostic,
    pub source_detail: String,
    pub repair_action: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeAssetsReady {
    pub app_id: String,
    pub runtime_kind: RuntimeKind,
    pub runtime_mode: RuntimeMode,
    pub manifest: AppBoxManifest,
    pub prompt_envelope_summary: PromptEnvelopeSummary,
}

#[derive(Debug, Error)]
pub enum RuntimeManagerError {
    #[error("pack error: {0}")]
    Pack(#[from] PackError),
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("static runtime error: {0}")]
    StaticRuntime(#[from] StaticRuntimeError),
    #[error("react-vite runtime error: {0}")]
    ReactViteRuntime(#[from] ReactViteRuntimeError),
    #[error("react-sqlite runtime error: {0}")]
    ReactSqliteRuntime(#[from] ReactSqliteRuntimeError),
    #[error("react-project runtime error: {0}")]
    ReactProjectRuntime(#[from] ReactProjectRuntimeError),
    #[error("canvas2d runtime error: {0}")]
    Canvas2dRuntime(#[from] Canvas2dRuntimeError),
    #[error("harness engine error: {0}")]
    HarnessEngine(#[from] HarnessEngineError),
    #[error("agent gateway error: {0}")]
    AgentGateway(#[from] AgentGatewayError),
    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),
    #[error("runtime auto-repair exhausted after {attempts} attempts: {summary}")]
    RuntimeRepairExhausted {
        attempts: usize,
        summary: String,
        diagnostic: RuntimeDiagnostic,
    },
    #[error("runtime blocked before preview: {summary}")]
    RuntimeDiagnosticBlocked {
        summary: String,
        diagnostic: RuntimeDiagnostic,
        source_detail: String,
        assets: Option<Box<RuntimeAssetsReady>>,
    },
    #[error("runtime lock poisoned")]
    LockPoisoned,
    #[error("imported workspace is invalid: {0}")]
    InvalidImportedWorkspace(String),
    #[error("continuation workspace is invalid: {0}")]
    InvalidContinuation(String),
    #[error("unsupported runtime kind '{0}'")]
    UnsupportedRuntimeKind(String),
}

pub struct RuntimeManager {
    harness_engine: HarnessEngine,
    static_runtime: StaticHtmlRuntime,
    react_vite_runtime: ReactViteRuntime,
    react_sqlite_runtime: ReactSqliteRuntime,
    canvas2d_runtime: Canvas2dRuntime,
    active_apps: Mutex<HashMap<String, ActiveRuntimeServer>>,
}

enum ActiveRuntimeServer {
    Static(StaticRuntimeServer),
    ReactVite(ReactViteRuntimeServer),
    ReactSqlite(ReactSqliteRuntimeServer),
    ReactProject(ReactProjectRuntimeServer),
    Canvas2d(Canvas2dRuntimeServer),
}

impl Drop for ActiveRuntimeServer {
    fn drop(&mut self) {
        match self {
            Self::Static(server) => server.stop(),
            Self::ReactVite(server) => server.stop(),
            Self::ReactSqlite(server) => server.stop(),
            Self::ReactProject(server) => server.stop(),
            Self::Canvas2d(server) => server.stop(),
        }
    }
}

#[derive(Clone)]
enum RuntimeAgentSelection {
    Mock,
    Configured {
        config: AgentConfig,
        event_sink: Option<AgentEventSink>,
        gateway_thread_id: Option<String>,
        gateway_event_sink: Option<GatewayUniEventSink>,
    },
}

impl RuntimeManager {
    pub fn new() -> Self {
        Self {
            harness_engine: HarnessEngine::new(),
            static_runtime: StaticHtmlRuntime::new(),
            react_vite_runtime: ReactViteRuntime::new(),
            react_sqlite_runtime: ReactSqliteRuntime::new(),
            canvas2d_runtime: Canvas2dRuntime::new(),
            active_apps: Mutex::new(HashMap::new()),
        }
    }

    pub fn build_and_preview_static_app(
        &self,
        requirement: String,
        workspace_manager: &WorkspaceManager,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        self.build_and_preview_app(
            requirement,
            "static-html".to_string(),
            RuntimeMode::Dev,
            workspace_manager,
        )
    }

    pub fn build_and_preview_app(
        &self,
        requirement: String,
        runtime_kind: RuntimeKind,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        self.build_and_preview_app_with_policy(
            requirement,
            runtime_kind,
            runtime_mode,
            workspace_manager,
            &PolicyApprovalSet::default(),
        )
    }

    pub fn build_and_preview_app_with_policy(
        &self,
        requirement: String,
        runtime_kind: RuntimeKind,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        self.build_and_preview_app_with_agent_selection(
            requirement,
            runtime_kind,
            runtime_mode,
            workspace_manager,
            approvals,
            &RuntimeAgentSelection::Mock,
        )
    }

    pub fn build_and_preview_app_with_agent_policy(
        &self,
        requirement: String,
        runtime_kind: RuntimeKind,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_config: &AgentConfig,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        self.build_and_preview_app_with_agent_selection(
            requirement,
            runtime_kind,
            runtime_mode,
            workspace_manager,
            approvals,
            &RuntimeAgentSelection::Configured {
                config: agent_config.clone(),
                event_sink: None,
                gateway_thread_id: None,
                gateway_event_sink: None,
            },
        )
    }

    pub fn build_and_preview_app_with_agent_policy_and_events(
        &self,
        requirement: String,
        runtime_kind: RuntimeKind,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_config: &AgentConfig,
        event_sink: Option<AgentEventSink>,
        gateway_thread_id: Option<String>,
        gateway_event_sink: Option<GatewayUniEventSink>,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        self.build_and_preview_app_with_agent_selection(
            requirement,
            runtime_kind,
            runtime_mode,
            workspace_manager,
            approvals,
            &RuntimeAgentSelection::Configured {
                config: agent_config.clone(),
                event_sink,
                gateway_thread_id,
                gateway_event_sink,
            },
        )
    }

    pub fn continue_existing_app_with_agent_policy_and_events(
        &self,
        requirement: String,
        app_id: String,
        runtime_kind: RuntimeKind,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_config: &AgentConfig,
        event_sink: Option<AgentEventSink>,
        gateway_thread_id: Option<String>,
        gateway_event_sink: Option<GatewayUniEventSink>,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let adapter = current_adapter();
        let manifest =
            workspace_manager.get_workspace_with_adapter(app_id.clone(), adapter.as_ref())?;
        if manifest.mode != runtime_kind {
            return Err(RuntimeManagerError::InvalidContinuation(format!(
                "thread runtime {:?} does not match workspace runtime {:?} for {app_id}",
                runtime_kind, manifest.mode
            )));
        }

        let pack_manager = PackManager::new_with_adapter(adapter.as_ref())?;
        let lockfile = workspace_manager.read_lockfile_for_manifest(&manifest)?;
        let (runtime_pack, harness_pack) =
            resolve_single_workspace_runtime_and_harness(&pack_manager, &lockfile)?;

        self.build_existing_workspace_with_agent_selection(
            requirement,
            manifest,
            runtime_mode,
            workspace_manager,
            &runtime_pack,
            &harness_pack,
            approvals,
            &RuntimeAgentSelection::Configured {
                config: agent_config.clone(),
                event_sink,
                gateway_thread_id,
                gateway_event_sink,
            },
        )
    }

    fn build_and_preview_app_with_agent_selection(
        &self,
        requirement: String,
        runtime_kind: RuntimeKind,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_runtime_packs_by_kind(&runtime_kind)?;
        let executor_kind = packs.runtime.manifest.executor.kind.clone();
        match executor_kind.as_str() {
            "static-html" => self.build_and_preview_static_app_inner(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
                packs,
            ),
            "react-vite" => self.build_and_preview_react_vite_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
                packs,
            ),
            "react-sqlite" => self.build_and_preview_react_sqlite_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
                packs,
            ),
            "react-project" => self.build_and_preview_react_project_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
                packs,
            ),
            "canvas2d" => self.build_and_preview_canvas2d_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
                packs,
            ),
            other => Err(RuntimeManagerError::UnsupportedRuntimeKind(format!(
                "{runtime_kind} uses unsupported executor {other}"
            ))),
        }
    }

    fn build_existing_workspace_with_agent_selection(
        &self,
        requirement: String,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        match runtime_pack.executor.kind.as_str() {
            "static-html" => self.build_existing_static_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            "react-vite" => self.build_existing_react_vite_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            "react-sqlite" => self.build_existing_react_sqlite_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            "react-project" => self.build_existing_react_project_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            "canvas2d" => self.build_existing_canvas2d_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            other => Err(RuntimeManagerError::UnsupportedRuntimeKind(format!(
                "{} uses unsupported executor {other}",
                runtime_pack.runtime.kind
            ))),
        }
    }

    #[allow(dead_code)]
    pub fn preview_existing_workspace(
        &self,
        app_id: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let adapter = current_adapter();
        self.preview_existing_workspace_with_adapter(
            app_id,
            runtime_mode,
            workspace_manager,
            adapter.as_ref(),
            &PolicyApprovalSet::default(),
        )
    }

    pub fn preview_existing_workspace_with_policy(
        &self,
        app_id: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let adapter = current_adapter();
        self.preview_existing_workspace_with_adapter(
            app_id,
            runtime_mode,
            workspace_manager,
            adapter.as_ref(),
            approvals,
        )
    }

    pub fn preview_existing_workspace_with_adapter(
        &self,
        app_id: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        adapter: &dyn PlatformAdapter,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let manifest = workspace_manager.get_workspace_with_adapter(app_id, adapter)?;
        let pack_manager = PackManager::new_with_adapter(adapter)?;
        let lockfile = workspace_manager.read_lockfile_for_manifest(&manifest)?;
        let (runtime_pack, harness_pack) =
            resolve_single_workspace_runtime_and_harness(&pack_manager, &lockfile)?;

        match runtime_pack.executor.kind.as_str() {
            "static-html" => self.preview_existing_static_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            "react-vite" => self.preview_existing_react_vite_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            "react-sqlite" => self.preview_existing_react_sqlite_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            "react-project" => self.preview_existing_react_project_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            "canvas2d" => self.preview_existing_canvas2d_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            other => Err(RuntimeManagerError::UnsupportedRuntimeKind(format!(
                "{} uses unsupported executor {other}",
                runtime_pack.runtime.kind
            ))),
        }
    }

    pub fn stop_active_app(&self, app_id: &str) -> Result<bool, RuntimeManagerError> {
        let removed = self
            .active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .remove(app_id)
            .is_some();
        Ok(removed)
    }

    fn build_and_preview_static_app_inner(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        packs: RuntimePackResolution,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let name = derive_app_name(&requirement);
        let adapter = current_adapter();
        let manifest = workspace_manager.create_workspace_for_resolved_packs(
            name,
            packs.runtime.clone(),
            packs.harness.clone(),
            adapter.as_ref(),
        )?;
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            &packs.runtime.manifest,
            &packs.harness.manifest,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, &packs.runtime.manifest.runtime.kind, approvals)?;
        let server = self
            .static_runtime
            .start_workspace_with_envelope(&manifest, &prompt_envelope)?;
        let preview = server.preview();
        let logs = [
            preview.logs,
            summarize_agent_events(&agent_session.events),
            vec![
                format!(
                    "Resolved runtime pack {}@{}",
                    packs.runtime.manifest.id, packs.runtime.manifest.version
                ),
                format!(
                    "Resolved harness pack {}@{}",
                    packs.harness.manifest.id, packs.harness.manifest.version
                ),
                format!(
                    "Compiled PromptEnvelope {} with {} acceptance criteria",
                    prompt_envelope.envelope_id,
                    prompt_envelope.acceptance_criteria.len()
                ),
                format!(
                    "Agent session {} completed for envelope {}",
                    agent_session.session_id, agent_session.envelope_id
                ),
            ],
        ]
        .concat();

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(manifest.app_id.clone(), ActiveRuntimeServer::Static(server));

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: packs.runtime.manifest.runtime.kind.clone(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_static_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, "static-html", approvals)?;
        let server = self
            .static_runtime
            .start_workspace_with_envelope(&manifest, &prompt_envelope)?;
        let preview = server.preview();
        let logs = preview_existing_logs(
            &preview.logs,
            &manifest.app_id,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(manifest.app_id.clone(), ActiveRuntimeServer::Static(server));

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: "static-html".to_string(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_react_vite_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        packs: RuntimePackResolution,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let name = derive_app_name(&requirement);
        let adapter = current_adapter();
        let manifest = workspace_manager.create_workspace_for_resolved_packs(
            name,
            packs.runtime.clone(),
            packs.harness.clone(),
            adapter.as_ref(),
        )?;
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            &packs.runtime.manifest,
            &packs.harness.manifest,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, &packs.runtime.manifest.runtime.kind, approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            runtime_mode,
            || {
                self.react_vite_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        &packs.runtime.manifest,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| {
                diagnostic_from_react_vite_error(packs.runtime.manifest.runtime.kind.clone(), error)
            },
            RuntimeManagerError::ReactViteRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_envelope(
                    repair_prompt,
                    &manifest,
                    &packs.runtime.manifest,
                    &packs.harness.manifest,
                )?)
            },
        )?;
        let preview = server.preview();
        let logs = [
            preview.logs,
            summarize_agent_events(&agent_session.events),
            vec![
                format!(
                    "Resolved runtime pack {}@{}",
                    packs.runtime.manifest.id, packs.runtime.manifest.version
                ),
                format!(
                    "Resolved harness pack {}@{}",
                    packs.harness.manifest.id, packs.harness.manifest.version
                ),
                format!(
                    "Compiled PromptEnvelope {} with {} acceptance criteria",
                    prompt_envelope.envelope_id,
                    prompt_envelope.acceptance_criteria.len()
                ),
                format!(
                    "Agent session {} completed for envelope {}",
                    agent_session.session_id, agent_session.envelope_id
                ),
            ],
        ]
        .concat();

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactVite(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: packs.runtime.manifest.runtime.kind.clone(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_react_vite_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, "react-vite", approvals)?;
        let server = self
            .react_vite_runtime
            .start_workspace_with_envelope_with_policy(
                &manifest,
                &prompt_envelope,
                runtime_pack,
                runtime_mode,
                approvals,
            )?;
        let preview = server.preview();
        let logs = preview_existing_logs(
            &preview.logs,
            &manifest.app_id,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactVite(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: "react-vite".to_string(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_react_sqlite_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        packs: RuntimePackResolution,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let name = derive_app_name(&requirement);
        let adapter = current_adapter();
        let manifest = workspace_manager.create_workspace_for_resolved_packs(
            name,
            packs.runtime.clone(),
            packs.harness.clone(),
            adapter.as_ref(),
        )?;
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            &packs.runtime.manifest,
            &packs.harness.manifest,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, &packs.runtime.manifest.runtime.kind, approvals)?;
        let (server, agent_session) = self.start_react_sqlite_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            &packs.runtime.manifest,
            &packs.harness.manifest,
            runtime_mode,
        )?;
        let preview = server.preview();
        let logs = [
            preview.logs,
            summarize_agent_events(&agent_session.events),
            vec![
                format!(
                    "Resolved runtime pack {}@{}",
                    packs.runtime.manifest.id, packs.runtime.manifest.version
                ),
                format!(
                    "Resolved harness pack {}@{}",
                    packs.harness.manifest.id, packs.harness.manifest.version
                ),
                format!(
                    "Compiled PromptEnvelope {} with {} acceptance criteria",
                    prompt_envelope.envelope_id,
                    prompt_envelope.acceptance_criteria.len()
                ),
                format!(
                    "Agent session {} completed for envelope {}",
                    agent_session.session_id, agent_session.envelope_id
                ),
            ],
        ]
        .concat();

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactSqlite(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: packs.runtime.manifest.runtime.kind.clone(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_react_sqlite_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        workspace_manager.prepare_react_sqlite_workspace_for_preview(&manifest)?;
        self.enforce_runtime_start(&manifest, "react-sqlite", approvals)?;
        let server = self
            .react_sqlite_runtime
            .start_workspace_with_envelope_with_policy(
                &manifest,
                &prompt_envelope,
                runtime_pack,
                runtime_mode,
                approvals,
            )?;
        let preview = server.preview();
        let logs = preview_existing_logs(
            &preview.logs,
            &manifest.app_id,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactSqlite(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: "react-sqlite".to_string(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_react_project_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, &runtime_pack.runtime.kind, approvals)?;
        let server = ReactProjectRuntime::for_runtime_pack(runtime_pack)
            .start_workspace_with_envelope_with_policy(
                &manifest,
                &prompt_envelope,
                runtime_pack,
                runtime_mode,
                approvals,
            )?;
        let preview = server.preview();
        let logs = preview_existing_logs(
            &preview.logs,
            &manifest.app_id,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactProject(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: runtime_pack.runtime.kind.clone(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_react_project_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        packs: RuntimePackResolution,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let name = derive_app_name(&requirement);
        let adapter = current_adapter();
        let manifest = workspace_manager.create_workspace_for_resolved_packs(
            name,
            packs.runtime.clone(),
            packs.harness.clone(),
            adapter.as_ref(),
        )?;
        let runtime_kind = packs.runtime.manifest.runtime.kind.clone();
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            &packs.runtime.manifest,
            &packs.harness.manifest,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, &runtime_kind, approvals)?;
        let runtime = ReactProjectRuntime::for_runtime_pack(&packs.runtime.manifest);
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            runtime_mode,
            || {
                runtime.start_workspace_with_envelope_with_policy(
                    &manifest,
                    &prompt_envelope,
                    &packs.runtime.manifest,
                    runtime_mode,
                    approvals,
                )
            },
            |error| diagnostic_from_react_project_error(runtime_kind.clone(), error),
            RuntimeManagerError::ReactProjectRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_envelope(
                    repair_prompt,
                    &manifest,
                    &packs.runtime.manifest,
                    &packs.harness.manifest,
                )?)
            },
        )?;
        let preview = server.preview();
        let logs = runtime_logs(
            &preview.logs,
            &agent_session.events,
            &packs.runtime.manifest.id,
            &packs.runtime.manifest.version,
            &packs.harness.manifest.id,
            &packs.harness.manifest.version,
            &prompt_envelope.envelope_id,
            prompt_envelope.acceptance_criteria.len(),
            &agent_session,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactProject(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_canvas2d_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        packs: RuntimePackResolution,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let name = derive_app_name(&requirement);
        let adapter = current_adapter();
        let manifest = workspace_manager.create_workspace_for_resolved_packs(
            name,
            packs.runtime.clone(),
            packs.harness.clone(),
            adapter.as_ref(),
        )?;
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            &packs.runtime.manifest,
            &packs.harness.manifest,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, &packs.runtime.manifest.runtime.kind, approvals)?;
        let server = self
            .canvas2d_runtime
            .start_workspace_with_envelope(&manifest, &prompt_envelope)?;
        let preview = server.preview();
        let logs = [
            preview.logs,
            summarize_agent_events(&agent_session.events),
            vec![
                format!(
                    "Resolved runtime pack {}@{}",
                    packs.runtime.manifest.id, packs.runtime.manifest.version
                ),
                format!(
                    "Resolved harness pack {}@{}",
                    packs.harness.manifest.id, packs.harness.manifest.version
                ),
                format!(
                    "Compiled PromptEnvelope {} with {} acceptance criteria",
                    prompt_envelope.envelope_id,
                    prompt_envelope.acceptance_criteria.len()
                ),
                format!(
                    "Agent session {} completed for envelope {}",
                    agent_session.session_id, agent_session.envelope_id
                ),
            ],
        ]
        .concat();

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::Canvas2d(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: packs.runtime.manifest.runtime.kind.clone(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_canvas2d_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, "canvas2d", approvals)?;
        let server = self
            .canvas2d_runtime
            .start_workspace_with_envelope(&manifest, &prompt_envelope)?;
        let preview = server.preview();
        let logs = preview_existing_logs(
            &preview.logs,
            &manifest.app_id,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::Canvas2d(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: "canvas2d".to_string(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_static_workspace(
        &self,
        requirement: String,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, "static-html", approvals)?;
        let server = self
            .static_runtime
            .start_workspace_with_envelope(&manifest, &prompt_envelope)?;
        let preview = server.preview();
        let logs = runtime_logs(
            &preview.logs,
            &agent_session.events,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
            &prompt_envelope.envelope_id,
            prompt_envelope.acceptance_criteria.len(),
            &agent_session,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(manifest.app_id.clone(), ActiveRuntimeServer::Static(server));

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: "static-html".to_string(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_react_vite_workspace(
        &self,
        requirement: String,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, "react-vite", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            runtime_mode,
            || {
                self.react_vite_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        runtime_pack,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_vite_error("react-vite".to_string(), error),
            RuntimeManagerError::ReactViteRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_envelope(
                    repair_prompt,
                    &manifest,
                    runtime_pack,
                    harness_pack,
                )?)
            },
        )?;
        let preview = server.preview();
        let logs = runtime_logs(
            &preview.logs,
            &agent_session.events,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
            &prompt_envelope.envelope_id,
            prompt_envelope.acceptance_criteria.len(),
            &agent_session,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactVite(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: "react-vite".to_string(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_react_sqlite_workspace(
        &self,
        requirement: String,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, "react-sqlite", approvals)?;
        let (server, agent_session) = self.start_react_sqlite_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            runtime_pack,
            harness_pack,
            runtime_mode,
        )?;
        let preview = server.preview();
        let logs = runtime_logs(
            &preview.logs,
            &agent_session.events,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
            &prompt_envelope.envelope_id,
            prompt_envelope.acceptance_criteria.len(),
            &agent_session,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactSqlite(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: "react-sqlite".to_string(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_react_project_workspace(
        &self,
        requirement: String,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let runtime_kind = runtime_pack.runtime.kind.clone();
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, &runtime_kind, approvals)?;
        let runtime = ReactProjectRuntime::for_runtime_pack(runtime_pack);
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            runtime_mode,
            || {
                runtime.start_workspace_with_envelope_with_policy(
                    &manifest,
                    &prompt_envelope,
                    runtime_pack,
                    runtime_mode,
                    approvals,
                )
            },
            |error| diagnostic_from_react_project_error(runtime_kind.clone(), error),
            RuntimeManagerError::ReactProjectRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_envelope(
                    repair_prompt,
                    &manifest,
                    runtime_pack,
                    harness_pack,
                )?)
            },
        )?;
        let preview = server.preview();
        let logs = runtime_logs(
            &preview.logs,
            &agent_session.events,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
            &prompt_envelope.envelope_id,
            prompt_envelope.acceptance_criteria.len(),
            &agent_session,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::ReactProject(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_canvas2d_workspace(
        &self,
        requirement: String,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_envelope(
            &requirement,
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        let agent_session = self.run_agent_for_build(
            &manifest,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
        )?;
        self.enforce_runtime_start(&manifest, "canvas2d", approvals)?;
        let server = self
            .canvas2d_runtime
            .start_workspace_with_envelope(&manifest, &prompt_envelope)?;
        let preview = server.preview();
        let logs = runtime_logs(
            &preview.logs,
            &agent_session.events,
            &runtime_pack.id,
            &runtime_pack.version,
            &harness_pack.id,
            &harness_pack.version,
            &prompt_envelope.envelope_id,
            prompt_envelope.acceptance_criteria.len(),
            &agent_session,
        );

        self.active_apps
            .lock()
            .map_err(|_| RuntimeManagerError::LockPoisoned)?
            .insert(
                manifest.app_id.clone(),
                ActiveRuntimeServer::Canvas2d(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: "canvas2d".to_string(),
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn enforce_runtime_start(
        &self,
        manifest: &AppBoxManifest,
        runtime_kind: &str,
        approvals: &PolicyApprovalSet,
    ) -> Result<(), RuntimeManagerError> {
        let engine = PolicyEngine::new();
        let decision = engine.evaluate_runtime_start(PolicyRuntimeStartRequest {
            workspace_root: manifest.paths.root.clone(),
            runtime_kind: runtime_kind.to_string(),
            bind: "127.0.0.1".to_string(),
            network: "local-only".to_string(),
        });
        Ok(engine.enforce(decision, approvals)?)
    }

    fn start_react_sqlite_runtime_with_repair(
        &self,
        manifest: &AppBoxManifest,
        original_requirement: &str,
        initial_envelope: &PromptEnvelope,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        agent_session: AgentSession,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        runtime_mode: RuntimeMode,
    ) -> Result<(ReactSqliteRuntimeServer, AgentSession), RuntimeManagerError> {
        self.start_runtime_with_repair_and_fallback(
            manifest,
            original_requirement,
            initial_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            runtime_mode,
            || {
                self.react_sqlite_runtime
                    .start_workspace_with_envelope_with_policy(
                        manifest,
                        initial_envelope,
                        runtime_pack,
                        runtime_mode,
                        approvals,
                    )
            },
            diagnostic_from_react_sqlite_error,
            RuntimeManagerError::ReactSqliteRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_envelope(
                    repair_prompt,
                    manifest,
                    runtime_pack,
                    harness_pack,
                )?)
            },
            |diagnostic, agent_session| {
                self.record_runtime_event(
                    agent_selection,
                    agent_session,
                    AgentEvent::Planning {
                        message: "Sofvary is applying the stable React + SQLite baseline after runtime repair attempts were exhausted.".to_string(),
                    },
                );
                let fallback_prompt =
                    stable_react_sqlite_fallback_prompt(original_requirement, diagnostic);
                let fallback_envelope = self.harness_engine.create_envelope(
                    &fallback_prompt,
                    manifest,
                    runtime_pack,
                    harness_pack,
                )?;
                let fallback_context =
                    AgentRunContext::with_runtime_diagnostic(diagnostic.clone());
                let fallback_session = self.run_agent_for_build_with_context(
                    manifest,
                    &fallback_envelope,
                    workspace_manager,
                    approvals,
                    &RuntimeAgentSelection::Mock,
                    &fallback_context,
                )?;
                agent_session.events.extend(fallback_session.events);
                let server = self
                    .react_sqlite_runtime
                    .start_workspace_with_envelope_with_policy(
                        manifest,
                        &fallback_envelope,
                        runtime_pack,
                        runtime_mode,
                        approvals,
                    )
                    .map_err(RuntimeManagerError::ReactSqliteRuntime)?;
                self.record_runtime_event(
                    agent_selection,
                    agent_session,
                    AgentEvent::RepairFinished {
                        attempt: MAX_RUNTIME_REPAIR_ATTEMPTS,
                        summary: "Applied Sofvary stable React + SQLite baseline and runtime start succeeded.".to_string(),
                    },
                );
                Ok(Some(server))
            },
        )
    }

    fn start_runtime_with_repair<
        S,
        E,
        StartRuntime,
        ClassifyError,
        MapError,
        CreateRepairEnvelope,
    >(
        &self,
        manifest: &AppBoxManifest,
        original_requirement: &str,
        initial_envelope: &PromptEnvelope,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        agent_session: AgentSession,
        runtime_mode: RuntimeMode,
        start_runtime: StartRuntime,
        classify_error: ClassifyError,
        map_error: MapError,
        create_repair_envelope: CreateRepairEnvelope,
    ) -> Result<(S, AgentSession), RuntimeManagerError>
    where
        StartRuntime: FnMut() -> Result<S, E>,
        ClassifyError: Fn(&E) -> RuntimeDiagnostic,
        MapError: Fn(E) -> RuntimeManagerError,
        CreateRepairEnvelope: Fn(&str) -> Result<PromptEnvelope, RuntimeManagerError>,
    {
        self.start_runtime_with_repair_and_fallback(
            manifest,
            original_requirement,
            initial_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            runtime_mode,
            start_runtime,
            classify_error,
            map_error,
            create_repair_envelope,
            |_, _| Ok(None),
        )
    }

    fn start_runtime_with_repair_and_fallback<
        S,
        E,
        StartRuntime,
        ClassifyError,
        MapError,
        CreateRepairEnvelope,
        RepairExhaustedFallback,
    >(
        &self,
        manifest: &AppBoxManifest,
        original_requirement: &str,
        initial_envelope: &PromptEnvelope,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        mut agent_session: AgentSession,
        runtime_mode: RuntimeMode,
        mut start_runtime: StartRuntime,
        classify_error: ClassifyError,
        map_error: MapError,
        create_repair_envelope: CreateRepairEnvelope,
        mut repair_exhausted_fallback: RepairExhaustedFallback,
    ) -> Result<(S, AgentSession), RuntimeManagerError>
    where
        StartRuntime: FnMut() -> Result<S, E>,
        ClassifyError: Fn(&E) -> RuntimeDiagnostic,
        MapError: Fn(E) -> RuntimeManagerError,
        CreateRepairEnvelope: Fn(&str) -> Result<PromptEnvelope, RuntimeManagerError>,
        RepairExhaustedFallback:
            FnMut(&RuntimeDiagnostic, &mut AgentSession) -> Result<Option<S>, RuntimeManagerError>,
    {
        for attempt_index in 0..=MAX_RUNTIME_REPAIR_ATTEMPTS {
            match start_runtime() {
                Ok(server) => return Ok((server, agent_session)),
                Err(error) => {
                    let diagnostic = classify_error(&error);
                    self.record_runtime_event(
                        agent_selection,
                        &mut agent_session,
                        AgentEvent::RuntimeDiagnostic {
                            diagnostic: diagnostic.clone(),
                        },
                    );

                    if attempt_index >= MAX_RUNTIME_REPAIR_ATTEMPTS
                        || !diagnostic.is_agent_repairable()
                    {
                        if diagnostic.is_agent_repairable() {
                            if let Some(server) =
                                repair_exhausted_fallback(&diagnostic, &mut agent_session)?
                            {
                                return Ok((server, agent_session));
                            }
                            return Err(RuntimeManagerError::RuntimeRepairExhausted {
                                attempts: MAX_RUNTIME_REPAIR_ATTEMPTS,
                                summary: diagnostic.summary(),
                                diagnostic,
                            });
                        }
                        let source_detail = map_error(error).to_string();
                        let assets = preview_blocked_assets(
                            manifest,
                            initial_envelope,
                            runtime_mode,
                            &diagnostic,
                        );
                        return Err(RuntimeManagerError::RuntimeDiagnosticBlocked {
                            summary: diagnostic.summary(),
                            diagnostic,
                            source_detail,
                            assets,
                        });
                    }

                    let repair_attempt = attempt_index + 1;
                    self.record_runtime_event(
                        agent_selection,
                        &mut agent_session,
                        AgentEvent::RepairStarted {
                            attempt: repair_attempt,
                            max_attempts: MAX_RUNTIME_REPAIR_ATTEMPTS,
                            summary: diagnostic.summary(),
                        },
                    );

                    let repair_prompt = runtime_repair_prompt(
                        original_requirement,
                        initial_envelope,
                        &diagnostic,
                        repair_attempt,
                        MAX_RUNTIME_REPAIR_ATTEMPTS,
                    );
                    let repair_envelope = create_repair_envelope(&repair_prompt)?;
                    let repair_context =
                        AgentRunContext::with_runtime_diagnostic(diagnostic.clone());
                    let repair_session = self.run_agent_for_build_with_context(
                        manifest,
                        &repair_envelope,
                        workspace_manager,
                        approvals,
                        agent_selection,
                        &repair_context,
                    )?;
                    agent_session.events.extend(repair_session.events);
                    self.record_runtime_event(
                        agent_selection,
                        &mut agent_session,
                        AgentEvent::RepairFinished {
                            attempt: repair_attempt,
                            summary: "Retrying runtime start after Agent repair.".to_string(),
                        },
                    );
                }
            }
        }

        unreachable!("runtime repair loop always returns or retries within bounded attempts")
    }

    fn record_runtime_event(
        &self,
        agent_selection: &RuntimeAgentSelection,
        agent_session: &mut AgentSession,
        event: AgentEvent,
    ) {
        if let RuntimeAgentSelection::Configured {
            event_sink: Some(event_sink),
            ..
        } = agent_selection
        {
            event_sink(event.clone());
        }
        agent_session.events.push(event);
    }

    fn run_agent_for_build(
        &self,
        manifest: &AppBoxManifest,
        prompt_envelope: &crate::core::harness_engine::PromptEnvelope,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<crate::core::agent_gateway::AgentSession, RuntimeManagerError> {
        self.run_agent_for_build_with_context(
            manifest,
            prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            &AgentRunContext::default(),
        )
    }

    fn run_agent_for_build_with_context(
        &self,
        manifest: &AppBoxManifest,
        prompt_envelope: &crate::core::harness_engine::PromptEnvelope,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
        context: &AgentRunContext,
    ) -> Result<crate::core::agent_gateway::AgentSession, RuntimeManagerError> {
        match agent_selection {
            RuntimeAgentSelection::Mock => Ok(AgentGateway::new(MockAgentAdapter)
                .run_with_policy_and_context(
                    manifest,
                    prompt_envelope,
                    workspace_manager,
                    approvals,
                    context,
                )?),
            RuntimeAgentSelection::Configured {
                config,
                event_sink,
                gateway_thread_id,
                gateway_event_sink,
            } => {
                let mut adapter = ConfiguredAgentAdapter::new(
                    config.clone(),
                    manifest.clone(),
                    approvals.clone(),
                );
                if let Some(event_sink) = event_sink.clone() {
                    adapter = adapter.with_event_sink(event_sink);
                }
                if let (Some(thread_id), Some(gateway_event_sink)) =
                    (gateway_thread_id.clone(), gateway_event_sink.clone())
                {
                    let transport =
                        if config.provider == crate::core::agent_config::AgentProvider::SofvaryPi {
                            crate::core::agent_config::AgentTransportKind::PiNative
                        } else if config.acp.is_some() {
                            crate::core::agent_config::AgentTransportKind::Acp
                        } else {
                            crate::core::agent_config::AgentTransportKind::Cli
                        };
                    adapter = adapter.with_gateway_event_emitter(GatewayUniEventEmitter::new(
                        thread_id,
                        config.id.clone(),
                        transport,
                        gateway_event_sink,
                    ));
                }
                Ok(AgentGateway::new(adapter).run_with_policy_and_context(
                    manifest,
                    prompt_envelope,
                    workspace_manager,
                    approvals,
                    context,
                )?)
            }
        }
    }
}

impl Default for RuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

fn derive_app_name(requirement: &str) -> String {
    suggest_software_name(requirement)
}

fn stable_react_sqlite_fallback_prompt(
    original_requirement: &str,
    diagnostic: &RuntimeDiagnostic,
) -> String {
    let requirement = original_requirement.trim();
    let requirement = if requirement.is_empty() {
        "Create a local customer management panel"
    } else {
        requirement
    };
    let software_name = suggest_software_name(requirement);
    let variables = HashMap::from([
        ("software.name".to_string(), software_name),
        ("user.intent".to_string(), requirement.to_string()),
        ("diagnostic.summary".to_string(), diagnostic.summary()),
    ]);
    render_builtin_prompt_template("prompt-templates/react-sqlite-fallback.md", &variables)
}

fn runtime_repair_prompt(
    original_requirement: &str,
    envelope: &PromptEnvelope,
    diagnostic: &RuntimeDiagnostic,
    attempt: usize,
    max_attempts: usize,
) -> String {
    let stdout = diagnostic
        .stdout_tail
        .as_deref()
        .unwrap_or("no stdout captured");
    let stderr = diagnostic
        .stderr_tail
        .as_deref()
        .unwrap_or("no stderr captured");
    let command = diagnostic
        .command_name
        .as_deref()
        .unwrap_or("runtime start");
    let log_path = diagnostic
        .log_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "not available".to_string());
    let variables = HashMap::from([
        (
            "software.name".to_string(),
            suggest_software_name(original_requirement),
        ),
        ("user.intent".to_string(), original_requirement.to_string()),
        (
            "runtime.kind".to_string(),
            envelope.runtime_policy.runtime_kind.clone(),
        ),
        ("repair.attempt".to_string(), attempt.to_string()),
        ("repair.maxAttempts".to_string(), max_attempts.to_string()),
        (
            "diagnostic.stage".to_string(),
            format!("{:?}", diagnostic.stage),
        ),
        ("diagnostic.command".to_string(), command.to_string()),
        (
            "diagnostic.statusCode".to_string(),
            format!("{:?}", diagnostic.status_code),
        ),
        (
            "diagnostic.category".to_string(),
            format!("{:?}", diagnostic.category),
        ),
        ("diagnostic.logPath".to_string(), log_path),
        ("diagnostic.stdoutTail".to_string(), stdout.to_string()),
        ("diagnostic.stderrTail".to_string(), stderr.to_string()),
    ]);
    render_builtin_prompt_template("prompt-templates/runtime-repair.md", &variables)
}

fn render_builtin_prompt_template(path: &str, variables: &HashMap<String, String>) -> String {
    let template = get_builtin_resource(path).unwrap_or_else(|| {
        panic!("missing builtin prompt template resource: {path}");
    });
    render_template(template, variables).unwrap_or_else(|error| {
        panic!("failed to render builtin prompt template {path}: {error}");
    })
}

fn runtime_logs(
    preview_logs: &[String],
    agent_events: &[crate::core::agent_gateway::AgentEvent],
    runtime_pack_id: &str,
    runtime_pack_version: &str,
    harness_pack_id: &str,
    harness_pack_version: &str,
    envelope_id: &str,
    acceptance_criteria_len: usize,
    agent_session: &AgentSession,
) -> Vec<String> {
    [
        preview_logs.to_vec(),
        summarize_agent_events(agent_events),
        vec![
            format!("Resolved runtime pack {runtime_pack_id}@{runtime_pack_version}"),
            format!("Resolved harness pack {harness_pack_id}@{harness_pack_version}"),
            format!(
                "Compiled PromptEnvelope {envelope_id} with {acceptance_criteria_len} acceptance criteria"
            ),
            format!(
                "Agent session {} completed for app {} using {:?}",
                agent_session.session_id, agent_session.app_id, agent_session.adapter
            ),
        ],
    ]
    .concat()
}

fn preview_existing_logs(
    preview_logs: &[String],
    app_id: &str,
    runtime_pack_id: &str,
    runtime_pack_version: &str,
    harness_pack_id: &str,
    harness_pack_version: &str,
) -> Vec<String> {
    [
        preview_logs.to_vec(),
        vec![
            format!("Previewing existing workspace {app_id}"),
            format!("Resolved runtime pack {runtime_pack_id}@{runtime_pack_version}"),
            format!("Resolved harness pack {harness_pack_id}@{harness_pack_version}"),
            "Agent Gateway was not run for imported workspace preview".to_string(),
        ],
    ]
    .concat()
}

fn preview_blocked_assets(
    manifest: &AppBoxManifest,
    initial_envelope: &PromptEnvelope,
    runtime_mode: RuntimeMode,
    diagnostic: &RuntimeDiagnostic,
) -> Option<Box<RuntimeAssetsReady>> {
    if !diagnostic_preserves_generated_assets(diagnostic) {
        return None;
    }
    Some(Box::new(RuntimeAssetsReady {
        app_id: manifest.app_id.clone(),
        runtime_kind: diagnostic.runtime_kind.clone(),
        runtime_mode,
        manifest: manifest.clone(),
        prompt_envelope_summary: summarize_prompt_envelope(initial_envelope),
    }))
}

fn diagnostic_preserves_generated_assets(diagnostic: &RuntimeDiagnostic) -> bool {
    matches!(
        diagnostic.repairable_by,
        RuntimeDiagnosticRepairTarget::Sofvary
    ) || matches!(
        diagnostic.category,
        RuntimeDiagnosticCategory::Environment | RuntimeDiagnosticCategory::RuntimeInfra
    )
}

pub fn runtime_preview_issue_from_diagnostic(
    runtime_kind: RuntimeKind,
    summary: String,
    diagnostic: RuntimeDiagnostic,
    source_detail: String,
) -> RuntimePreviewIssue {
    let lower_source = source_detail.to_lowercase();
    let kind = if lower_source.contains("sidecar") && lower_source.contains("pnpm") {
        "managed-pnpm-missing"
    } else if lower_source.contains("sidecar") {
        "managed-sidecar-missing"
    } else {
        "runtime-environment"
    };
    RuntimePreviewIssue {
        kind: kind.to_string(),
        runtime_kind,
        summary,
        diagnostic,
        source_detail,
        repair_action: "install-runtime-environment".to_string(),
    }
}

fn resolve_single_workspace_runtime_and_harness(
    pack_manager: &PackManager,
    lockfile: &crate::core::workspace_types::SofvaryLockfile,
) -> Result<(RuntimePackManifest, HarnessPackManifest), RuntimeManagerError> {
    if lockfile.runtime_packs.len() != 1 {
        return Err(RuntimeManagerError::InvalidImportedWorkspace(
            "workspace lockfile must pin exactly one runtime pack".to_string(),
        ));
    }
    if lockfile.harness_packs.len() != 1 {
        return Err(RuntimeManagerError::InvalidImportedWorkspace(
            "workspace lockfile must pin exactly one harness pack".to_string(),
        ));
    }

    let (runtime_id, runtime_version) = lockfile.runtime_packs.iter().next().ok_or_else(|| {
        RuntimeManagerError::InvalidImportedWorkspace("missing runtime pack".to_string())
    })?;
    let (harness_id, harness_version) = lockfile.harness_packs.iter().next().ok_or_else(|| {
        RuntimeManagerError::InvalidImportedWorkspace("missing harness pack".to_string())
    })?;

    let runtime = pack_manager
        .resolver()
        .resolve_runtime(runtime_id, runtime_version)?
        .manifest;
    let harness = pack_manager
        .resolver()
        .resolve_harness(harness_id, harness_version)?
        .manifest;

    Ok((runtime, harness))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runtime_diagnostic::{
        diagnostic_from_command_failure, RuntimeDiagnosticRepairTarget,
    };
    use crate::core::workspace_types::{WorkspaceConstraints, WorkspacePaths, WorkspacePreview};
    use std::cell::Cell;
    use std::fs;
    use std::path::Path;

    #[test]
    fn runtime_repair_retries_agent_repairable_failure_then_succeeds() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let workspace_manager = WorkspaceManager::new();
        let manager = RuntimeManager::new();
        let envelope = test_react_vite_envelope();
        let agent_session = AgentSession {
            session_id: "agent_session_initial".to_string(),
            adapter: crate::core::agent_gateway::AgentAdapterKind::Mock,
            app_id: manifest.app_id.clone(),
            envelope_id: envelope.envelope_id.clone(),
            events: Vec::new(),
        };
        let attempts = Cell::new(0);

        let result: Result<((), AgentSession), RuntimeManagerError> = manager
            .start_runtime_with_repair(
                &manifest,
                "Build a task board",
                &envelope,
                &workspace_manager,
                &PolicyApprovalSet::default(),
                &RuntimeAgentSelection::Mock,
                agent_session,
                RuntimeMode::Dev,
                || {
                    let attempt = attempts.get();
                    attempts.set(attempt + 1);
                    if attempt == 0 {
                        Err("src/App.tsx: expected closing tag".to_string())
                    } else {
                        Ok(())
                    }
                },
                |error| {
                    diagnostic_from_command_failure(
                        "react-vite".to_string(),
                        "build",
                        Some(1),
                        "",
                        error,
                        None,
                    )
                },
                RuntimeManagerError::InvalidContinuation,
                |repair_prompt| {
                    let mut repair_envelope = envelope.clone();
                    repair_envelope.user_intent = repair_prompt.to_string();
                    Ok(repair_envelope)
                },
            );

        let (_, session) = result.expect("runtime should succeed after one repair");
        assert_eq!(attempts.get(), 2);
        assert!(session
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::RepairStarted { attempt: 1, .. })));
        assert!(!matches!(
            manager.start_runtime_with_repair(
                &manifest,
                "Build a task board",
                &envelope,
                &workspace_manager,
                &PolicyApprovalSet::default(),
                &RuntimeAgentSelection::Mock,
                AgentSession {
                    session_id: "agent_session_second".to_string(),
                    adapter: crate::core::agent_gateway::AgentAdapterKind::Mock,
                    app_id: manifest.app_id.clone(),
                    envelope_id: envelope.envelope_id.clone(),
                    events: Vec::new(),
                },
                RuntimeMode::Dev,
                || Ok::<(), String>(()),
                |error| diagnostic_from_command_failure(
                    "react-vite".to_string(),
                    "build",
                    Some(1),
                    "",
                    error,
                    None,
                ),
                RuntimeManagerError::InvalidContinuation,
                |repair_prompt| {
                    let mut repair_envelope = envelope.clone();
                    repair_envelope.user_intent = repair_prompt.to_string();
                    Ok(repair_envelope)
                },
            ),
            Err(RuntimeManagerError::RuntimeRepairExhausted { .. })
        ));
    }

    #[test]
    fn runtime_repair_exhausts_after_two_agent_attempts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let workspace_manager = WorkspaceManager::new();
        let manager = RuntimeManager::new();
        let envelope = test_react_vite_envelope();
        let agent_session = AgentSession {
            session_id: "agent_session_initial".to_string(),
            adapter: crate::core::agent_gateway::AgentAdapterKind::Mock,
            app_id: manifest.app_id.clone(),
            envelope_id: envelope.envelope_id.clone(),
            events: Vec::new(),
        };
        let attempts = Cell::new(0);

        let result: Result<((), AgentSession), RuntimeManagerError> = manager
            .start_runtime_with_repair(
                &manifest,
                "Build a task board",
                &envelope,
                &workspace_manager,
                &PolicyApprovalSet::default(),
                &RuntimeAgentSelection::Mock,
                agent_session,
                RuntimeMode::Dev,
                || {
                    let attempt = attempts.get();
                    attempts.set(attempt + 1);
                    Err("src/App.tsx: expected closing tag".to_string())
                },
                |error| {
                    diagnostic_from_command_failure(
                        "react-vite".to_string(),
                        "build",
                        Some(1),
                        "",
                        error,
                        None,
                    )
                },
                RuntimeManagerError::InvalidContinuation,
                |repair_prompt| {
                    let mut repair_envelope = envelope.clone();
                    repair_envelope.user_intent = repair_prompt.to_string();
                    Ok(repair_envelope)
                },
            );

        assert!(matches!(
            result,
            Err(RuntimeManagerError::RuntimeRepairExhausted { attempts: 2, .. })
        ));
        assert_eq!(attempts.get(), 3);
    }

    #[test]
    fn runtime_diagnostic_blocked_preserves_assets_for_sofvary_environment() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let workspace_manager = WorkspaceManager::new();
        let manager = RuntimeManager::new();
        let envelope = test_react_vite_envelope();
        let agent_session = AgentSession {
            session_id: "agent_session_initial".to_string(),
            adapter: crate::core::agent_gateway::AgentAdapterKind::Mock,
            app_id: manifest.app_id.clone(),
            envelope_id: envelope.envelope_id.clone(),
            events: Vec::new(),
        };

        let result: Result<((), AgentSession), RuntimeManagerError> = manager
            .start_runtime_with_repair(
                &manifest,
                "Build a task board",
                &envelope,
                &workspace_manager,
                &PolicyApprovalSet::default(),
                &RuntimeAgentSelection::Mock,
                agent_session,
                RuntimeMode::Dev,
                || Err("sidecar executable 'pnpm' was not found".to_string()),
                |error| {
                    diagnostic_from_command_failure(
                        "react-vite".to_string(),
                        "install",
                        None,
                        "",
                        error,
                        None,
                    )
                },
                RuntimeManagerError::InvalidContinuation,
                |repair_prompt| {
                    let mut repair_envelope = envelope.clone();
                    repair_envelope.user_intent = repair_prompt.to_string();
                    Ok(repair_envelope)
                },
            );

        match result {
            Err(RuntimeManagerError::RuntimeDiagnosticBlocked {
                diagnostic,
                assets: Some(assets),
                ..
            }) => {
                assert_eq!(
                    diagnostic.repairable_by,
                    RuntimeDiagnosticRepairTarget::Sofvary
                );
                assert_eq!(assets.app_id, manifest.app_id);
                assert_eq!(assets.runtime_kind, "react-vite".to_string());
                assert_eq!(assets.runtime_mode, RuntimeMode::Dev);
                assert!(assets
                    .prompt_envelope_summary
                    .runtime
                    .contains("react-vite"));
            }
            other => panic!("expected preview-blocking diagnostic with assets, got {other:?}"),
        }
    }

    #[test]
    fn runtime_repair_can_fallback_after_two_agent_attempts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let workspace_manager = WorkspaceManager::new();
        let manager = RuntimeManager::new();
        let envelope = test_react_vite_envelope();
        let agent_session = AgentSession {
            session_id: "agent_session_initial".to_string(),
            adapter: crate::core::agent_gateway::AgentAdapterKind::Mock,
            app_id: manifest.app_id.clone(),
            envelope_id: envelope.envelope_id.clone(),
            events: Vec::new(),
        };
        let attempts = Cell::new(0);
        let fallback_used = Cell::new(false);

        let result: Result<((), AgentSession), RuntimeManagerError> = manager
            .start_runtime_with_repair_and_fallback(
                &manifest,
                "Build a task board",
                &envelope,
                &workspace_manager,
                &PolicyApprovalSet::default(),
                &RuntimeAgentSelection::Mock,
                agent_session,
                RuntimeMode::Dev,
                || {
                    let attempt = attempts.get();
                    attempts.set(attempt + 1);
                    Err("src/App.tsx: expected closing tag".to_string())
                },
                |error| {
                    diagnostic_from_command_failure(
                        "react-vite".to_string(),
                        "build",
                        Some(1),
                        "",
                        error,
                        None,
                    )
                },
                RuntimeManagerError::InvalidContinuation,
                |repair_prompt| {
                    let mut repair_envelope = envelope.clone();
                    repair_envelope.user_intent = repair_prompt.to_string();
                    Ok(repair_envelope)
                },
                |_, _| {
                    fallback_used.set(true);
                    Ok(Some(()))
                },
            );

        let (_, session) = result.expect("runtime should succeed through fallback");
        assert_eq!(attempts.get(), 3);
        assert!(fallback_used.get());
        assert_eq!(
            session
                .events
                .iter()
                .filter(|event| matches!(event, AgentEvent::RepairStarted { .. }))
                .count(),
            2
        );
        assert_eq!(
            session
                .events
                .iter()
                .filter(|event| matches!(event, AgentEvent::RuntimeDiagnostic { .. }))
                .count(),
            3
        );
    }

    fn test_manifest(root: &Path) -> AppBoxManifest {
        let workspace = root.join("app_test");
        let generated = workspace.join("generated");
        let generated_static = generated.join("static");
        let runtime = workspace.join("runtime");
        let snapshots = workspace.join("snapshots");
        fs::create_dir_all(&generated_static).expect("generated static");
        fs::create_dir_all(generated.join("react")).expect("react root");
        fs::create_dir_all(&runtime).expect("runtime");
        fs::create_dir_all(&snapshots).expect("snapshots");

        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test App".to_string(),
            mode: "react-vite".to_string(),
            created_at: "2026-06-15T00:00:00Z".to_string(),
            updated_at: "2026-06-15T00:00:00Z".to_string(),
            stack: vec!["react".to_string(), "vite".to_string()],
            paths: WorkspacePaths {
                root: workspace.clone(),
                generated,
                generated_static,
                runtime,
                snapshots,
            },
            constraints: WorkspaceConstraints {
                boundary: workspace,
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: WorkspacePreview {
                state: "idle".to_string(),
                url: None,
            },
        }
    }

    fn test_react_vite_envelope() -> PromptEnvelope {
        serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/react-vite-prompt-envelope.golden.json"
        ))
        .expect("fixture")
    }
}
