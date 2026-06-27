use crate::core::pack_manager::{read_pack_resource_text, PackError, PackKind};
use crate::core::pack_types::{HarnessPackManifest, RuntimePackManifest};
use crate::core::prompt_template::{render_template_list, PromptTemplateError};
use crate::core::workspace_types::AppBoxManifest;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

const PROMPT_ENVELOPE_SCHEMA_VERSION: &str = "1.0";
const MAX_CONTEXT_FILE_BYTES: usize = 32 * 1024;
const MAX_CONTEXT_TOTAL_BYTES: usize = 128 * 1024;

#[derive(Debug, Error)]
pub enum HarnessEngineError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("pack error: {0}")]
    Pack(#[from] PackError),
    #[error("prompt template error: {0}")]
    PromptTemplate(#[from] PromptTemplateError),
    #[error("runtime pack is not compatible with the selected harness compiler: {0}")]
    IncompatibleRuntime(String),
    #[error("harness pack is not compatible with runtime pack {runtime_id}: {harness_id}")]
    IncompatibleHarness {
        runtime_id: String,
        harness_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BoxRuntimeContext {
    pub runtime_pack: PackReference,
    pub harness_packs: Vec<PackReference>,
    pub runtime_kind: String,
    pub generated_root: String,
    pub entrypoint: String,
    pub bind: String,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentAppFileContext {
    pub relative_path: String,
    pub contents: String,
    pub byte_size: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentAppState {
    pub app_id: String,
    pub workspace_name: String,
    pub mode: String,
    pub existing_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_context: Vec<CurrentAppFileContext>,
    pub preview_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePolicy {
    pub runtime_kind: String,
    pub allowed_entrypoints: Vec<String>,
    pub allowed_server_bind: String,
    pub network: String,
    pub package_install: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessPolicy {
    pub system_instructions: Vec<String>,
    pub file_system_rules: Vec<String>,
    pub output_rules: Vec<String>,
    pub blocked_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemPolicy {
    pub root: String,
    pub allowed_files: Vec<String>,
    pub allow_external_files: bool,
    pub allow_path_traversal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandPolicy {
    pub allow_shell: bool,
    pub allow_package_install: bool,
    pub allow_global_install: bool,
    pub allowed_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutputContract {
    pub format: String,
    pub files: Vec<String>,
    pub shell_ui_included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptEnvelope {
    pub schema_version: String,
    pub envelope_id: String,
    pub created_at: String,
    pub box_runtime_context: BoxRuntimeContext,
    pub user_intent: String,
    pub current_app_state: CurrentAppState,
    pub runtime_policy: RuntimePolicy,
    pub harness_policy: HarnessPolicy,
    pub file_system_policy: FileSystemPolicy,
    pub command_policy: CommandPolicy,
    pub output_contract: OutputContract,
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptEnvelopeSummary {
    pub runtime: String,
    pub harnesses: Vec<String>,
    pub allowed_files: Vec<String>,
    pub blocked_capabilities: Vec<String>,
    pub output_contract: Vec<String>,
    pub acceptance_criteria_count: usize,
}

#[derive(Default)]
pub struct HarnessEngine;

impl HarnessEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn create_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        create_configured_envelope(user_intent, manifest, runtime_pack, harness_pack)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimePromptEnvelopeConfig {
    output_format: String,
    allowed_files: Vec<String>,
    file_system_root: String,
    runtime_policy: RuntimePolicyConfig,
    command_policy: CommandPolicy,
    harness_policy: HarnessPolicy,
    acceptance_criteria: Vec<String>,
    #[serde(default)]
    update_mode: Option<UpdateModePromptConfig>,
    #[serde(default)]
    shell_ui_included: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimePolicyConfig {
    #[serde(default)]
    allowed_entrypoints: Vec<String>,
    package_install: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateModePromptConfig {
    #[serde(default)]
    system_instructions: Vec<String>,
    #[serde(default)]
    output_rules: Vec<String>,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
}

fn create_configured_envelope(
    user_intent: &str,
    manifest: &AppBoxManifest,
    runtime_pack: &RuntimePackManifest,
    harness_pack: &HarnessPackManifest,
) -> Result<PromptEnvelope, HarnessEngineError> {
    validate_runtime_harness(runtime_pack, harness_pack)?;

    let envelope_config: RuntimePromptEnvelopeConfig =
        serde_json::from_str(&read_pack_resource_text(
            PackKind::Runtime,
            &runtime_pack.id,
            &runtime_pack.version,
            &runtime_pack.prompt_envelope,
        )?)?;
    let harness_config: HarnessPolicy = serde_json::from_str(&read_pack_resource_text(
        PackKind::Harness,
        &harness_pack.id,
        &harness_pack.version,
        &harness_pack.prompt_policy,
    )?)?;

    if envelope_config.allowed_files.is_empty() {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime prompt envelope {} must declare allowedFiles",
            runtime_pack.prompt_envelope
        )));
    }

    let variables = template_variables(user_intent, manifest, runtime_pack, harness_pack);
    let allowed_files = render_template_list(&envelope_config.allowed_files, &variables)?;
    let context_root = runtime_pack
        .executor
        .context_root
        .as_deref()
        .unwrap_or(&envelope_config.file_system_root);
    let context_root_path = manifest.paths.root.join(context_root);
    let existing_files = existing_files_under(&context_root_path)?;
    let current_app_state =
        build_current_app_state(manifest, existing_files, &context_root_path, &allowed_files)?;

    let mut harness_policy = render_policy(envelope_config.harness_policy, &variables)?;
    let harness_policy_overlay = render_policy(harness_config, &variables)?;
    merge_policy(&mut harness_policy, harness_policy_overlay);

    let mut acceptance_criteria =
        render_template_list(&envelope_config.acceptance_criteria, &variables)?;
    if current_app_state.mode == "update" {
        if let Some(update_mode) = envelope_config.update_mode {
            append_unique(
                &mut harness_policy.system_instructions,
                &render_template_list(&update_mode.system_instructions, &variables)?,
            );
            append_unique(
                &mut harness_policy.output_rules,
                &render_template_list(&update_mode.output_rules, &variables)?,
            );
            append_unique(
                &mut acceptance_criteria,
                &render_template_list(&update_mode.acceptance_criteria, &variables)?,
            );
        }
    }

    let allowed_entrypoints = if envelope_config
        .runtime_policy
        .allowed_entrypoints
        .is_empty()
    {
        vec![runtime_pack.runtime.entrypoint.clone()]
    } else {
        render_template_list(
            &envelope_config.runtime_policy.allowed_entrypoints,
            &variables,
        )?
    };

    Ok(PromptEnvelope {
        schema_version: PROMPT_ENVELOPE_SCHEMA_VERSION.to_string(),
        envelope_id: format!("penv_{}", Uuid::new_v4().simple()),
        created_at: Utc::now().to_rfc3339(),
        box_runtime_context: BoxRuntimeContext {
            runtime_pack: PackReference {
                id: runtime_pack.id.clone(),
                version: runtime_pack.version.clone(),
            },
            harness_packs: vec![PackReference {
                id: harness_pack.id.clone(),
                version: harness_pack.version.clone(),
            }],
            runtime_kind: runtime_pack.runtime.kind.clone(),
            generated_root: runtime_pack.runtime.generated_root.clone(),
            entrypoint: runtime_pack.runtime.entrypoint.clone(),
            bind: runtime_pack.runtime.bind.clone(),
            network: runtime_pack.runtime.network.clone(),
        },
        user_intent: user_intent.trim().to_string(),
        current_app_state,
        runtime_policy: RuntimePolicy {
            runtime_kind: runtime_pack.runtime.kind.clone(),
            allowed_entrypoints,
            allowed_server_bind: runtime_pack.runtime.bind.clone(),
            network: runtime_pack.runtime.network.clone(),
            package_install: envelope_config.runtime_policy.package_install,
        },
        harness_policy,
        file_system_policy: FileSystemPolicy {
            root: envelope_config.file_system_root,
            allowed_files: allowed_files.clone(),
            allow_external_files: false,
            allow_path_traversal: false,
        },
        command_policy: CommandPolicy {
            allow_shell: envelope_config.command_policy.allow_shell,
            allow_package_install: envelope_config.command_policy.allow_package_install,
            allow_global_install: envelope_config.command_policy.allow_global_install,
            allowed_commands: render_template_list(
                &envelope_config.command_policy.allowed_commands,
                &variables,
            )?,
        },
        output_contract: OutputContract {
            format: envelope_config.output_format,
            files: allowed_files,
            shell_ui_included: envelope_config.shell_ui_included,
        },
        acceptance_criteria,
    })
}

fn validate_runtime_harness(
    runtime_pack: &RuntimePackManifest,
    harness_pack: &HarnessPackManifest,
) -> Result<(), HarnessEngineError> {
    if harness_pack.runtime.as_deref() != Some(runtime_pack.id.as_str()) {
        return Err(HarnessEngineError::IncompatibleHarness {
            runtime_id: runtime_pack.id.clone(),
            harness_id: harness_pack.id.clone(),
        });
    }
    Ok(())
}

fn template_variables(
    user_intent: &str,
    manifest: &AppBoxManifest,
    runtime_pack: &RuntimePackManifest,
    harness_pack: &HarnessPackManifest,
) -> HashMap<String, String> {
    HashMap::from([
        (
            "runtime.kind".to_string(),
            runtime_pack.runtime.kind.clone(),
        ),
        ("runtime.id".to_string(), runtime_pack.id.clone()),
        ("runtime.version".to_string(), runtime_pack.version.clone()),
        (
            "runtime.generatedRoot".to_string(),
            runtime_pack.runtime.generated_root.clone(),
        ),
        (
            "runtime.entrypoint".to_string(),
            runtime_pack.runtime.entrypoint.clone(),
        ),
        (
            "runtime.bind".to_string(),
            runtime_pack.runtime.bind.clone(),
        ),
        (
            "runtime.network".to_string(),
            runtime_pack.runtime.network.clone(),
        ),
        (
            "executor.kind".to_string(),
            runtime_pack.executor.kind.clone(),
        ),
        ("harness.id".to_string(), harness_pack.id.clone()),
        ("harness.version".to_string(), harness_pack.version.clone()),
        ("workspace.name".to_string(), manifest.name.clone()),
        ("workspace.id".to_string(), manifest.app_id.clone()),
        ("user.intent".to_string(), user_intent.trim().to_string()),
        ("diagnostic.summary".to_string(), String::new()),
    ])
}

fn render_policy(
    policy: HarnessPolicy,
    variables: &HashMap<String, String>,
) -> Result<HarnessPolicy, PromptTemplateError> {
    Ok(HarnessPolicy {
        system_instructions: render_template_list(&policy.system_instructions, variables)?,
        file_system_rules: render_template_list(&policy.file_system_rules, variables)?,
        output_rules: render_template_list(&policy.output_rules, variables)?,
        blocked_capabilities: render_template_list(&policy.blocked_capabilities, variables)?,
    })
}

fn merge_policy(policy: &mut HarnessPolicy, overlay: HarnessPolicy) {
    append_unique(
        &mut policy.system_instructions,
        &overlay.system_instructions,
    );
    append_unique(&mut policy.file_system_rules, &overlay.file_system_rules);
    append_unique(&mut policy.output_rules, &overlay.output_rules);
    append_unique(
        &mut policy.blocked_capabilities,
        &overlay.blocked_capabilities,
    );
}

fn build_current_app_state(
    manifest: &AppBoxManifest,
    existing_files: Vec<String>,
    context_root: &Path,
    allowed_files: &[String],
) -> Result<CurrentAppState, std::io::Error> {
    let file_context = if existing_files.is_empty() {
        Vec::new()
    } else {
        read_existing_file_context(context_root, &existing_files, allowed_files)?
    };

    Ok(CurrentAppState {
        app_id: manifest.app_id.clone(),
        workspace_name: manifest.name.clone(),
        mode: if existing_files.is_empty() {
            "create".to_string()
        } else {
            "update".to_string()
        },
        existing_files,
        file_context,
        preview_state: manifest.preview.state.clone(),
    })
}

fn read_existing_file_context(
    root: &Path,
    existing_files: &[String],
    allowed_files: &[String],
) -> Result<Vec<CurrentAppFileContext>, std::io::Error> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut used_bytes = 0_usize;
    let mut context = Vec::new();

    for relative_path in existing_files {
        if used_bytes >= MAX_CONTEXT_TOTAL_BYTES {
            break;
        }
        if !allowed_files
            .iter()
            .any(|allowed_file| allowed_file == relative_path)
        {
            continue;
        }
        if !is_safe_relative_context_path(relative_path) {
            continue;
        }

        let path = root.join(relative_path);
        if !path.is_file() {
            continue;
        }

        let bytes = fs::read(&path)?;
        let remaining_bytes = MAX_CONTEXT_TOTAL_BYTES.saturating_sub(used_bytes);
        let take_bytes = bytes.len().min(MAX_CONTEXT_FILE_BYTES).min(remaining_bytes);
        if take_bytes == 0 {
            break;
        }

        let truncated = bytes.len() > take_bytes;
        let contents = String::from_utf8_lossy(&bytes[..take_bytes]).to_string();
        used_bytes += take_bytes;
        context.push(CurrentAppFileContext {
            relative_path: relative_path.clone(),
            contents,
            byte_size: bytes.len(),
            truncated,
        });
    }

    Ok(context)
}

fn existing_files_under(root: &Path) -> Result<Vec<String>, std::io::Error> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_relative_files(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_relative_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_relative_files(root, &path, files)?;
        } else if entry.file_type()?.is_file() {
            if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

fn is_safe_relative_context_path(relative_path: &str) -> bool {
    let path = Path::new(relative_path);
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

fn append_unique(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        if !value.trim().is_empty() && !target.contains(value) {
            target.push(value.clone());
        }
    }
}

pub fn summarize_prompt_envelope(envelope: &PromptEnvelope) -> PromptEnvelopeSummary {
    PromptEnvelopeSummary {
        runtime: format!(
            "{}@{}",
            envelope.box_runtime_context.runtime_pack.id,
            envelope.box_runtime_context.runtime_pack.version
        ),
        harnesses: envelope
            .box_runtime_context
            .harness_packs
            .iter()
            .map(|pack| format!("{}@{}", pack.id, pack.version))
            .collect(),
        allowed_files: envelope.file_system_policy.allowed_files.clone(),
        blocked_capabilities: envelope.harness_policy.blocked_capabilities.clone(),
        output_contract: envelope.output_contract.files.clone(),
        acceptance_criteria_count: envelope.acceptance_criteria.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pack_manager::{
        parse_harness_pack_manifest, parse_runtime_pack_manifest, read_pack_resource_text,
    };
    use crate::core::workspace_types::{
        RuntimeKind, WorkspaceConstraints, WorkspacePaths, WorkspacePreview,
    };

    #[test]
    fn creates_prompt_envelope_from_pack_resources() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path(), "static-html".to_string());
        let runtime =
            parse_runtime_pack_manifest(&builtin_runtime_manifest("sofvary.runtime.static-html"))
                .expect("runtime");
        let harness =
            parse_harness_pack_manifest(&builtin_harness_manifest("sofvary.harness.static-html"))
                .expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_envelope("Build a timer", &manifest, &runtime, &harness)
            .expect("envelope");

        assert_eq!(envelope.schema_version, "1.0");
        assert_eq!(envelope.user_intent, "Build a timer");
        assert_eq!(envelope.runtime_policy.runtime_kind, "static-html");
        assert_eq!(envelope.file_system_policy.root, "generated/static");
        assert_eq!(
            envelope.file_system_policy.allowed_files,
            vec!["index.html", "style.css", "app.js"]
        );
        assert!(envelope
            .harness_policy
            .file_system_rules
            .contains(&"Use localStorage only for small local preferences.".to_string()));
        assert!(envelope
            .acceptance_criteria
            .iter()
            .any(|criterion| criterion.contains("sofvary.harness.static-html@0.1.0")));
    }

    #[test]
    fn update_envelope_includes_existing_allowed_file_context() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path(), "static-html".to_string());
        fs::write(
            manifest.paths.generated_static.join("index.html"),
            "<main>Original timer</main>",
        )
        .expect("write index");
        fs::write(
            manifest.paths.generated_static.join("notes.txt"),
            "not part of the output contract",
        )
        .expect("write notes");
        let runtime =
            parse_runtime_pack_manifest(&builtin_runtime_manifest("sofvary.runtime.static-html"))
                .expect("runtime");
        let harness =
            parse_harness_pack_manifest(&builtin_harness_manifest("sofvary.harness.static-html"))
                .expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_envelope("Add a reset button", &manifest, &runtime, &harness)
            .expect("envelope");

        assert_eq!(envelope.current_app_state.mode, "update");
        assert!(envelope
            .current_app_state
            .existing_files
            .contains(&"index.html".to_string()));
        assert!(envelope
            .current_app_state
            .existing_files
            .contains(&"notes.txt".to_string()));
        assert_eq!(envelope.current_app_state.file_context.len(), 1);
        assert_eq!(
            envelope.current_app_state.file_context[0].relative_path,
            "index.html"
        );
        assert!(envelope.current_app_state.file_context[0]
            .contents
            .contains("Original timer"));
        assert!(envelope
            .acceptance_criteria
            .iter()
            .any(|criterion| criterion.contains("currentAppState.fileContext")));
    }

    #[test]
    fn creates_react_vite_prompt_envelope_from_pack_resources() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path(), "react-vite".to_string());
        let runtime =
            parse_runtime_pack_manifest(&builtin_runtime_manifest("sofvary.runtime.react-vite"))
                .expect("runtime");
        let harness =
            parse_harness_pack_manifest(&builtin_harness_manifest("sofvary.harness.react-vite"))
                .expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_envelope("Build a task board", &manifest, &runtime, &harness)
            .expect("envelope");

        assert_eq!(envelope.runtime_policy.runtime_kind, "react-vite");
        assert_eq!(envelope.file_system_policy.root, "generated/react");
        assert_eq!(envelope.output_contract.format, "react-vite-project");
        assert!(envelope
            .file_system_policy
            .allowed_files
            .contains(&"src/components/TaskBoard.tsx".to_string()));
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"nextjs-runtime".to_string()));
    }

    #[test]
    fn prompt_envelope_accepts_typescript_golden_fixture_shape() {
        let envelope: PromptEnvelope = serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/static-html-prompt-envelope.golden.json"
        ))
        .expect("golden prompt envelope");

        assert_eq!(envelope.envelope_id, "penv_golden");
        assert_eq!(envelope.runtime_policy.runtime_kind, "static-html");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.static-html"
        );
    }

    fn test_manifest(root: &Path, mode: RuntimeKind) -> AppBoxManifest {
        let generated = root.join("generated");
        let generated_static = generated.join("static");
        fs::create_dir_all(&generated_static).expect("static root");
        fs::create_dir_all(generated.join("react")).expect("react root");

        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: WorkspacePaths {
                root: root.to_path_buf(),
                generated,
                generated_static,
                runtime: root.join("runtime"),
                snapshots: root.join("snapshots"),
            },
            constraints: WorkspaceConstraints {
                boundary: root.to_path_buf(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        }
    }

    fn builtin_runtime_manifest(id: &str) -> String {
        read_pack_resource_text(PackKind::Runtime, id, "0.1.0", "manifest.json")
            .expect("runtime manifest")
    }

    fn builtin_harness_manifest(id: &str) -> String {
        read_pack_resource_text(PackKind::Harness, id, "0.1.0", "manifest.json")
            .expect("harness manifest")
    }
}
