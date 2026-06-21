use crate::core::harness_engine::{PromptEnvelope, FILE_PROCESSOR_ALLOWED_FILES};
use crate::core::pack_types::RuntimePackManifest;
use crate::core::policy_types::PolicyApprovalSet;
use crate::core::react_project_runtime::{
    ReactProjectRuntime, ReactProjectRuntimeError, ReactProjectRuntimeServer,
    ReactProjectRuntimeSpec,
};
use crate::core::workspace_types::{AppBoxManifest, RuntimeMode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub type FileProcessorRuntimeServer = ReactProjectRuntimeServer;

#[derive(Debug, Error)]
pub enum FileProcessorRuntimeError {
    #[error("react project runtime error: {0}")]
    ReactProject(#[from] ReactProjectRuntimeError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("path was not explicitly selected: {0}")]
    UnselectedPath(PathBuf),
    #[error("write confirmation requires a prior dry-run plan")]
    MissingDryRun,
    #[error("file processor path escapes its workspace boundary: {0}")]
    PathEscape(PathBuf),
    #[error("invalid file processor plan: {0}")]
    InvalidPlan(String),
}

#[derive(Default)]
pub struct FileProcessorRuntime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileProcessorOperationLogEntry {
    pub event: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileProcessorSelectedFileMetadata {
    pub name: String,
    pub extension: String,
    pub size_bytes: Option<u64>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileProcessorDryRunOperation {
    pub from: String,
    pub to: String,
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct FileProcessorPermissionSession {
    selected_paths: HashSet<PathBuf>,
}

impl FileProcessorPermissionSession {
    pub fn from_selected_paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            selected_paths: paths
                .into_iter()
                .map(|path| normalize_path_lexically(&path))
                .collect(),
        }
    }

    pub fn ensure_selected_path(&self, path: &Path) -> Result<(), FileProcessorRuntimeError> {
        let candidate = normalize_path_lexically(path);
        if self
            .selected_paths
            .iter()
            .any(|selected| candidate == *selected || candidate.starts_with(selected))
        {
            Ok(())
        } else {
            Err(FileProcessorRuntimeError::UnselectedPath(candidate))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileProcessorDryRunGate {
    has_dry_run: bool,
}

impl FileProcessorDryRunGate {
    pub fn record_dry_run(&mut self) {
        self.has_dry_run = true;
    }

    pub fn confirm_write_plan(&self) -> Result<(), FileProcessorRuntimeError> {
        if self.has_dry_run {
            Ok(())
        } else {
            Err(FileProcessorRuntimeError::MissingDryRun)
        }
    }
}

impl FileProcessorRuntime {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), FileProcessorRuntimeError> {
        Ok(ReactProjectRuntime::new(file_processor_spec()).validate_prompt_envelope(envelope)?)
    }

    #[allow(dead_code)]
    pub fn start_workspace_with_envelope(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
    ) -> Result<FileProcessorRuntimeServer, FileProcessorRuntimeError> {
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
    ) -> Result<FileProcessorRuntimeServer, FileProcessorRuntimeError> {
        self.validate_prompt_envelope(envelope)?;
        append_operation_log(
            manifest,
            &FileProcessorOperationLogEntry {
                event: "runtime-started".to_string(),
                detail: "Phase 14 MVP starts read-only and records dry-run plans only.".to_string(),
            },
        )?;
        Ok(ReactProjectRuntime::new(file_processor_spec())
            .start_workspace_with_envelope_with_policy(
                manifest,
                envelope,
                runtime_pack,
                mode,
                approvals,
            )?)
    }
}

pub fn operation_log_path(manifest: &AppBoxManifest) -> Result<PathBuf, FileProcessorRuntimeError> {
    let expected_runtime = normalize_path_lexically(&manifest.paths.root.join("runtime"));
    let runtime_root = normalize_path_lexically(&manifest.paths.runtime);
    if runtime_root != expected_runtime {
        return Err(FileProcessorRuntimeError::PathEscape(runtime_root));
    }

    fs::create_dir_all(&runtime_root)?;
    let boundary = manifest.paths.root.canonicalize()?;
    let runtime_root = runtime_root.canonicalize()?;
    if !runtime_root.starts_with(&boundary) {
        return Err(FileProcessorRuntimeError::PathEscape(runtime_root));
    }

    let logs_root = runtime_root.join("logs");
    fs::create_dir_all(&logs_root)?;
    let logs_root = logs_root.canonicalize()?;
    if !logs_root.starts_with(&boundary) {
        return Err(FileProcessorRuntimeError::PathEscape(logs_root));
    }

    Ok(logs_root.join("file-processor-operations.jsonl"))
}

pub fn append_operation_log(
    manifest: &AppBoxManifest,
    entry: &FileProcessorOperationLogEntry,
) -> Result<(), FileProcessorRuntimeError> {
    let path = operation_log_path(manifest)?;
    let mut line = serde_json::to_string(entry).expect("operation log serializes");
    line.push('\n');
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(line.as_bytes())?;
    Ok(())
}

pub fn record_selected_files(
    manifest: &AppBoxManifest,
    selected_files: &[FileProcessorSelectedFileMetadata],
) -> Result<(), FileProcessorRuntimeError> {
    validate_selected_files(selected_files)?;
    append_operation_log(
        manifest,
        &FileProcessorOperationLogEntry {
            event: "files-selected".to_string(),
            detail: serde_json::to_string(selected_files)
                .expect("selected file metadata serializes"),
        },
    )
}

pub fn confirm_dry_run_plan(
    manifest: &AppBoxManifest,
    selected_files: &[FileProcessorSelectedFileMetadata],
    operations: &[FileProcessorDryRunOperation],
) -> Result<(), FileProcessorRuntimeError> {
    validate_selected_files(selected_files)?;
    if operations.is_empty() {
        return Err(FileProcessorRuntimeError::InvalidPlan(
            "dry-run plan must contain at least one operation".to_string(),
        ));
    }

    let selected_names = selected_files
        .iter()
        .map(|file| file.name.as_str())
        .collect::<HashSet<_>>();
    let selected_paths = selected_files
        .iter()
        .filter_map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let session = FileProcessorPermissionSession::from_selected_paths(selected_paths);

    for operation in operations {
        if operation.from.trim().is_empty() || operation.to.trim().is_empty() {
            return Err(FileProcessorRuntimeError::InvalidPlan(
                "dry-run operation source and target names are required".to_string(),
            ));
        }
        if !selected_names.contains(operation.from.as_str()) {
            return Err(FileProcessorRuntimeError::InvalidPlan(format!(
                "dry-run source '{}' was not selected by the user",
                operation.from
            )));
        }
        if let Some(source_path) = &operation.source_path {
            session.ensure_selected_path(source_path)?;
        }
    }

    let mut gate = FileProcessorDryRunGate::default();
    gate.record_dry_run();
    gate.confirm_write_plan()?;

    append_operation_log(
        manifest,
        &FileProcessorOperationLogEntry {
            event: "dry-run-plan-confirmed".to_string(),
            detail: serde_json::to_string(operations).expect("dry-run operations serialize"),
        },
    )
}

fn validate_selected_files(
    selected_files: &[FileProcessorSelectedFileMetadata],
) -> Result<(), FileProcessorRuntimeError> {
    if selected_files.is_empty() {
        return Err(FileProcessorRuntimeError::InvalidPlan(
            "at least one explicitly selected file is required".to_string(),
        ));
    }

    let mut names = HashSet::new();
    for file in selected_files {
        if file.name.trim().is_empty() {
            return Err(FileProcessorRuntimeError::InvalidPlan(
                "selected file name is required".to_string(),
            ));
        }
        if !names.insert(file.name.as_str()) {
            return Err(FileProcessorRuntimeError::InvalidPlan(format!(
                "selected file '{}' is duplicated",
                file.name
            )));
        }
    }

    Ok(())
}

fn file_processor_spec() -> ReactProjectRuntimeSpec {
    ReactProjectRuntimeSpec {
        runtime_kind: "file-processor",
        generated_root: "generated",
        entrypoint: "react/src/main.tsx",
        output_format: "file-processor-project",
        allowed_files: &FILE_PROCESSOR_ALLOWED_FILES,
        label: "File Processor",
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

    #[test]
    fn file_processor_session_rejects_unselected_paths() {
        let session =
            FileProcessorPermissionSession::from_selected_paths([PathBuf::from("/tmp/selected")]);

        session
            .ensure_selected_path(Path::new("/tmp/selected/report.txt"))
            .expect("selected child");
        let result = session.ensure_selected_path(Path::new("/tmp/other/report.txt"));

        assert!(matches!(
            result,
            Err(FileProcessorRuntimeError::UnselectedPath(_))
        ));
    }

    #[test]
    fn file_processor_write_requires_dry_run() {
        let mut gate = FileProcessorDryRunGate::default();
        assert!(matches!(
            gate.confirm_write_plan(),
            Err(FileProcessorRuntimeError::MissingDryRun)
        ));
        gate.record_dry_run();
        gate.confirm_write_plan().expect("dry-run recorded");
    }

    #[test]
    fn file_processor_log_rejects_tampered_runtime_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join("runtime")).expect("runtime");
        let outside = temp.path().join("outside-runtime");
        let manifest = test_manifest(&root).with_runtime(outside.clone());

        let result = append_operation_log(
            &manifest,
            &FileProcessorOperationLogEntry {
                event: "test".to_string(),
                detail: "test".to_string(),
            },
        );

        assert!(matches!(
            result,
            Err(FileProcessorRuntimeError::PathEscape(_))
        ));
        assert!(!outside
            .join("logs/file-processor-operations.jsonl")
            .exists());
    }

    #[test]
    fn file_processor_confirm_records_plan_without_mutating_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join("runtime")).expect("runtime");
        let manifest = test_manifest(&root);
        let selected_files = vec![FileProcessorSelectedFileMetadata {
            name: "report.txt".to_string(),
            extension: ".txt".to_string(),
            size_bytes: Some(128),
            path: None,
        }];
        let operations = vec![FileProcessorDryRunOperation {
            from: "report.txt".to_string(),
            to: "report-renamed.txt".to_string(),
            source_path: None,
        }];

        confirm_dry_run_plan(&manifest, &selected_files, &operations).expect("confirm plan");

        let log = fs::read_to_string(operation_log_path(&manifest).expect("log path"))
            .expect("operation log");
        assert!(log.contains("dry-run-plan-confirmed"));
        assert!(log.contains("report-renamed.txt"));
        assert!(!root.join("report-renamed.txt").exists());
    }

    #[test]
    fn file_processor_confirm_rejects_unselected_plan_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join("runtime")).expect("runtime");
        let manifest = test_manifest(&root);
        let selected_files = vec![FileProcessorSelectedFileMetadata {
            name: "report.txt".to_string(),
            extension: ".txt".to_string(),
            size_bytes: None,
            path: None,
        }];
        let operations = vec![FileProcessorDryRunOperation {
            from: "secret.txt".to_string(),
            to: "secret-renamed.txt".to_string(),
            source_path: None,
        }];

        let result = confirm_dry_run_plan(&manifest, &selected_files, &operations);

        assert!(matches!(
            result,
            Err(FileProcessorRuntimeError::InvalidPlan(_))
        ));
    }

    trait TestManifestExt {
        fn with_runtime(self, runtime: PathBuf) -> Self;
    }

    impl TestManifestExt for AppBoxManifest {
        fn with_runtime(mut self, runtime: PathBuf) -> Self {
            self.paths.runtime = runtime;
            self
        }
    }

    fn test_manifest(root: &Path) -> AppBoxManifest {
        let generated = root.join("generated");
        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: crate::core::workspace_types::RuntimeKind::FileProcessor,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: Vec::new(),
            paths: crate::core::workspace_types::WorkspacePaths {
                root: root.to_path_buf(),
                generated: generated.clone(),
                generated_static: generated.join("static"),
                runtime: root.join("runtime"),
                snapshots: root.join("snapshots"),
            },
            constraints: crate::core::workspace_types::WorkspaceConstraints {
                boundary: root.to_path_buf(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: crate::core::workspace_types::WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        }
    }
}
