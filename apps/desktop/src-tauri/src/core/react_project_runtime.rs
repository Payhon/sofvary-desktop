use crate::core::harness_engine::PromptEnvelope;
use crate::core::pack_types::RuntimePackManifest;
use crate::core::policy_types::PolicyApprovalSet;
use crate::core::react_vite_runtime::{
    ReactViteRuntime, ReactViteRuntimeError, ReactViteRuntimeServer,
};
use crate::core::workspace_types::{AppBoxManifest, RuntimeMode};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReactProjectRuntimeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("react-vite runtime error: {0}")]
    ReactVite(#[from] ReactViteRuntimeError),
    #[error("request attempted to escape generated project root")]
    PathEscape,
    #[error("invalid prompt envelope: {0}")]
    InvalidPromptEnvelope(String),
}

pub type ReactProjectRuntimeServer = ReactViteRuntimeServer;

#[derive(Debug, Clone)]
pub struct ReactProjectRuntimeSpec {
    pub runtime_kind: String,
    pub generated_root: String,
    pub entrypoint: String,
    pub output_format: String,
    pub label: String,
}

pub struct ReactProjectRuntime {
    spec: ReactProjectRuntimeSpec,
}

impl ReactProjectRuntime {
    pub fn new(spec: ReactProjectRuntimeSpec) -> Self {
        Self { spec }
    }

    pub fn for_runtime_pack(runtime_pack: &RuntimePackManifest) -> Self {
        Self::new(ReactProjectRuntimeSpec {
            runtime_kind: runtime_pack.runtime.kind.clone(),
            generated_root: runtime_pack.runtime.generated_root.clone(),
            entrypoint: runtime_pack.runtime.entrypoint.clone(),
            output_format: String::new(),
            label: runtime_pack.name.clone(),
        })
    }

    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), ReactProjectRuntimeError> {
        validate_prompt_envelope(&self.spec, envelope)
    }

    #[allow(dead_code)]
    pub fn start_workspace_with_envelope(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
    ) -> Result<ReactProjectRuntimeServer, ReactProjectRuntimeError> {
        self.start_workspace_with_envelope_with_policy(
            manifest,
            envelope,
            runtime_pack,
            mode,
            &PolicyApprovalSet::default(),
        )
    }

    pub fn start_workspace_with_envelope_with_policy(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
        approvals: &PolicyApprovalSet,
    ) -> Result<ReactProjectRuntimeServer, ReactProjectRuntimeError> {
        self.validate_prompt_envelope(envelope)?;
        ensure_exact_workspace_project_files(
            manifest,
            &self.spec,
            &envelope.output_contract.files,
        )?;
        Ok(
            ReactViteRuntime::new().start_verified_react_project_with_policy(
                manifest,
                runtime_pack,
                mode,
                approvals,
            )?,
        )
    }
}

pub fn validate_prompt_envelope(
    spec: &ReactProjectRuntimeSpec,
    envelope: &PromptEnvelope,
) -> Result<(), ReactProjectRuntimeError> {
    if envelope.box_runtime_context.runtime_kind != spec.runtime_kind {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "runtimeKind must be {}, found {}",
            spec.runtime_kind, envelope.box_runtime_context.runtime_kind
        )));
    }
    if envelope.box_runtime_context.generated_root != spec.generated_root {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "generatedRoot must be {}, found {}",
            spec.generated_root, envelope.box_runtime_context.generated_root
        )));
    }
    if envelope.box_runtime_context.entrypoint != spec.entrypoint {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "entrypoint must be {}, found {}",
            spec.entrypoint, envelope.box_runtime_context.entrypoint
        )));
    }
    if envelope.file_system_policy.root != spec.generated_root {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "fileSystemPolicy.root must be {}, found {}",
            spec.generated_root, envelope.file_system_policy.root
        )));
    }
    if envelope.runtime_policy.runtime_kind != spec.runtime_kind {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "runtimePolicy.runtimeKind must be {}, found {}",
            spec.runtime_kind, envelope.runtime_policy.runtime_kind
        )));
    }
    if envelope.runtime_policy.allowed_entrypoints.len() != 1
        || envelope.runtime_policy.allowed_entrypoints[0] != spec.entrypoint
    {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "runtimePolicy.allowedEntrypoints must contain exactly {}",
            spec.entrypoint
        )));
    }
    if envelope.runtime_policy.allowed_server_bind != "127.0.0.1"
        || envelope.box_runtime_context.bind != "127.0.0.1"
    {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "{} envelope must bind to 127.0.0.1",
            spec.runtime_kind
        )));
    }
    if envelope.runtime_policy.network != "local-only"
        || envelope.box_runtime_context.network != "local-only"
    {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "{} envelope must stay local-only",
            spec.runtime_kind
        )));
    }
    if envelope.command_policy.allow_shell
        || envelope.command_policy.allow_package_install
        || envelope.command_policy.allow_global_install
    {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "{} envelope must not allow agent shell commands or package installs",
            spec.runtime_kind
        )));
    }
    if envelope.file_system_policy.allow_external_files
        || envelope.file_system_policy.allow_path_traversal
    {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "{} envelope must not allow external files or path traversal",
            spec.runtime_kind
        )));
    }
    if envelope.output_contract.shell_ui_included {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "{} output contract must exclude Sofvary shell UI",
            spec.runtime_kind
        )));
    }
    if !spec.output_format.is_empty() && envelope.output_contract.format != spec.output_format {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "outputContract.format must be {}, found {}",
            spec.output_format, envelope.output_contract.format
        )));
    }
    ensure_allowed_files_match_output_contract(envelope, spec)?;

    Ok(())
}

pub fn ensure_exact_workspace_project_files(
    manifest: &AppBoxManifest,
    spec: &ReactProjectRuntimeSpec,
    allowed_files: &[String],
) -> Result<(), ReactProjectRuntimeError> {
    let generated_root = prepare_generated_root(manifest)?;
    let expected: HashSet<String> = allowed_files.iter().cloned().collect();
    let mut actual = HashSet::new();
    collect_relative_files(&generated_root, &generated_root, &mut actual)?;
    actual.retain(|path| !is_react_project_runtime_artifact(path));

    if actual != expected {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "generated must contain exactly the {} output contract files",
            spec.label
        )));
    }

    Ok(())
}

fn is_react_project_runtime_artifact(path: &str) -> bool {
    path == "react/pnpm-lock.yaml"
        || path == "react/package-lock.json"
        || path == "react/yarn.lock"
        || path.starts_with("react/node_modules/")
        || path.starts_with("react/.vite/")
        || path.starts_with("react/dist/")
}

fn ensure_allowed_files_match_output_contract(
    envelope: &PromptEnvelope,
    spec: &ReactProjectRuntimeSpec,
) -> Result<(), ReactProjectRuntimeError> {
    if envelope.file_system_policy.allowed_files.is_empty() {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "{} output contract must declare at least one allowed file",
            spec.label
        )));
    }

    for field in [
        &envelope.file_system_policy.allowed_files,
        &envelope.output_contract.files,
    ] {
        for file in field {
            validate_relative_contract_file(file)?;
        }
    }

    let expected: HashSet<&str> = envelope
        .file_system_policy
        .allowed_files
        .iter()
        .map(String::as_str)
        .collect();
    let actual: HashSet<&str> = envelope
        .output_contract
        .files
        .iter()
        .map(String::as_str)
        .collect();
    if expected == actual && expected.len() == envelope.output_contract.files.len() {
        return Ok(());
    }

    Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
        "fileSystemPolicy.allowedFiles and outputContract.files must match for {}",
        spec.label
    )))
}

fn collect_relative_files<'a>(
    root: &'a Path,
    current: &'a Path,
    files: &mut HashSet<String>,
) -> Result<(), ReactProjectRuntimeError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_relative_files(root, &path, files)?;
        } else if entry.file_type()?.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| ReactProjectRuntimeError::PathEscape)?;
            files.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn validate_relative_contract_file(path: &str) -> Result<(), ReactProjectRuntimeError> {
    if path.trim().is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "..")
        || Path::new(path).components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "output contract file path must stay relative inside generated root: {path}"
        )));
    }
    Ok(())
}

fn prepare_generated_root(manifest: &AppBoxManifest) -> Result<PathBuf, ReactProjectRuntimeError> {
    ensure_same_path(
        &manifest.paths.generated,
        &manifest.paths.root.join("generated"),
    )?;
    ensure_same_path(&manifest.constraints.boundary, &manifest.paths.root)?;

    fs::create_dir_all(&manifest.paths.generated)?;
    let boundary = manifest.paths.root.canonicalize()?;
    let generated_root = manifest.paths.generated.canonicalize()?;
    if generated_root.starts_with(boundary) {
        Ok(generated_root)
    } else {
        Err(ReactProjectRuntimeError::PathEscape)
    }
}

fn ensure_same_path(actual: &Path, expected: &Path) -> Result<(), ReactProjectRuntimeError> {
    let normalized_actual = normalize_path_lexically(actual);
    let normalized_expected = normalize_path_lexically(expected);
    if normalized_actual == normalized_expected {
        Ok(())
    } else {
        Err(ReactProjectRuntimeError::PathEscape)
    }
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::workspace_types::{WorkspaceConstraints, WorkspacePaths, WorkspacePreview};
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn exact_workspace_project_files_ignore_dependency_install_artifacts() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("app_test");
        let generated = root.join("generated");
        fs::create_dir_all(generated.join("react/node_modules/.pnpm/dependency")).unwrap();
        fs::create_dir_all(generated.join("react/src")).unwrap();
        fs::create_dir_all(generated.join("ai")).unwrap();
        fs::write(generated.join("ai/agents.json"), "{}").unwrap();
        fs::write(
            generated.join("react/src/App.tsx"),
            "export function App() { return null; }",
        )
        .unwrap();
        fs::write(
            generated.join("react/pnpm-lock.yaml"),
            "lockfileVersion: '9.0'",
        )
        .unwrap();
        fs::write(
            generated.join("react/node_modules/.pnpm-workspace-state.json"),
            "{}",
        )
        .unwrap();
        fs::write(
            generated.join("react/node_modules/.pnpm/dependency/package.json"),
            "{}",
        )
        .unwrap();

        let manifest = test_manifest(root);
        let spec = ReactProjectRuntimeSpec {
            runtime_kind: "ai-agent-app".to_string(),
            generated_root: "generated".to_string(),
            entrypoint: "react/src/main.tsx".to_string(),
            output_format: "ai-agent-app-project".to_string(),
            label: "AI Agent App Runtime".to_string(),
        };
        let allowed_files = vec![
            "ai/agents.json".to_string(),
            "react/src/App.tsx".to_string(),
        ];

        ensure_exact_workspace_project_files(&manifest, &spec, &allowed_files).unwrap();
    }

    #[test]
    fn exact_workspace_project_files_reject_extra_source_files() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("app_test");
        let generated = root.join("generated");
        fs::create_dir_all(generated.join("react/src")).unwrap();
        fs::write(
            generated.join("react/src/App.tsx"),
            "export function App() { return null; }",
        )
        .unwrap();
        fs::write(
            generated.join("react/src/Extra.tsx"),
            "export const extra = true;",
        )
        .unwrap();

        let manifest = test_manifest(root);
        let spec = ReactProjectRuntimeSpec {
            runtime_kind: "ai-agent-app".to_string(),
            generated_root: "generated".to_string(),
            entrypoint: "react/src/main.tsx".to_string(),
            output_format: "ai-agent-app-project".to_string(),
            label: "AI Agent App Runtime".to_string(),
        };
        let allowed_files = vec!["react/src/App.tsx".to_string()];

        assert!(matches!(
            ensure_exact_workspace_project_files(&manifest, &spec, &allowed_files),
            Err(ReactProjectRuntimeError::InvalidPromptEnvelope(_))
        ));
    }

    fn test_manifest(root: PathBuf) -> AppBoxManifest {
        let now = Utc::now().to_rfc3339();
        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "ai-agent-app".to_string(),
            created_at: now.clone(),
            updated_at: now,
            stack: vec!["React".to_string()],
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
}
