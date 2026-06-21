use crate::core::agent_config::AgentConfig;
use crate::core::agent_gateway::{
    summarize_agent_events, AgentEvent, AgentEventSink, AgentGateway, AgentGatewayError,
    AgentRunContext, AgentSession, ConfiguredAgentAdapter, MockAgentAdapter,
};
use crate::core::ai_agent_app_runtime::{
    AiAgentAppRuntime, AiAgentAppRuntimeError, AiAgentAppRuntimeServer,
};
use crate::core::canvas2d_runtime::{Canvas2dRuntime, Canvas2dRuntimeError, Canvas2dRuntimeServer};
use crate::core::data_table_runtime::{
    DataTableRuntime, DataTableRuntimeError, DataTableRuntimeServer,
};
use crate::core::desktop_widget_runtime::{
    DesktopWidgetRuntime, DesktopWidgetRuntimeError, DesktopWidgetRuntimeServer,
};
use crate::core::file_processor_runtime::{
    FileProcessorRuntime, FileProcessorRuntimeError, FileProcessorRuntimeServer,
};
use crate::core::harness_engine::{
    summarize_prompt_envelope, HarnessEngine, HarnessEngineError, PromptEnvelope,
    PromptEnvelopeSummary,
};
use crate::core::markdown_knowledge_runtime::{
    MarkdownKnowledgeRuntime, MarkdownKnowledgeRuntimeError, MarkdownKnowledgeRuntimeServer,
};
use crate::core::pack_manager::{PackError, PackManager};
use crate::core::pack_types::{HarnessPackManifest, RuntimePackManifest};
use crate::core::policy_engine::{PolicyEngine, PolicyError};
use crate::core::policy_types::{PolicyApprovalSet, PolicyRuntimeStartRequest};
use crate::core::react_sqlite_runtime::{
    ReactSqliteRuntime, ReactSqliteRuntimeError, ReactSqliteRuntimeServer,
};
use crate::core::react_vite_runtime::{
    ReactViteRuntime, ReactViteRuntimeError, ReactViteRuntimeServer,
};
use crate::core::runtime_diagnostic::{
    diagnostic_from_file_processor_error, diagnostic_from_react_project_error,
    diagnostic_from_react_sqlite_error, diagnostic_from_react_vite_error, RuntimeDiagnostic,
};
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
    #[error("ai-agent-app runtime error: {0}")]
    AiAgentAppRuntime(AiAgentAppRuntimeError),
    #[error("canvas2d runtime error: {0}")]
    Canvas2dRuntime(#[from] Canvas2dRuntimeError),
    #[error("markdown-knowledge runtime error: {0}")]
    MarkdownKnowledgeRuntime(MarkdownKnowledgeRuntimeError),
    #[error("data-table runtime error: {0}")]
    DataTableRuntime(DataTableRuntimeError),
    #[error("file-processor runtime error: {0}")]
    FileProcessorRuntime(#[from] FileProcessorRuntimeError),
    #[error("desktop-widget runtime error: {0}")]
    DesktopWidgetRuntime(DesktopWidgetRuntimeError),
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
    },
    #[error("runtime lock poisoned")]
    LockPoisoned,
    #[error("imported workspace is invalid: {0}")]
    InvalidImportedWorkspace(String),
    #[error("continuation workspace is invalid: {0}")]
    InvalidContinuation(String),
}

pub struct RuntimeManager {
    harness_engine: HarnessEngine,
    static_runtime: StaticHtmlRuntime,
    react_vite_runtime: ReactViteRuntime,
    react_sqlite_runtime: ReactSqliteRuntime,
    ai_agent_app_runtime: AiAgentAppRuntime,
    canvas2d_runtime: Canvas2dRuntime,
    markdown_knowledge_runtime: MarkdownKnowledgeRuntime,
    data_table_runtime: DataTableRuntime,
    file_processor_runtime: FileProcessorRuntime,
    desktop_widget_runtime: DesktopWidgetRuntime,
    active_apps: Mutex<HashMap<String, ActiveRuntimeServer>>,
}

enum ActiveRuntimeServer {
    Static(StaticRuntimeServer),
    ReactVite(ReactViteRuntimeServer),
    ReactSqlite(ReactSqliteRuntimeServer),
    AiAgentApp(AiAgentAppRuntimeServer),
    Canvas2d(Canvas2dRuntimeServer),
    MarkdownKnowledge(MarkdownKnowledgeRuntimeServer),
    DataTable(DataTableRuntimeServer),
    FileProcessor(FileProcessorRuntimeServer),
    DesktopWidget(DesktopWidgetRuntimeServer),
}

impl Drop for ActiveRuntimeServer {
    fn drop(&mut self) {
        match self {
            Self::Static(server) => server.stop(),
            Self::ReactVite(server) => server.stop(),
            Self::ReactSqlite(server) => server.stop(),
            Self::AiAgentApp(server) => server.stop(),
            Self::Canvas2d(server) => server.stop(),
            Self::MarkdownKnowledge(server) => server.stop(),
            Self::DataTable(server) => server.stop(),
            Self::FileProcessor(server) => server.stop(),
            Self::DesktopWidget(server) => server.stop(),
        }
    }
}

#[derive(Clone)]
enum RuntimeAgentSelection {
    Mock,
    Configured {
        config: AgentConfig,
        event_sink: Option<AgentEventSink>,
    },
}

impl RuntimeManager {
    pub fn new() -> Self {
        Self {
            harness_engine: HarnessEngine::new(),
            static_runtime: StaticHtmlRuntime::new(),
            react_vite_runtime: ReactViteRuntime::new(),
            react_sqlite_runtime: ReactSqliteRuntime::new(),
            ai_agent_app_runtime: AiAgentAppRuntime::new(),
            canvas2d_runtime: Canvas2dRuntime::new(),
            markdown_knowledge_runtime: MarkdownKnowledgeRuntime::new(),
            data_table_runtime: DataTableRuntime::new(),
            file_processor_runtime: FileProcessorRuntime::new(),
            desktop_widget_runtime: DesktopWidgetRuntime::new(),
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
            RuntimeKind::StaticHtml,
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
        match runtime_kind {
            RuntimeKind::StaticHtml => self.build_and_preview_static_app_inner(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
            RuntimeKind::ReactVite => self.build_and_preview_react_vite_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
            RuntimeKind::ReactSqlite => self.build_and_preview_react_sqlite_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
            RuntimeKind::AiAgentApp => self.build_and_preview_ai_agent_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
            RuntimeKind::Canvas2d => self.build_and_preview_canvas2d_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
            RuntimeKind::MarkdownKnowledge => self.build_and_preview_markdown_knowledge_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
            RuntimeKind::DataTable => self.build_and_preview_data_table_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
            RuntimeKind::FileProcessor => self.build_and_preview_file_processor_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
            RuntimeKind::DesktopWidget => self.build_and_preview_desktop_widget_app(
                requirement,
                runtime_mode,
                workspace_manager,
                approvals,
                agent_selection,
            ),
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
        match manifest.mode {
            RuntimeKind::StaticHtml => self.build_existing_static_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            RuntimeKind::ReactVite => self.build_existing_react_vite_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            RuntimeKind::ReactSqlite => self.build_existing_react_sqlite_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            RuntimeKind::AiAgentApp => self.build_existing_ai_agent_app_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            RuntimeKind::Canvas2d => self.build_existing_canvas2d_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            RuntimeKind::MarkdownKnowledge => self.build_existing_markdown_knowledge_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            RuntimeKind::DataTable => self.build_existing_data_table_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            RuntimeKind::FileProcessor => self.build_existing_file_processor_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
            RuntimeKind::DesktopWidget => self.build_existing_desktop_widget_workspace(
                requirement,
                manifest,
                runtime_mode,
                workspace_manager,
                runtime_pack,
                harness_pack,
                approvals,
                agent_selection,
            ),
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

        match manifest.mode {
            RuntimeKind::StaticHtml => self.preview_existing_static_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            RuntimeKind::ReactVite => self.preview_existing_react_vite_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            RuntimeKind::ReactSqlite => self.preview_existing_react_sqlite_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            RuntimeKind::AiAgentApp => self.preview_existing_ai_agent_app_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            RuntimeKind::Canvas2d => self.preview_existing_canvas2d_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            RuntimeKind::MarkdownKnowledge => self.preview_existing_markdown_knowledge_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            RuntimeKind::DataTable => self.preview_existing_data_table_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            RuntimeKind::FileProcessor => self.preview_existing_file_processor_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
            RuntimeKind::DesktopWidget => self.preview_existing_desktop_widget_workspace(
                manifest,
                runtime_mode,
                workspace_manager,
                &runtime_pack,
                &harness_pack,
                approvals,
            ),
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
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_static_html_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::StaticHtml)?;
        let prompt_envelope = self.harness_engine.create_static_html_envelope(
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
        self.enforce_runtime_start(&manifest, "static-html", approvals)?;
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
            runtime_kind: RuntimeKind::StaticHtml,
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
        let prompt_envelope = self.harness_engine.create_static_html_envelope(
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
            runtime_kind: RuntimeKind::StaticHtml,
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
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_react_vite_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::ReactVite)?;
        let prompt_envelope = self.harness_engine.create_react_vite_envelope(
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
        self.enforce_runtime_start(&manifest, "react-vite", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
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
            |error| diagnostic_from_react_vite_error(RuntimeKind::ReactVite, error),
            RuntimeManagerError::ReactViteRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_react_vite_envelope(
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
            runtime_kind: RuntimeKind::ReactVite,
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
        let prompt_envelope = self.harness_engine.create_react_vite_envelope(
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
            runtime_kind: RuntimeKind::ReactVite,
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
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_react_sqlite_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::ReactSqlite)?;
        let prompt_envelope = self.harness_engine.create_react_sqlite_envelope(
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
        self.enforce_runtime_start(&manifest, "react-sqlite", approvals)?;
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
            runtime_kind: RuntimeKind::ReactSqlite,
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
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_react_sqlite_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
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
            runtime_kind: RuntimeKind::ReactSqlite,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_ai_agent_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_ai_agent_app_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::AiAgentApp)?;
        let prompt_envelope = self.harness_engine.create_ai_agent_app_envelope(
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
        self.enforce_runtime_start(&manifest, "ai-agent-app", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.ai_agent_app_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        &packs.runtime.manifest,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_project_error(RuntimeKind::AiAgentApp, error),
            RuntimeManagerError::AiAgentAppRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_ai_agent_app_envelope(
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
                ActiveRuntimeServer::AiAgentApp(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::AiAgentApp,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_ai_agent_app_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_ai_agent_app_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, "ai-agent-app", approvals)?;
        let server = self
            .ai_agent_app_runtime
            .start_workspace_with_envelope_with_policy(
                &manifest,
                &prompt_envelope,
                runtime_pack,
                runtime_mode,
                approvals,
            )
            .map_err(RuntimeManagerError::AiAgentAppRuntime)?;
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
                ActiveRuntimeServer::AiAgentApp(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::AiAgentApp,
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
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_canvas2d_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::Canvas2d)?;
        let prompt_envelope = self.harness_engine.create_canvas2d_envelope(
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
        self.enforce_runtime_start(&manifest, "canvas2d", approvals)?;
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
            runtime_kind: RuntimeKind::Canvas2d,
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
        let prompt_envelope = self.harness_engine.create_canvas2d_envelope(
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
            runtime_kind: RuntimeKind::Canvas2d,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_markdown_knowledge_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_markdown_knowledge_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::MarkdownKnowledge)?;
        let prompt_envelope = self.harness_engine.create_markdown_knowledge_envelope(
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
        self.enforce_runtime_start(&manifest, "markdown-knowledge", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.markdown_knowledge_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        &packs.runtime.manifest,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_project_error(RuntimeKind::MarkdownKnowledge, error),
            RuntimeManagerError::MarkdownKnowledgeRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_markdown_knowledge_envelope(
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
                ActiveRuntimeServer::MarkdownKnowledge(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::MarkdownKnowledge,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_markdown_knowledge_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_markdown_knowledge_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, "markdown-knowledge", approvals)?;
        let server = self
            .markdown_knowledge_runtime
            .start_workspace_with_envelope_with_policy(
                &manifest,
                &prompt_envelope,
                runtime_pack,
                runtime_mode,
                approvals,
            )
            .map_err(RuntimeManagerError::MarkdownKnowledgeRuntime)?;
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
                ActiveRuntimeServer::MarkdownKnowledge(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::MarkdownKnowledge,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_data_table_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_data_table_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::DataTable)?;
        let prompt_envelope = self.harness_engine.create_data_table_envelope(
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
        self.enforce_runtime_start(&manifest, "data-table", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.data_table_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        &packs.runtime.manifest,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_project_error(RuntimeKind::DataTable, error),
            RuntimeManagerError::DataTableRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_data_table_envelope(
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
                ActiveRuntimeServer::DataTable(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::DataTable,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_data_table_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_data_table_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, "data-table", approvals)?;
        let server = self
            .data_table_runtime
            .start_workspace_with_envelope_with_policy(
                &manifest,
                &prompt_envelope,
                runtime_pack,
                runtime_mode,
                approvals,
            )
            .map_err(RuntimeManagerError::DataTableRuntime)?;
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
                ActiveRuntimeServer::DataTable(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::DataTable,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_file_processor_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_file_processor_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::FileProcessor)?;
        let prompt_envelope = self.harness_engine.create_file_processor_envelope(
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
        self.enforce_runtime_start(&manifest, "file-processor", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.file_processor_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        &packs.runtime.manifest,
                        runtime_mode,
                        approvals,
                    )
            },
            diagnostic_from_file_processor_error,
            RuntimeManagerError::FileProcessorRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_file_processor_envelope(
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
                ActiveRuntimeServer::FileProcessor(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::FileProcessor,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_file_processor_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_file_processor_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, "file-processor", approvals)?;
        let server = self
            .file_processor_runtime
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
                ActiveRuntimeServer::FileProcessor(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::FileProcessor,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_and_preview_desktop_widget_app(
        &self,
        requirement: String,
        runtime_mode: RuntimeMode,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        agent_selection: &RuntimeAgentSelection,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let pack_manager = PackManager::new()?;
        let packs = pack_manager.resolve_desktop_widget_packs()?;
        let name = derive_app_name(&requirement);
        let manifest =
            workspace_manager.create_workspace_for_runtime(name, RuntimeKind::DesktopWidget)?;
        let prompt_envelope = self.harness_engine.create_desktop_widget_envelope(
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
        self.enforce_runtime_start(&manifest, "desktop-widget", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.desktop_widget_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        &packs.runtime.manifest,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_project_error(RuntimeKind::DesktopWidget, error),
            RuntimeManagerError::DesktopWidgetRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_desktop_widget_envelope(
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
                ActiveRuntimeServer::DesktopWidget(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::DesktopWidget,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn preview_existing_desktop_widget_workspace(
        &self,
        manifest: AppBoxManifest,
        runtime_mode: RuntimeMode,
        _workspace_manager: &WorkspaceManager,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<RuntimePreview, RuntimeManagerError> {
        let prompt_envelope = self.harness_engine.create_desktop_widget_envelope(
            "Preview imported app capsule",
            &manifest,
            runtime_pack,
            harness_pack,
        )?;
        let prompt_envelope_summary = summarize_prompt_envelope(&prompt_envelope);
        self.enforce_runtime_start(&manifest, "desktop-widget", approvals)?;
        let server = self
            .desktop_widget_runtime
            .start_workspace_with_envelope_with_policy(
                &manifest,
                &prompt_envelope,
                runtime_pack,
                runtime_mode,
                approvals,
            )
            .map_err(RuntimeManagerError::DesktopWidgetRuntime)?;
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
                ActiveRuntimeServer::DesktopWidget(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::DesktopWidget,
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
        let prompt_envelope = self.harness_engine.create_static_html_envelope(
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
            runtime_kind: RuntimeKind::StaticHtml,
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
        let prompt_envelope = self.harness_engine.create_react_vite_envelope(
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
            |error| diagnostic_from_react_vite_error(RuntimeKind::ReactVite, error),
            RuntimeManagerError::ReactViteRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_react_vite_envelope(
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
            runtime_kind: RuntimeKind::ReactVite,
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
        let prompt_envelope = self.harness_engine.create_react_sqlite_envelope(
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
            runtime_kind: RuntimeKind::ReactSqlite,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_ai_agent_app_workspace(
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
        let prompt_envelope = self.harness_engine.create_ai_agent_app_envelope(
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
        self.enforce_runtime_start(&manifest, "ai-agent-app", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.ai_agent_app_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        runtime_pack,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_project_error(RuntimeKind::AiAgentApp, error),
            RuntimeManagerError::AiAgentAppRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_ai_agent_app_envelope(
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
                ActiveRuntimeServer::AiAgentApp(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::AiAgentApp,
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
        let prompt_envelope = self.harness_engine.create_canvas2d_envelope(
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
            runtime_kind: RuntimeKind::Canvas2d,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_markdown_knowledge_workspace(
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
        let prompt_envelope = self.harness_engine.create_markdown_knowledge_envelope(
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
        self.enforce_runtime_start(&manifest, "markdown-knowledge", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.markdown_knowledge_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        runtime_pack,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_project_error(RuntimeKind::MarkdownKnowledge, error),
            RuntimeManagerError::MarkdownKnowledgeRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_markdown_knowledge_envelope(
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
                ActiveRuntimeServer::MarkdownKnowledge(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::MarkdownKnowledge,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_data_table_workspace(
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
        let prompt_envelope = self.harness_engine.create_data_table_envelope(
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
        self.enforce_runtime_start(&manifest, "data-table", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.data_table_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        runtime_pack,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_project_error(RuntimeKind::DataTable, error),
            RuntimeManagerError::DataTableRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_data_table_envelope(
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
                ActiveRuntimeServer::DataTable(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::DataTable,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_file_processor_workspace(
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
        let prompt_envelope = self.harness_engine.create_file_processor_envelope(
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
        self.enforce_runtime_start(&manifest, "file-processor", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.file_processor_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        runtime_pack,
                        runtime_mode,
                        approvals,
                    )
            },
            diagnostic_from_file_processor_error,
            RuntimeManagerError::FileProcessorRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_file_processor_envelope(
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
                ActiveRuntimeServer::FileProcessor(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::FileProcessor,
            runtime_mode,
            preview_url: preview.preview_url,
            logs,
            manifest,
            prompt_envelope_summary,
        })
    }

    fn build_existing_desktop_widget_workspace(
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
        let prompt_envelope = self.harness_engine.create_desktop_widget_envelope(
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
        self.enforce_runtime_start(&manifest, "desktop-widget", approvals)?;
        let (server, agent_session) = self.start_runtime_with_repair(
            &manifest,
            &requirement,
            &prompt_envelope,
            workspace_manager,
            approvals,
            agent_selection,
            agent_session,
            || {
                self.desktop_widget_runtime
                    .start_workspace_with_envelope_with_policy(
                        &manifest,
                        &prompt_envelope,
                        runtime_pack,
                        runtime_mode,
                        approvals,
                    )
            },
            |error| diagnostic_from_react_project_error(RuntimeKind::DesktopWidget, error),
            RuntimeManagerError::DesktopWidgetRuntime,
            |repair_prompt| {
                Ok(self.harness_engine.create_desktop_widget_envelope(
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
                ActiveRuntimeServer::DesktopWidget(server),
            );

        Ok(RuntimePreview {
            app_id: manifest.app_id.clone(),
            runtime_kind: RuntimeKind::DesktopWidget,
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
                Ok(self.harness_engine.create_react_sqlite_envelope(
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
                let fallback_envelope = self.harness_engine.create_react_sqlite_envelope(
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
                        return Err(RuntimeManagerError::RuntimeDiagnosticBlocked {
                            summary: diagnostic.summary(),
                            diagnostic,
                            source_detail,
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
            RuntimeAgentSelection::Configured { config, event_sink } => {
                let mut adapter = ConfiguredAgentAdapter::new(
                    config.clone(),
                    manifest.clone(),
                    approvals.clone(),
                );
                if let Some(event_sink) = event_sink.clone() {
                    adapter = adapter.with_event_sink(event_sink);
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
    let trimmed = requirement.trim();
    if trimmed.is_empty() {
        return "Static HTML App".to_string();
    }

    trimmed
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(60)
        .collect()
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
    format!(
        "Create a stable React + SQLite app for this requirement: {requirement}\n\nRuntime repair fallback reason: {}\nUse the Sofvary managed React + SQLite baseline with local CRUD, a Vite frontend, and a local API server.",
        diagnostic.summary()
    )
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

    format!(
        "Repair the generated Sofvary app so it starts successfully.\n\
Original user intent:\n{original_requirement}\n\n\
Runtime kind: {}\n\
Repair attempt: {attempt}/{max_attempts}\n\
Failed stage: {:?}\n\
Failed command: {command}\n\
Status code: {:?}\n\
Diagnostic category: {:?}\n\
Runtime log path: {log_path}\n\n\
stdout tail:\n{stdout}\n\n\
stderr tail:\n{stderr}\n\n\
Keep the same output contract and regenerate every required file exactly. Do not add files outside the allowed set. Do not include Sofvary shell UI.",
        envelope.runtime_policy.runtime_kind,
        diagnostic.stage,
        diagnostic.status_code,
        diagnostic.category,
    )
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
    use crate::core::runtime_diagnostic::diagnostic_from_command_failure;
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
                        RuntimeKind::ReactVite,
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
                || Ok::<(), String>(()),
                |error| diagnostic_from_command_failure(
                    RuntimeKind::ReactVite,
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
                || {
                    let attempt = attempts.get();
                    attempts.set(attempt + 1);
                    Err("src/App.tsx: expected closing tag".to_string())
                },
                |error| {
                    diagnostic_from_command_failure(
                        RuntimeKind::ReactVite,
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
                || {
                    let attempt = attempts.get();
                    attempts.set(attempt + 1);
                    Err("src/App.tsx: expected closing tag".to_string())
                },
                |error| {
                    diagnostic_from_command_failure(
                        RuntimeKind::ReactVite,
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
            mode: RuntimeKind::ReactVite,
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
