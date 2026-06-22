use crate::core::acp_client::{run_acp_agent, AcpRunRequest};
use crate::core::agent_cli_bridge::{run_cli_agent, CliRunRequest};
use crate::core::agent_config::{AgentConfig, AgentProvider, AgentTransportKind};
use crate::core::gateway_uni_event::GatewayUniEventEmitter;
use crate::core::harness_engine::PromptEnvelope;
use crate::core::pi_agent::{run_pi_agent, PiRunRequest};
use crate::core::policy_engine::PolicyEngine;
use crate::core::policy_types::{
    PolicyApprovalSet, PolicyCommandRequest, PolicyExternalAgentProcessRequest,
};
use crate::core::runtime_diagnostic::RuntimeDiagnostic;
use crate::core::workspace_manager::{
    GeneratedCanvas2dFile, GeneratedProjectFile, GeneratedReactFile, GeneratedReactSqliteFile,
    GeneratedStaticFile, WorkspaceError, WorkspaceManager,
};
use crate::core::workspace_types::AppBoxManifest;
use crate::platform::CommandSpec;
use html_escape::encode_text;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_AGENT_TIMEOUT_MS: u64 = 180_000;
const CODEX_AGENT_TIMEOUT_MS: u64 = 360_000;
const PI_AGENT_TIMEOUT_MS: u64 = 180_000;

#[derive(Debug, Error)]
pub enum AgentGatewayError {
    #[error("agent adapter is not implemented: {0}")]
    #[allow(dead_code)]
    AdapterNotImplemented(String),
    #[error("agent adapter error: {0}")]
    Adapter(String),
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("policy error: {0}")]
    Policy(#[from] crate::core::policy_engine::PolicyError),
}

pub type AgentGatewayResult<T> = Result<T, AgentGatewayError>;
pub type AgentEventSink = Arc<dyn Fn(AgentEvent) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentAdapterKind {
    Mock,
    Acp,
    Cli,
    Pi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFileWriteRequest {
    pub relative_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, Default)]
pub struct AgentAdapterOutput {
    pub events: Vec<AgentEvent>,
    pub file_writes: Vec<AgentFileWriteRequest>,
    pub command_requests: Vec<CommandSpec>,
}

#[derive(Debug, Clone, Default)]
pub struct AgentRunContext {
    pub runtime_diagnostics: Vec<RuntimeDiagnostic>,
}

impl AgentRunContext {
    pub fn with_runtime_diagnostic(diagnostic: RuntimeDiagnostic) -> Self {
        Self {
            runtime_diagnostics: vec![diagnostic],
        }
    }

    pub fn diagnostics(&self) -> &[RuntimeDiagnostic] {
        &self.runtime_diagnostics
    }
}

pub trait AgentAdapter {
    fn kind(&self) -> AgentAdapterKind;
    fn adapter_id(&self) -> String {
        format!("{:?}", self.kind()).to_ascii_lowercase()
    }
    fn generate(&self, envelope: &PromptEnvelope) -> AgentGatewayResult<AgentAdapterOutput>;
    fn generate_with_context(
        &self,
        envelope: &PromptEnvelope,
        _context: &AgentRunContext,
    ) -> AgentGatewayResult<AgentAdapterOutput> {
        self.generate(envelope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum AgentEvent {
    SessionStarted {
        session_id: String,
        adapter: AgentAdapterKind,
    },
    Planning {
        message: String,
    },
    TextDelta {
        text: String,
    },
    FileWriteRequested {
        relative_path: String,
    },
    FileWritten {
        relative_path: String,
    },
    CommandRequested {
        executable: String,
    },
    CommandApproved {
        executable: String,
    },
    CommandRejected {
        executable: String,
        reason: String,
    },
    BuildStarted {
        target: String,
    },
    BuildFinished {
        target: String,
    },
    RuntimeDiagnostic {
        diagnostic: RuntimeDiagnostic,
    },
    RepairStarted {
        attempt: usize,
        max_attempts: usize,
        summary: String,
    },
    RepairFinished {
        attempt: usize,
        summary: String,
    },
    Error {
        message: String,
    },
    Completed,
}

#[derive(Debug, Clone)]
pub struct AgentSession {
    pub session_id: String,
    pub adapter: AgentAdapterKind,
    pub app_id: String,
    pub envelope_id: String,
    pub events: Vec<AgentEvent>,
}

pub struct AgentGateway<A: AgentAdapter> {
    adapter: A,
}

impl<A: AgentAdapter> AgentGateway<A> {
    pub fn new(adapter: A) -> Self {
        Self { adapter }
    }

    #[allow(dead_code)]
    pub fn run(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        workspace_manager: &WorkspaceManager,
    ) -> AgentGatewayResult<AgentSession> {
        self.run_with_policy(
            manifest,
            envelope,
            workspace_manager,
            &PolicyApprovalSet::default(),
        )
    }

    pub fn run_with_policy(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
    ) -> AgentGatewayResult<AgentSession> {
        self.run_with_policy_and_context(
            manifest,
            envelope,
            workspace_manager,
            approvals,
            &AgentRunContext::default(),
        )
    }

    pub fn run_with_policy_and_context(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        workspace_manager: &WorkspaceManager,
        approvals: &PolicyApprovalSet,
        context: &AgentRunContext,
    ) -> AgentGatewayResult<AgentSession> {
        let session_id = format!("agent_session_{}", Uuid::new_v4());
        let adapter_kind = self.adapter.kind();
        let mut events = vec![AgentEvent::SessionStarted {
            session_id: session_id.clone(),
            adapter: adapter_kind,
        }];

        let output = self.adapter.generate_with_context(envelope, context)?;
        events.extend(output.events);

        match envelope.runtime_policy.runtime_kind.as_str() {
            "static-html" => {
                let generated_files = output
                    .file_writes
                    .iter()
                    .map(|file| GeneratedStaticFile {
                        relative_path: file.relative_path.clone(),
                        contents: file.contents.clone(),
                    })
                    .collect::<Vec<_>>();
                workspace_manager.replace_generated_static_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            "react-vite" => {
                let generated_files = output
                    .file_writes
                    .iter()
                    .map(|file| GeneratedReactFile {
                        relative_path: file.relative_path.clone(),
                        contents: file.contents.clone(),
                    })
                    .collect::<Vec<_>>();
                workspace_manager.replace_generated_react_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            "react-sqlite" => {
                let generated_files = output
                    .file_writes
                    .iter()
                    .map(|file| GeneratedReactSqliteFile {
                        relative_path: file.relative_path.clone(),
                        contents: file.contents.clone(),
                    })
                    .collect::<Vec<_>>();
                workspace_manager.replace_generated_react_sqlite_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            "ai-agent-app" => {
                let generated_files = generated_project_files(&output.file_writes);
                workspace_manager.replace_generated_ai_agent_app_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            "canvas2d" => {
                let generated_files = output
                    .file_writes
                    .iter()
                    .map(|file| GeneratedCanvas2dFile {
                        relative_path: file.relative_path.clone(),
                        contents: file.contents.clone(),
                    })
                    .collect::<Vec<_>>();
                workspace_manager.replace_generated_canvas2d_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            "markdown-knowledge" => {
                let generated_files = generated_project_files(&output.file_writes);
                workspace_manager.replace_generated_markdown_knowledge_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            "data-table" => {
                let generated_files = generated_project_files(&output.file_writes);
                workspace_manager.replace_generated_data_table_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            "file-processor" => {
                let generated_files = generated_project_files(&output.file_writes);
                workspace_manager.replace_generated_file_processor_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            "desktop-widget" => {
                let generated_files = generated_project_files(&output.file_writes);
                workspace_manager.replace_generated_desktop_widget_files(
                    manifest,
                    &generated_files,
                    &envelope.output_contract.files,
                )?;
            }
            runtime_kind => {
                return Err(AgentGatewayError::Adapter(format!(
                    "unsupported runtime kind: {runtime_kind}"
                )));
            }
        }

        for file in &output.file_writes {
            events.push(AgentEvent::FileWritten {
                relative_path: file.relative_path.clone(),
            });
        }

        if manifest.paths.root.join("sofvary.lock.json").exists() {
            workspace_manager
                .update_lockfile_agent_adapter_for_manifest(manifest, self.adapter.adapter_id())?;
        }

        for command in &output.command_requests {
            let executable = command.executable.display().to_string();
            events.push(AgentEvent::CommandRequested {
                executable: executable.clone(),
            });
            let engine = PolicyEngine::new();
            let decision = engine.evaluate_command(PolicyCommandRequest {
                name: "agent-command".to_string(),
                command: command.clone(),
            });
            match engine.enforce(decision, approvals) {
                Ok(()) => events.push(AgentEvent::CommandApproved { executable }),
                Err(error) => events.push(AgentEvent::CommandRejected {
                    executable,
                    reason: error.to_string(),
                }),
            }
        }

        events.push(AgentEvent::BuildStarted {
            target: envelope.runtime_policy.runtime_kind.clone(),
        });
        events.push(AgentEvent::BuildFinished {
            target: envelope.runtime_policy.runtime_kind.clone(),
        });
        events.push(AgentEvent::Completed);

        Ok(AgentSession {
            session_id,
            adapter: adapter_kind,
            app_id: manifest.app_id.clone(),
            envelope_id: envelope.envelope_id.clone(),
            events,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MockAgentAdapter;

impl AgentAdapter for MockAgentAdapter {
    fn kind(&self) -> AgentAdapterKind {
        AgentAdapterKind::Mock
    }

    fn adapter_id(&self) -> String {
        "mock".to_string()
    }

    fn generate(&self, envelope: &PromptEnvelope) -> AgentGatewayResult<AgentAdapterOutput> {
        let file_writes = match envelope.runtime_policy.runtime_kind.as_str() {
            "static-html" => static_html_file_writes(envelope),
            "react-vite" => react_vite_file_writes(envelope)?,
            "react-sqlite" => react_sqlite_file_writes(envelope)?,
            "ai-agent-app" => ai_agent_app_file_writes(envelope)?,
            "canvas2d" => canvas2d_file_writes(envelope)?,
            "markdown-knowledge" => markdown_knowledge_file_writes(envelope)?,
            "data-table" => data_table_file_writes(envelope)?,
            "file-processor" => file_processor_file_writes(envelope)?,
            "desktop-widget" => desktop_widget_file_writes(envelope)?,
            runtime_kind => {
                return Err(AgentGatewayError::Adapter(format!(
                    "mock adapter cannot generate runtime kind: {runtime_kind}"
                )));
            }
        };
        let runtime_label = match envelope.runtime_policy.runtime_kind.as_str() {
            "react-vite" => "React + Vite",
            "react-sqlite" => "React + SQLite",
            "ai-agent-app" => "AI Agent App",
            "canvas2d" => "Canvas 2D",
            "markdown-knowledge" => "Markdown Knowledge",
            "data-table" => "Data Table",
            "file-processor" => "File Processor",
            "desktop-widget" => "Desktop Widget",
            _ => "static HTML",
        };
        let mut events = vec![
            AgentEvent::Planning {
                message: format!("Preparing constrained {runtime_label} output"),
            },
            AgentEvent::TextDelta {
                text: format!("Created local {runtime_label} assets from the prompt envelope"),
            },
        ];
        events.extend(
            file_writes
                .iter()
                .map(|file| AgentEvent::FileWriteRequested {
                    relative_path: file.relative_path.clone(),
                }),
        );

        Ok(AgentAdapterOutput {
            events,
            file_writes,
            command_requests: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct AcpAgentAdapter;

impl AgentAdapter for AcpAgentAdapter {
    fn kind(&self) -> AgentAdapterKind {
        AgentAdapterKind::Acp
    }

    fn generate(&self, _envelope: &PromptEnvelope) -> AgentGatewayResult<AgentAdapterOutput> {
        Err(AgentGatewayError::AdapterNotImplemented(
            "ACP stdio subprocess support is a Phase 8 skeleton only".to_string(),
        ))
    }
}

#[derive(Clone)]
pub struct ConfiguredAgentAdapter {
    config: AgentConfig,
    manifest: AppBoxManifest,
    approvals: PolicyApprovalSet,
    timeout_ms: u64,
    event_sink: Option<AgentEventSink>,
    gateway_event_emitter: Option<GatewayUniEventEmitter>,
}

impl ConfiguredAgentAdapter {
    pub fn new(
        config: AgentConfig,
        manifest: AppBoxManifest,
        approvals: PolicyApprovalSet,
    ) -> Self {
        Self {
            timeout_ms: timeout_ms_for_provider(config.provider),
            config,
            manifest,
            approvals,
            event_sink: None,
            gateway_event_emitter: None,
        }
    }

    pub fn with_event_sink(mut self, event_sink: AgentEventSink) -> Self {
        self.event_sink = Some(event_sink);
        self
    }

    pub fn with_gateway_event_emitter(mut self, emitter: GatewayUniEventEmitter) -> Self {
        self.gateway_event_emitter = Some(emitter);
        self
    }

    fn run_acp(
        &self,
        envelope: &PromptEnvelope,
        context: &AgentRunContext,
    ) -> AgentGatewayResult<AgentAdapterOutput> {
        let command = self.config.acp.as_ref().ok_or_else(|| {
            AgentGatewayError::Adapter(format!("{} has no ACP command", self.config.label))
        })?;
        self.enforce_external_agent_policy(command, AgentTransportKind::Acp)?;
        let output = run_acp_agent(AcpRunRequest {
            agent_id: &self.config.id,
            command,
            workspace_root: &self.manifest.paths.root,
            staging_root: &staging_root_for_runtime(&self.manifest, envelope),
            envelope,
            diagnostics: context.diagnostics(),
            timeout_ms: self.timeout_ms,
            event_sink: self.event_sink.clone(),
            gateway_events: self.gateway_event_emitter.clone(),
        })?;

        Ok(AgentAdapterOutput {
            events: output.events,
            file_writes: output.file_writes,
            command_requests: Vec::new(),
        })
    }

    fn run_cli(
        &self,
        envelope: &PromptEnvelope,
        context: &AgentRunContext,
    ) -> AgentGatewayResult<AgentAdapterOutput> {
        if !self.config.allow_cli_fallback {
            return Err(AgentGatewayError::Adapter(format!(
                "{} CLI fallback is not enabled",
                self.config.label
            )));
        }
        if !cli_fallback_is_verified(&self.config) {
            return Err(AgentGatewayError::Adapter(format!(
                "{} CLI fallback has not passed a connection test",
                self.config.label
            )));
        }
        let command = self.config.cli.as_ref().ok_or_else(|| {
            AgentGatewayError::Adapter(format!("{} has no CLI fallback command", self.config.label))
        })?;
        self.enforce_external_agent_policy(command, AgentTransportKind::Cli)?;
        let output = run_cli_agent(CliRunRequest {
            config: &self.config,
            command,
            workspace_root: &self.manifest.paths.root,
            staging_root: &staging_root_for_runtime(&self.manifest, envelope),
            envelope,
            diagnostics: context.diagnostics(),
            timeout_ms: self.timeout_ms,
            event_sink: self.event_sink.clone(),
            gateway_events: self.gateway_event_emitter.clone(),
        })?;

        Ok(AgentAdapterOutput {
            events: output.events,
            file_writes: output.file_writes,
            command_requests: Vec::new(),
        })
    }

    fn run_pi(
        &self,
        envelope: &PromptEnvelope,
        context: &AgentRunContext,
    ) -> AgentGatewayResult<AgentAdapterOutput> {
        let command = self.config.cli.as_ref().ok_or_else(|| {
            AgentGatewayError::Adapter(format!("{} has no Pi RPC command", self.config.label))
        })?;
        self.enforce_external_agent_policy(command, AgentTransportKind::PiRpc)?;
        let output = run_pi_agent(PiRunRequest {
            command,
            workspace_root: &self.manifest.paths.root,
            staging_root: &staging_root_for_runtime(&self.manifest, envelope),
            envelope,
            diagnostics: context.diagnostics(),
            thread_id: &self.manifest.app_id,
            timeout_ms: self.timeout_ms,
            event_sink: self.event_sink.clone(),
            gateway_events: self.gateway_event_emitter.clone(),
        })?;

        Ok(AgentAdapterOutput {
            events: output.events,
            file_writes: output.file_writes,
            command_requests: Vec::new(),
        })
    }

    fn enforce_external_agent_policy(
        &self,
        command: &crate::core::agent_config::AgentCommandConfig,
        transport: AgentTransportKind,
    ) -> AgentGatewayResult<()> {
        let engine = PolicyEngine::new();
        let decision = engine.evaluate_external_agent_process(PolicyExternalAgentProcessRequest {
            agent_id: self.config.id.clone(),
            provider: self.config.provider.as_str().to_string(),
            transport: transport.as_str().to_string(),
            executable: command.executable.display().to_string(),
        });
        engine.enforce(decision, &self.approvals)?;
        Ok(())
    }
}

fn cli_fallback_is_verified(config: &AgentConfig) -> bool {
    config
        .last_test
        .as_ref()
        .is_some_and(|record| record.ok && matches!(record.transport, AgentTransportKind::Cli))
}

impl AgentAdapter for ConfiguredAgentAdapter {
    fn kind(&self) -> AgentAdapterKind {
        if self.config.provider == AgentProvider::SofvaryPi {
            AgentAdapterKind::Pi
        } else if self.config.acp.is_some() {
            AgentAdapterKind::Acp
        } else {
            AgentAdapterKind::Cli
        }
    }

    fn adapter_id(&self) -> String {
        self.config.id.clone()
    }

    fn generate(&self, envelope: &PromptEnvelope) -> AgentGatewayResult<AgentAdapterOutput> {
        self.generate_with_context(envelope, &AgentRunContext::default())
    }

    fn generate_with_context(
        &self,
        envelope: &PromptEnvelope,
        context: &AgentRunContext,
    ) -> AgentGatewayResult<AgentAdapterOutput> {
        if self.config.provider == AgentProvider::SofvaryPi {
            return self.run_pi(envelope, context);
        }
        if self.config.acp.is_some() {
            match self.run_acp(envelope, context) {
                Ok(output) => return Ok(output),
                Err(error) if self.config.allow_cli_fallback && self.config.cli.is_some() => {
                    if !cli_fallback_is_verified(&self.config) {
                        return Err(AgentGatewayError::Adapter(format!(
                            "ACP generation failed: {error}; CLI fallback is configured but has not passed a CLI connection test"
                        )));
                    }
                    let mut output = self.run_cli(envelope, context)?;
                    output.events.insert(
                        0,
                        AgentEvent::Error {
                            message: format!("ACP failed; using CLI fallback: {error}"),
                        },
                    );
                    return Ok(output);
                }
                Err(error) => return Err(error),
            }
        }

        self.run_cli(envelope, context)
    }
}

fn timeout_ms_for_provider(provider: AgentProvider) -> u64 {
    match provider {
        AgentProvider::Codex => CODEX_AGENT_TIMEOUT_MS,
        AgentProvider::SofvaryPi => PI_AGENT_TIMEOUT_MS,
        _ => DEFAULT_AGENT_TIMEOUT_MS,
    }
}

pub fn summarize_agent_events(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| match event {
            AgentEvent::SessionStarted { adapter, .. } => {
                format!("Agent session started with {:?} adapter", adapter)
            }
            AgentEvent::Planning { message } => format!("Agent plan: {}", summarize_text(message)),
            AgentEvent::TextDelta { text } => format!("Agent message: {}", summarize_text(text)),
            AgentEvent::FileWriteRequested { relative_path } => {
                format!("Agent requested file write: {relative_path}")
            }
            AgentEvent::FileWritten { relative_path } => {
                format!("Workspace wrote generated file: {relative_path}")
            }
            AgentEvent::CommandRequested { executable } => {
                format!("Agent requested command: {executable}")
            }
            AgentEvent::CommandApproved { executable } => {
                format!("Command approved: {executable}")
            }
            AgentEvent::CommandRejected { executable, .. } => {
                format!("Command rejected: {executable}")
            }
            AgentEvent::BuildStarted { target } => format!("Build started: {target}"),
            AgentEvent::BuildFinished { target } => format!("Build finished: {target}"),
            AgentEvent::RuntimeDiagnostic { diagnostic } => {
                format!("Runtime diagnostic: {}", diagnostic.summary())
            }
            AgentEvent::RepairStarted {
                attempt,
                max_attempts,
                summary,
            } => format!("Runtime repair attempt {attempt}/{max_attempts}: {summary}"),
            AgentEvent::RepairFinished { attempt, summary } => {
                format!("Runtime repair attempt {attempt} finished: {summary}")
            }
            AgentEvent::Error { message } => format!("Agent error: {message}"),
            AgentEvent::Completed => "Agent session completed".to_string(),
        })
        .collect()
}

fn summarize_text(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 320 {
        return compact;
    }
    let mut output = compact.chars().take(320).collect::<String>();
    output.push_str("...");
    output
}

fn generated_project_files(files: &[AgentFileWriteRequest]) -> Vec<GeneratedProjectFile> {
    files
        .iter()
        .map(|file| GeneratedProjectFile {
            relative_path: file.relative_path.clone(),
            contents: file.contents.clone(),
        })
        .collect()
}

fn staging_root_for_runtime(manifest: &AppBoxManifest, envelope: &PromptEnvelope) -> PathBuf {
    match envelope.runtime_policy.runtime_kind.as_str() {
        "static-html" => manifest.paths.generated_static.clone(),
        "react-vite" => manifest.paths.generated.join("react"),
        "canvas2d" => manifest.paths.generated.join("canvas"),
        "react-sqlite" | "markdown-knowledge" | "data-table" | "file-processor"
        | "desktop-widget" => manifest.paths.generated.clone(),
        _ => manifest.paths.generated.clone(),
    }
}

fn static_html_file_writes(envelope: &PromptEnvelope) -> Vec<AgentFileWriteRequest> {
    let title = envelope.user_intent.trim();
    let display_title = if title.is_empty() {
        "Untitled Sofvary App"
    } else {
        title
    };
    let escaped_title = encode_text(display_title);

    vec![
        AgentFileWriteRequest {
            relative_path: "index.html".to_string(),
            contents: format!(
                r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{escaped_title}</title>
    <link rel="stylesheet" href="./style.css" />
  </head>
  <body>
    <main class="app-shell">
      <section class="hero">
        <p class="eyebrow">Sofvary Static HTML</p>
        <h1>{escaped_title}</h1>
        <p class="summary">This local app was generated from a constrained PromptEnvelope.</p>
      </section>
      <section class="panel">
        <h2>Generated workspace</h2>
        <p id="status">Ready for local preview.</p>
      </section>
    </main>
    <script src="./app.js"></script>
  </body>
</html>
"#
            ),
        },
        AgentFileWriteRequest {
            relative_path: "style.css".to_string(),
            contents: r#":root {
  color: #111827;
  background: #f4f7fb;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

body {
  min-height: 100vh;
  margin: 0;
  display: grid;
  place-items: center;
}

.app-shell {
  width: min(880px, calc(100vw - 40px));
}

.hero {
  padding: 48px 0 28px;
}

.eyebrow {
  margin: 0 0 12px;
  color: #0f766e;
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

h1 {
  margin: 0;
  font-size: clamp(2.2rem, 6vw, 5rem);
  line-height: 0.95;
}

.summary {
  max-width: 640px;
  color: #4b5563;
  font-size: 1.1rem;
  line-height: 1.7;
}

.panel {
  border: 1px solid #d5dce8;
  border-radius: 8px;
  background: #ffffff;
  padding: 24px;
  box-shadow: 0 14px 40px rgba(15, 23, 42, 0.08);
}

.panel h2 {
  margin: 0 0 8px;
  font-size: 1rem;
}

.panel p {
  margin: 0;
  color: #4b5563;
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "app.js".to_string(),
            contents: r##"const statusNode = document.querySelector("#status");
if (statusNode) {
  statusNode.textContent = "Preview served from a local Sofvary workspace.";
}
"##
            .to_string(),
        },
    ]
}

fn react_vite_file_writes(
    envelope: &PromptEnvelope,
) -> AgentGatewayResult<Vec<AgentFileWriteRequest>> {
    let title = envelope.user_intent.trim();
    let display_title = if title.is_empty() {
        "Task Board"
    } else {
        title
    };
    let escaped_title = encode_text(display_title);
    let js_title = serde_json::to_string(display_title)
        .map_err(|error| AgentGatewayError::Adapter(error.to_string()))?;

    Ok(vec![
        AgentFileWriteRequest {
            relative_path: "package.json".to_string(),
            contents: r#"{
  "name": "generated-react-vite-app",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite --host 127.0.0.1",
    "build": "tsc --noEmit && vite build",
    "preview": "vite preview --host 127.0.0.1"
  },
  "dependencies": {
    "@vitejs/plugin-react": "5.2.0",
    "@types/react": "19.2.15",
    "@types/react-dom": "19.2.3",
    "typescript": "5.9.3",
    "vite": "7.3.3",
    "react": "19.2.6",
    "react-dom": "19.2.6"
  },
  "devDependencies": {}
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "index.html".to_string(),
            contents: format!(
                r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{escaped_title}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#
            ),
        },
        AgentFileWriteRequest {
            relative_path: "vite.config.ts".to_string(),
            contents: r#"import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    strictPort: true,
  },
  preview: {
    host: "127.0.0.1",
  },
});
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "tsconfig.json".to_string(),
            contents: r#"{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2022"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx"
  },
  "include": ["src"],
  "references": []
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/main.tsx".to_string(),
            contents: r#"import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/app.css";

createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/App.tsx".to_string(),
            contents: format!(
                r#"import {{ TaskBoard }} from "./components/TaskBoard";

const appTitle = {js_title};

export function App() {{
  return (
    <main className="app-shell">
      <TaskBoard title={{appTitle}} />
    </main>
  );
}}
"#
            ),
        },
        AgentFileWriteRequest {
            relative_path: "src/components/TaskBoard.tsx".to_string(),
            contents: r#"interface Task {
  id: number;
  title: string;
  owner: string;
  status: "Backlog" | "Today" | "Done";
}

const tasks: Task[] = [
  { id: 1, title: "Shape the core workflow", owner: "Product", status: "Today" },
  { id: 2, title: "Review generated files", owner: "Engineering", status: "Backlog" },
  { id: 3, title: "Run local build checks", owner: "QA", status: "Done" },
];

const statuses: Task["status"][] = ["Backlog", "Today", "Done"];

interface TaskBoardProps {
  title: string;
}

export function TaskBoard({ title }: TaskBoardProps) {
  return (
    <section className="task-board" aria-label="Task board">
      <header className="task-board__header">
        <p>Local React app</p>
        <h1>{title}</h1>
      </header>
      <div className="task-board__columns">
        {statuses.map((status) => (
          <section className="task-column" key={status} aria-label={status}>
            <h2>{status}</h2>
            <div className="task-column__items">
              {tasks
                .filter((task) => task.status === status)
                .map((task) => (
                  <article className="task-card" key={task.id}>
                    <strong>{task.title}</strong>
                    <span>{task.owner}</span>
                  </article>
                ))}
            </div>
          </section>
        ))}
      </div>
    </section>
  );
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/styles/app.css".to_string(),
            contents: r#":root {
  color: #172033;
  background: #eef2f7;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

* {
  box-sizing: border-box;
}

body {
  min-width: 320px;
  min-height: 100vh;
  margin: 0;
}

.app-shell {
  min-height: 100vh;
  padding: 40px;
  background:
    linear-gradient(135deg, rgba(20, 184, 166, 0.16), transparent 34%),
    linear-gradient(180deg, #f8fafc 0%, #e7edf5 100%);
}

.task-board {
  width: min(1120px, 100%);
  margin: 0 auto;
}

.task-board__header {
  padding: 28px 0 24px;
}

.task-board__header p {
  margin: 0 0 10px;
  color: #0f766e;
  font-size: 13px;
  font-weight: 800;
  letter-spacing: 0;
  text-transform: uppercase;
}

.task-board__header h1 {
  max-width: 760px;
  margin: 0;
  color: #101827;
  font-size: 42px;
  line-height: 1.04;
}

.task-board__columns {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.task-column {
  min-height: 320px;
  border: 1px solid #d7dee8;
  border-radius: 8px;
  padding: 16px;
  background: rgba(255, 255, 255, 0.82);
  box-shadow: 0 18px 60px rgba(15, 23, 42, 0.08);
}

.task-column h2 {
  margin: 0 0 14px;
  color: #334155;
  font-size: 14px;
}

.task-column__items {
  display: grid;
  gap: 12px;
}

.task-card {
  display: grid;
  gap: 8px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 14px;
  background: #ffffff;
}

.task-card strong {
  color: #101827;
  font-size: 15px;
}

.task-card span {
  color: #64748b;
  font-size: 13px;
}

@media (max-width: 760px) {
  .app-shell {
    padding: 24px;
  }

  .task-board__header h1 {
    font-size: 30px;
  }

  .task-board__columns {
    grid-template-columns: 1fr;
  }
}
"#
            .to_string(),
        },
    ])
}

fn react_sqlite_file_writes(
    envelope: &PromptEnvelope,
) -> AgentGatewayResult<Vec<AgentFileWriteRequest>> {
    let title = envelope.user_intent.trim();
    let display_title = if title.is_empty() {
        "Customer Manager"
    } else {
        title
    };
    let escaped_title = encode_text(display_title);
    let js_title = serde_json::to_string(display_title)
        .map_err(|error| AgentGatewayError::Adapter(error.to_string()))?;

    Ok(vec![
        AgentFileWriteRequest {
            relative_path: "react/package.json".to_string(),
            contents: r#"{
  "name": "generated-react-sqlite-app",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite --host 127.0.0.1",
    "build": "tsc --noEmit && vite build",
    "api": "tsx server/index.ts"
  },
  "dependencies": {
    "@vitejs/plugin-react": "5.2.0",
    "@types/node": "24.12.4",
    "@types/react": "19.2.15",
    "@types/react-dom": "19.2.3",
    "sql.js": "1.14.1",
    "tsx": "4.22.3",
    "typescript": "5.9.3",
    "vite": "7.3.3",
    "react": "19.2.6",
    "react-dom": "19.2.6"
  },
  "devDependencies": {}
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/index.html".to_string(),
            contents: format!(
                r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{escaped_title}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#
            ),
        },
        AgentFileWriteRequest {
            relative_path: "react/vite.config.ts".to_string(),
            contents: r#"import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const apiPort = process.env.SOFVARY_API_PORT ?? "0";
const apiToken = process.env.SOFVARY_API_TOKEN ?? "";

export default defineConfig({
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    strictPort: true,
    proxy: {
      "/api": {
        target: `http://127.0.0.1:${apiPort}`,
        changeOrigin: false,
        headers: apiToken ? { "x-sofvary-workspace-token": apiToken } : {},
      },
    },
  },
  preview: {
    host: "127.0.0.1",
  },
});
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/tsconfig.json".to_string(),
            contents: r#"{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2022"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "types": ["node"]
  },
  "include": ["src", "server"],
  "references": []
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/src/main.tsx".to_string(),
            contents: r#"import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/app.css";

createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/src/App.tsx".to_string(),
            contents: format!(
                r#"import {{ CustomerManager }} from "./components/CustomerManager";

const appTitle = {js_title};

export function App() {{
  return (
    <main className="app-shell">
      <CustomerManager title={{appTitle}} />
    </main>
  );
}}
"#
            ),
        },
        AgentFileWriteRequest {
            relative_path: "react/src/components/CustomerManager.tsx".to_string(),
            contents: r#"import { FormEvent, useEffect, useMemo, useState } from "react";

interface Customer {
  id: number;
  name: string;
  email: string;
  company: string;
  notes: string;
  updatedAt: string;
}

interface CustomerManagerProps {
  title: string;
}

const emptyForm = { name: "", email: "", company: "", notes: "" };

export function CustomerManager({ title }: CustomerManagerProps) {
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [form, setForm] = useState(emptyForm);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("Loading local customers...");

  const filtered = useMemo(() => customers, [customers]);

  async function loadCustomers(search = query) {
    const response = await fetch(`/api/customers?search=${encodeURIComponent(search)}`);
    const payload = (await response.json()) as { customers: Customer[] };
    setCustomers(payload.customers);
    setStatus(`${payload.customers.length} local customer records`);
  }

  useEffect(() => {
    void loadCustomers("");
  }, []);

  async function submitCustomer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const method = editingId ? "PUT" : "POST";
    const url = editingId ? `/api/customers/${editingId}` : "/api/customers";
    await fetch(url, {
      method,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(form),
    });
    setForm(emptyForm);
    setEditingId(null);
    await loadCustomers();
  }

  async function deleteCustomer(id: number) {
    await fetch(`/api/customers/${id}`, { method: "DELETE" });
    await loadCustomers();
  }

  return (
    <section className="customer-manager" aria-label="Customer manager">
      <header className="customer-manager__header">
        <p>React + SQLite local app</p>
        <h1>{title}</h1>
        <span>{status}</span>
      </header>

      <form className="customer-form" onSubmit={submitCustomer}>
        <input
          value={form.name}
          onChange={(event) => setForm({ ...form, name: event.target.value })}
          placeholder="Name"
          required
        />
        <input
          value={form.email}
          onChange={(event) => setForm({ ...form, email: event.target.value })}
          placeholder="Email"
          type="email"
          required
        />
        <input
          value={form.company}
          onChange={(event) => setForm({ ...form, company: event.target.value })}
          placeholder="Company"
        />
        <input
          value={form.notes}
          onChange={(event) => setForm({ ...form, notes: event.target.value })}
          placeholder="Notes"
        />
        <button type="submit">{editingId ? "Save" : "Add"}</button>
      </form>

      <div className="toolbar">
        <input
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            void loadCustomers(event.target.value);
          }}
          placeholder="Search local customers"
        />
      </div>

      <div className="customer-list">
        {filtered.map((customer) => (
          <article className="customer-card" key={customer.id}>
            <div>
              <strong>{customer.name}</strong>
              <span>{customer.company || "No company"}</span>
            </div>
            <p>{customer.email}</p>
            <small>{customer.notes || "No notes"}</small>
            <div className="customer-card__actions">
              <button
                type="button"
                onClick={() => {
                  setEditingId(customer.id);
                  setForm({
                    name: customer.name,
                    email: customer.email,
                    company: customer.company,
                    notes: customer.notes,
                  });
                }}
              >
                Edit
              </button>
              <button type="button" onClick={() => void deleteCustomer(customer.id)}>
                Delete
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/src/styles/app.css".to_string(),
            contents: r#":root {
  color: #18212f;
  background: #eef3f8;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

* {
  box-sizing: border-box;
}

body {
  min-width: 320px;
  min-height: 100vh;
  margin: 0;
}

button,
input {
  font: inherit;
}

.app-shell {
  min-height: 100vh;
  padding: 36px;
  background: linear-gradient(180deg, #f8fafc 0%, #e8eef6 100%);
}

.customer-manager {
  width: min(1080px, 100%);
  margin: 0 auto;
}

.customer-manager__header {
  display: grid;
  gap: 10px;
  margin-bottom: 22px;
}

.customer-manager__header p {
  margin: 0;
  color: #0f766e;
  font-size: 13px;
  font-weight: 800;
  text-transform: uppercase;
}

.customer-manager__header h1 {
  max-width: 760px;
  margin: 0;
  color: #111827;
  font-size: 40px;
  line-height: 1.08;
}

.customer-manager__header span {
  color: #566274;
}

.customer-form,
.toolbar {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 16px;
}

.toolbar {
  grid-template-columns: 1fr;
}

input {
  min-width: 0;
  border: 1px solid #cbd5e1;
  border-radius: 8px;
  padding: 11px 12px;
  background: #fff;
  color: #111827;
}

button {
  border: 0;
  border-radius: 8px;
  padding: 11px 14px;
  background: #0f766e;
  color: #fff;
  cursor: pointer;
  font-weight: 700;
}

.customer-list {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 14px;
}

.customer-card {
  display: grid;
  gap: 10px;
  border: 1px solid #dbe3ee;
  border-radius: 8px;
  padding: 16px;
  background: #fff;
  box-shadow: 0 16px 48px rgba(15, 23, 42, 0.08);
}

.customer-card div:first-child {
  display: grid;
  gap: 4px;
}

.customer-card strong {
  color: #111827;
}

.customer-card span,
.customer-card p,
.customer-card small {
  margin: 0;
  color: #64748b;
}

.customer-card__actions {
  display: flex;
  gap: 8px;
}

.customer-card__actions button:last-child {
  background: #b91c1c;
}

@media (max-width: 820px) {
  .app-shell {
    padding: 22px;
  }

  .customer-form,
  .customer-list {
    grid-template-columns: 1fr;
  }
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/server/index.ts".to_string(),
            contents: r#"import http from "node:http";
import { URL } from "node:url";
import {
  createCustomer,
  deleteCustomer,
  getCustomer,
  initDatabase,
  listCustomers,
  updateCustomer,
} from "./db";

const port = Number(process.env.SOFVARY_API_PORT ?? "0");
const apiToken = process.env.SOFVARY_API_TOKEN ?? "";
const host = "127.0.0.1";
const db = await initDatabase();

const server = http.createServer(async (request, response) => {
  try {
    const url = new URL(request.url ?? "/", `http://${host}:${port}`);
    if (request.method === "GET" && url.pathname === "/api/health") {
      return sendJson(response, 200, { ok: true });
    }
    if (url.pathname.startsWith("/api/") && !hasWorkspaceToken(request)) {
      return sendJson(response, 403, { error: "Forbidden" });
    }
    if (request.method === "GET" && url.pathname === "/api/customers") {
      return sendJson(response, 200, { customers: listCustomers(db, url.searchParams.get("search") ?? "") });
    }
    if (request.method === "POST" && url.pathname === "/api/customers") {
      return sendJson(response, 201, { customer: createCustomer(db, await readBody(request)) });
    }
    const match = url.pathname.match(/^\/api\/customers\/(\d+)$/);
    if (match && request.method === "PUT") {
      const id = Number(match[1]);
      updateCustomer(db, id, await readBody(request));
      return sendJson(response, 200, { customer: getCustomer(db, id) });
    }
    if (match && request.method === "DELETE") {
      deleteCustomer(db, Number(match[1]));
      return sendJson(response, 200, { ok: true });
    }
    return sendJson(response, 404, { error: "Not found" });
  } catch (error) {
    return sendJson(response, 500, { error: error instanceof Error ? error.message : "Unknown error" });
  }
});

function hasWorkspaceToken(request: http.IncomingMessage): boolean {
  if (!apiToken) return false;
  const supplied = request.headers["x-sofvary-workspace-token"];
  return supplied === apiToken;
}

server.listen(port, host, () => {
  const address = server.address();
  const actualPort = typeof address === "object" && address ? address.port : port;
  console.log(`Sofvary local API listening at http://${host}:${actualPort}`);
});

function sendJson(response: http.ServerResponse, status: number, payload: unknown) {
  const body = JSON.stringify(payload);
  response.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
  });
  response.end(body);
}

function readBody(request: http.IncomingMessage): Promise<Record<string, string>> {
  return new Promise((resolve, reject) => {
    let body = "";
    request.on("data", (chunk) => {
      body += chunk;
      if (body.length > 1_000_000) {
        request.destroy();
        reject(new Error("Request body too large"));
      }
    });
    request.on("end", () => resolve(body.trim() ? JSON.parse(body) : {}));
    request.on("error", reject);
  });
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/server/db.ts".to_string(),
            contents: r#"import fs from "node:fs";
import path from "node:path";
import initSqlJs, { Database } from "sql.js";

export interface CustomerInput {
  name?: string;
  email?: string;
  company?: string;
  notes?: string;
}

interface CustomerRow {
  id: number;
  name: string;
  email: string;
  company: string;
  notes: string;
  updated_at: string;
}

export async function initDatabase(): Promise<Database> {
  const sqlitePath = requiredWorkspaceSqlitePath();
  fs.mkdirSync(path.dirname(sqlitePath), { recursive: true });
  const SQL = await initSqlJs();
  const db = fs.existsSync(sqlitePath) ? new SQL.Database(fs.readFileSync(sqlitePath)) : new SQL.Database();

  const migration = fs.readFileSync(path.join(path.dirname(sqlitePath), "migrations", "001_create_customers.sql"), "utf8");
  db.exec(migration);
  const existing = db.exec("SELECT COUNT(*) AS total FROM customers");
  if ((existing[0]?.values[0]?.[0] as number | undefined) === 0) {
    db.exec(fs.readFileSync(path.join(path.dirname(sqlitePath), "seed.sql"), "utf8"));
  }
  persist(db, sqlitePath);
  return db;
}

export function listCustomers(db: Database, search: string): CustomerRow[] {
  const term = `%${search.trim()}%`;
  const stmt = db.prepare(
    "SELECT id, name, email, company, notes, updated_at FROM customers WHERE name LIKE $term OR email LIKE $term OR company LIKE $term ORDER BY updated_at DESC",
  );
  stmt.bind({ $term: term });
  const rows: CustomerRow[] = [];
  while (stmt.step()) rows.push(stmt.getAsObject() as unknown as CustomerRow);
  stmt.free();
  return rows;
}

export function getCustomer(db: Database, id: number): CustomerRow | null {
  const stmt = db.prepare("SELECT id, name, email, company, notes, updated_at FROM customers WHERE id = $id");
  stmt.bind({ $id: id });
  const row = stmt.step() ? (stmt.getAsObject() as unknown as CustomerRow) : null;
  stmt.free();
  return row;
}

export function createCustomer(db: Database, input: CustomerInput): CustomerRow | null {
  const stmt = db.prepare(
    "INSERT INTO customers (name, email, company, notes, updated_at) VALUES ($name, $email, $company, $notes, datetime('now'))",
  );
  stmt.run(normalizeInput(input));
  stmt.free();
  save(db);
  return getCustomer(db, Number(db.exec("SELECT last_insert_rowid() AS id")[0].values[0][0]));
}

export function updateCustomer(db: Database, id: number, input: CustomerInput) {
  const stmt = db.prepare(
    "UPDATE customers SET name = $name, email = $email, company = $company, notes = $notes, updated_at = datetime('now') WHERE id = $id",
  );
  stmt.run({ ...normalizeInput(input), $id: id });
  stmt.free();
  save(db);
}

export function deleteCustomer(db: Database, id: number) {
  const stmt = db.prepare("DELETE FROM customers WHERE id = $id");
  stmt.run({ $id: id });
  stmt.free();
  save(db);
}

function normalizeInput(input: CustomerInput) {
  return {
    $name: String(input.name ?? "").trim(),
    $email: String(input.email ?? "").trim(),
    $company: String(input.company ?? "").trim(),
    $notes: String(input.notes ?? "").trim(),
  };
}

function requiredWorkspaceSqlitePath(): string {
  const sqlitePath = process.env.SOFVARY_SQLITE_PATH;
  if (!sqlitePath) throw new Error("SOFVARY_SQLITE_PATH is required");
  return sqlitePath;
}

function save(db: Database) {
  persist(db, requiredWorkspaceSqlitePath());
}

function persist(db: Database, sqlitePath: string) {
  fs.writeFileSync(sqlitePath, Buffer.from(db.export()));
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/server/routes/customers.ts".to_string(),
            contents: r#"export const customerRoutes = [
  "GET /api/customers",
  "POST /api/customers",
  "PUT /api/customers/:id",
  "DELETE /api/customers/:id",
] as const;
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "data/schema.json".to_string(),
            contents: r#"{
  "database": "app.sqlite",
  "tables": [
    {
      "name": "customers",
      "columns": ["id", "name", "email", "company", "notes", "updated_at"]
    }
  ]
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "data/migrations/001_create_customers.sql".to_string(),
            contents: r#"CREATE TABLE IF NOT EXISTS customers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  email TEXT NOT NULL,
  company TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "data/seed.sql".to_string(),
            contents: r#"INSERT INTO customers (name, email, company, notes, updated_at) VALUES
  ('Ada Lovelace', 'ada@example.local', 'Analytical Engines', 'Prefers concise weekly summaries.', datetime('now')),
  ('Grace Hopper', 'grace@example.local', 'Compiler Labs', 'Interested in onboarding automation.', datetime('now')),
  ('Katherine Johnson', 'katherine@example.local', 'Orbit Systems', 'Needs high-confidence delivery dates.', datetime('now'));
"#
            .to_string(),
        },
    ])
}

fn ai_agent_app_file_writes(
    envelope: &PromptEnvelope,
) -> AgentGatewayResult<Vec<AgentFileWriteRequest>> {
    let title = project_title(envelope, "AI Agent Studio");
    let escaped_title = encode_text(&title);
    let mut files = react_project_base_files(&escaped_title, "AiAgentApp", "AiAgentApp");
    files.push(AgentFileWriteRequest {
        relative_path: "ai/provider-requirements.json".to_string(),
        contents: r#"{
  "requirements": [
    {
      "id": "text-generation",
      "provider": "openai",
      "capabilities": ["text"],
      "models": ["gpt-5.1"],
      "credentialKind": "api-key",
      "required": true,
      "purpose": "Draft, edit, and summarize text jobs through the local Sofvary AI Gateway."
    },
    {
      "id": "image-generation",
      "provider": "openai",
      "capabilities": ["image"],
      "models": ["gpt-image-1"],
      "credentialKind": "api-key",
      "required": false,
      "purpose": "Create image artifacts through the local Sofvary AI Gateway."
    },
    {
      "id": "video-generation",
      "provider": "openai",
      "capabilities": ["video"],
      "models": ["sora-2"],
      "credentialKind": "api-key",
      "required": false,
      "purpose": "Create video artifacts through the local Sofvary AI Gateway."
    }
  ],
  "secretsIncluded": false,
  "bindingStatus": "needs-provider-binding"
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "ai/agents.json".to_string(),
        contents: r#"{
  "agents": [
    {
      "id": "article-agent",
      "name": "Article Agent",
      "capabilities": ["text"],
      "jobEndpoint": "/__sofvary/ai/text"
    },
    {
      "id": "image-agent",
      "name": "Image Agent",
      "capabilities": ["image"],
      "jobEndpoint": "/__sofvary/ai/image"
    },
    {
      "id": "video-agent",
      "name": "Video Agent",
      "capabilities": ["video"],
      "jobEndpoint": "/__sofvary/ai/video"
    }
  ]
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "ai/jobs.seed.json".to_string(),
        contents: r#"[
  {
    "id": "job-text-demo",
    "kind": "text",
    "status": "draft",
    "title": "Launch article outline"
  },
  {
    "id": "job-image-demo",
    "kind": "image",
    "status": "draft",
    "title": "Hero image prompt"
  },
  {
    "id": "job-video-demo",
    "kind": "video",
    "status": "draft",
    "title": "Short video brief"
  }
]
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/components/AiAgentApp.tsx".to_string(),
        contents: r#"import { FormEvent, useMemo, useState } from "react";
import { ArtifactGallery } from "./ArtifactGallery";
import { ProviderSettings } from "./ProviderSettings";

type JobKind = "text" | "image" | "video";
type JobStatus = "draft" | "queued" | "blocked" | "complete";

interface AgentJob {
  id: string;
  kind: JobKind;
  title: string;
  prompt: string;
  status: JobStatus;
  endpoint: string;
}

const initialJobs: AgentJob[] = [
  {
    id: "job-text-demo",
    kind: "text",
    title: "Article draft",
    prompt: "Write a concise product article outline.",
    status: "draft",
    endpoint: "/__sofvary/ai/text",
  },
  {
    id: "job-image-demo",
    kind: "image",
    title: "Image concept",
    prompt: "A bright desktop workspace for an AI-native app.",
    status: "draft",
    endpoint: "/__sofvary/ai/image",
  },
  {
    id: "job-video-demo",
    kind: "video",
    title: "Video brief",
    prompt: "A 10-second intro for a personal AI studio.",
    status: "draft",
    endpoint: "/__sofvary/ai/video",
  },
];

const endpointByKind: Record<JobKind, string> = {
  text: "/__sofvary/ai/text",
  image: "/__sofvary/ai/image",
  video: "/__sofvary/ai/video",
};

export function AiAgentApp() {
  const [jobs, setJobs] = useState(initialJobs);
  const [kind, setKind] = useState<JobKind>("text");
  const [prompt, setPrompt] = useState("");
  const readyCount = useMemo(() => jobs.filter((job) => job.status === "complete").length, [jobs]);

  function submitJob(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = prompt.trim();
    if (!trimmed) return;
    setJobs((current) => [
      {
        id: `job-${Date.now()}`,
        kind,
        title: `${kind[0].toUpperCase()}${kind.slice(1)} request`,
        prompt: trimmed,
        status: "queued",
        endpoint: endpointByKind[kind],
      },
      ...current,
    ]);
    setPrompt("");
  }

  function markComplete(id: string) {
    setJobs((current) =>
      current.map((job) => (job.id === id ? { ...job, status: "complete" } : job)),
    );
  }

  return (
    <main className="agent-shell">
      <section className="agent-workbench">
        <ProviderSettings />
        <section className="composer" aria-label="AI job composer">
          <header>
            <p>AI Agent App</p>
            <h1>Create with bound local providers</h1>
            <span>{readyCount} artifacts ready</span>
          </header>
          <form onSubmit={submitJob}>
            <div className="mode-switch" role="group" aria-label="Job type">
              {(["text", "image", "video"] as const).map((value) => (
                <button
                  key={value}
                  className={kind === value ? "is-active" : ""}
                  type="button"
                  onClick={() => setKind(value)}
                >
                  {value}
                </button>
              ))}
            </div>
            <textarea
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              placeholder="Describe the next agent job"
            />
            <button type="submit">Queue job</button>
          </form>
          <div className="job-list">
            {jobs.map((job) => (
              <article className="job-row" key={job.id}>
                <div>
                  <strong>{job.title}</strong>
                  <span>{job.kind} · {job.status}</span>
                </div>
                <p>{job.prompt}</p>
                <code>{job.endpoint}</code>
                <button type="button" onClick={() => markComplete(job.id)}>
                  Mark done
                </button>
              </article>
            ))}
          </div>
        </section>
        <ArtifactGallery jobs={jobs} />
      </section>
    </main>
  );
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/components/ProviderSettings.tsx".to_string(),
        contents: r#"const requirements = [
  { id: "text-generation", provider: "OpenAI", capabilities: ["text"], state: "needs-provider-binding" },
  { id: "image-generation", provider: "OpenAI", capabilities: ["image"], state: "needs-provider-binding" },
  { id: "video-generation", provider: "OpenAI", capabilities: ["video"], state: "needs-provider-binding" },
];

export function ProviderSettings() {
  return (
    <aside className="provider-panel" aria-label="Provider bindings">
      <header>
        <p>Provider binding</p>
        <h2>Needs binding</h2>
      </header>
      <div className="requirement-list">
        {requirements.map((requirement) => (
          <article key={requirement.id}>
            <strong>{requirement.provider}</strong>
            <span>{requirement.capabilities.join(", ")}</span>
            <small>{requirement.state}</small>
          </article>
        ))}
      </div>
      <p className="gateway-note">Jobs use the local Sofvary AI Gateway.</p>
    </aside>
  );
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/components/ArtifactGallery.tsx".to_string(),
        contents: r#"type JobKind = "text" | "image" | "video";
type JobStatus = "draft" | "queued" | "blocked" | "complete";

interface AgentJob {
  id: string;
  kind: JobKind;
  title: string;
  prompt: string;
  status: JobStatus;
  endpoint: string;
}

interface ArtifactGalleryProps {
  jobs: AgentJob[];
}

export function ArtifactGallery({ jobs }: ArtifactGalleryProps) {
  const completed = jobs.filter((job) => job.status === "complete");

  return (
    <section className="artifact-panel" aria-label="Artifacts">
      <header>
        <p>Artifacts</p>
        <h2>{completed.length || "No"} ready</h2>
      </header>
      <div className="artifact-grid">
        {completed.map((job) => (
          <article key={job.id}>
            <span>{job.kind}</span>
            <strong>{job.title}</strong>
            <small>{job.endpoint}/artifacts/{job.id}</small>
          </article>
        ))}
      </div>
    </section>
  );
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/styles/app.css".to_string(),
        contents: ai_agent_app_css(),
    });
    Ok(files)
}

fn canvas2d_file_writes(
    envelope: &PromptEnvelope,
) -> AgentGatewayResult<Vec<AgentFileWriteRequest>> {
    let title = envelope.user_intent.trim();
    let display_title = if title.is_empty() {
        "Coin Field"
    } else {
        title
    };
    let escaped_title = encode_text(display_title);
    let js_title = serde_json::to_string(display_title)
        .map_err(|error| AgentGatewayError::Adapter(error.to_string()))?;

    Ok(vec![
        AgentFileWriteRequest {
            relative_path: "index.html".to_string(),
            contents: format!(
                r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{escaped_title}</title>
    <link rel="stylesheet" href="./style.css" />
  </head>
  <body>
    <main class="game-shell">
      <section class="hud" aria-label="Game status">
        <div>
          <p>Coin Field</p>
          <h1>{escaped_title}</h1>
        </div>
        <div class="stats">
          <span id="score">Score 0</span>
          <span id="state">Ready</span>
        </div>
      </section>
      <canvas id="game" width="960" height="540" aria-label="Coin collector game"></canvas>
    </main>
    <script type="module" src="./src/main.js"></script>
  </body>
</html>
"#
            ),
        },
        AgentFileWriteRequest {
            relative_path: "style.css".to_string(),
            contents: r#":root {
  color: #f8fafc;
  background: #091019;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

* {
  box-sizing: border-box;
}

body {
  min-width: 320px;
  min-height: 100vh;
  margin: 0;
}

.game-shell {
  min-height: 100vh;
  display: grid;
  align-content: center;
  gap: 14px;
  padding: 28px;
  background:
    linear-gradient(135deg, rgba(20, 184, 166, 0.18), transparent 36%),
    linear-gradient(180deg, #0b1320 0%, #111827 100%);
}

.hud {
  width: min(960px, 100%);
  margin: 0 auto;
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 16px;
}

.hud p {
  margin: 0 0 6px;
  color: #5eead4;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0;
  text-transform: uppercase;
}

.hud h1 {
  margin: 0;
  font-size: 30px;
  line-height: 1.08;
}

.stats {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.stats span {
  min-width: 92px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  padding: 8px 10px;
  background: rgba(15, 23, 42, 0.86);
  color: #dbeafe;
  text-align: center;
}

canvas {
  width: min(960px, 100%);
  aspect-ratio: 16 / 9;
  margin: 0 auto;
  display: block;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 8px;
  background: #0f172a;
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.36);
}

footer {
  width: min(960px, 100%);
  margin: 0 auto;
  color: #94a3b8;
  font-size: 13px;
}

@media (max-width: 680px) {
  .game-shell {
    padding: 18px;
  }

  .hud {
    align-items: start;
    flex-direction: column;
  }

  .hud h1 {
    font-size: 24px;
  }
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/main.js".to_string(),
            contents: format!(
                r##"import {{ createLoop }} from "./engine/loop.js";
import {{ createInput }} from "./engine/input.js";
import {{ createScene }} from "./engine/scene.js";
import {{ intersects }} from "./engine/collision.js";
import {{ loadAssets }} from "./engine/assets.js";
import {{ GAME_CONFIG }} from "./game/config.js";
import {{ createPlayer, updatePlayer }} from "./game/player.js";
import {{ createEnemies, updateEnemies }} from "./game/enemies.js";
import {{ LEVELS, createCoins }} from "./game/levels.js";

const title = {js_title};
const canvas = document.querySelector("#game");
const scoreNode = document.querySelector("#score");
const stateNode = document.querySelector("#state");
if (!canvas) {{
  throw new Error("Canvas 2D context is required");
}}
const context = canvas.getContext("2d");
if (!context) {{
  throw new Error("Canvas 2D context is required");
}}
const input = createInput();
const assets = loadAssets();

let levelIndex = 0;
let state = createState();

function createState() {{
  const level = LEVELS[levelIndex % LEVELS.length];
  return {{
    title,
    level,
    player: createPlayer(level.spawn),
    enemies: createEnemies(level.enemies),
    coins: createCoins(level.coins),
    score: 0,
    paused: false,
    gameOver: false,
    won: false,
  }};
}}

function restart() {{
  state = createState();
}}

function nextLevel() {{
  levelIndex += 1;
  state = createState();
}}

function update(delta) {{
  if (input.consumeRestart()) restart();
  if (input.consumePause()) state.paused = !state.paused;
  if (state.paused || state.gameOver || state.won) return;

  updatePlayer(state.player, input, delta, GAME_CONFIG.bounds);
  updateEnemies(state.enemies, delta, GAME_CONFIG.bounds);

  for (const coin of state.coins) {{
    if (!coin.collected && intersects(state.player, coin)) {{
      coin.collected = true;
      state.score += 10;
    }}
  }}

  if (state.coins.every((coin) => coin.collected)) {{
    state.won = true;
    window.setTimeout(nextLevel, 900);
  }}

  if (state.enemies.some((enemy) => intersects(state.player, enemy))) {{
    state.gameOver = true;
  }}
}}

function render() {{
  createScene(context, canvas, state, assets);
  scoreNode.textContent = `Score ${{state.score}}`;
  stateNode.textContent = state.gameOver
    ? "Hit - R to restart"
    : state.paused
      ? "Paused"
      : state.won
        ? "Level clear"
        : `Level ${{levelIndex + 1}}`;
}}

createLoop(update, render).start();
"##
            ),
        },
        AgentFileWriteRequest {
            relative_path: "src/engine/loop.js".to_string(),
            contents: r#"export function createLoop(update, render) {
  let lastTime = performance.now();
  let running = false;

  function frame(now) {
    if (!running) return;
    const delta = Math.min((now - lastTime) / 1000, 0.05);
    lastTime = now;
    update(delta);
    render();
    requestAnimationFrame(frame);
  }

  return {
    start() {
      if (running) return;
      running = true;
      lastTime = performance.now();
      requestAnimationFrame(frame);
    },
    stop() {
      running = false;
    },
  };
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/engine/input.js".to_string(),
            contents: r#"const movementKeys = new Map([
  ["arrowup", "up"],
  ["w", "up"],
  ["arrowdown", "down"],
  ["s", "down"],
  ["arrowleft", "left"],
  ["a", "left"],
  ["arrowright", "right"],
  ["d", "right"],
]);

export function createInput() {
  const pressed = new Set();
  let pauseRequested = false;
  let restartRequested = false;

  window.addEventListener("keydown", (event) => {
    const key = event.key.toLowerCase();
    const direction = movementKeys.get(key);
    if (direction) {
      pressed.add(direction);
      event.preventDefault();
    }
    if (key === "p") pauseRequested = true;
    if (key === "r") restartRequested = true;
  });

  window.addEventListener("keyup", (event) => {
    const direction = movementKeys.get(event.key.toLowerCase());
    if (direction) pressed.delete(direction);
  });

  return {
    axisX() {
      return Number(pressed.has("right")) - Number(pressed.has("left"));
    },
    axisY() {
      return Number(pressed.has("down")) - Number(pressed.has("up"));
    },
    consumePause() {
      const value = pauseRequested;
      pauseRequested = false;
      return value;
    },
    consumeRestart() {
      const value = restartRequested;
      restartRequested = false;
      return value;
    },
  };
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/engine/scene.js".to_string(),
            contents: r##"export function createScene(context, canvas, state, assets) {
  context.clearRect(0, 0, canvas.width, canvas.height);
  drawBackground(context, canvas, state.level);
  drawCoins(context, state.coins, assets);
  drawEnemies(context, state.enemies, assets);
  drawPlayer(context, state.player, assets);
  drawOverlay(context, canvas, state);
}

function drawBackground(context, canvas, level) {
  context.fillStyle = level.background;
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.strokeStyle = "rgba(255, 255, 255, 0.06)";
  for (let x = 0; x < canvas.width; x += 48) {
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
  }
  for (let y = 0; y < canvas.height; y += 48) {
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }
}

function drawPlayer(context, player, assets) {
  context.fillStyle = assets.player;
  context.beginPath();
  context.arc(player.x, player.y, player.radius, 0, Math.PI * 2);
  context.fill();
}

function drawEnemies(context, enemies, assets) {
  context.fillStyle = assets.enemy;
  for (const enemy of enemies) {
    context.beginPath();
    context.arc(enemy.x, enemy.y, enemy.radius, 0, Math.PI * 2);
    context.fill();
  }
}

function drawCoins(context, coins, assets) {
  context.fillStyle = assets.coin;
  for (const coin of coins) {
    if (coin.collected) continue;
    context.beginPath();
    context.arc(coin.x, coin.y, coin.radius, 0, Math.PI * 2);
    context.fill();
  }
}

function drawOverlay(context, canvas, state) {
  const message = state.gameOver ? "R to restart" : state.paused ? "Paused" : state.won ? "Level clear" : "";
  if (!message) return;
  context.fillStyle = "rgba(8, 12, 20, 0.72)";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = "#f8fafc";
  context.font = "700 42px Inter, sans-serif";
  context.textAlign = "center";
  context.fillText(message, canvas.width / 2, canvas.height / 2);
}
"##
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/engine/collision.js".to_string(),
            contents: r#"export function intersects(a, b) {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  const distance = Math.hypot(dx, dy);
  return distance < a.radius + b.radius;
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/engine/assets.js".to_string(),
            contents: r##"export function loadAssets() {
  return {
    player: "#38bdf8",
    coin: "#facc15",
    enemy: "#fb7185",
  };
}
"##
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/game/config.js".to_string(),
            contents: r#"export const GAME_CONFIG = {
  bounds: {
    width: 960,
    height: 540,
    padding: 24,
  },
  playerSpeed: 260,
};
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/game/player.js".to_string(),
            contents: r#"import { GAME_CONFIG } from "./config.js";

export function createPlayer(spawn) {
  return {
    x: spawn.x,
    y: spawn.y,
    radius: 16,
    speed: GAME_CONFIG.playerSpeed,
  };
}

export function updatePlayer(player, input, delta, bounds) {
  const x = input.axisX();
  const y = input.axisY();
  const length = Math.hypot(x, y) || 1;
  player.x += (x / length) * player.speed * delta;
  player.y += (y / length) * player.speed * delta;
  player.x = clamp(player.x, bounds.padding, bounds.width - bounds.padding);
  player.y = clamp(player.y, bounds.padding, bounds.height - bounds.padding);
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/game/enemies.js".to_string(),
            contents: r#"export function createEnemies(configs) {
  return configs.map((enemy) => ({
    ...enemy,
    radius: enemy.radius ?? 18,
  }));
}

export function updateEnemies(enemies, delta, bounds) {
  for (const enemy of enemies) {
    enemy.x += enemy.vx * delta;
    enemy.y += enemy.vy * delta;
    if (enemy.x < bounds.padding || enemy.x > bounds.width - bounds.padding) {
      enemy.vx *= -1;
    }
    if (enemy.y < bounds.padding || enemy.y > bounds.height - bounds.padding) {
      enemy.vy *= -1;
    }
  }
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "src/game/levels.js".to_string(),
            contents: r##"export const LEVELS = [
  {
    background: "#0f172a",
    spawn: { x: 120, y: 270 },
    coins: [
      { x: 260, y: 140 },
      { x: 440, y: 390 },
      { x: 710, y: 190 },
      { x: 820, y: 430 },
    ],
    enemies: [
      { x: 520, y: 260, vx: 120, vy: 0 },
      { x: 760, y: 330, vx: 0, vy: 100 },
    ],
  },
  {
    background: "#102620",
    spawn: { x: 110, y: 110 },
    coins: [
      { x: 280, y: 430 },
      { x: 480, y: 160 },
      { x: 620, y: 430 },
      { x: 840, y: 260 },
    ],
    enemies: [
      { x: 360, y: 260, vx: 0, vy: 140 },
      { x: 690, y: 210, vx: 150, vy: 80 },
      { x: 820, y: 390, vx: -110, vy: 0 },
    ],
  },
];

export function createCoins(coins) {
  return coins.map((coin) => ({
    ...coin,
    radius: 12,
    collected: false,
  }));
}
"##
            .to_string(),
        },
    ])
}

fn markdown_knowledge_file_writes(
    envelope: &PromptEnvelope,
) -> AgentGatewayResult<Vec<AgentFileWriteRequest>> {
    let title = project_title(envelope, "Reading Notes");
    let js_title = json_string(&title)?;
    let escaped_title = encode_text(&title);
    let mut files = react_project_base_files(
        &escaped_title,
        "MarkdownKnowledgeApp",
        "MarkdownKnowledgeApp",
    );
    files.push(AgentFileWriteRequest {
        relative_path: "markdown/index.json".to_string(),
        contents: r#"{
  "notes": [
    {
      "id": "getting-started",
      "title": "Getting Started",
      "category": "Reading",
      "tags": ["local", "markdown", "sofvary"],
      "path": "content/getting-started.md"
    }
  ]
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "markdown/content/getting-started.md".to_string(),
        contents: format!(
            r#"# {title}

This note is stored inside `generated/markdown/content` and searched locally.

## Highlights

- Categories and tags keep reading notes organized.
- The editor updates local preview state.
- No arbitrary user notes are read or uploaded.
"#
        ),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/components/MarkdownKnowledgeApp.tsx".to_string(),
        contents: format!(
            r##"import {{ useMemo, useState }} from "react";

interface Note {{
  id: string;
  title: string;
  category: string;
  tags: string[];
  body: string;
}}

const initialNotes: Note[] = [
  {{
    id: "getting-started",
    title: {js_title},
    category: "Reading",
    tags: ["local", "markdown", "sofvary"],
    body: "# {title}\n\nThis note is stored inside generated/markdown/content and searched locally.\n\n## Highlights\n\n- Categories and tags keep reading notes organized.\n- The editor updates local preview state.\n- No arbitrary user notes are read or uploaded.",
  }},
];

function previewMarkdown(markdown: string) {{
  return markdown
    .split("\n")
    .map((line) => line.replace(/^#+\s*/, ""))
    .join("\n");
}}

export function MarkdownKnowledgeApp({{ title }}: {{ title: string }}) {{
  const [query, setQuery] = useState("");
  const [notes, setNotes] = useState(initialNotes);
  const [activeId, setActiveId] = useState(initialNotes[0].id);
  const activeNote = notes.find((note) => note.id === activeId) ?? notes[0];
  const filteredNotes = useMemo(() => {{
    const normalized = query.trim().toLowerCase();
    if (!normalized) return notes;
    return notes.filter((note) =>
      [note.title, note.category, note.body, ...note.tags]
        .join(" ")
        .toLowerCase()
        .includes(normalized),
    );
  }}, [notes, query]);

  function updateActiveBody(body: string) {{
    setNotes((current) =>
      current.map((note) => (note.id === activeNote.id ? {{ ...note, body }} : note)),
    );
  }}

  return (
    <main className="knowledge-shell">
      <aside className="sidebar">
        <p className="eyebrow">Markdown Knowledge</p>
        <h1>{{title}}</h1>
        <input
          value={{query}}
          placeholder="Search local notes"
          onChange={{(event) => setQuery(event.target.value)}}
        />
        <div className="note-list">
          {{filteredNotes.map((note) => (
            <button
              type="button"
              key={{note.id}}
              className={{note.id === activeNote.id ? "is-active" : ""}}
              onClick={{() => setActiveId(note.id)}}
            >
              <strong>{{note.title}}</strong>
              <span>{{note.category}} / {{note.tags.join(", ")}}</span>
            </button>
          ))}}
        </div>
      </aside>
      <section className="editor-pane">
        <textarea
          aria-label="Markdown editor"
          value={{activeNote.body}}
          onChange={{(event) => updateActiveBody(event.target.value)}}
        />
        <article className="preview-pane">
          <p className="eyebrow">Preview</p>
          <pre>{{previewMarkdown(activeNote.body)}}</pre>
        </article>
      </section>
    </main>
  );
}}
"##
        ),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/styles/app.css".to_string(),
        contents: markdown_knowledge_css(),
    });
    Ok(files)
}

fn data_table_file_writes(
    envelope: &PromptEnvelope,
) -> AgentGatewayResult<Vec<AgentFileWriteRequest>> {
    let title = project_title(envelope, "Inventory Table");
    let js_title = json_string(&title)?;
    let escaped_title = encode_text(&title);
    let mut files = react_project_base_files(&escaped_title, "DataTableApp", "DataTableApp");
    files.push(AgentFileWriteRequest {
        relative_path: "data/schema.json".to_string(),
        contents: r#"{
  "table": "inventory",
  "primaryKey": "id",
  "columns": ["id", "name", "category", "quantity", "location", "status"]
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "data/tables/inventory.json".to_string(),
        contents: r#"[
  { "id": 1, "name": "USB-C Hub", "category": "Hardware", "quantity": 4, "location": "Studio", "status": "Ready" },
  { "id": 2, "name": "Notebook", "category": "Supplies", "quantity": 12, "location": "Desk", "status": "Low" },
  { "id": 3, "name": "Prototype Board", "category": "Hardware", "quantity": 2, "location": "Lab", "status": "Review" }
]
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/components/DataTableApp.tsx".to_string(),
        contents: format!(
            r#"import {{ useMemo, useState }} from "react";

interface InventoryItem {{
  id: number;
  name: string;
  category: string;
  quantity: number;
  location: string;
  status: string;
}}

const initialItems: InventoryItem[] = [
  {{ id: 1, name: "USB-C Hub", category: "Hardware", quantity: 4, location: "Studio", status: "Ready" }},
  {{ id: 2, name: "Notebook", category: "Supplies", quantity: 12, location: "Desk", status: "Low" }},
  {{ id: 3, name: "Prototype Board", category: "Hardware", quantity: 2, location: "Lab", status: "Review" }},
];

export function DataTableApp({{ title = {js_title} }}: {{ title?: string }}) {{
  const [items, setItems] = useState(initialItems);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("All");
  const [sortKey, setSortKey] = useState<keyof InventoryItem>("name");
  const categories = ["All", ...Array.from(new Set(items.map((item) => item.category)))];
  const visibleItems = useMemo(() => {{
    const normalized = query.trim().toLowerCase();
    return items
      .filter((item) => category === "All" || item.category === category)
      .filter((item) => JSON.stringify(item).toLowerCase().includes(normalized))
      .sort((a, b) => String(a[sortKey]).localeCompare(String(b[sortKey])));
  }}, [category, items, query, sortKey]);

  function addItem() {{
    const id = Math.max(...items.map((item) => item.id)) + 1;
    setItems((current) => [
      ...current,
      {{ id, name: "New item", category: "Supplies", quantity: 1, location: "Inbox", status: "Draft" }},
    ]);
  }}

  function updateItem(id: number, patch: Partial<InventoryItem>) {{
    setItems((current) => current.map((item) => (item.id === id ? {{ ...item, ...patch }} : item)));
  }}

  return (
    <main className="table-shell">
      <header>
        <p className="eyebrow">Data Table Runtime</p>
        <h1>{{title}}</h1>
        <button type="button" onClick={{addItem}}>Add row</button>
      </header>
      <div className="toolbar">
        <input value={{query}} placeholder="Search local rows" onChange={{(event) => setQuery(event.target.value)}} />
        <select value={{category}} onChange={{(event) => setCategory(event.target.value)}}>
          {{categories.map((value) => <option key={{value}}>{{value}}</option>)}}
        </select>
        <select value={{sortKey}} onChange={{(event) => setSortKey(event.target.value as keyof InventoryItem)}}>
          <option value="name">Name</option>
          <option value="category">Category</option>
          <option value="quantity">Quantity</option>
          <option value="status">Status</option>
        </select>
        <button type="button" disabled title="CSV import requires an explicit user-selected file">
          CSV import
        </button>
      </div>
      <table>
        <thead>
          <tr><th>Name</th><th>Category</th><th>Qty</th><th>Location</th><th>Status</th><th>Action</th></tr>
        </thead>
        <tbody>
          {{visibleItems.map((item) => (
            <tr key={{item.id}}>
              <td><input value={{item.name}} onChange={{(event) => updateItem(item.id, {{ name: event.target.value }})}} /></td>
              <td>{{item.category}}</td>
              <td><input type="number" value={{item.quantity}} onChange={{(event) => updateItem(item.id, {{ quantity: Number(event.target.value) }})}} /></td>
              <td>{{item.location}}</td>
              <td>{{item.status}}</td>
              <td><button type="button" onClick={{() => setItems((current) => current.filter((row) => row.id !== item.id))}}>Delete</button></td>
            </tr>
          ))}}
        </tbody>
      </table>
    </main>
  );
}}
"#
        ),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/styles/app.css".to_string(),
        contents: data_table_css(),
    });
    Ok(files)
}

fn file_processor_file_writes(
    envelope: &PromptEnvelope,
) -> AgentGatewayResult<Vec<AgentFileWriteRequest>> {
    let title = project_title(envelope, "Batch Rename Tool");
    let js_title = json_string(&title)?;
    let js_app_id = json_string(&envelope.current_app_state.app_id)?;
    let escaped_title = encode_text(&title);
    let mut files =
        react_project_base_files(&escaped_title, "FileProcessorApp", "FileProcessorApp");
    files.push(AgentFileWriteRequest {
        relative_path: "file-processor/policy.json".to_string(),
        contents: r#"{
  "mode": "read-only-first",
  "requiresUserSelection": true,
  "requiresDryRunBeforeWrite": true,
  "operationLog": "runtime/logs/file-processor-operations.jsonl"
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "file-processor/dry-run-template.json".to_string(),
        contents: r#"{
  "operation": "batch-rename",
  "writesFiles": false,
  "steps": ["select-files", "preview-renames", "confirm-plan"]
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/components/FileProcessorApp.tsx".to_string(),
        contents: format!(
            r#"import {{ type ChangeEvent, useMemo, useRef, useState }} from "react";

interface SelectedFileMetadata {{
  name: string;
  extension: string;
  sizeBytes?: number;
  path?: string;
}}

interface DryRunOperation {{
  from: string;
  to: string;
  sourcePath?: string;
}}

interface TauriCore {{
  invoke<T>(command: string, args: Record<string, unknown>): Promise<T>;
}}

type HostGlobal = typeof globalThis & {{
  __TAURI__?: {{ core?: TauriCore }};
  __SOFVARY_FILE_PROCESSOR__?: {{
    recordSelectedFiles?: (payload: {{ appId: string; selectedFiles: SelectedFileMetadata[] }}) => Promise<void>;
    confirmDryRunPlan?: (payload: {{
      appId: string;
      selectedFiles: SelectedFileMetadata[];
      operations: DryRunOperation[];
    }}) => Promise<void>;
  }};
}};

const appId = {js_app_id};

function fileExtension(name: string) {{
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot) : "";
}}

async function invokeHost(command: string, payload: Record<string, unknown>) {{
  const host = globalThis as HostGlobal;
  if (command === "record_file_processor_selected_files" && host.__SOFVARY_FILE_PROCESSOR__?.recordSelectedFiles) {{
    await host.__SOFVARY_FILE_PROCESSOR__.recordSelectedFiles(payload as {{ appId: string; selectedFiles: SelectedFileMetadata[] }});
    return;
  }}
  if (command === "confirm_file_processor_dry_run_plan" && host.__SOFVARY_FILE_PROCESSOR__?.confirmDryRunPlan) {{
    await host.__SOFVARY_FILE_PROCESSOR__.confirmDryRunPlan(
      payload as {{ appId: string; selectedFiles: SelectedFileMetadata[]; operations: DryRunOperation[] }},
    );
    return;
  }}

  const invoke = host.__TAURI__?.core?.invoke;
  if (!invoke) {{
    throw new Error("Sofvary file processor host bridge is not available.");
  }}
  await invoke(command, {{ payload }});
}}

export function FileProcessorApp({{ title = {js_title} }}: {{ title?: string }}) {{
  const [prefix, setPrefix] = useState("project-photo");
  const [selectedFiles, setSelectedFiles] = useState<SelectedFileMetadata[]>([]);
  const [confirmed, setConfirmed] = useState(false);
  const [status, setStatus] = useState("Select files to create a local dry-run plan.");
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const dryRun = useMemo(
    () =>
      selectedFiles.map((file, index) => ({{
        from: file.name,
        to: prefix + "-" + String(index + 1).padStart(3, "0") + file.extension,
      }})),
    [prefix],
  );

  async function handleFileSelection(event: ChangeEvent<HTMLInputElement>) {{
    const files = Array.from(event.target.files ?? []).map((file) => ({{
      name: file.name,
      extension: fileExtension(file.name),
      sizeBytes: file.size,
    }}));
    setSelectedFiles(files);
    setConfirmed(false);
    if (files.length === 0) {{
      setStatus("No files selected.");
      return;
    }}
    try {{
      await invokeHost("record_file_processor_selected_files", {{ appId, selectedFiles: files }});
      setStatus("Selected file metadata recorded by the host.");
    }} catch (error) {{
      setStatus(error instanceof Error ? error.message : "Host bridge did not record selected files.");
    }}
  }}

  async function confirmPlan() {{
    if (dryRun.length === 0) {{
      setStatus("Select at least one file before confirming a dry-run plan.");
      return;
    }}
    try {{
      await invokeHost("confirm_file_processor_dry_run_plan", {{ appId, selectedFiles, operations: dryRun }});
      setConfirmed(true);
      setStatus("Dry-run plan recorded. Phase 14 MVP did not mutate files.");
    }} catch (error) {{
      setConfirmed(false);
      setStatus(error instanceof Error ? error.message : "Host bridge did not record the dry-run plan.");
    }}
  }}

  return (
    <main className="processor-shell">
      <header>
        <p className="eyebrow">File Processor Runtime</p>
        <h1>{{title}}</h1>
        <p>Phase 14 starts read-only. Confirmation records the dry-run plan only.</p>
      </header>
      <section className="selection-panel">
        <input ref={{fileInputRef}} type="file" multiple onChange={{handleFileSelection}} />
        <button type="button" onClick={{() => fileInputRef.current?.click()}}>
          Select files
        </button>
        <span>{{selectedFiles.length}} selected files</span>
      </section>
      <label>
        Rename prefix
        <input value={{prefix}} onChange={{(event) => setPrefix(event.target.value)}} />
      </label>
      <section>
        <h2>Dry-run preview</h2>
        {{dryRun.map((item) => (
          <article className="rename-row" key={{item.from}}>
            <span>{{item.from}}</span>
            <strong>{{item.to}}</strong>
          </article>
        ))}}
      </section>
      <button type="button" onClick={{confirmPlan}} disabled={{dryRun.length === 0}}>Confirm dry-run plan</button>
      <p className="status">
        {{confirmed ? status : status + " No file writes have been performed."}}
      </p>
    </main>
  );
}}
"#
        ),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/styles/app.css".to_string(),
        contents: file_processor_css(),
    });
    Ok(files)
}

fn desktop_widget_file_writes(
    envelope: &PromptEnvelope,
) -> AgentGatewayResult<Vec<AgentFileWriteRequest>> {
    let title = project_title(envelope, "Pomodoro Widget");
    let js_title = json_string(&title)?;
    let escaped_title = encode_text(&title);
    let mut files =
        react_project_base_files(&escaped_title, "DesktopWidgetApp", "DesktopWidgetApp");
    files.push(AgentFileWriteRequest {
        relative_path: "widget/manifest.json".to_string(),
        contents: r#"{
  "kind": "pomodoro",
  "layout": "compact",
  "runsInsidePreview": true,
  "usesAlwaysOnTopWindow": false,
  "usesSystemAutomation": false
}
"#
        .to_string(),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/components/DesktopWidgetApp.tsx".to_string(),
        contents: format!(
            r#"import {{ useMemo, useState }} from "react";

export function DesktopWidgetApp({{ title = {js_title} }}: {{ title?: string }}) {{
  const [seconds, setSeconds] = useState(25 * 60);
  const [running, setRunning] = useState(false);
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  const display = useMemo(() => `${{minutes}}:${{String(remainder).padStart(2, "0")}}`, [minutes, remainder]);

  function tick() {{
    if (running) setSeconds((value) => Math.max(0, value - 60));
  }}

  return (
    <main className="widget-shell">
      <p className="eyebrow">Desktop Widget Runtime</p>
      <h1>{{title}}</h1>
      <div className="timer" aria-label="Pomodoro timer">{{display}}</div>
      <div className="controls">
        <button type="button" onClick={{() => setRunning((value) => !value)}}>{{running ? "Pause" : "Start"}}</button>
        <button type="button" onClick={{tick}}>+ Tick</button>
        <button type="button" onClick={{() => {{ setSeconds(25 * 60); setRunning(false); }}}}>Reset</button>
      </div>
      <p className="note">Runs inside the main preview; no transparent window, tray integration, notification plugin, or system automation.</p>
    </main>
  );
}}
"#
        ),
    });
    files.push(AgentFileWriteRequest {
        relative_path: "react/src/styles/app.css".to_string(),
        contents: desktop_widget_css(),
    });
    Ok(files)
}

fn project_title(envelope: &PromptEnvelope, fallback: &str) -> String {
    let title = envelope.user_intent.trim();
    if title.is_empty() {
        fallback.to_string()
    } else {
        title.to_string()
    }
}

fn json_string(value: &str) -> AgentGatewayResult<String> {
    serde_json::to_string(value).map_err(|error| AgentGatewayError::Adapter(error.to_string()))
}

fn react_project_base_files(
    escaped_title: &str,
    component_name: &str,
    component_file: &str,
) -> Vec<AgentFileWriteRequest> {
    vec![
        AgentFileWriteRequest {
            relative_path: "react/package.json".to_string(),
            contents: r#"{
  "name": "generated-sofvary-react-app",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite --host 127.0.0.1",
    "build": "tsc --noEmit && vite build",
    "preview": "vite preview --host 127.0.0.1"
  },
  "dependencies": {
    "@vitejs/plugin-react": "5.2.0",
    "@types/react": "19.2.15",
    "@types/react-dom": "19.2.3",
    "typescript": "5.9.3",
    "vite": "7.3.3",
    "react": "19.2.6",
    "react-dom": "19.2.6"
  },
  "devDependencies": {}
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/index.html".to_string(),
            contents: format!(
                r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{escaped_title}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#
            ),
        },
        AgentFileWriteRequest {
            relative_path: "react/vite.config.ts".to_string(),
            contents: r#"import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    strictPort: true,
  },
  preview: {
    host: "127.0.0.1",
  },
});
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/tsconfig.json".to_string(),
            contents: r#"{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2022"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx"
  },
  "include": ["src"],
  "references": []
}
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/src/main.tsx".to_string(),
            contents: r#"import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/app.css";

createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
"#
            .to_string(),
        },
        AgentFileWriteRequest {
            relative_path: "react/src/App.tsx".to_string(),
            contents: format!(
                r#"import {{ {component_name} }} from "./components/{component_file}";

export function App() {{
  return <{component_name} />;
}}
"#
            ),
        },
    ]
}

fn markdown_knowledge_css() -> String {
    r#":root {
  color: #172033;
  background: #f4f7fb;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

* { box-sizing: border-box; }
body { margin: 0; min-width: 320px; min-height: 100vh; }
button, input, textarea, select { font: inherit; }

.knowledge-shell {
  min-height: 100vh;
  display: grid;
  grid-template-columns: minmax(260px, 340px) minmax(0, 1fr);
  background: #eef3f8;
}

.sidebar {
  border-right: 1px solid #d8e0ea;
  padding: 24px;
  background: #ffffff;
}

.eyebrow { margin: 0 0 10px; color: #0f766e; font-size: 12px; font-weight: 800; }
h1 { margin: 0 0 20px; font-size: 30px; line-height: 1.08; }
.sidebar input, textarea { width: 100%; border: 1px solid #cbd5e1; border-radius: 8px; padding: 12px; }
.note-list { display: grid; gap: 10px; margin-top: 16px; }
.note-list button { text-align: left; border: 1px solid #d8e0ea; border-radius: 8px; background: #f8fafc; padding: 12px; }
.note-list button.is-active { border-color: #0f766e; background: #ecfdf5; }
.note-list span { display: block; color: #64748b; font-size: 12px; margin-top: 4px; }
.editor-pane { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 16px; padding: 24px; }
textarea { min-height: calc(100vh - 48px); resize: none; background: #ffffff; }
.preview-pane { border: 1px solid #d8e0ea; border-radius: 8px; background: #ffffff; padding: 20px; }
pre { white-space: pre-wrap; line-height: 1.6; margin: 0; }
"#
    .to_string()
}

fn data_table_css() -> String {
    r#":root {
  color: #162033;
  background: #eef2f7;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
* { box-sizing: border-box; }
body { margin: 0; min-width: 320px; min-height: 100vh; }
button, input, select { font: inherit; }
.table-shell { min-height: 100vh; padding: 28px; }
header { display: flex; align-items: end; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
.eyebrow { margin: 0 0 8px; color: #0f766e; font-size: 12px; font-weight: 800; }
h1 { margin: 0; font-size: 32px; }
.toolbar { display: flex; gap: 10px; margin-bottom: 14px; }
input, select { border: 1px solid #cbd5e1; border-radius: 8px; min-height: 38px; padding: 0 10px; }
button { border: 0; border-radius: 8px; min-height: 38px; padding: 0 12px; background: #172033; color: #fff; }
button:disabled { background: #94a3b8; cursor: not-allowed; }
table { width: 100%; border-collapse: collapse; background: #fff; border: 1px solid #d8e0ea; }
th, td { border-bottom: 1px solid #e2e8f0; padding: 10px; text-align: left; }
td input { width: 100%; }
"#
    .to_string()
}

fn ai_agent_app_css() -> String {
    r#":root {
  color: #172033;
  background: #eef3f8;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

* { box-sizing: border-box; }
body { margin: 0; min-width: 320px; min-height: 100vh; }
button, textarea { font: inherit; }

.agent-shell {
  min-height: 100vh;
  padding: 28px;
  background:
    linear-gradient(135deg, rgba(20, 184, 166, 0.16), transparent 35%),
    linear-gradient(180deg, #f8fafc 0%, #e8eef6 100%);
}

.agent-workbench {
  display: grid;
  grid-template-columns: minmax(230px, 280px) minmax(0, 1fr) minmax(240px, 320px);
  gap: 16px;
  width: min(1280px, 100%);
  margin: 0 auto;
}

.provider-panel,
.composer,
.artifact-panel {
  border: 1px solid #d8e0ea;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.9);
  box-shadow: 0 18px 52px rgba(15, 23, 42, 0.08);
}

.provider-panel,
.artifact-panel {
  padding: 18px;
}

.composer {
  min-height: calc(100vh - 56px);
  padding: 22px;
}

header p {
  margin: 0 0 8px;
  color: #0f766e;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0;
  text-transform: uppercase;
}

h1, h2 { margin: 0; color: #111827; line-height: 1.08; }
h1 { font-size: 34px; }
h2 { font-size: 22px; }
header span { display: block; margin-top: 8px; color: #64748b; }

form {
  display: grid;
  gap: 12px;
  margin: 20px 0;
}

.mode-switch {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
}

button {
  border: 0;
  border-radius: 8px;
  min-height: 38px;
  padding: 0 12px;
  background: #172033;
  color: #fff;
  cursor: pointer;
  font-weight: 700;
}

.mode-switch button {
  background: #e2e8f0;
  color: #334155;
}

.mode-switch button.is-active {
  background: #0f766e;
  color: #fff;
}

textarea {
  width: 100%;
  min-height: 132px;
  resize: vertical;
  border: 1px solid #cbd5e1;
  border-radius: 8px;
  padding: 12px;
  background: #fff;
  color: #111827;
}

.job-list,
.requirement-list,
.artifact-grid {
  display: grid;
  gap: 10px;
}

.job-row,
.requirement-list article,
.artifact-grid article {
  display: grid;
  gap: 8px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 12px;
  background: #ffffff;
}

.job-row div {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.job-row strong,
.requirement-list strong,
.artifact-grid strong {
  color: #111827;
}

.job-row span,
.job-row p,
.job-row code,
.requirement-list span,
.requirement-list small,
.artifact-grid span,
.artifact-grid small,
.gateway-note {
  margin: 0;
  color: #64748b;
  font-size: 13px;
}

.job-row code,
.artifact-grid small {
  overflow-wrap: anywhere;
}

.job-row button {
  justify-self: start;
  background: #475569;
}

.gateway-note {
  margin-top: 14px;
}

@media (max-width: 980px) {
  .agent-shell { padding: 18px; }
  .agent-workbench { grid-template-columns: 1fr; }
  .composer { min-height: auto; }
}
"#
    .to_string()
}

fn file_processor_css() -> String {
    r#":root {
  color: #172033;
  background: #f1f5f9;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
* { box-sizing: border-box; }
body { margin: 0; min-width: 320px; min-height: 100vh; }
button, input { font: inherit; }
.processor-shell { width: min(920px, calc(100vw - 40px)); margin: 0 auto; padding: 34px 0; }
.eyebrow { margin: 0 0 8px; color: #0f766e; font-size: 12px; font-weight: 800; }
h1 { margin: 0 0 12px; font-size: 34px; }
.selection-panel { display: flex; align-items: center; gap: 12px; border: 1px solid #d8e0ea; border-radius: 8px; background: #fff; padding: 14px; margin: 18px 0; }
label { display: grid; gap: 8px; margin-bottom: 18px; font-weight: 700; }
input { border: 1px solid #cbd5e1; border-radius: 8px; min-height: 40px; padding: 0 10px; }
button { border: 0; border-radius: 8px; min-height: 40px; padding: 0 14px; background: #172033; color: #fff; }
button:disabled { background: #94a3b8; cursor: not-allowed; }
.rename-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; border: 1px solid #d8e0ea; border-radius: 8px; background: #fff; padding: 12px; margin-bottom: 8px; }
.status { min-height: 24px; color: #0f766e; font-weight: 700; }
"#
    .to_string()
}

fn desktop_widget_css() -> String {
    r#":root {
  color: #111827;
  background: #e9eef5;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
* { box-sizing: border-box; }
body { margin: 0; min-width: 320px; min-height: 100vh; display: grid; place-items: center; }
button { font: inherit; }
.widget-shell { width: min(360px, calc(100vw - 32px)); border: 1px solid #d5dce8; border-radius: 8px; background: #fff; padding: 22px; box-shadow: 0 20px 60px rgba(15, 23, 42, 0.12); }
.eyebrow { margin: 0 0 8px; color: #0f766e; font-size: 12px; font-weight: 800; }
h1 { margin: 0 0 18px; font-size: 24px; }
.timer { display: grid; place-items: center; min-height: 120px; border-radius: 8px; background: #172033; color: #fff; font-size: 54px; font-weight: 800; }
.controls { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; margin-top: 12px; }
button { border: 0; border-radius: 8px; min-height: 38px; background: #0f766e; color: #fff; }
.note { color: #64748b; font-size: 13px; line-height: 1.5; }
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness_engine::HarnessEngine;
    use crate::core::pack_manager::{parse_harness_pack_manifest, parse_runtime_pack_manifest};
    use crate::core::workspace_manager::WorkspaceManager;
    use crate::core::workspace_types::{
        AppBoxManifest, WorkspaceConstraints, WorkspaceMode, WorkspacePaths, WorkspacePreview,
    };
    use crate::platform::CommandSpec;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.static-html/0.1.0/manifest.json"
    );
    const HARNESS_MANIFEST: &str =
        include_str!("../../builtin-packs/harness/sofvary.harness.static-html/0.1.0/manifest.json");
    const REACT_RUNTIME_MANIFEST: &str =
        include_str!("../../builtin-packs/runtimes/sofvary.runtime.react-vite/0.1.0/manifest.json");
    const REACT_HARNESS_MANIFEST: &str =
        include_str!("../../builtin-packs/harness/sofvary.harness.react-vite/0.1.0/manifest.json");
    const REACT_SQLITE_RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.react-sqlite/0.1.0/manifest.json"
    );
    const REACT_SQLITE_HARNESS_MANIFEST: &str = include_str!(
        "../../builtin-packs/harness/sofvary.harness.react-sqlite/0.1.0/manifest.json"
    );
    const CANVAS2D_RUNTIME_MANIFEST: &str =
        include_str!("../../builtin-packs/runtimes/sofvary.runtime.canvas2d/0.1.0/manifest.json");
    const CANVAS2D_HARNESS_MANIFEST: &str =
        include_str!("../../builtin-packs/harness/sofvary.harness.canvas2d/0.1.0/manifest.json");
    const MARKDOWN_KNOWLEDGE_RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.markdown-knowledge/0.1.0/manifest.json"
    );
    const MARKDOWN_KNOWLEDGE_HARNESS_MANIFEST: &str = include_str!(
        "../../builtin-packs/harness/sofvary.harness.markdown-knowledge/0.1.0/manifest.json"
    );
    const DATA_TABLE_RUNTIME_MANIFEST: &str =
        include_str!("../../builtin-packs/runtimes/sofvary.runtime.data-table/0.1.0/manifest.json");
    const DATA_TABLE_HARNESS_MANIFEST: &str =
        include_str!("../../builtin-packs/harness/sofvary.harness.data-table/0.1.0/manifest.json");
    const FILE_PROCESSOR_RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.file-processor/0.1.0/manifest.json"
    );
    const FILE_PROCESSOR_HARNESS_MANIFEST: &str = include_str!(
        "../../builtin-packs/harness/sofvary.harness.file-processor/0.1.0/manifest.json"
    );
    const DESKTOP_WIDGET_RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.desktop-widget/0.1.0/manifest.json"
    );
    const DESKTOP_WIDGET_HARNESS_MANIFEST: &str = include_str!(
        "../../builtin-packs/harness/sofvary.harness.desktop-widget/0.1.0/manifest.json"
    );
    const AI_AGENT_APP_RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.ai-agent-app/0.1.0/manifest.json"
    );
    const AI_AGENT_APP_HARNESS_MANIFEST: &str = include_str!(
        "../../builtin-packs/harness/sofvary.harness.multimodal-studio-agent/0.1.0/manifest.json"
    );

    #[test]
    fn cli_fallback_requires_a_passing_cli_test() {
        let mut config = AgentConfig {
            id: "codex".to_string(),
            provider: crate::core::agent_config::AgentProvider::Codex,
            label: "Codex".to_string(),
            enabled: true,
            acp: None,
            cli: Some(crate::core::agent_config::AgentCommandConfig {
                executable: PathBuf::from("/bin/codex"),
                args: vec![],
                env: HashMap::new(),
                source: crate::core::agent_config::AgentInstallSource::ExternalPath,
            }),
            allow_cli_fallback: true,
            last_test: None,
        };

        assert!(!cli_fallback_is_verified(&config));

        config.last_test = Some(crate::core::agent_config::AgentTestRecord {
            ok: true,
            transport: AgentTransportKind::Acp,
            checked_at: "2026-06-10T00:00:00Z".to_string(),
            detail: "ACP ok".to_string(),
        });
        assert!(!cli_fallback_is_verified(&config));

        config.last_test = Some(crate::core::agent_config::AgentTestRecord {
            ok: true,
            transport: AgentTransportKind::Cli,
            checked_at: "2026-06-10T00:00:00Z".to_string(),
            detail: "CLI ok".to_string(),
        });
        assert!(cli_fallback_is_verified(&config));
    }

    #[test]
    fn mock_adapter_event_flow_writes_static_files() {
        let temp = TempDir::new().expect("tempdir");
        let workspace_manager = WorkspaceManager::new();
        let manifest = test_manifest(temp.path());
        let envelope = test_prompt_envelope("Build a tiny notes app");

        let gateway = AgentGateway::new(MockAgentAdapter);
        let session = gateway
            .run(&manifest, &envelope, &workspace_manager)
            .expect("mock gateway runs");

        assert_eq!(session.adapter, AgentAdapterKind::Mock);
        assert_eq!(session.app_id, manifest.app_id);
        assert_eq!(session.envelope_id, envelope.envelope_id);
        assert!(matches!(
            session.events.first(),
            Some(AgentEvent::SessionStarted { .. })
        ));
        assert!(session
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::Planning { .. })));
        assert_eq!(
            session
                .events
                .iter()
                .filter(|event| matches!(event, AgentEvent::FileWriteRequested { .. }))
                .count(),
            3
        );
        assert_eq!(
            session
                .events
                .iter()
                .filter(|event| matches!(event, AgentEvent::FileWritten { .. }))
                .count(),
            3
        );
        assert!(session
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::BuildStarted { .. })));
        assert!(matches!(session.events.last(), Some(AgentEvent::Completed)));

        let index = std::fs::read_to_string(manifest.paths.generated_static.join("index.html"))
            .expect("index");
        assert!(index.contains("Build a tiny notes app"));
        assert!(!index.contains("floating command"));
        assert!(!index.contains("Sofvary UI"));
    }

    #[test]
    fn mock_adapter_event_flow_writes_react_vite_files() {
        let temp = TempDir::new().expect("tempdir");
        let workspace_manager = WorkspaceManager::new();
        let manifest = test_manifest_with_mode(temp.path(), WorkspaceMode::ReactVite);
        let envelope = test_react_prompt_envelope("Build a React task board");

        let gateway = AgentGateway::new(MockAgentAdapter);
        let session = gateway
            .run(&manifest, &envelope, &workspace_manager)
            .expect("mock gateway runs");

        assert_eq!(session.adapter, AgentAdapterKind::Mock);
        assert_eq!(
            session
                .events
                .iter()
                .filter(|event| matches!(event, AgentEvent::FileWriteRequested { .. }))
                .count(),
            8
        );
        assert!(session.events.iter().any(
            |event| matches!(event, AgentEvent::BuildStarted { target } if target == "react-vite")
        ));

        let react_root = manifest.paths.generated.join("react");
        assert!(react_root.join("package.json").exists());
        assert!(react_root.join("src/components/TaskBoard.tsx").exists());

        let package_json =
            std::fs::read_to_string(react_root.join("package.json")).expect("package");
        assert!(package_json.contains("\"@types/react\""));
        assert!(package_json.contains("\"@types/react-dom\""));

        let app = std::fs::read_to_string(react_root.join("src/App.tsx")).expect("app");
        let board = std::fs::read_to_string(react_root.join("src/components/TaskBoard.tsx"))
            .expect("board");
        let generated = [app, board].join("\n");
        assert!(generated.contains("Build a React task board"));
        assert!(!generated.contains("FloatingCommandMenu"));
        assert!(!generated.contains("BuildOverlay"));
        assert!(!generated.contains("Sofvary UI"));
    }

    #[test]
    fn mock_adapter_event_flow_writes_react_sqlite_files() {
        let temp = TempDir::new().expect("tempdir");
        let workspace_manager = WorkspaceManager::new();
        let manifest = test_manifest_with_mode(temp.path(), WorkspaceMode::ReactSqlite);
        let envelope = test_react_sqlite_prompt_envelope("Build a customer CRM");

        let gateway = AgentGateway::new(MockAgentAdapter);
        let session = gateway
            .run(&manifest, &envelope, &workspace_manager)
            .expect("mock gateway runs");

        assert_eq!(session.adapter, AgentAdapterKind::Mock);
        assert_eq!(
            session
                .events
                .iter()
                .filter(|event| matches!(event, AgentEvent::FileWriteRequested { .. }))
                .count(),
            14
        );
        assert!(session.events.iter().any(
            |event| matches!(event, AgentEvent::BuildStarted { target } if target == "react-sqlite")
        ));

        let generated_root = &manifest.paths.generated;
        assert!(generated_root
            .join("react/src/components/CustomerManager.tsx")
            .exists());
        assert!(generated_root
            .join("data/migrations/001_create_customers.sql")
            .exists());
        assert!(generated_root.join("data/seed.sql").exists());

        let server_db =
            std::fs::read_to_string(generated_root.join("react/server/db.ts")).expect("server db");
        let api_server = std::fs::read_to_string(generated_root.join("react/server/index.ts"))
            .expect("api server");
        let vite_config = std::fs::read_to_string(generated_root.join("react/vite.config.ts"))
            .expect("vite config");
        let frontend = std::fs::read_to_string(
            generated_root.join("react/src/components/CustomerManager.tsx"),
        )
        .expect("frontend");
        assert!(server_db.contains("db.prepare("));
        assert!(server_db.contains("stmt.bind({ $term: term })"));
        assert!(api_server.contains("SOFVARY_API_TOKEN"));
        assert!(api_server.contains("hasWorkspaceToken"));
        assert!(vite_config.contains("x-sofvary-workspace-token"));
        assert!(frontend.contains("fetch(`/api/customers"));
        assert!(!frontend.contains("sql.js"));
        assert!(!frontend.contains("app.sqlite"));
        assert!(!frontend.contains("FloatingCommandMenu"));
        assert!(!server_db.contains("FloatingCommandMenu"));
        assert!(!api_server.contains("FloatingCommandMenu"));
    }

    #[test]
    fn mock_adapter_event_flow_writes_canvas2d_files() {
        let temp = TempDir::new().expect("tempdir");
        let workspace_manager = WorkspaceManager::new();
        let manifest = test_manifest_with_mode(temp.path(), WorkspaceMode::Canvas2d);
        let envelope = test_canvas2d_prompt_envelope("Build a coin chase game");

        let gateway = AgentGateway::new(MockAgentAdapter);
        let session = gateway
            .run(&manifest, &envelope, &workspace_manager)
            .expect("mock gateway runs");

        assert_eq!(session.adapter, AgentAdapterKind::Mock);
        assert_eq!(
            session
                .events
                .iter()
                .filter(|event| matches!(event, AgentEvent::FileWriteRequested { .. }))
                .count(),
            12
        );
        assert!(session.events.iter().any(
            |event| matches!(event, AgentEvent::BuildStarted { target } if target == "canvas2d")
        ));

        let canvas_root = manifest.paths.generated.join("canvas");
        assert!(canvas_root.join("index.html").exists());
        assert!(canvas_root.join("src/main.js").exists());
        assert!(canvas_root.join("src/engine/loop.js").exists());
        assert!(canvas_root.join("src/game/levels.js").exists());
        assert!(canvas_root.join("assets").is_dir());

        let main = std::fs::read_to_string(canvas_root.join("src/main.js")).expect("main");
        let loop_js =
            std::fs::read_to_string(canvas_root.join("src/engine/loop.js")).expect("loop");
        let index = std::fs::read_to_string(canvas_root.join("index.html")).expect("index");
        let generated = [main, loop_js, index].join("\n");

        assert!(generated.contains("Build a coin chase game"));
        assert!(generated.contains("requestAnimationFrame"));
        assert!(generated.contains("getContext(\"2d\")"));
        assert!(!generated.contains("http://"));
        assert!(!generated.contains("https://"));
        assert!(!generated.contains("FloatingCommandMenu"));
        assert!(!generated.contains("BuildOverlay"));
        assert!(!generated.contains("Sofvary UI"));
        assert!(!generated.contains("React"));
    }

    #[test]
    fn mock_adapter_event_flow_writes_phase12_to15_project_files() {
        let cases = [
            (
                WorkspaceMode::MarkdownKnowledge,
                "markdown-knowledge",
                "Build reading notes",
                "markdown/index.json",
                "react/src/components/MarkdownKnowledgeApp.tsx",
                "No arbitrary user notes are read or uploaded.",
                10,
            ),
            (
                WorkspaceMode::DataTable,
                "data-table",
                "Build inventory table",
                "data/tables/inventory.json",
                "react/src/components/DataTableApp.tsx",
                "CSV import requires an explicit user-selected file",
                10,
            ),
            (
                WorkspaceMode::FileProcessor,
                "file-processor",
                "Build batch rename",
                "file-processor/policy.json",
                "react/src/components/FileProcessorApp.tsx",
                "record_file_processor_selected_files",
                10,
            ),
            (
                WorkspaceMode::DesktopWidget,
                "desktop-widget",
                "Build pomodoro widget",
                "widget/manifest.json",
                "react/src/components/DesktopWidgetApp.tsx",
                "no transparent window",
                9,
            ),
        ];

        for (
            mode,
            runtime_kind,
            intent,
            required_data_file,
            required_component,
            required_component_text,
            expected_write_count,
        ) in cases
        {
            let temp = TempDir::new().expect("tempdir");
            let workspace_manager = WorkspaceManager::new();
            let manifest = test_manifest_with_mode(temp.path(), mode);
            let envelope = test_phase12_to15_prompt_envelope(intent, mode);

            let session = AgentGateway::new(MockAgentAdapter)
                .run(&manifest, &envelope, &workspace_manager)
                .expect("mock gateway runs");

            assert_eq!(
                session
                    .events
                    .iter()
                    .filter(|event| matches!(event, AgentEvent::FileWriteRequested { .. }))
                    .count(),
                expected_write_count
            );
            assert!(session.events.iter().any(
                |event| matches!(event, AgentEvent::BuildStarted { target } if target == runtime_kind)
            ));

            let generated_root = &manifest.paths.generated;
            assert!(generated_root.join(required_data_file).exists());
            assert!(generated_root.join(required_component).exists());
            let component = std::fs::read_to_string(generated_root.join(required_component))
                .expect("component");
            assert!(component.contains(required_component_text));
            assert!(!component.contains("FloatingCommandMenu"));
            assert!(!component.contains("BuildOverlay"));
            assert!(!component.contains("Sofvary UI"));

            if mode == WorkspaceMode::FileProcessor {
                assert!(!component.contains("selected demo files"));
                assert!(!component.contains("Plan confirmed and logged"));
            }
        }
    }

    #[test]
    fn mock_adapter_event_flow_writes_ai_agent_app_files() {
        let temp = TempDir::new().expect("tempdir");
        let workspace_manager = WorkspaceManager::new();
        let manifest = test_manifest_with_mode(temp.path(), WorkspaceMode::AiAgentApp);
        let envelope =
            test_ai_agent_app_prompt_envelope("Build an article, image, and video agent app");

        let session = AgentGateway::new(MockAgentAdapter)
            .run(&manifest, &envelope, &workspace_manager)
            .expect("mock gateway runs");

        assert_eq!(
            session
                .events
                .iter()
                .filter(|event| matches!(event, AgentEvent::FileWriteRequested { .. }))
                .count(),
            13
        );
        assert!(session.events.iter().any(
            |event| matches!(event, AgentEvent::BuildStarted { target } if target == "ai-agent-app")
        ));

        let generated_root = &manifest.paths.generated;
        assert!(generated_root
            .join("ai/provider-requirements.json")
            .exists());
        assert!(generated_root.join("ai/agents.json").exists());
        assert!(generated_root
            .join("react/src/components/AiAgentApp.tsx")
            .exists());
        assert!(generated_root
            .join("react/src/components/ProviderSettings.tsx")
            .exists());
        assert!(generated_root
            .join("react/src/components/ArtifactGallery.tsx")
            .exists());

        let provider_metadata =
            std::fs::read_to_string(generated_root.join("ai/provider-requirements.json"))
                .expect("provider metadata");
        assert!(provider_metadata.contains("\"secretsIncluded\": false"));
        assert!(provider_metadata.contains("needs-provider-binding"));
        assert!(!provider_metadata.contains("apiKey"));
        assert!(!provider_metadata.contains("secureKeyRef"));
        assert!(!provider_metadata.contains("providerId"));

        let component =
            std::fs::read_to_string(generated_root.join("react/src/components/AiAgentApp.tsx"))
                .expect("agent component");
        assert!(component.contains("/__sofvary/ai/text"));
        assert!(component.contains("/__sofvary/ai/image"));
        assert!(component.contains("/__sofvary/ai/video"));
        assert!(!component.contains("FloatingCommandMenu"));
        assert!(!component.contains("BuildOverlay"));
        assert!(!component.contains("Coding Agent Gateway"));
    }

    #[test]
    fn acp_adapter_skeleton_is_not_implemented() {
        let envelope = test_prompt_envelope("Build anything");
        let error = AcpAgentAdapter
            .generate(&envelope)
            .expect_err("ACP skeleton should not run");

        assert!(matches!(
            error,
            AgentGatewayError::AdapterNotImplemented(message)
                if message.contains("Phase 8 skeleton")
        ));
    }

    #[test]
    fn command_requests_are_rejected_without_approval() {
        let temp = TempDir::new().expect("tempdir");
        let workspace_manager = WorkspaceManager::new();
        let manifest = test_manifest(temp.path());
        let envelope = test_prompt_envelope("Build command test");

        let gateway = AgentGateway::new(CommandRequestAdapter);
        let session = gateway
            .run(&manifest, &envelope, &workspace_manager)
            .expect("gateway runs");

        assert!(session
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::CommandRequested { executable } if executable == "node")));
        assert!(session
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::CommandRejected { executable, .. } if executable == "node")));
        assert!(!session
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::CommandApproved { .. })));
    }

    #[test]
    fn file_write_escape_is_rejected_by_workspace_boundary() {
        let temp = TempDir::new().expect("tempdir");
        let workspace_manager = WorkspaceManager::new();
        let manifest = test_manifest(temp.path());
        let envelope = test_prompt_envelope("Build escape test");

        let gateway = AgentGateway::new(EscapeAdapter);
        let error = gateway
            .run(&manifest, &envelope, &workspace_manager)
            .expect_err("extra path should be rejected");

        assert!(matches!(error, AgentGatewayError::Workspace(_)));
        assert!(!temp.path().join("secret.txt").exists());
    }

    #[test]
    fn event_summary_does_not_include_generated_file_contents() {
        let envelope = test_prompt_envelope("Very secret full prompt");
        let output = MockAgentAdapter.generate(&envelope).expect("output");

        let summaries = summarize_agent_events(&output.events);
        let joined = summaries.join("\n");

        assert!(joined.contains("Agent requested file write: index.html"));
        assert!(!joined.contains("<!doctype html>"));
        assert!(!joined.contains("Very secret full prompt"));
    }

    struct CommandRequestAdapter;

    impl AgentAdapter for CommandRequestAdapter {
        fn kind(&self) -> AgentAdapterKind {
            AgentAdapterKind::Mock
        }

        fn generate(&self, envelope: &PromptEnvelope) -> AgentGatewayResult<AgentAdapterOutput> {
            let mut output = MockAgentAdapter.generate(envelope)?;
            output.command_requests.push(CommandSpec {
                executable: PathBuf::from("node"),
                args: vec!["--version".to_string()],
                cwd: PathBuf::from("."),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(1_000),
                kill_on_drop: true,
            });
            Ok(output)
        }
    }

    struct EscapeAdapter;

    impl AgentAdapter for EscapeAdapter {
        fn kind(&self) -> AgentAdapterKind {
            AgentAdapterKind::Mock
        }

        fn generate(&self, envelope: &PromptEnvelope) -> AgentGatewayResult<AgentAdapterOutput> {
            let mut output = MockAgentAdapter.generate(envelope)?;
            output.file_writes.push(AgentFileWriteRequest {
                relative_path: "../secret.txt".to_string(),
                contents: "escape".to_string(),
            });
            output.events.push(AgentEvent::FileWriteRequested {
                relative_path: "../secret.txt".to_string(),
            });
            Ok(output)
        }
    }

    fn test_prompt_envelope(user_intent: &str) -> PromptEnvelope {
        let temp = TempDir::new().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = parse_runtime_pack_manifest(RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(HARNESS_MANIFEST).expect("harness");
        HarnessEngine::new()
            .create_static_html_envelope(user_intent, &manifest, &runtime, &harness)
            .expect("prompt envelope")
    }

    fn test_react_prompt_envelope(user_intent: &str) -> PromptEnvelope {
        let temp = TempDir::new().expect("tempdir");
        let manifest = test_manifest_with_mode(temp.path(), WorkspaceMode::ReactVite);
        let runtime = parse_runtime_pack_manifest(REACT_RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(REACT_HARNESS_MANIFEST).expect("harness");
        HarnessEngine::new()
            .create_react_vite_envelope(user_intent, &manifest, &runtime, &harness)
            .expect("prompt envelope")
    }

    fn test_react_sqlite_prompt_envelope(user_intent: &str) -> PromptEnvelope {
        let temp = TempDir::new().expect("tempdir");
        let manifest = test_manifest_with_mode(temp.path(), WorkspaceMode::ReactSqlite);
        let runtime = parse_runtime_pack_manifest(REACT_SQLITE_RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(REACT_SQLITE_HARNESS_MANIFEST).expect("harness");
        HarnessEngine::new()
            .create_react_sqlite_envelope(user_intent, &manifest, &runtime, &harness)
            .expect("prompt envelope")
    }

    fn test_ai_agent_app_prompt_envelope(user_intent: &str) -> PromptEnvelope {
        let temp = TempDir::new().expect("tempdir");
        let manifest = test_manifest_with_mode(temp.path(), WorkspaceMode::AiAgentApp);
        let runtime = parse_runtime_pack_manifest(AI_AGENT_APP_RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(AI_AGENT_APP_HARNESS_MANIFEST).expect("harness");
        HarnessEngine::new()
            .create_ai_agent_app_envelope(user_intent, &manifest, &runtime, &harness)
            .expect("prompt envelope")
    }

    fn test_canvas2d_prompt_envelope(user_intent: &str) -> PromptEnvelope {
        let temp = TempDir::new().expect("tempdir");
        let manifest = test_manifest_with_mode(temp.path(), WorkspaceMode::Canvas2d);
        let runtime = parse_runtime_pack_manifest(CANVAS2D_RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(CANVAS2D_HARNESS_MANIFEST).expect("harness");
        HarnessEngine::new()
            .create_canvas2d_envelope(user_intent, &manifest, &runtime, &harness)
            .expect("prompt envelope")
    }

    fn test_phase12_to15_prompt_envelope(user_intent: &str, mode: WorkspaceMode) -> PromptEnvelope {
        let temp = TempDir::new().expect("tempdir");
        let manifest = test_manifest_with_mode(temp.path(), mode);
        let engine = HarnessEngine::new();
        match mode {
            WorkspaceMode::MarkdownKnowledge => {
                let runtime = parse_runtime_pack_manifest(MARKDOWN_KNOWLEDGE_RUNTIME_MANIFEST)
                    .expect("runtime");
                let harness = parse_harness_pack_manifest(MARKDOWN_KNOWLEDGE_HARNESS_MANIFEST)
                    .expect("harness");
                engine
                    .create_markdown_knowledge_envelope(user_intent, &manifest, &runtime, &harness)
                    .expect("prompt envelope")
            }
            WorkspaceMode::DataTable => {
                let runtime =
                    parse_runtime_pack_manifest(DATA_TABLE_RUNTIME_MANIFEST).expect("runtime");
                let harness =
                    parse_harness_pack_manifest(DATA_TABLE_HARNESS_MANIFEST).expect("harness");
                engine
                    .create_data_table_envelope(user_intent, &manifest, &runtime, &harness)
                    .expect("prompt envelope")
            }
            WorkspaceMode::FileProcessor => {
                let runtime =
                    parse_runtime_pack_manifest(FILE_PROCESSOR_RUNTIME_MANIFEST).expect("runtime");
                let harness =
                    parse_harness_pack_manifest(FILE_PROCESSOR_HARNESS_MANIFEST).expect("harness");
                engine
                    .create_file_processor_envelope(user_intent, &manifest, &runtime, &harness)
                    .expect("prompt envelope")
            }
            WorkspaceMode::DesktopWidget => {
                let runtime =
                    parse_runtime_pack_manifest(DESKTOP_WIDGET_RUNTIME_MANIFEST).expect("runtime");
                let harness =
                    parse_harness_pack_manifest(DESKTOP_WIDGET_HARNESS_MANIFEST).expect("harness");
                engine
                    .create_desktop_widget_envelope(user_intent, &manifest, &runtime, &harness)
                    .expect("prompt envelope")
            }
            _ => panic!("unsupported generated project mode"),
        }
    }

    fn test_manifest(base: &std::path::Path) -> AppBoxManifest {
        test_manifest_with_mode(base, WorkspaceMode::StaticHtml)
    }

    fn test_manifest_with_mode(base: &std::path::Path, mode: WorkspaceMode) -> AppBoxManifest {
        let root = base.join("apps").join("app_test");
        let generated = root.join("generated");
        let generated_static = generated.join("static");
        std::fs::create_dir_all(&generated_static).expect("static root");
        std::fs::create_dir_all(generated.join("react")).expect("react root");
        std::fs::create_dir_all(generated.join("data")).expect("data root");
        std::fs::create_dir_all(generated.join("canvas")).expect("canvas root");

        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test App".to_string(),
            mode,
            created_at: "2026-05-29T00:00:00Z".to_string(),
            updated_at: "2026-05-29T00:00:00Z".to_string(),
            stack: vec![],
            paths: WorkspacePaths {
                root: root.clone(),
                generated,
                generated_static,
                runtime: root.join("runtime"),
                snapshots: root.join("snapshots"),
            },
            constraints: WorkspaceConstraints {
                boundary: root,
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        }
    }
}
