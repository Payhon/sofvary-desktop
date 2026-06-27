use crate::core::harness_engine::PromptEnvelope;
use crate::core::workspace_types::AppBoxManifest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffFileState {
    pub relative_path: String,
    pub exists: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffScanResult {
    pub complete: bool,
    pub generated_root: String,
    pub files: Vec<HandoffFileState>,
    pub missing_files: Vec<String>,
    pub status_entries: Vec<String>,
    pub validate_requested: bool,
    pub preview_requested: bool,
}

pub fn scan_handoff_workspace(
    manifest: &AppBoxManifest,
    envelope: &PromptEnvelope,
) -> std::io::Result<HandoffScanResult> {
    let generated_root = envelope.box_runtime_context.generated_root.clone();
    let base = manifest.paths.root.join(&generated_root);
    let mut files = Vec::new();
    let mut missing_files = Vec::new();

    for relative_path in &envelope.output_contract.files {
        let full_path = safe_join(&base, relative_path)?;
        let metadata = fs::metadata(&full_path).ok();
        let exists = metadata.as_ref().is_some_and(|item| item.is_file());
        if !exists {
            missing_files.push(relative_path.clone());
        }
        files.push(HandoffFileState {
            relative_path: relative_path.clone(),
            exists,
            size_bytes: metadata.map(|item| item.len()).unwrap_or(0),
        });
    }

    Ok(HandoffScanResult {
        complete: missing_files.is_empty(),
        generated_root,
        files,
        missing_files,
        status_entries: read_agent_status_entries(&manifest.paths.root.join(".sofvary"))?,
        validate_requested: manifest
            .paths
            .root
            .join(".sofvary")
            .join("requests")
            .join("validate.json")
            .exists(),
        preview_requested: manifest
            .paths
            .root
            .join(".sofvary")
            .join("requests")
            .join("preview.json")
            .exists(),
    })
}

pub fn wait_for_handoff_assets(
    manifest: &AppBoxManifest,
    envelope: &PromptEnvelope,
    timeout: Duration,
    interval: Duration,
) -> std::io::Result<HandoffScanResult> {
    let started = Instant::now();
    loop {
        let scan = scan_handoff_workspace(manifest, envelope)?;
        if scan.complete || started.elapsed() >= timeout {
            return Ok(scan);
        }
        std::thread::sleep(interval);
    }
}

fn read_agent_status_entries(sofvary_dir: &Path) -> std::io::Result<Vec<String>> {
    let path = sofvary_dir.join("agent-status.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(trimmed)
                .ok()
                .and_then(|value| {
                    let phase = value
                        .get("phase")
                        .and_then(Value::as_str)
                        .unwrap_or("status");
                    let detail = value.get("detail").and_then(Value::as_str).unwrap_or("");
                    if detail.is_empty() {
                        Some(phase.to_string())
                    } else {
                        Some(format!("{phase}: {detail}"))
                    }
                })
                .or_else(|| Some(trimmed.to_string()))
        })
        .collect())
}

fn safe_join(base: &Path, relative_path: &str) -> std::io::Result<PathBuf> {
    let path = Path::new(relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("handoff file path escapes generated root: {relative_path}"),
        ));
    }
    Ok(base.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness_engine::{
        BoxRuntimeContext, CommandPolicy, CurrentAppState, FileSystemPolicy, HarnessPolicy,
        OutputContract, PackReference, RuntimePolicy,
    };
    use crate::core::workspace_types::{
        AppBoxManifest, WorkspaceConstraints, WorkspacePaths, WorkspacePreview,
    };

    #[test]
    fn detects_missing_and_complete_handoff_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = manifest_for_root(temp.path().to_path_buf());
        let envelope = envelope();

        let initial = scan_handoff_workspace(&manifest, &envelope).expect("scan");
        assert!(!initial.complete);
        assert_eq!(initial.missing_files, ["index.html", "style.css"]);

        fs::create_dir_all(temp.path().join("generated/static")).expect("generated root");
        fs::write(
            temp.path().join("generated/static/index.html"),
            "<main></main>",
        )
        .expect("html");
        fs::write(temp.path().join("generated/static/style.css"), "body{}").expect("css");
        fs::create_dir_all(temp.path().join(".sofvary/requests")).expect("requests");
        fs::write(temp.path().join(".sofvary/requests/preview.json"), "{}").expect("preview");
        fs::write(
            temp.path().join(".sofvary/agent-status.jsonl"),
            "{\"phase\":\"generating\",\"detail\":\"files ready\"}\n",
        )
        .expect("status");

        let complete = scan_handoff_workspace(&manifest, &envelope).expect("scan");
        assert!(complete.complete);
        assert!(complete.preview_requested);
        assert_eq!(complete.status_entries, ["generating: files ready"]);
    }

    #[test]
    fn rejects_escaping_allowed_file_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = manifest_for_root(temp.path().to_path_buf());
        let mut envelope = envelope();
        envelope.output_contract.files = vec!["../outside.txt".to_string()];

        assert!(scan_handoff_workspace(&manifest, &envelope).is_err());
    }

    fn manifest_for_root(root: PathBuf) -> AppBoxManifest {
        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "static-html".to_string(),
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
            user_intent: "test".to_string(),
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
                allowed_files: vec!["index.html".to_string(), "style.css".to_string()],
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
                files: vec!["index.html".to_string(), "style.css".to_string()],
                shell_ui_included: false,
            },
            acceptance_criteria: Vec::new(),
        }
    }
}
