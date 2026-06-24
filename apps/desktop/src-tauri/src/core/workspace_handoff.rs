use crate::core::agent_config::AgentConfig;
use crate::core::harness_engine::{
    summarize_prompt_envelope, HarnessEngine, HarnessEngineError, PromptEnvelope,
    PromptEnvelopeSummary,
};
use crate::core::pack_manager::{PackError, PackManager};
use crate::core::pack_types::{HarnessPackManifest, RuntimePackManifest};
use crate::core::runtime_diagnostic::RuntimeDiagnostic;
use crate::core::software_naming::suggest_software_name;
use crate::core::workspace_handoff_prompt::{
    build_agents_md, build_claude_md, build_handoff_prompt, build_repair_prompt, build_tools_md,
};
use crate::core::workspace_manager::{WorkspaceError, WorkspaceManager};
use crate::core::workspace_types::{AppBoxManifest, RuntimeKind};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceHandoffError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("pack error: {0}")]
    Pack(#[from] PackError),
    #[error("harness error: {0}")]
    Harness(#[from] HarnessEngineError),
    #[error("workspace handoff path escapes its boundary: {0}")]
    PathEscape(PathBuf),
}

pub type WorkspaceHandoffResult<T> = Result<T, WorkspaceHandoffError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceHandoffPreparation {
    pub manifest: AppBoxManifest,
    pub prompt_envelope: PromptEnvelope,
    pub prompt_envelope_summary: PromptEnvelopeSummary,
    pub prompt_path: PathBuf,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRuntimeContract {
    pub runtime_kind: String,
    pub generated_root: String,
    pub entrypoint: String,
    pub allowed_files: Vec<String>,
    pub bind: String,
    pub network: String,
    pub allow_shell: bool,
    pub allow_package_install: bool,
    pub shell_ui_included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffDiagnosticsFile {
    pub status: String,
    #[serde(default)]
    pub diagnostic: Option<RuntimeDiagnostic>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub source_detail: Option<String>,
}

pub fn prepare_new_handoff_workspace(
    requirement: &str,
    runtime_kind: RuntimeKind,
    workspace_manager: &WorkspaceManager,
    agent_config: &AgentConfig,
) -> WorkspaceHandoffResult<WorkspaceHandoffPreparation> {
    let name = derive_handoff_workspace_name(requirement);
    let manifest = workspace_manager.create_workspace_for_runtime(name, runtime_kind)?;
    let preparation =
        prepare_handoff_for_manifest(requirement, manifest, workspace_manager, agent_config)?;
    Ok(preparation)
}

pub fn prepare_existing_handoff_workspace(
    requirement: &str,
    manifest: AppBoxManifest,
    workspace_manager: &WorkspaceManager,
    agent_config: &AgentConfig,
) -> WorkspaceHandoffResult<WorkspaceHandoffPreparation> {
    prepare_handoff_for_manifest(requirement, manifest, workspace_manager, agent_config)
}

pub fn prepare_handoff_for_manifest(
    requirement: &str,
    manifest: AppBoxManifest,
    workspace_manager: &WorkspaceManager,
    agent_config: &AgentConfig,
) -> WorkspaceHandoffResult<WorkspaceHandoffPreparation> {
    let (runtime_pack, harness_pack) = resolve_packs_for_runtime(manifest.mode)?;
    let envelope = create_prompt_envelope(requirement, &manifest, &runtime_pack, &harness_pack)?;
    let prompt = build_handoff_prompt(&envelope, &agent_config.label);
    write_handoff_files(&manifest, &envelope, &prompt)?;
    workspace_manager
        .update_lockfile_agent_adapter_for_manifest(&manifest, "workspace-handoff".to_string())?;
    Ok(WorkspaceHandoffPreparation {
        prompt_envelope_summary: summarize_prompt_envelope(&envelope),
        prompt_path: manifest.paths.root.join("SOFVARY_AGENT_PROMPT.md"),
        manifest,
        prompt_envelope: envelope,
        prompt,
    })
}

pub fn write_handoff_files(
    manifest: &AppBoxManifest,
    envelope: &PromptEnvelope,
    prompt: &str,
) -> WorkspaceHandoffResult<()> {
    let sofvary_dir = manifest.paths.root.join(".sofvary");
    let requests_dir = sofvary_dir.join("requests");
    ensure_inside_workspace(&manifest.paths.root, &sofvary_dir)?;
    fs::create_dir_all(&requests_dir)?;
    fs::create_dir_all(
        manifest
            .paths
            .root
            .join(&envelope.box_runtime_context.generated_root),
    )?;

    fs::write(
        manifest.paths.root.join("AGENTS.md"),
        build_agents_md(envelope),
    )?;
    fs::write(
        manifest.paths.root.join("CLAUDE.md"),
        build_claude_md(envelope),
    )?;
    fs::write(manifest.paths.root.join("SOFVARY_AGENT_PROMPT.md"), prompt)?;
    fs::write(
        sofvary_dir.join("task.md"),
        task_markdown(manifest, envelope),
    )?;
    write_json(sofvary_dir.join("prompt-envelope.json"), envelope)?;
    write_json(
        sofvary_dir.join("runtime-contract.json"),
        &runtime_contract(envelope),
    )?;
    write_json(
        sofvary_dir.join("allowed-files.json"),
        &json!({
            "generatedRoot": envelope.box_runtime_context.generated_root,
            "files": envelope.output_contract.files,
        }),
    )?;
    write_json(
        sofvary_dir.join("diagnostics.json"),
        &HandoffDiagnosticsFile {
            status: "empty".to_string(),
            diagnostic: None,
            summary: None,
            source_detail: None,
        },
    )?;
    fs::write(sofvary_dir.join("tools.md"), build_tools_md())?;
    if !sofvary_dir.join("agent-status.jsonl").exists() {
        fs::write(sofvary_dir.join("agent-status.jsonl"), "")?;
    }
    Ok(())
}

pub fn read_handoff_prompt(manifest: &AppBoxManifest) -> WorkspaceHandoffResult<String> {
    Ok(fs::read_to_string(
        manifest.paths.root.join("SOFVARY_AGENT_PROMPT.md"),
    )?)
}

pub fn read_handoff_envelope(manifest: &AppBoxManifest) -> WorkspaceHandoffResult<PromptEnvelope> {
    let bytes = fs::read(manifest.paths.root.join(".sofvary/prompt-envelope.json"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_handoff_diagnostics(
    manifest: &AppBoxManifest,
    envelope: &PromptEnvelope,
    diagnostic: &RuntimeDiagnostic,
    source_detail: &str,
) -> WorkspaceHandoffResult<PathBuf> {
    let sofvary_dir = manifest.paths.root.join(".sofvary");
    let diagnostics = HandoffDiagnosticsFile {
        status: "failed".to_string(),
        diagnostic: Some(diagnostic.clone()),
        summary: Some(diagnostic.summary()),
        source_detail: Some(source_detail.to_string()),
    };
    write_json(sofvary_dir.join("diagnostics.json"), &diagnostics)?;
    let repair_prompt = build_repair_prompt(envelope, diagnostic, 1);
    let repair_path = sofvary_dir.join("repair-prompt.md");
    fs::write(&repair_path, repair_prompt)?;
    Ok(repair_path)
}

pub fn read_handoff_repair_prompt(manifest: &AppBoxManifest) -> WorkspaceHandoffResult<String> {
    Ok(fs::read_to_string(
        manifest.paths.root.join(".sofvary/repair-prompt.md"),
    )?)
}

pub fn append_handoff_request_consumed(
    manifest: &AppBoxManifest,
    request_name: &str,
) -> WorkspaceHandoffResult<()> {
    let request_path = manifest
        .paths
        .root
        .join(".sofvary")
        .join("requests")
        .join(request_name);
    if request_path.exists() {
        fs::remove_file(request_path)?;
    }
    Ok(())
}

fn resolve_packs_for_runtime(
    runtime_kind: RuntimeKind,
) -> WorkspaceHandoffResult<(RuntimePackManifest, HarnessPackManifest)> {
    let pack_manager = PackManager::new()?;
    let packs = match runtime_kind {
        RuntimeKind::StaticHtml => {
            let packs = pack_manager.resolve_static_html_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
        RuntimeKind::ReactVite => {
            let packs = pack_manager.resolve_react_vite_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
        RuntimeKind::ReactSqlite => {
            let packs = pack_manager.resolve_react_sqlite_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
        RuntimeKind::AiAgentApp => {
            let packs = pack_manager.resolve_ai_agent_app_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
        RuntimeKind::Canvas2d => {
            let packs = pack_manager.resolve_canvas2d_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
        RuntimeKind::MarkdownKnowledge => {
            let packs = pack_manager.resolve_markdown_knowledge_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
        RuntimeKind::DataTable => {
            let packs = pack_manager.resolve_data_table_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
        RuntimeKind::FileProcessor => {
            let packs = pack_manager.resolve_file_processor_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
        RuntimeKind::DesktopWidget => {
            let packs = pack_manager.resolve_desktop_widget_packs()?;
            (packs.runtime.manifest, packs.harness.manifest)
        }
    };
    Ok(packs)
}

fn create_prompt_envelope(
    requirement: &str,
    manifest: &AppBoxManifest,
    runtime_pack: &RuntimePackManifest,
    harness_pack: &HarnessPackManifest,
) -> WorkspaceHandoffResult<PromptEnvelope> {
    let engine = HarnessEngine::new();
    Ok(match manifest.mode {
        RuntimeKind::StaticHtml => {
            engine.create_static_html_envelope(requirement, manifest, runtime_pack, harness_pack)?
        }
        RuntimeKind::ReactVite => {
            engine.create_react_vite_envelope(requirement, manifest, runtime_pack, harness_pack)?
        }
        RuntimeKind::ReactSqlite => engine.create_react_sqlite_envelope(
            requirement,
            manifest,
            runtime_pack,
            harness_pack,
        )?,
        RuntimeKind::AiAgentApp => engine.create_ai_agent_app_envelope(
            requirement,
            manifest,
            runtime_pack,
            harness_pack,
        )?,
        RuntimeKind::Canvas2d => {
            engine.create_canvas2d_envelope(requirement, manifest, runtime_pack, harness_pack)?
        }
        RuntimeKind::MarkdownKnowledge => engine.create_markdown_knowledge_envelope(
            requirement,
            manifest,
            runtime_pack,
            harness_pack,
        )?,
        RuntimeKind::DataTable => {
            engine.create_data_table_envelope(requirement, manifest, runtime_pack, harness_pack)?
        }
        RuntimeKind::FileProcessor => engine.create_file_processor_envelope(
            requirement,
            manifest,
            runtime_pack,
            harness_pack,
        )?,
        RuntimeKind::DesktopWidget => engine.create_desktop_widget_envelope(
            requirement,
            manifest,
            runtime_pack,
            harness_pack,
        )?,
    })
}

fn runtime_contract(envelope: &PromptEnvelope) -> WorkspaceRuntimeContract {
    WorkspaceRuntimeContract {
        runtime_kind: envelope.box_runtime_context.runtime_kind.clone(),
        generated_root: envelope.box_runtime_context.generated_root.clone(),
        entrypoint: envelope.box_runtime_context.entrypoint.clone(),
        allowed_files: envelope.output_contract.files.clone(),
        bind: envelope.box_runtime_context.bind.clone(),
        network: envelope.box_runtime_context.network.clone(),
        allow_shell: envelope.command_policy.allow_shell,
        allow_package_install: envelope.command_policy.allow_package_install,
        shell_ui_included: envelope.output_contract.shell_ui_included,
    }
}

fn task_markdown(manifest: &AppBoxManifest, envelope: &PromptEnvelope) -> String {
    format!(
        "# Sofvary Handoff Task\n\nWorkspace: {}\nRuntime: {}\nGenerated root: {}\nEntrypoint: {}\n\n## User Intent\n\n{}\n",
        manifest.name,
        envelope.box_runtime_context.runtime_kind,
        envelope.box_runtime_context.generated_root,
        envelope.box_runtime_context.entrypoint,
        envelope.user_intent,
    )
}

fn write_json(path: impl AsRef<Path>, value: &impl Serialize) -> WorkspaceHandoffResult<()> {
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
}

fn ensure_inside_workspace(workspace_root: &Path, path: &Path) -> WorkspaceHandoffResult<()> {
    let workspace_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let candidate = path
        .parent()
        .unwrap_or(path)
        .canonicalize()
        .unwrap_or_else(|_| path.parent().unwrap_or(path).to_path_buf());
    if candidate.starts_with(&workspace_root) {
        Ok(())
    } else {
        Err(WorkspaceHandoffError::PathEscape(path.to_path_buf()))
    }
}

fn derive_handoff_workspace_name(requirement: &str) -> String {
    suggest_software_name(requirement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness_engine::{
        BoxRuntimeContext, CommandPolicy, CurrentAppState, FileSystemPolicy, HarnessPolicy,
        OutputContract, PackReference, RuntimePolicy,
    };
    use crate::core::workspace_types::{
        RuntimeKind, WorkspaceConstraints, WorkspacePaths, WorkspacePreview,
    };

    #[test]
    fn writes_required_handoff_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = manifest_for_root(temp.path().to_path_buf());
        let envelope = envelope();
        let prompt = build_handoff_prompt(&envelope, "Codex");

        write_handoff_files(&manifest, &envelope, &prompt).expect("write handoff");

        for path in [
            "AGENTS.md",
            "CLAUDE.md",
            "SOFVARY_AGENT_PROMPT.md",
            ".sofvary/task.md",
            ".sofvary/prompt-envelope.json",
            ".sofvary/runtime-contract.json",
            ".sofvary/allowed-files.json",
            ".sofvary/diagnostics.json",
            ".sofvary/tools.md",
            ".sofvary/agent-status.jsonl",
        ] {
            assert!(temp.path().join(path).exists(), "{path} should exist");
        }
        assert!(temp.path().join(".sofvary/requests").is_dir());
        let prompt_text =
            fs::read_to_string(temp.path().join("SOFVARY_AGENT_PROMPT.md")).expect("prompt");
        assert!(prompt_text.contains("Build a timer"));
        assert!(prompt_text.contains("generated/static"));
    }

    #[test]
    fn writes_diagnostics_and_repair_prompt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = manifest_for_root(temp.path().to_path_buf());
        let envelope = envelope();
        write_handoff_files(&manifest, &envelope, "prompt").expect("write handoff");
        let diagnostic = RuntimeDiagnostic {
            runtime_kind: RuntimeKind::StaticHtml,
            stage: crate::core::runtime_diagnostic::RuntimeDiagnosticStage::WorkspaceValidation,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some("missing index.html".to_string()),
            log_path: None,
            category: crate::core::runtime_diagnostic::RuntimeDiagnosticCategory::GeneratedCode,
            repairable_by: crate::core::runtime_diagnostic::RuntimeDiagnosticRepairTarget::Agent,
        };

        let repair_path =
            write_handoff_diagnostics(&manifest, &envelope, &diagnostic, "validation failed")
                .expect("diagnostics");

        assert!(repair_path.exists());
        let repair = fs::read_to_string(repair_path).expect("repair");
        assert!(repair.contains("Sofvary Repair Handoff"));
        let diagnostics =
            fs::read_to_string(temp.path().join(".sofvary/diagnostics.json")).expect("json");
        assert!(diagnostics.contains("validation failed"));
    }

    fn manifest_for_root(root: PathBuf) -> AppBoxManifest {
        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: RuntimeKind::StaticHtml,
            created_at: "2026-06-22T00:00:00Z".to_string(),
            updated_at: "2026-06-22T00:00:00Z".to_string(),
            stack: vec!["static".to_string()],
            paths: WorkspacePaths {
                root: root.clone(),
                generated: root.join("generated"),
                generated_static: root.join("generated/static"),
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

    fn envelope() -> PromptEnvelope {
        PromptEnvelope {
            schema_version: "1.0".to_string(),
            envelope_id: "penv_test".to_string(),
            created_at: "2026-06-22T00:00:00Z".to_string(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: "runtime".to_string(),
                    version: "0.1.0".to_string(),
                },
                harness_packs: vec![PackReference {
                    id: "harness".to_string(),
                    version: "0.1.0".to_string(),
                }],
                runtime_kind: "static-html".to_string(),
                generated_root: "generated/static".to_string(),
                entrypoint: "index.html".to_string(),
                bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
            },
            user_intent: "Build a timer".to_string(),
            current_app_state: CurrentAppState {
                app_id: "app_test".to_string(),
                workspace_name: "Test".to_string(),
                mode: "static-html".to_string(),
                existing_files: Vec::new(),
                file_context: Vec::new(),
                preview_state: "empty".to_string(),
            },
            runtime_policy: RuntimePolicy {
                runtime_kind: "static-html".to_string(),
                allowed_entrypoints: vec!["index.html".to_string()],
                allowed_server_bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
                package_install: false,
            },
            harness_policy: HarnessPolicy {
                system_instructions: Vec::new(),
                file_system_rules: Vec::new(),
                output_rules: Vec::new(),
                blocked_capabilities: Vec::new(),
            },
            file_system_policy: FileSystemPolicy {
                root: "generated/static".to_string(),
                allowed_files: vec!["index.html".to_string()],
                allow_external_files: false,
                allow_path_traversal: false,
            },
            command_policy: CommandPolicy {
                allow_shell: false,
                allow_package_install: false,
                allow_global_install: false,
                allowed_commands: Vec::new(),
            },
            output_contract: OutputContract {
                format: "static-html-files".to_string(),
                files: vec!["index.html".to_string()],
                shell_ui_included: false,
            },
            acceptance_criteria: vec!["Runs locally".to_string()],
        }
    }
}
