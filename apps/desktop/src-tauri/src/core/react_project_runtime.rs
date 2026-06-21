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

#[derive(Debug, Clone, Copy)]
pub struct ReactProjectRuntimeSpec {
    pub runtime_kind: &'static str,
    pub generated_root: &'static str,
    pub entrypoint: &'static str,
    pub output_format: &'static str,
    pub allowed_files: &'static [&'static str],
    pub label: &'static str,
}

pub struct ReactProjectRuntime {
    spec: ReactProjectRuntimeSpec,
}

impl ReactProjectRuntime {
    pub fn new(spec: ReactProjectRuntimeSpec) -> Self {
        Self { spec }
    }

    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), ReactProjectRuntimeError> {
        validate_prompt_envelope(self.spec, envelope)
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
        ensure_exact_workspace_project_files(manifest, self.spec)?;
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
    spec: ReactProjectRuntimeSpec,
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
    if envelope.output_contract.format != spec.output_format {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "outputContract.format must be {}, found {}",
            spec.output_format, envelope.output_contract.format
        )));
    }
    ensure_exact_files(
        "fileSystemPolicy.allowedFiles",
        &envelope.file_system_policy.allowed_files,
        spec,
    )?;
    ensure_exact_files(
        "outputContract.files",
        &envelope.output_contract.files,
        spec,
    )?;

    Ok(())
}

pub fn ensure_exact_workspace_project_files(
    manifest: &AppBoxManifest,
    spec: ReactProjectRuntimeSpec,
) -> Result<(), ReactProjectRuntimeError> {
    let generated_root = prepare_generated_root(manifest)?;
    let expected: HashSet<String> = spec
        .allowed_files
        .iter()
        .map(|value| value.to_string())
        .collect();
    let mut actual = HashSet::new();
    collect_relative_files(&generated_root, &generated_root, &mut actual)?;

    if actual != expected {
        return Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "generated must contain exactly the {} output contract files",
            spec.label
        )));
    }

    Ok(())
}

fn ensure_exact_files(
    field: &str,
    files: &[String],
    spec: ReactProjectRuntimeSpec,
) -> Result<(), ReactProjectRuntimeError> {
    let expected: HashSet<&str> = spec.allowed_files.iter().copied().collect();
    let actual: HashSet<&str> = files.iter().map(String::as_str).collect();
    if expected == actual && files.len() == spec.allowed_files.len() {
        Ok(())
    } else {
        Err(ReactProjectRuntimeError::InvalidPromptEnvelope(format!(
            "{field} must contain exactly the {} project file set",
            spec.label
        )))
    }
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
