use crate::core::pack_manager::{
    AI_AGENT_APP_HARNESS_PACK_ID, AI_AGENT_APP_PACK_VERSION, AI_AGENT_APP_RUNTIME_PACK_ID,
    CANVAS2D_HARNESS_PACK_ID, CANVAS2D_PACK_VERSION, CANVAS2D_RUNTIME_PACK_ID,
    DATA_TABLE_HARNESS_PACK_ID, DATA_TABLE_PACK_VERSION, DATA_TABLE_RUNTIME_PACK_ID,
    DESKTOP_WIDGET_HARNESS_PACK_ID, DESKTOP_WIDGET_PACK_VERSION, DESKTOP_WIDGET_RUNTIME_PACK_ID,
    FILE_PROCESSOR_HARNESS_PACK_ID, FILE_PROCESSOR_PACK_VERSION, FILE_PROCESSOR_RUNTIME_PACK_ID,
    MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID, MARKDOWN_KNOWLEDGE_PACK_VERSION,
    MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID, REACT_SQLITE_HARNESS_PACK_ID, REACT_SQLITE_PACK_VERSION,
    REACT_SQLITE_RUNTIME_PACK_ID, REACT_VITE_HARNESS_PACK_ID, REACT_VITE_PACK_VERSION,
    REACT_VITE_RUNTIME_PACK_ID, STATIC_HTML_HARNESS_PACK_ID, STATIC_HTML_PACK_VERSION,
    STATIC_HTML_RUNTIME_PACK_ID,
};
use crate::core::policy_engine::{PolicyEngine, PolicyError};
use crate::core::policy_types::{
    PolicyApprovalSet, PolicyFileWriteRequest, PolicyWorkspaceLockfileUpdateRequest,
};
use crate::core::software_naming::clean_display_name;
use crate::core::workspace_types::{
    AppBoxManifest, RuntimeKind, SnapshotSummary, SofvaryLockfile, WorkspaceConstraints,
    WorkspacePaths, WorkspacePreview, WorkspaceSummary,
};
use crate::platform::{current_adapter, PlatformAdapter, PlatformError};
use chrono::Utc;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("workspace path escapes its boundary: {0}")]
    PathEscape(PathBuf),
    #[error("workspace manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("workspace not found: {0}")]
    NotFound(String),
    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),
}

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedStaticFile {
    pub relative_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedReactFile {
    pub relative_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedReactSqliteFile {
    pub relative_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCanvas2dFile {
    pub relative_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedProjectFile {
    pub relative_path: String,
    pub contents: String,
}

const REACT_SQLITE_MANAGED_PACKAGE_DEPENDENCIES: &[(&str, &str)] = &[
    ("@types/cors", "2.8.19"),
    ("@types/express", "5.0.6"),
    ("@vitejs/plugin-react", "5.2.0"),
    ("@types/node", "24.12.4"),
    ("@types/react", "19.2.15"),
    ("@types/react-dom", "19.2.3"),
    ("cors", "2.8.6"),
    ("express", "5.2.1"),
    ("sql.js", "1.14.1"),
    ("tsx", "4.22.3"),
    ("typescript", "5.9.3"),
    ("vite", "7.3.3"),
    ("react", "19.2.6"),
    ("react-dom", "19.2.6"),
];

#[derive(Clone, Copy, Default)]
pub struct WorkspaceManager;

impl WorkspaceManager {
    pub fn new() -> Self {
        Self
    }

    pub fn create_workspace(&self, name: String) -> WorkspaceResult<AppBoxManifest> {
        self.create_workspace_for_runtime(name, RuntimeKind::StaticHtml)
    }

    pub fn create_workspace_for_runtime(
        &self,
        name: String,
        runtime_kind: RuntimeKind,
    ) -> WorkspaceResult<AppBoxManifest> {
        self.create_workspace_for_runtime_with_adapter(
            name,
            runtime_kind,
            current_adapter().as_ref(),
        )
    }

    #[allow(dead_code)]
    pub fn create_workspace_with_adapter(
        &self,
        name: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<AppBoxManifest> {
        self.create_workspace_for_runtime_with_adapter(name, RuntimeKind::StaticHtml, adapter)
    }

    pub fn create_workspace_for_runtime_with_adapter(
        &self,
        name: String,
        runtime_kind: RuntimeKind,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<AppBoxManifest> {
        let app_id = format!("app_{}", Uuid::new_v4().simple());
        let apps_root = self.apps_root(adapter)?;
        let root = apps_root.join(&app_id);
        let generated = root.join("generated");
        let generated_static = generated.join("static");
        let snapshots = root.join("snapshots");
        let runtime = root.join("runtime");

        for dir in [
            &generated_static,
            &generated.join("react"),
            &generated.join("ai"),
            &generated.join("ai").join("artifacts"),
            &generated.join("canvas"),
            &generated.join("markdown"),
            &generated.join("data"),
            &generated.join("file-processor"),
            &generated.join("widget"),
            &snapshots,
            &runtime.join("logs"),
        ] {
            fs::create_dir_all(dir)?;
        }

        fs::write(runtime.join("ports.json"), "{}\n")?;
        fs::write(runtime.join("process.json"), "{}\n")?;
        fs::write(root.join("prompt.history.jsonl"), "")?;

        let now = Utc::now().to_rfc3339();
        let paths = WorkspacePaths {
            root: root.clone(),
            generated,
            generated_static,
            runtime,
            snapshots,
        };
        let manifest = AppBoxManifest {
            app_id,
            name: clean_workspace_name(&name),
            mode: runtime_kind,
            created_at: now.clone(),
            updated_at: now,
            stack: stack_for_runtime(runtime_kind),
            paths: paths.clone(),
            constraints: WorkspaceConstraints {
                boundary: root.clone(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        };

        let lockfile = SofvaryLockfile {
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_packs: runtime_packs_for_runtime(runtime_kind),
            harness_packs: harness_packs_for_runtime(runtime_kind),
            plugin_packs: HashMap::new(),
            agent_adapter: "unconfigured".to_string(),
        };

        self.write_json(&root.join("app.box.json"), &manifest)?;
        self.write_json(&root.join("sofvary.lock.json"), &lockfile)?;

        Ok(manifest)
    }

    pub fn list_workspaces(&self) -> WorkspaceResult<Vec<WorkspaceSummary>> {
        let adapter = current_adapter();
        let apps_root = self.apps_root(adapter.as_ref())?;
        if !apps_root.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();
        for entry in fs::read_dir(apps_root)? {
            let entry = entry?;
            let workspace_root = entry.path();
            let manifest_path = workspace_root.join("app.box.json");
            if manifest_path.exists() {
                let manifest = match self.read_manifest_at_root(&workspace_root) {
                    Ok(manifest) => manifest,
                    Err(WorkspaceError::PathEscape(_))
                    | Err(WorkspaceError::InvalidManifest(_)) => {
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                summaries.push(WorkspaceSummary {
                    app_id: manifest.app_id,
                    name: manifest.name,
                    mode: manifest.mode,
                    updated_at: manifest.updated_at,
                    root: workspace_root,
                });
            }
        }
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(summaries)
    }

    pub fn get_workspace(&self, app_id: String) -> WorkspaceResult<AppBoxManifest> {
        let root = self.workspace_root(&app_id)?;
        self.read_manifest_at_root(&root)
    }

    pub fn get_workspace_with_adapter(
        &self,
        app_id: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<AppBoxManifest> {
        let root = self.workspace_root_with_adapter(&app_id, adapter)?;
        self.read_manifest_at_root(&root)
    }

    pub fn rename_workspace(
        &self,
        app_id: String,
        name: String,
    ) -> WorkspaceResult<AppBoxManifest> {
        let adapter = current_adapter();
        self.rename_workspace_with_adapter(app_id, name, adapter.as_ref())
    }

    pub fn rename_workspace_with_adapter(
        &self,
        app_id: String,
        name: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<AppBoxManifest> {
        let root = self.workspace_root_with_adapter(&app_id, adapter)?;
        if !root.exists() {
            return Err(WorkspaceError::NotFound(app_id));
        }

        let mut manifest = self.read_manifest_at_root(&root)?;
        manifest.name = clean_workspace_name(&name);
        manifest.updated_at = Utc::now().to_rfc3339();
        self.write_json(&manifest.paths.root.join("app.box.json"), &manifest)?;
        Ok(manifest)
    }

    pub fn delete_workspace(&self, app_id: String) -> WorkspaceResult<AppBoxManifest> {
        let adapter = current_adapter();
        self.delete_workspace_with_adapter(app_id, adapter.as_ref())
    }

    pub fn delete_workspace_with_adapter(
        &self,
        app_id: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<AppBoxManifest> {
        let apps_root = self.apps_root(adapter)?;
        let root = self.workspace_root_with_adapter(&app_id, adapter)?;
        if !root.exists() {
            return Err(WorkspaceError::NotFound(app_id));
        }

        let manifest = self.read_manifest_at_root(&root)?;
        let apps_root = normalize_for_boundary(&apps_root)?;
        let root = normalize_for_boundary(&root)?;
        if root == apps_root || !root.starts_with(&apps_root) {
            return Err(WorkspaceError::PathEscape(root));
        }

        fs::remove_dir_all(&root)?;
        Ok(manifest)
    }

    pub fn read_lockfile_for_manifest(
        &self,
        manifest: &AppBoxManifest,
    ) -> WorkspaceResult<SofvaryLockfile> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        Ok(serde_json::from_slice(&fs::read(
            manifest.paths.root.join("sofvary.lock.json"),
        )?)?)
    }

    #[allow(dead_code)]
    pub fn update_lockfile_pack(
        &self,
        app_id: String,
        kind: &str,
        pack_id: String,
        version: String,
    ) -> WorkspaceResult<SofvaryLockfile> {
        let adapter = current_adapter();
        self.update_lockfile_pack_with_adapter(app_id, kind, pack_id, version, adapter.as_ref())
    }

    #[allow(dead_code)]
    pub fn update_lockfile_pack_with_adapter(
        &self,
        app_id: String,
        kind: &str,
        pack_id: String,
        version: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<SofvaryLockfile> {
        self.update_lockfile_pack_inner(app_id, kind, pack_id, version, adapter)
    }

    pub fn update_lockfile_pack_with_policy(
        &self,
        app_id: String,
        kind: &str,
        pack_id: String,
        version: String,
        adapter: &dyn PlatformAdapter,
        approvals: &PolicyApprovalSet,
    ) -> WorkspaceResult<SofvaryLockfile> {
        let engine = PolicyEngine::new();
        engine.enforce(
            engine.evaluate_workspace_lockfile_update(PolicyWorkspaceLockfileUpdateRequest {
                app_id: app_id.clone(),
                kind: kind.to_string(),
                id: pack_id.clone(),
                version: version.clone(),
            }),
            approvals,
        )?;
        if kind == "plugin" {
            engine.enforce(engine.evaluate_plugin_enablement(&pack_id), approvals)?;
        }

        self.update_lockfile_pack_inner(app_id, kind, pack_id, version, adapter)
    }

    fn update_lockfile_pack_inner(
        &self,
        app_id: String,
        kind: &str,
        pack_id: String,
        version: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<SofvaryLockfile> {
        let manifest = self.get_workspace_with_adapter(app_id, adapter)?;
        let root = manifest.paths.root.clone();
        let manifest = self.validate_manifest_paths(manifest, &root)?;
        let mut lockfile = self.read_lockfile_for_manifest(&manifest)?;
        match kind {
            "runtime" => {
                lockfile.runtime_packs.clear();
                lockfile.runtime_packs.insert(pack_id, version);
            }
            "harness" => {
                lockfile.harness_packs.clear();
                lockfile.harness_packs.insert(pack_id, version);
            }
            "plugin" => {
                lockfile.plugin_packs.insert(pack_id, version);
            }
            _ => {
                return Err(WorkspaceError::InvalidManifest(format!(
                    "unsupported lockfile pack kind '{kind}'"
                )));
            }
        }
        self.write_json(&manifest.paths.root.join("sofvary.lock.json"), &lockfile)?;
        Ok(lockfile)
    }

    #[allow(dead_code)]
    pub fn update_lockfile_agent_adapter(
        &self,
        app_id: String,
        agent_adapter: String,
    ) -> WorkspaceResult<SofvaryLockfile> {
        let adapter = current_adapter();
        self.update_lockfile_agent_adapter_with_adapter(app_id, agent_adapter, adapter.as_ref())
    }

    #[allow(dead_code)]
    pub fn update_lockfile_agent_adapter_with_adapter(
        &self,
        app_id: String,
        agent_adapter: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<SofvaryLockfile> {
        let manifest = self.get_workspace_with_adapter(app_id, adapter)?;
        self.update_lockfile_agent_adapter_for_manifest(&manifest, agent_adapter)
    }

    pub fn update_lockfile_agent_adapter_for_manifest(
        &self,
        manifest: &AppBoxManifest,
        agent_adapter: String,
    ) -> WorkspaceResult<SofvaryLockfile> {
        let root = manifest.paths.root.clone();
        let manifest = self.validate_manifest_paths(manifest.clone(), &root)?;
        let mut lockfile = self.read_lockfile_for_manifest(&manifest)?;
        lockfile.agent_adapter = agent_adapter;
        self.write_json(&manifest.paths.root.join("sofvary.lock.json"), &lockfile)?;
        Ok(lockfile)
    }

    pub fn create_snapshot(&self, app_id: String) -> WorkspaceResult<SnapshotSummary> {
        let manifest = self.get_workspace(app_id)?;
        self.create_snapshot_for_manifest(&manifest)
    }

    #[allow(dead_code)]
    pub fn create_snapshot_with_adapter(
        &self,
        app_id: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<SnapshotSummary> {
        let manifest = self.get_workspace_with_adapter(app_id, adapter)?;
        self.create_snapshot_for_manifest(&manifest)
    }

    pub fn list_snapshots(&self, app_id: String) -> WorkspaceResult<Vec<SnapshotSummary>> {
        let manifest = self.get_workspace(app_id)?;
        if !manifest.paths.snapshots.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots: Vec<SnapshotSummary> = Vec::new();
        for entry in fs::read_dir(manifest.paths.snapshots)? {
            let entry = entry?;
            let summary_path = entry.path().join("snapshot.json");
            if summary_path.exists() {
                snapshots.push(serde_json::from_slice(&fs::read(summary_path)?)?);
            }
        }
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(snapshots)
    }

    pub fn rollback_snapshot(
        &self,
        app_id: String,
        snapshot_id: String,
    ) -> WorkspaceResult<AppBoxManifest> {
        let manifest = self.get_workspace(app_id)?;
        self.rollback_snapshot_for_manifest(manifest, snapshot_id)
    }

    #[allow(dead_code)]
    pub fn rollback_snapshot_with_adapter(
        &self,
        app_id: String,
        snapshot_id: String,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<AppBoxManifest> {
        let manifest = self.get_workspace_with_adapter(app_id, adapter)?;
        self.rollback_snapshot_for_manifest(manifest, snapshot_id)
    }

    pub fn replace_generated_static_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedStaticFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        validate_static_file_set(files, allowed_files)?;
        let static_root = self.ensure_static_root_inside_workspace(&manifest)?;

        for entry in fs::read_dir(&static_root)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }

        let mut written = Vec::new();
        for file in files {
            let target = self.ensure_child(&static_root, Path::new(&file.relative_path))?;
            self.enforce_file_write_policy(&manifest, &target)?;
            fs::File::create(&target)?.write_all(file.contents.as_bytes())?;
            written.push(target);
        }

        Ok(written)
    }

    pub fn write_generated_file_delta(
        &self,
        manifest: &AppBoxManifest,
        runtime_kind: &str,
        relative_path: &str,
        contents: &str,
        allowed_files: &[String],
    ) -> WorkspaceResult<PathBuf> {
        if !allowed_files.iter().any(|allowed| allowed == relative_path) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "{runtime_kind} file '{relative_path}' is not allowed by the output contract"
            )));
        }

        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        let (target_root, normalized_contents) = match runtime_kind {
            "static-html" => {
                validate_static_relative_file(relative_path)?;
                (
                    self.ensure_static_root_inside_workspace(&manifest)?,
                    contents.to_string(),
                )
            }
            "react-vite" => {
                validate_react_relative_file(relative_path)?;
                (
                    self.ensure_react_root_inside_workspace(&manifest)?,
                    contents.to_string(),
                )
            }
            "react-sqlite" => {
                validate_react_relative_file(relative_path)?;
                if !relative_path.starts_with("react/") && !relative_path.starts_with("data/") {
                    return Err(WorkspaceError::InvalidManifest(format!(
                        "react-sqlite file '{relative_path}' must be under generated/react or generated/data"
                    )));
                }
                let file = GeneratedReactSqliteFile {
                    relative_path: relative_path.to_string(),
                    contents: contents.to_string(),
                };
                (
                    self.ensure_generated_root_inside_workspace(&manifest)?,
                    normalized_react_sqlite_file_contents(&file)?,
                )
            }
            "canvas2d" => {
                validate_react_relative_file(relative_path)?;
                let generated_root = self.ensure_generated_root_inside_workspace(&manifest)?;
                (
                    self.ensure_child(&generated_root, Path::new("canvas"))?,
                    contents.to_string(),
                )
            }
            "markdown-knowledge" => {
                validate_generated_project_file_delta(
                    relative_path,
                    runtime_kind,
                    &["markdown", "react"],
                )?;
                (
                    self.ensure_generated_root_inside_workspace(&manifest)?,
                    contents.to_string(),
                )
            }
            "data-table" => {
                validate_generated_project_file_delta(
                    relative_path,
                    runtime_kind,
                    &["data", "react"],
                )?;
                (
                    self.ensure_generated_root_inside_workspace(&manifest)?,
                    contents.to_string(),
                )
            }
            "file-processor" => {
                validate_generated_project_file_delta(
                    relative_path,
                    runtime_kind,
                    &["file-processor", "react"],
                )?;
                (
                    self.ensure_generated_root_inside_workspace(&manifest)?,
                    contents.to_string(),
                )
            }
            "desktop-widget" => {
                validate_generated_project_file_delta(
                    relative_path,
                    runtime_kind,
                    &["widget", "react"],
                )?;
                (
                    self.ensure_generated_root_inside_workspace(&manifest)?,
                    contents.to_string(),
                )
            }
            "ai-agent-app" => {
                validate_generated_project_file_delta(
                    relative_path,
                    runtime_kind,
                    &["ai", "react"],
                )?;
                (
                    self.ensure_generated_root_inside_workspace(&manifest)?,
                    contents.to_string(),
                )
            }
            runtime_kind => {
                return Err(WorkspaceError::InvalidManifest(format!(
                    "unsupported runtime kind '{runtime_kind}'"
                )));
            }
        };

        fs::create_dir_all(&target_root)?;
        let target = self.ensure_child(&target_root, Path::new(relative_path))?;
        let parent = target.parent().ok_or_else(|| {
            WorkspaceError::InvalidManifest(format!(
                "{runtime_kind} file '{relative_path}' has no parent directory"
            ))
        })?;
        fs::create_dir_all(parent)?;
        self.enforce_file_write_policy(&manifest, &target)?;
        fs::File::create(&target)?.write_all(normalized_contents.as_bytes())?;
        Ok(target)
    }

    pub fn replace_generated_react_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedReactFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        validate_react_file_set(files, allowed_files)?;
        let react_root = self.ensure_react_root_inside_workspace(&manifest)?;

        clear_directory(&react_root)?;

        let mut written = Vec::new();
        for file in files {
            let target = self.ensure_child(&react_root, Path::new(&file.relative_path))?;
            let parent = target.parent().ok_or_else(|| {
                WorkspaceError::InvalidManifest(format!(
                    "react file '{}' has no parent directory",
                    file.relative_path
                ))
            })?;
            fs::create_dir_all(parent)?;
            self.enforce_file_write_policy(&manifest, &target)?;
            fs::File::create(&target)?.write_all(file.contents.as_bytes())?;
            written.push(target);
        }

        Ok(written)
    }

    pub fn replace_generated_react_sqlite_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedReactSqliteFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        validate_react_sqlite_file_set(files, allowed_files)?;
        let generated_root = self.ensure_generated_root_inside_workspace(&manifest)?;
        let react_root = self.ensure_child(&generated_root, Path::new("react"))?;
        let data_root = self.ensure_child(&generated_root, Path::new("data"))?;

        fs::create_dir_all(&react_root)?;
        fs::create_dir_all(&data_root)?;
        clear_directory(&react_root)?;
        clear_directory_except(&data_root, &["app.sqlite"])?;

        let mut written = Vec::new();
        for file in files {
            let target = self.ensure_child(&generated_root, Path::new(&file.relative_path))?;
            let contents = normalized_react_sqlite_file_contents(file)?;
            let parent = target.parent().ok_or_else(|| {
                WorkspaceError::InvalidManifest(format!(
                    "react-sqlite file '{}' has no parent directory",
                    file.relative_path
                ))
            })?;
            fs::create_dir_all(parent)?;
            self.enforce_file_write_policy(&manifest, &target)?;
            fs::File::create(&target)?.write_all(contents.as_bytes())?;
            written.push(target);
        }

        Ok(written)
    }

    pub fn prepare_react_sqlite_workspace_for_preview(
        &self,
        manifest: &AppBoxManifest,
    ) -> WorkspaceResult<()> {
        self.normalize_react_sqlite_workspace_package(manifest)?;
        self.normalize_react_sqlite_vite_config(manifest)?;
        Ok(())
    }

    pub fn normalize_react_sqlite_workspace_package(
        &self,
        manifest: &AppBoxManifest,
    ) -> WorkspaceResult<()> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        let generated_root = self.ensure_generated_root_inside_workspace(&manifest)?;
        let package_path = self.ensure_child(&generated_root, Path::new("react/package.json"))?;
        let contents = fs::read_to_string(&package_path)?;
        let normalized = normalize_react_sqlite_package_json(&contents)?;
        if normalized != contents {
            self.enforce_file_write_policy(&manifest, &package_path)?;
            fs::File::create(&package_path)?.write_all(normalized.as_bytes())?;
        }
        Ok(())
    }

    fn normalize_react_sqlite_vite_config(&self, manifest: &AppBoxManifest) -> WorkspaceResult<()> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        let generated_root = self.ensure_generated_root_inside_workspace(&manifest)?;
        let vite_config_path =
            self.ensure_child(&generated_root, Path::new("react/vite.config.ts"))?;
        let contents = fs::read_to_string(&vite_config_path)?;
        let normalized = normalize_react_sqlite_vite_config(&contents);
        if normalized != contents {
            self.enforce_file_write_policy(&manifest, &vite_config_path)?;
            fs::File::create(&vite_config_path)?.write_all(normalized.as_bytes())?;
        }
        Ok(())
    }

    pub fn replace_generated_canvas2d_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedCanvas2dFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        validate_canvas2d_file_set(files, allowed_files)?;
        let generated_root = self.ensure_generated_root_inside_workspace(&manifest)?;
        let canvas_root = self.ensure_child(&generated_root, Path::new("canvas"))?;

        fs::create_dir_all(&canvas_root)?;
        clear_directory(&canvas_root)?;
        fs::create_dir_all(canvas_root.join("assets"))?;

        let mut written = Vec::new();
        for file in files {
            let target = self.ensure_child(&canvas_root, Path::new(&file.relative_path))?;
            let parent = target.parent().ok_or_else(|| {
                WorkspaceError::InvalidManifest(format!(
                    "canvas2d file '{}' has no parent directory",
                    file.relative_path
                ))
            })?;
            fs::create_dir_all(parent)?;
            self.enforce_file_write_policy(&manifest, &target)?;
            fs::File::create(&target)?.write_all(file.contents.as_bytes())?;
            written.push(target);
        }

        Ok(written)
    }

    pub fn replace_generated_markdown_knowledge_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedProjectFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        self.replace_generated_project_files(
            manifest,
            files,
            allowed_files,
            "markdown-knowledge",
            &["markdown", "react"],
        )
    }

    pub fn replace_generated_data_table_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedProjectFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        self.replace_generated_project_files(
            manifest,
            files,
            allowed_files,
            "data-table",
            &["data", "react"],
        )
    }

    pub fn replace_generated_file_processor_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedProjectFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        self.replace_generated_project_files(
            manifest,
            files,
            allowed_files,
            "file-processor",
            &["file-processor", "react"],
        )
    }

    pub fn replace_generated_desktop_widget_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedProjectFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        self.replace_generated_project_files(
            manifest,
            files,
            allowed_files,
            "desktop-widget",
            &["widget", "react"],
        )
    }

    pub fn replace_generated_ai_agent_app_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedProjectFile],
        allowed_files: &[String],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        self.replace_generated_project_files(
            manifest,
            files,
            allowed_files,
            "ai-agent-app",
            &["ai", "react"],
        )
    }

    fn replace_generated_project_files(
        &self,
        manifest: &AppBoxManifest,
        files: &[GeneratedProjectFile],
        allowed_files: &[String],
        runtime_label: &str,
        allowed_top_level_dirs: &[&str],
    ) -> WorkspaceResult<Vec<PathBuf>> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        validate_generated_project_file_set(
            files,
            allowed_files,
            runtime_label,
            allowed_top_level_dirs,
        )?;
        let generated_root = self.ensure_generated_root_inside_workspace(&manifest)?;
        let boundary = manifest.paths.root.canonicalize()?;

        let top_level_dirs = allowed_top_level_dirs
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        for dir in top_level_dirs {
            let root = self.ensure_child(&generated_root, Path::new(dir))?;
            fs::create_dir_all(&root)?;
            let root = ensure_existing_path_inside_boundary(&root, &boundary)?;
            clear_directory(&root)?;
        }

        let mut written = Vec::new();
        for file in files {
            let target = self.ensure_child(&generated_root, Path::new(&file.relative_path))?;
            let parent = target.parent().ok_or_else(|| {
                WorkspaceError::InvalidManifest(format!(
                    "{runtime_label} file '{}' has no parent directory",
                    file.relative_path
                ))
            })?;
            fs::create_dir_all(parent)?;
            ensure_existing_path_inside_boundary(parent, &boundary)?;
            self.enforce_file_write_policy(&manifest, &target)?;
            fs::File::create(&target)?.write_all(file.contents.as_bytes())?;
            written.push(target);
        }

        Ok(written)
    }

    fn create_snapshot_for_manifest(
        &self,
        manifest: &AppBoxManifest,
    ) -> WorkspaceResult<SnapshotSummary> {
        let manifest = self.validate_manifest_paths(manifest.clone(), &manifest.paths.root)?;
        let snapshot_id = format!("snapshot_{}", Uuid::new_v4().simple());
        let snapshot_root = manifest.paths.snapshots.join(&snapshot_id);
        fs::create_dir_all(&snapshot_root)?;
        let generated_snapshot = snapshot_root.join("generated");
        copy_dir_all(&manifest.paths.generated, &generated_snapshot)?;
        let summary = SnapshotSummary {
            snapshot_id,
            created_at: Utc::now().to_rfc3339(),
            path: snapshot_root,
        };
        self.write_json(&summary.path.join("snapshot.json"), &summary)?;
        Ok(summary)
    }

    fn rollback_snapshot_for_manifest(
        &self,
        manifest: AppBoxManifest,
        snapshot_id: String,
    ) -> WorkspaceResult<AppBoxManifest> {
        let workspace_root = manifest.paths.root.clone();
        let manifest = self.validate_manifest_paths(manifest, &workspace_root)?;
        let snapshot_root = self.ensure_child(&manifest.paths.snapshots, snapshot_id.as_ref())?;
        let snapshot_generated = snapshot_root.join("generated");
        if !snapshot_generated.exists() {
            return Err(WorkspaceError::NotFound(
                snapshot_root.display().to_string(),
            ));
        }

        if manifest.paths.generated.exists() {
            fs::remove_dir_all(&manifest.paths.generated)?;
        }
        copy_dir_all(&snapshot_generated, &manifest.paths.generated)?;

        let mut updated = manifest;
        updated.updated_at = Utc::now().to_rfc3339();
        self.write_json(&updated.paths.root.join("app.box.json"), &updated)?;
        Ok(updated)
    }

    pub fn apps_root(&self, adapter: &dyn PlatformAdapter) -> WorkspaceResult<PathBuf> {
        let root = adapter.dirs()?.data_dir.join("apps");
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    pub fn workspace_root(&self, app_id: &str) -> WorkspaceResult<PathBuf> {
        let adapter = current_adapter();
        self.workspace_root_with_adapter(app_id, adapter.as_ref())
    }

    pub fn workspace_root_with_adapter(
        &self,
        app_id: &str,
        adapter: &dyn PlatformAdapter,
    ) -> WorkspaceResult<PathBuf> {
        validate_workspace_app_id(app_id)?;
        let apps_root = self.apps_root(adapter)?;
        self.ensure_child(&apps_root, Path::new(app_id))
    }

    pub fn ensure_child(&self, root: &Path, child: &Path) -> WorkspaceResult<PathBuf> {
        let candidate = root.join(child);
        let normalized_root = normalize_for_boundary(root)?;
        let normalized_candidate = normalize_for_boundary(&candidate)?;
        if normalized_candidate.starts_with(&normalized_root) {
            Ok(normalized_candidate)
        } else {
            Err(WorkspaceError::PathEscape(normalized_candidate))
        }
    }

    fn read_manifest_at_root(&self, root: &Path) -> WorkspaceResult<AppBoxManifest> {
        let manifest = self.read_manifest_path(&root.join("app.box.json"))?;
        self.validate_manifest_paths(manifest, root)
    }

    fn read_manifest_path(&self, path: &Path) -> WorkspaceResult<AppBoxManifest> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    fn validate_manifest_paths(
        &self,
        mut manifest: AppBoxManifest,
        root: &Path,
    ) -> WorkspaceResult<AppBoxManifest> {
        let expected = expected_workspace_paths(root);
        let workspace_name = root
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                WorkspaceError::InvalidManifest("workspace root has no directory name".to_string())
            })?;

        if manifest.app_id != workspace_name {
            return Err(WorkspaceError::InvalidManifest(format!(
                "app_id '{}' does not match workspace directory '{}'",
                manifest.app_id, workspace_name
            )));
        }

        ensure_same_path(&manifest.paths.root, &expected.root)?;
        ensure_same_path(&manifest.paths.generated, &expected.generated)?;
        ensure_same_path(&manifest.paths.generated_static, &expected.generated_static)?;
        ensure_same_path(&manifest.paths.runtime, &expected.runtime)?;
        ensure_same_path(&manifest.paths.snapshots, &expected.snapshots)?;
        ensure_same_path(&manifest.constraints.boundary, &expected.root)?;

        if manifest.constraints.allow_external_files {
            return Err(WorkspaceError::InvalidManifest(
                "external files are not allowed in Phase 1-5 workspaces".to_string(),
            ));
        }

        manifest.paths = expected;
        manifest.constraints.boundary = manifest.paths.root.clone();
        Ok(manifest)
    }

    fn write_json<T: serde::Serialize>(&self, path: &Path, value: &T) -> WorkspaceResult<()> {
        fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
        Ok(())
    }

    fn enforce_file_write_policy(
        &self,
        manifest: &AppBoxManifest,
        target: &Path,
    ) -> WorkspaceResult<()> {
        let engine = PolicyEngine::new();
        let decision = engine.evaluate_file_write(PolicyFileWriteRequest {
            workspace_root: manifest.paths.root.clone(),
            target_path: target.to_path_buf(),
        });
        Ok(engine.enforce(decision, &PolicyApprovalSet::default())?)
    }

    fn ensure_static_root_inside_workspace(
        &self,
        manifest: &AppBoxManifest,
    ) -> WorkspaceResult<PathBuf> {
        fs::create_dir_all(&manifest.paths.generated_static)?;
        let boundary = manifest.paths.root.canonicalize()?;
        let static_root = manifest.paths.generated_static.canonicalize()?;
        if static_root.starts_with(&boundary) {
            Ok(static_root)
        } else {
            Err(WorkspaceError::PathEscape(static_root))
        }
    }

    fn ensure_react_root_inside_workspace(
        &self,
        manifest: &AppBoxManifest,
    ) -> WorkspaceResult<PathBuf> {
        let react_root = self.ensure_child(&manifest.paths.generated, Path::new("react"))?;
        fs::create_dir_all(&react_root)?;
        let boundary = manifest.paths.root.canonicalize()?;
        let react_root = react_root.canonicalize()?;
        if react_root.starts_with(&boundary) {
            Ok(react_root)
        } else {
            Err(WorkspaceError::PathEscape(react_root))
        }
    }

    fn ensure_generated_root_inside_workspace(
        &self,
        manifest: &AppBoxManifest,
    ) -> WorkspaceResult<PathBuf> {
        fs::create_dir_all(&manifest.paths.generated)?;
        let boundary = manifest.paths.root.canonicalize()?;
        let generated_root = manifest.paths.generated.canonicalize()?;
        if generated_root.starts_with(&boundary) {
            Ok(generated_root)
        } else {
            Err(WorkspaceError::PathEscape(generated_root))
        }
    }
}

fn validate_static_file_set(
    files: &[GeneratedStaticFile],
    allowed_files: &[String],
) -> WorkspaceResult<()> {
    if allowed_files.is_empty() {
        return Err(WorkspaceError::InvalidManifest(
            "allowed static file set must not be empty".to_string(),
        ));
    }

    let allowed: HashSet<&str> = allowed_files.iter().map(String::as_str).collect();
    let mut actual = HashSet::new();

    for file in files {
        validate_static_relative_file(&file.relative_path)?;
        if !allowed.contains(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "static file '{}' is not allowed by the output contract",
                file.relative_path
            )));
        }
        if !actual.insert(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "static file '{}' is duplicated",
                file.relative_path
            )));
        }
    }

    if actual != allowed {
        return Err(WorkspaceError::InvalidManifest(
            "static file set must match the output contract exactly".to_string(),
        ));
    }

    Ok(())
}

fn validate_static_relative_file(relative_path: &str) -> WorkspaceResult<()> {
    if relative_path.trim().is_empty() || relative_path.contains('\\') {
        return Err(WorkspaceError::InvalidManifest(format!(
            "invalid static file path '{relative_path}'"
        )));
    }

    let path = Path::new(relative_path);
    let mut components = path.components();
    let Some(first) = components.next() else {
        return Err(WorkspaceError::InvalidManifest(
            "static file path must not be empty".to_string(),
        ));
    };

    if !matches!(first, std::path::Component::Normal(_)) || components.next().is_some() {
        return Err(WorkspaceError::PathEscape(PathBuf::from(relative_path)));
    }

    Ok(())
}

fn validate_react_file_set(
    files: &[GeneratedReactFile],
    allowed_files: &[String],
) -> WorkspaceResult<()> {
    if allowed_files.is_empty() {
        return Err(WorkspaceError::InvalidManifest(
            "allowed react file set must not be empty".to_string(),
        ));
    }

    let allowed: HashSet<&str> = allowed_files.iter().map(String::as_str).collect();
    let mut actual = HashSet::new();

    for file in files {
        validate_react_relative_file(&file.relative_path)?;
        if !allowed.contains(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "react file '{}' is not allowed by the output contract",
                file.relative_path
            )));
        }
        if !actual.insert(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "react file '{}' is duplicated",
                file.relative_path
            )));
        }
    }

    if actual != allowed {
        return Err(WorkspaceError::InvalidManifest(
            "react file set must match the output contract exactly".to_string(),
        ));
    }

    Ok(())
}

fn validate_react_sqlite_file_set(
    files: &[GeneratedReactSqliteFile],
    allowed_files: &[String],
) -> WorkspaceResult<()> {
    if allowed_files.is_empty() {
        return Err(WorkspaceError::InvalidManifest(
            "allowed react-sqlite file set must not be empty".to_string(),
        ));
    }

    let allowed: HashSet<&str> = allowed_files.iter().map(String::as_str).collect();
    let mut actual = HashSet::new();

    for file in files {
        validate_react_relative_file(&file.relative_path)?;
        if !file.relative_path.starts_with("react/") && !file.relative_path.starts_with("data/") {
            return Err(WorkspaceError::InvalidManifest(format!(
                "react-sqlite file '{}' must be under generated/react or generated/data",
                file.relative_path
            )));
        }
        if !allowed.contains(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "react-sqlite file '{}' is not allowed by the output contract",
                file.relative_path
            )));
        }
        if !actual.insert(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "react-sqlite file '{}' is duplicated",
                file.relative_path
            )));
        }
    }

    if actual != allowed {
        return Err(WorkspaceError::InvalidManifest(
            "react-sqlite file set must match the output contract exactly".to_string(),
        ));
    }

    Ok(())
}

fn normalized_react_sqlite_file_contents(
    file: &GeneratedReactSqliteFile,
) -> WorkspaceResult<String> {
    if file.relative_path == "react/package.json" {
        normalize_react_sqlite_package_json(&file.contents)
    } else {
        Ok(file.contents.clone())
    }
}

fn normalize_react_sqlite_package_json(contents: &str) -> WorkspaceResult<String> {
    let value: Value = serde_json::from_str(contents).map_err(|error| {
        WorkspaceError::InvalidManifest(format!("react/package.json must be valid JSON: {error}"))
    })?;
    let root = value.as_object().ok_or_else(|| {
        WorkspaceError::InvalidManifest("react/package.json must be a JSON object".to_string())
    })?;
    let mut normalized = root.clone();
    normalized.insert(
        "name".to_string(),
        Value::String("generated-react-sqlite-app".to_string()),
    );
    normalized.insert("version".to_string(), Value::String("0.1.0".to_string()));
    normalized.insert("private".to_string(), Value::Bool(true));
    normalized.insert("type".to_string(), Value::String("module".to_string()));
    normalized.insert(
        "scripts".to_string(),
        Value::Object(react_sqlite_managed_package_scripts()),
    );
    normalized.insert(
        "dependencies".to_string(),
        Value::Object(react_sqlite_managed_package_dependencies()),
    );
    normalized.insert("devDependencies".to_string(), Value::Object(Map::new()));

    serde_json::to_string_pretty(&Value::Object(normalized))
        .map(|value| value + "\n")
        .map_err(WorkspaceError::from)
}

fn normalize_react_sqlite_vite_config(contents: &str) -> String {
    let mut normalized = contents.to_string();
    let replacements = [
        (
            "'http://127.0.0.1:4177'",
            "`http://127.0.0.1:${sofvaryApiPort}`",
        ),
        (
            "\"http://127.0.0.1:4177\"",
            "`http://127.0.0.1:${sofvaryApiPort}`",
        ),
        (
            "'http://localhost:4177'",
            "`http://127.0.0.1:${sofvaryApiPort}`",
        ),
        (
            "\"http://localhost:4177\"",
            "`http://127.0.0.1:${sofvaryApiPort}`",
        ),
    ];
    let mut changed = false;
    for (from, to) in replacements {
        if normalized.contains(from) {
            normalized = normalized.replace(from, to);
            changed = true;
        }
    }
    if changed && !normalized.contains("SOFVARY_API_PORT") {
        let api_port_config =
            "const sofvaryApiPort = process.env.SOFVARY_API_PORT ?? \"4177\";\n\n";
        normalized = if normalized.contains("export default") {
            normalized.replacen(
                "export default",
                &(api_port_config.to_string() + "export default"),
                1,
            )
        } else {
            api_port_config.to_string() + &normalized
        };
    }
    normalized
}

fn react_sqlite_managed_package_scripts() -> Map<String, Value> {
    [
        ("dev", "vite --host 127.0.0.1"),
        ("build", "tsc --noEmit && vite build"),
        ("api", "tsx server/index.ts"),
        ("server", "tsx server/index.ts"),
        ("start", "tsx server/index.ts"),
        ("preview", "vite preview --host 127.0.0.1"),
    ]
    .into_iter()
    .map(|(name, script)| (name.to_string(), Value::String(script.to_string())))
    .collect()
}

fn react_sqlite_managed_package_dependencies() -> Map<String, Value> {
    REACT_SQLITE_MANAGED_PACKAGE_DEPENDENCIES
        .iter()
        .map(|(name, version)| (name.to_string(), Value::String(version.to_string())))
        .collect()
}

fn validate_canvas2d_file_set(
    files: &[GeneratedCanvas2dFile],
    allowed_files: &[String],
) -> WorkspaceResult<()> {
    if allowed_files.is_empty() {
        return Err(WorkspaceError::InvalidManifest(
            "allowed canvas2d file set must not be empty".to_string(),
        ));
    }

    let allowed: HashSet<&str> = allowed_files.iter().map(String::as_str).collect();
    let mut actual = HashSet::new();

    for file in files {
        validate_react_relative_file(&file.relative_path)?;
        if !allowed.contains(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "canvas2d file '{}' is not allowed by the output contract",
                file.relative_path
            )));
        }
        if !actual.insert(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "canvas2d file '{}' is duplicated",
                file.relative_path
            )));
        }
    }

    if actual != allowed {
        return Err(WorkspaceError::InvalidManifest(
            "canvas2d file set must match the output contract exactly".to_string(),
        ));
    }

    Ok(())
}

fn validate_generated_project_file_set(
    files: &[GeneratedProjectFile],
    allowed_files: &[String],
    runtime_label: &str,
    allowed_top_level_dirs: &[&str],
) -> WorkspaceResult<()> {
    if allowed_files.is_empty() {
        return Err(WorkspaceError::InvalidManifest(format!(
            "allowed {runtime_label} file set must not be empty"
        )));
    }

    let allowed: HashSet<&str> = allowed_files.iter().map(String::as_str).collect();
    let top_level: HashSet<&str> = allowed_top_level_dirs.iter().copied().collect();
    let mut actual = HashSet::new();

    for file in files {
        validate_react_relative_file(&file.relative_path)?;
        let first_component = Path::new(&file.relative_path)
            .components()
            .next()
            .and_then(|component| match component {
                std::path::Component::Normal(value) => value.to_str(),
                _ => None,
            })
            .ok_or_else(|| {
                WorkspaceError::InvalidManifest(format!(
                    "{runtime_label} file '{}' has no top-level directory",
                    file.relative_path
                ))
            })?;
        if !top_level.contains(first_component) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "{runtime_label} file '{}' must stay under one of {:?}",
                file.relative_path, allowed_top_level_dirs
            )));
        }
        if !allowed.contains(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "{runtime_label} file '{}' is not allowed by the output contract",
                file.relative_path
            )));
        }
        if !actual.insert(file.relative_path.as_str()) {
            return Err(WorkspaceError::InvalidManifest(format!(
                "{runtime_label} file '{}' is duplicated",
                file.relative_path
            )));
        }
    }

    if actual != allowed {
        return Err(WorkspaceError::InvalidManifest(format!(
            "{runtime_label} file set must match the output contract exactly"
        )));
    }

    Ok(())
}

fn validate_generated_project_file_delta(
    relative_path: &str,
    runtime_label: &str,
    allowed_top_level_dirs: &[&str],
) -> WorkspaceResult<()> {
    validate_react_relative_file(relative_path)?;
    let top_level: HashSet<&str> = allowed_top_level_dirs.iter().copied().collect();
    let first_component = Path::new(relative_path)
        .components()
        .next()
        .and_then(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .ok_or_else(|| {
            WorkspaceError::InvalidManifest(format!(
                "{runtime_label} file '{relative_path}' has no top-level directory"
            ))
        })?;
    if !top_level.contains(first_component) {
        return Err(WorkspaceError::InvalidManifest(format!(
            "{runtime_label} file '{relative_path}' must stay under one of {:?}",
            allowed_top_level_dirs
        )));
    }
    Ok(())
}

fn validate_react_relative_file(relative_path: &str) -> WorkspaceResult<()> {
    if relative_path.trim().is_empty() || relative_path.contains('\\') {
        return Err(WorkspaceError::InvalidManifest(format!(
            "invalid react file path '{relative_path}'"
        )));
    }

    let path = Path::new(relative_path);
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(WorkspaceError::PathEscape(PathBuf::from(relative_path)));
    }

    if !path
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(WorkspaceError::PathEscape(PathBuf::from(relative_path)));
    }

    Ok(())
}

fn clear_directory(root: &Path) -> WorkspaceResult<()> {
    clear_directory_except(root, &[])
}

fn clear_directory_except(root: &Path, preserved_file_names: &[&str]) -> WorkspaceResult<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(path)?;
        } else if preserved_file_names.iter().any(|file_name| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(|value| value == *file_name)
                .unwrap_or(false)
        }) {
            continue;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn validate_workspace_app_id(app_id: &str) -> WorkspaceResult<()> {
    if app_id.trim().is_empty()
        || app_id.contains('/')
        || app_id.contains('\\')
        || app_id.contains("..")
    {
        return Err(WorkspaceError::InvalidManifest(
            "workspace app_id must be a single path segment".to_string(),
        ));
    }

    Ok(())
}

fn stack_for_runtime(runtime_kind: RuntimeKind) -> Vec<String> {
    match runtime_kind {
        RuntimeKind::StaticHtml => {
            vec!["Static HTML".to_string(), "Vanilla JavaScript".to_string()]
        }
        RuntimeKind::ReactVite => vec![
            "React".to_string(),
            "TypeScript".to_string(),
            "Vite".to_string(),
        ],
        RuntimeKind::ReactSqlite => vec![
            "React".to_string(),
            "TypeScript".to_string(),
            "Vite".to_string(),
            "Node local API".to_string(),
            "SQLite".to_string(),
        ],
        RuntimeKind::AiAgentApp => vec![
            "React".to_string(),
            "TypeScript".to_string(),
            "Vite".to_string(),
            "Sofvary AI Gateway".to_string(),
            "Provider Binding".to_string(),
        ],
        RuntimeKind::Canvas2d => vec!["Canvas 2D".to_string(), "JavaScript".to_string()],
        RuntimeKind::MarkdownKnowledge => vec![
            "React".to_string(),
            "TypeScript".to_string(),
            "Vite".to_string(),
            "Markdown".to_string(),
            "JSON".to_string(),
        ],
        RuntimeKind::DataTable => vec![
            "React".to_string(),
            "TypeScript".to_string(),
            "Vite".to_string(),
            "JSON".to_string(),
        ],
        RuntimeKind::FileProcessor => vec![
            "React".to_string(),
            "TypeScript".to_string(),
            "Vite".to_string(),
            "Dry-run file operations".to_string(),
        ],
        RuntimeKind::DesktopWidget => vec![
            "React".to_string(),
            "TypeScript".to_string(),
            "Vite".to_string(),
            "Widget".to_string(),
        ],
    }
}

fn runtime_packs_for_runtime(runtime_kind: RuntimeKind) -> HashMap<String, String> {
    match runtime_kind {
        RuntimeKind::StaticHtml => HashMap::from([(
            STATIC_HTML_RUNTIME_PACK_ID.to_string(),
            STATIC_HTML_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::ReactVite => HashMap::from([(
            REACT_VITE_RUNTIME_PACK_ID.to_string(),
            REACT_VITE_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::ReactSqlite => HashMap::from([(
            REACT_SQLITE_RUNTIME_PACK_ID.to_string(),
            REACT_SQLITE_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::AiAgentApp => HashMap::from([(
            AI_AGENT_APP_RUNTIME_PACK_ID.to_string(),
            AI_AGENT_APP_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::Canvas2d => HashMap::from([(
            CANVAS2D_RUNTIME_PACK_ID.to_string(),
            CANVAS2D_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::MarkdownKnowledge => HashMap::from([(
            MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID.to_string(),
            MARKDOWN_KNOWLEDGE_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::DataTable => HashMap::from([(
            DATA_TABLE_RUNTIME_PACK_ID.to_string(),
            DATA_TABLE_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::FileProcessor => HashMap::from([(
            FILE_PROCESSOR_RUNTIME_PACK_ID.to_string(),
            FILE_PROCESSOR_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::DesktopWidget => HashMap::from([(
            DESKTOP_WIDGET_RUNTIME_PACK_ID.to_string(),
            DESKTOP_WIDGET_PACK_VERSION.to_string(),
        )]),
    }
}

fn harness_packs_for_runtime(runtime_kind: RuntimeKind) -> HashMap<String, String> {
    match runtime_kind {
        RuntimeKind::StaticHtml => HashMap::from([(
            STATIC_HTML_HARNESS_PACK_ID.to_string(),
            STATIC_HTML_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::ReactVite => HashMap::from([(
            REACT_VITE_HARNESS_PACK_ID.to_string(),
            REACT_VITE_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::ReactSqlite => HashMap::from([(
            REACT_SQLITE_HARNESS_PACK_ID.to_string(),
            REACT_SQLITE_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::AiAgentApp => HashMap::from([(
            AI_AGENT_APP_HARNESS_PACK_ID.to_string(),
            AI_AGENT_APP_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::Canvas2d => HashMap::from([(
            CANVAS2D_HARNESS_PACK_ID.to_string(),
            CANVAS2D_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::MarkdownKnowledge => HashMap::from([(
            MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID.to_string(),
            MARKDOWN_KNOWLEDGE_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::DataTable => HashMap::from([(
            DATA_TABLE_HARNESS_PACK_ID.to_string(),
            DATA_TABLE_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::FileProcessor => HashMap::from([(
            FILE_PROCESSOR_HARNESS_PACK_ID.to_string(),
            FILE_PROCESSOR_PACK_VERSION.to_string(),
        )]),
        RuntimeKind::DesktopWidget => HashMap::from([(
            DESKTOP_WIDGET_HARNESS_PACK_ID.to_string(),
            DESKTOP_WIDGET_PACK_VERSION.to_string(),
        )]),
    }
}

fn clean_workspace_name(name: &str) -> String {
    clean_display_name(name, "Untitled App", 40)
}

fn normalize_for_boundary(path: &Path) -> WorkspaceResult<PathBuf> {
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
    Ok(normalized)
}

fn ensure_existing_path_inside_boundary(path: &Path, boundary: &Path) -> WorkspaceResult<PathBuf> {
    let canonical = path.canonicalize()?;
    if canonical.starts_with(boundary) {
        Ok(canonical)
    } else {
        Err(WorkspaceError::PathEscape(canonical))
    }
}

fn expected_workspace_paths(root: &Path) -> WorkspacePaths {
    let root = root.to_path_buf();
    let generated = root.join("generated");
    WorkspacePaths {
        root: root.clone(),
        generated: generated.clone(),
        generated_static: generated.join("static"),
        runtime: root.join("runtime"),
        snapshots: root.join("snapshots"),
    }
}

fn ensure_same_path(actual: &Path, expected: &Path) -> WorkspaceResult<()> {
    let normalized_actual = normalize_for_boundary(actual)?;
    let normalized_expected = normalize_for_boundary(expected)?;
    if normalized_actual == normalized_expected {
        Ok(())
    } else {
        Err(WorkspaceError::PathEscape(normalized_actual))
    }
}

fn copy_dir_all(from: &Path, to: &Path) -> WorkspaceResult<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::policy_engine::workspace_lockfile_update_subject;
    use crate::core::policy_types::{PolicyActionKind, PolicyApprovalGrant};
    use crate::platform::windows::WindowsPlatformAdapter;
    use crate::platform::{
        ArchKind, CommandSpec, OsKind, PlatformAdapter, PlatformDirs, PlatformResult,
        ProcessHandle, ProcessOutput, WebviewProfile, WorkArea,
    };
    use std::path::Path;

    struct TempAdapter {
        dirs: PlatformDirs,
    }

    impl PlatformAdapter for TempAdapter {
        fn os(&self) -> OsKind {
            OsKind::Macos
        }

        fn arch(&self) -> ArchKind {
            ArchKind::Arm64
        }

        fn dirs(&self) -> PlatformResult<PlatformDirs> {
            Ok(self.dirs.clone())
        }

        fn normalize_path(&self, input: &str) -> PlatformResult<PathBuf> {
            WindowsPlatformAdapter.normalize_path(input)
        }

        fn ensure_executable(&self, path: &Path) -> PlatformResult<()> {
            WindowsPlatformAdapter.ensure_executable(path)
        }

        fn resolve_sidecar_executable(&self, name: &str) -> PlatformResult<PathBuf> {
            WindowsPlatformAdapter.resolve_sidecar_executable(name)
        }

        fn run_process(&self, spec: CommandSpec) -> PlatformResult<ProcessOutput> {
            WindowsPlatformAdapter.run_process(spec)
        }

        fn spawn_process(&self, spec: CommandSpec) -> PlatformResult<ProcessHandle> {
            WindowsPlatformAdapter.spawn_process(spec)
        }

        fn kill_process_tree(&self, pid: u32) -> PlatformResult<()> {
            WindowsPlatformAdapter.kill_process_tree(pid)
        }

        fn allocate_local_port(&self) -> PlatformResult<u16> {
            WindowsPlatformAdapter.allocate_local_port()
        }

        fn open_external(&self, url: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.open_external(url)
        }

        fn reveal_path(&self, _path: &Path) -> PlatformResult<()> {
            Ok(())
        }

        fn register_protocol_handler(&self, protocol: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.register_protocol_handler(protocol)
        }

        fn register_global_shortcut(&self, accelerator: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.register_global_shortcut(accelerator)
        }

        fn unregister_global_shortcut(&self, accelerator: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.unregister_global_shortcut(accelerator)
        }

        fn show_tray_or_menu_bar_item(&self) -> PlatformResult<()> {
            Ok(())
        }

        fn get_active_monitor_work_area(&self) -> PlatformResult<WorkArea> {
            WindowsPlatformAdapter.get_active_monitor_work_area()
        }

        fn secure_store_set(&self, key: &str, value: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.secure_store_set(key, value)
        }

        fn secure_store_get(&self, key: &str) -> PlatformResult<Option<String>> {
            WindowsPlatformAdapter.secure_store_get(key)
        }

        fn current_webview_profile(&self) -> WebviewProfile {
            WindowsPlatformAdapter.current_webview_profile()
        }
    }

    #[test]
    fn creates_workspace_layout() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_with_adapter("Example".to_string(), &adapter)
            .expect("workspace");

        assert!(manifest.paths.generated_static.exists());
        assert!(manifest.paths.root.join("sofvary.lock.json").exists());
        assert_eq!(manifest.name, "Example");

        let lockfile: SofvaryLockfile = serde_json::from_slice(
            &fs::read(manifest.paths.root.join("sofvary.lock.json")).unwrap(),
        )
        .expect("lockfile");
        assert_eq!(
            lockfile
                .runtime_packs
                .get(STATIC_HTML_RUNTIME_PACK_ID)
                .map(String::as_str),
            Some(STATIC_HTML_PACK_VERSION)
        );
        assert_eq!(
            lockfile
                .harness_packs
                .get(STATIC_HTML_HARNESS_PACK_ID)
                .map(String::as_str),
            Some(STATIC_HTML_PACK_VERSION)
        );
    }

    #[test]
    fn renames_workspace_manifest_without_moving_workspace_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_with_adapter("Original".to_string(), &adapter)
            .expect("workspace");
        let root = manifest.paths.root.clone();

        let renamed = manager
            .rename_workspace_with_adapter(
                manifest.app_id.clone(),
                "  排课助手  ".to_string(),
                &adapter,
            )
            .expect("rename");

        assert_eq!(renamed.name, "排课助手");
        assert_eq!(renamed.paths.root, root);
        let reloaded = manager
            .get_workspace_with_adapter(manifest.app_id, &adapter)
            .expect("reloaded");
        assert_eq!(reloaded.name, "排课助手");
        assert_eq!(reloaded.paths.root, root);
    }

    #[test]
    fn deletes_workspace_directory_after_manifest_validation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_with_adapter("Delete Me".to_string(), &adapter)
            .expect("workspace");
        let root = manifest.paths.root.clone();

        let deleted = manager
            .delete_workspace_with_adapter(manifest.app_id.clone(), &adapter)
            .expect("delete workspace");

        assert_eq!(deleted.app_id, manifest.app_id);
        assert!(!root.exists());
        assert!(manager
            .get_workspace_with_adapter(manifest.app_id, &adapter)
            .is_err());
    }

    #[test]
    fn rejects_workspace_app_id_path_segments() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();

        assert!(matches!(
            manager.workspace_root_with_adapter("", &adapter),
            Err(WorkspaceError::InvalidManifest(_))
        ));
        assert!(matches!(
            manager.workspace_root_with_adapter("../escape", &adapter),
            Err(WorkspaceError::InvalidManifest(_))
        ));
    }

    #[test]
    fn explicit_pack_lockfile_update_only_changes_target_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let first = manager
            .create_workspace_with_adapter("First".to_string(), &adapter)
            .expect("first workspace");
        let second = manager
            .create_workspace_with_adapter("Second".to_string(), &adapter)
            .expect("second workspace");

        let updated = manager
            .update_lockfile_pack_with_adapter(
                first.app_id.clone(),
                "runtime",
                "sofvary.runtime.remote-test".to_string(),
                "0.2.0".to_string(),
                &adapter,
            )
            .expect("update lockfile");

        assert_eq!(
            updated
                .runtime_packs
                .get("sofvary.runtime.remote-test")
                .map(String::as_str),
            Some("0.2.0")
        );
        assert_eq!(updated.runtime_packs.len(), 1);
        assert!(!updated
            .runtime_packs
            .contains_key(STATIC_HTML_RUNTIME_PACK_ID));
        let second_lockfile = manager
            .read_lockfile_for_manifest(&second)
            .expect("second lockfile");
        assert!(!second_lockfile
            .runtime_packs
            .contains_key("sofvary.runtime.remote-test"));
    }

    #[test]
    fn lockfile_update_policy_requires_exact_workspace_subject() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let workspace = manager
            .create_workspace_with_adapter("Policy Target".to_string(), &adapter)
            .expect("workspace");

        let missing_approval = manager.update_lockfile_pack_with_policy(
            workspace.app_id.clone(),
            "runtime",
            "sofvary.runtime.remote-test".to_string(),
            "0.2.0".to_string(),
            &adapter,
            &PolicyApprovalSet::default(),
        );
        assert!(matches!(
            missing_approval,
            Err(WorkspaceError::Policy(
                PolicyError::RequiresConfirmation { .. }
            ))
        ));

        let approvals = PolicyApprovalSet {
            approved: vec![PolicyApprovalGrant {
                action: PolicyActionKind::WorkspaceLockfileUpdate,
                subject: Some(workspace_lockfile_update_subject(
                    &workspace.app_id,
                    "runtime",
                    "sofvary.runtime.remote-test",
                    "0.2.0",
                )),
            }],
        };
        let updated = manager
            .update_lockfile_pack_with_policy(
                workspace.app_id.clone(),
                "runtime",
                "sofvary.runtime.remote-test".to_string(),
                "0.2.0".to_string(),
                &adapter,
                &approvals,
            )
            .expect("approved update");

        assert_eq!(updated.runtime_packs.len(), 1);
        assert_eq!(
            updated
                .runtime_packs
                .get("sofvary.runtime.remote-test")
                .map(String::as_str),
            Some("0.2.0")
        );
    }

    #[test]
    fn harness_lockfile_update_replaces_existing_harness_pack() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let workspace = manager
            .create_workspace_with_adapter("Harness Target".to_string(), &adapter)
            .expect("workspace");

        let updated = manager
            .update_lockfile_pack_with_adapter(
                workspace.app_id,
                "harness",
                "sofvary.harness.remote-test".to_string(),
                "0.2.0".to_string(),
                &adapter,
            )
            .expect("update harness lockfile");

        assert_eq!(updated.harness_packs.len(), 1);
        assert_eq!(
            updated
                .harness_packs
                .get("sofvary.harness.remote-test")
                .map(String::as_str),
            Some("0.2.0")
        );
        assert!(!updated
            .harness_packs
            .contains_key(STATIC_HTML_HARNESS_PACK_ID));
    }

    #[test]
    fn creates_react_vite_workspace_with_react_pack_lockfile() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "React Board".to_string(),
                RuntimeKind::ReactVite,
                &adapter,
            )
            .expect("workspace");

        assert_eq!(manifest.mode, RuntimeKind::ReactVite);
        assert!(manifest.paths.generated.join("react").exists());
        assert_eq!(manifest.stack, ["React", "TypeScript", "Vite"]);

        let lockfile: SofvaryLockfile = serde_json::from_slice(
            &fs::read(manifest.paths.root.join("sofvary.lock.json")).unwrap(),
        )
        .expect("lockfile");
        assert_eq!(
            lockfile
                .runtime_packs
                .get(REACT_VITE_RUNTIME_PACK_ID)
                .map(String::as_str),
            Some(REACT_VITE_PACK_VERSION)
        );
        assert_eq!(
            lockfile
                .harness_packs
                .get(REACT_VITE_HARNESS_PACK_ID)
                .map(String::as_str),
            Some(REACT_VITE_PACK_VERSION)
        );
        assert!(!lockfile
            .runtime_packs
            .contains_key(STATIC_HTML_RUNTIME_PACK_ID));
    }

    #[test]
    fn creates_react_sqlite_workspace_with_react_sqlite_pack_lockfile() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "Customer Manager".to_string(),
                RuntimeKind::ReactSqlite,
                &adapter,
            )
            .expect("workspace");

        assert_eq!(manifest.mode, RuntimeKind::ReactSqlite);
        assert!(manifest.paths.generated.join("react").exists());
        assert!(manifest.paths.generated.join("data").exists());
        assert_eq!(
            manifest.stack,
            ["React", "TypeScript", "Vite", "Node local API", "SQLite"]
        );

        let lockfile: SofvaryLockfile = serde_json::from_slice(
            &fs::read(manifest.paths.root.join("sofvary.lock.json")).unwrap(),
        )
        .expect("lockfile");
        assert_eq!(
            lockfile
                .runtime_packs
                .get(REACT_SQLITE_RUNTIME_PACK_ID)
                .map(String::as_str),
            Some(REACT_SQLITE_PACK_VERSION)
        );
        assert_eq!(
            lockfile
                .harness_packs
                .get(REACT_SQLITE_HARNESS_PACK_ID)
                .map(String::as_str),
            Some(REACT_SQLITE_PACK_VERSION)
        );
        assert!(!lockfile
            .runtime_packs
            .contains_key(REACT_VITE_RUNTIME_PACK_ID));
    }

    #[test]
    fn creates_canvas2d_workspace_with_canvas2d_pack_lockfile() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "Canvas Game".to_string(),
                RuntimeKind::Canvas2d,
                &adapter,
            )
            .expect("workspace");

        assert_eq!(manifest.mode, RuntimeKind::Canvas2d);
        assert!(manifest.paths.generated.join("canvas").exists());
        assert_eq!(manifest.stack, ["Canvas 2D", "JavaScript"]);

        let lockfile: SofvaryLockfile = serde_json::from_slice(
            &fs::read(manifest.paths.root.join("sofvary.lock.json")).unwrap(),
        )
        .expect("lockfile");
        assert_eq!(
            lockfile
                .runtime_packs
                .get(CANVAS2D_RUNTIME_PACK_ID)
                .map(String::as_str),
            Some(CANVAS2D_PACK_VERSION)
        );
        assert_eq!(
            lockfile
                .harness_packs
                .get(CANVAS2D_HARNESS_PACK_ID)
                .map(String::as_str),
            Some(CANVAS2D_PACK_VERSION)
        );
        assert!(!lockfile
            .runtime_packs
            .contains_key(STATIC_HTML_RUNTIME_PACK_ID));
    }

    #[test]
    fn creates_phase12_to15_workspaces_with_exact_pack_lockfiles() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();

        for (name, runtime, runtime_pack, harness_pack, version, required_dirs) in [
            (
                "Markdown Knowledge",
                RuntimeKind::MarkdownKnowledge,
                MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID,
                MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID,
                MARKDOWN_KNOWLEDGE_PACK_VERSION,
                &["markdown", "react"][..],
            ),
            (
                "Data Table",
                RuntimeKind::DataTable,
                DATA_TABLE_RUNTIME_PACK_ID,
                DATA_TABLE_HARNESS_PACK_ID,
                DATA_TABLE_PACK_VERSION,
                &["data", "react"][..],
            ),
            (
                "File Processor",
                RuntimeKind::FileProcessor,
                FILE_PROCESSOR_RUNTIME_PACK_ID,
                FILE_PROCESSOR_HARNESS_PACK_ID,
                FILE_PROCESSOR_PACK_VERSION,
                &["file-processor", "react"][..],
            ),
            (
                "Desktop Widget",
                RuntimeKind::DesktopWidget,
                DESKTOP_WIDGET_RUNTIME_PACK_ID,
                DESKTOP_WIDGET_HARNESS_PACK_ID,
                DESKTOP_WIDGET_PACK_VERSION,
                &["widget", "react"][..],
            ),
            (
                "AI Agent App",
                RuntimeKind::AiAgentApp,
                AI_AGENT_APP_RUNTIME_PACK_ID,
                AI_AGENT_APP_HARNESS_PACK_ID,
                AI_AGENT_APP_PACK_VERSION,
                &["ai", "react"][..],
            ),
        ] {
            let manifest = manager
                .create_workspace_for_runtime_with_adapter(name.to_string(), runtime, &adapter)
                .expect("workspace");
            assert_eq!(manifest.mode, runtime);
            for dir in required_dirs {
                assert!(manifest.paths.generated.join(dir).exists());
            }

            let lockfile: SofvaryLockfile = serde_json::from_slice(
                &fs::read(manifest.paths.root.join("sofvary.lock.json")).unwrap(),
            )
            .expect("lockfile");
            assert_eq!(
                lockfile.runtime_packs.get(runtime_pack).map(String::as_str),
                Some(version)
            );
            assert_eq!(
                lockfile.harness_packs.get(harness_pack).map(String::as_str),
                Some(version)
            );
            assert_eq!(lockfile.runtime_packs.len(), 1);
            assert_eq!(lockfile.harness_packs.len(), 1);
            assert!(lockfile.plugin_packs.is_empty());
        }
    }

    #[test]
    fn prevents_path_traversal() {
        let manager = WorkspaceManager::new();
        let root = PathBuf::from("/tmp/sofvary/apps");
        let result = manager.ensure_child(&root, Path::new("../outside"));
        assert!(matches!(result, Err(WorkspaceError::PathEscape(_))));
    }

    #[test]
    fn creates_and_rolls_back_snapshot() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_with_adapter("Snapshot".to_string(), &adapter)
            .expect("workspace");
        fs::write(manifest.paths.generated_static.join("index.html"), "v1").expect("write");
        let snapshot = manager
            .create_snapshot_with_adapter(manifest.app_id.clone(), &adapter)
            .expect("snapshot");
        fs::write(manifest.paths.generated_static.join("index.html"), "v2").expect("write");
        manager
            .rollback_snapshot_with_adapter(manifest.app_id.clone(), snapshot.snapshot_id, &adapter)
            .expect("rollback");

        let restored =
            fs::read_to_string(manifest.paths.generated_static.join("index.html")).expect("read");
        assert_eq!(restored, "v1");
    }

    #[test]
    fn rejects_manifest_paths_that_escape_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let mut manifest = manager
            .create_workspace_with_adapter("Tamper".to_string(), &adapter)
            .expect("workspace");
        manifest.paths.generated = temp.path().join("outside");
        manager
            .write_json(&manifest.paths.root.join("app.box.json"), &manifest)
            .expect("write tampered manifest");

        let result = manager.get_workspace_with_adapter(manifest.app_id.clone(), &adapter);

        assert!(matches!(
            result,
            Err(WorkspaceError::PathEscape(_)) | Err(WorkspaceError::InvalidManifest(_))
        ));
    }

    #[test]
    fn rollback_rejects_tampered_generated_path_before_deleting() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let mut manifest = manager
            .create_workspace_with_adapter("Rollback Tamper".to_string(), &adapter)
            .expect("workspace");
        fs::write(manifest.paths.generated_static.join("index.html"), "v1").expect("write");
        let snapshot = manager
            .create_snapshot_with_adapter(manifest.app_id.clone(), &adapter)
            .expect("snapshot");
        let outside = temp.path().join("outside-generated");
        fs::create_dir_all(&outside).expect("outside dir");
        fs::write(outside.join("keep.txt"), "keep").expect("outside file");

        manifest.paths.generated = outside.clone();
        manager
            .write_json(&manifest.paths.root.join("app.box.json"), &manifest)
            .expect("write tampered manifest");
        let result = manager.rollback_snapshot_with_adapter(
            manifest.app_id.clone(),
            snapshot.snapshot_id,
            &adapter,
        );

        assert!(matches!(
            result,
            Err(WorkspaceError::PathEscape(_)) | Err(WorkspaceError::InvalidManifest(_))
        ));
        assert_eq!(
            fs::read_to_string(outside.join("keep.txt")).expect("outside untouched"),
            "keep"
        );
    }

    #[test]
    fn replaces_generated_static_files_and_removes_stale_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_with_adapter("Static Replace".to_string(), &adapter)
            .expect("workspace");
        fs::write(manifest.paths.generated_static.join("extra.html"), "stale").expect("stale");

        let written = manager
            .replace_generated_static_files(
                &manifest,
                &static_test_files(),
                &[
                    "index.html".to_string(),
                    "style.css".to_string(),
                    "app.js".to_string(),
                ],
            )
            .expect("replace static");

        assert_eq!(written.len(), 3);
        assert!(manifest.paths.generated_static.join("index.html").exists());
        assert!(!manifest.paths.generated_static.join("extra.html").exists());
    }

    #[test]
    fn rejects_generated_static_file_escape_and_extra_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_with_adapter("Static Escape".to_string(), &adapter)
            .expect("workspace");

        let escape = manager.replace_generated_static_files(
            &manifest,
            &[GeneratedStaticFile {
                relative_path: "../secret.txt".to_string(),
                contents: "secret".to_string(),
            }],
            &["../secret.txt".to_string()],
        );
        assert!(matches!(
            escape,
            Err(WorkspaceError::PathEscape(_)) | Err(WorkspaceError::InvalidManifest(_))
        ));

        let mut extra_files = static_test_files();
        extra_files.push(GeneratedStaticFile {
            relative_path: "extra.html".to_string(),
            contents: "extra".to_string(),
        });
        let extra = manager.replace_generated_static_files(
            &manifest,
            &extra_files,
            &[
                "index.html".to_string(),
                "style.css".to_string(),
                "app.js".to_string(),
            ],
        );
        assert!(matches!(extra, Err(WorkspaceError::InvalidManifest(_))));
        assert!(!manifest.paths.generated_static.join("extra.html").exists());
    }

    #[test]
    fn replaces_generated_react_files_and_removes_stale_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "React Replace".to_string(),
                RuntimeKind::ReactVite,
                &adapter,
            )
            .expect("workspace");
        let react_root = manifest.paths.generated.join("react");
        fs::write(react_root.join("stale.txt"), "stale").expect("stale");

        let written = manager
            .replace_generated_react_files(&manifest, &react_test_files(), &react_allowed_files())
            .expect("replace react");

        assert_eq!(written.len(), react_allowed_files().len());
        assert!(react_root.join("src/components/TaskBoard.tsx").exists());
        assert!(!react_root.join("stale.txt").exists());
    }

    #[test]
    fn rejects_generated_react_file_escape_backslash_extra_and_missing_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "React Reject".to_string(),
                RuntimeKind::ReactVite,
                &adapter,
            )
            .expect("workspace");

        let escape = manager.replace_generated_react_files(
            &manifest,
            &[GeneratedReactFile {
                relative_path: "../package.json".to_string(),
                contents: "escape".to_string(),
            }],
            &["../package.json".to_string()],
        );
        assert!(matches!(
            escape,
            Err(WorkspaceError::PathEscape(_)) | Err(WorkspaceError::InvalidManifest(_))
        ));

        let backslash = manager.replace_generated_react_files(
            &manifest,
            &[GeneratedReactFile {
                relative_path: "src\\App.tsx".to_string(),
                contents: "bad".to_string(),
            }],
            &["src\\App.tsx".to_string()],
        );
        assert!(matches!(backslash, Err(WorkspaceError::InvalidManifest(_))));

        let mut extra_files = react_test_files();
        extra_files.push(GeneratedReactFile {
            relative_path: "src/extra.ts".to_string(),
            contents: "extra".to_string(),
        });
        let extra =
            manager.replace_generated_react_files(&manifest, &extra_files, &react_allowed_files());
        assert!(matches!(extra, Err(WorkspaceError::InvalidManifest(_))));

        let mut missing_files = react_test_files();
        missing_files.pop();
        let missing = manager.replace_generated_react_files(
            &manifest,
            &missing_files,
            &react_allowed_files(),
        );
        assert!(matches!(missing, Err(WorkspaceError::InvalidManifest(_))));
    }

    #[test]
    fn replaces_generated_react_sqlite_files_and_removes_stale_react_and_data_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "SQLite Replace".to_string(),
                RuntimeKind::ReactSqlite,
                &adapter,
            )
            .expect("workspace");
        fs::write(
            manifest.paths.generated.join("react").join("stale.txt"),
            "stale",
        )
        .expect("react stale");
        fs::write(
            manifest.paths.generated.join("data").join("app.sqlite"),
            "stale",
        )
        .expect("data stale");

        let written = manager
            .replace_generated_react_sqlite_files(
                &manifest,
                &react_sqlite_test_files(),
                &react_sqlite_allowed_files(),
            )
            .expect("replace react sqlite");

        assert_eq!(written.len(), react_sqlite_allowed_files().len());
        assert!(manifest
            .paths
            .generated
            .join("react/src/components/CustomerManager.tsx")
            .exists());
        assert!(manifest
            .paths
            .generated
            .join("data/migrations/001_create_customers.sql")
            .exists());
        assert!(!manifest.paths.generated.join("react/stale.txt").exists());
        assert!(manifest.paths.generated.join("data/app.sqlite").exists());
        assert_eq!(
            fs::read_to_string(manifest.paths.generated.join("data/app.sqlite")).expect("sqlite"),
            "stale"
        );
    }

    #[test]
    fn writes_generated_file_delta_without_clearing_react_sqlite_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "SQLite Live Delta".to_string(),
                RuntimeKind::ReactSqlite,
                &adapter,
            )
            .expect("workspace");
        fs::write(
            manifest.paths.generated.join("data").join("app.sqlite"),
            "runtime-db",
        )
        .expect("sqlite");

        let target = manager
            .write_generated_file_delta(
                &manifest,
                "react-sqlite",
                "react/src/App.tsx",
                "export function App() { return null; }\n",
                &react_sqlite_allowed_files(),
            )
            .expect("delta");

        assert!(target.ends_with("generated/react/src/App.tsx"));
        assert_eq!(
            fs::read_to_string(&target).expect("app"),
            "export function App() { return null; }\n"
        );
        assert_eq!(
            fs::read_to_string(manifest.paths.generated.join("data/app.sqlite")).expect("sqlite"),
            "runtime-db"
        );

        let extra = manager.write_generated_file_delta(
            &manifest,
            "react-sqlite",
            "react/src/extra.ts",
            "extra",
            &react_sqlite_allowed_files(),
        );
        assert!(matches!(extra, Err(WorkspaceError::InvalidManifest(_))));
    }

    #[test]
    fn normalizes_react_sqlite_package_json_to_managed_dependency_versions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "SQLite Package Normalize".to_string(),
                RuntimeKind::ReactSqlite,
                &adapter,
            )
            .expect("workspace");

        manager
            .replace_generated_react_sqlite_files(
                &manifest,
                &react_sqlite_test_files(),
                &react_sqlite_allowed_files(),
            )
            .expect("replace react sqlite");

        let package_json =
            fs::read_to_string(manifest.paths.generated.join("react").join("package.json"))
                .expect("package");
        let package: serde_json::Value = serde_json::from_str(&package_json).expect("package json");

        assert_eq!(package["dependencies"]["vite"], "7.3.3");
        assert_eq!(package["dependencies"]["@vitejs/plugin-react"], "5.2.0");
        assert_eq!(package["dependencies"]["react"], "19.2.6");
        assert_eq!(package["dependencies"]["express"], "5.2.1");
        assert_eq!(package["dependencies"]["cors"], "2.8.6");
        assert_eq!(package["dependencies"]["sql.js"], "1.14.1");
        assert_eq!(package["dependencies"]["@types/node"], "24.12.4");
        assert_eq!(package["dependencies"]["@types/express"], "5.0.6");
        assert_eq!(package["dependencies"]["@types/cors"], "2.8.19");
        assert!(package["dependencies"]["better-sqlite3"].is_null());
        assert_eq!(package["devDependencies"].as_object().unwrap().len(), 0);
        assert_eq!(package["scripts"]["api"], "tsx server/index.ts");
    }

    #[test]
    fn normalizes_existing_react_sqlite_workspace_package_json_before_preview() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "SQLite Existing Package Normalize".to_string(),
                RuntimeKind::ReactSqlite,
                &adapter,
            )
            .expect("workspace");
        let package_path = manifest.paths.generated.join("react/package.json");
        fs::create_dir_all(package_path.parent().expect("package parent")).expect("parent");
        fs::write(
            &package_path,
            r#"{"name":"old","dependencies":{"vite":"0.0.1","better-sqlite3":"12.0.0"}}"#,
        )
        .expect("old package");

        manager
            .normalize_react_sqlite_workspace_package(&manifest)
            .expect("normalize existing package");

        let package: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(package_path).expect("package"))
                .expect("package json");
        assert_eq!(package["dependencies"]["vite"], "7.3.3");
        assert_eq!(package["dependencies"]["express"], "5.2.1");
        assert_eq!(package["dependencies"]["cors"], "2.8.6");
        assert_eq!(package["dependencies"]["sql.js"], "1.14.1");
        assert!(package["dependencies"]["better-sqlite3"].is_null());
    }

    #[test]
    fn preview_preparation_normalizes_hardcoded_react_sqlite_vite_proxy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "SQLite Existing Vite Normalize".to_string(),
                RuntimeKind::ReactSqlite,
                &adapter,
            )
            .expect("workspace");
        let react_root = manifest.paths.generated.join("react");
        fs::create_dir_all(&react_root).expect("react");
        fs::write(
            react_root.join("package.json"),
            r#"{"name":"old","dependencies":{"vite":"0.0.1"}}"#,
        )
        .expect("package");
        fs::write(
            react_root.join("vite.config.ts"),
            r#"import { defineConfig } from 'vite';

export default defineConfig({
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:4177'
    }
  }
});
"#,
        )
        .expect("vite config");

        manager
            .prepare_react_sqlite_workspace_for_preview(&manifest)
            .expect("prepare preview");

        let vite_config =
            fs::read_to_string(react_root.join("vite.config.ts")).expect("vite config");
        assert!(vite_config.contains("SOFVARY_API_PORT"));
        assert!(vite_config.contains("`http://127.0.0.1:${sofvaryApiPort}`"));
        assert!(!vite_config.contains("http://127.0.0.1:4177"));
    }

    #[test]
    fn rejects_generated_react_sqlite_escape_backslash_extra_missing_and_wrong_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "SQLite Reject".to_string(),
                RuntimeKind::ReactSqlite,
                &adapter,
            )
            .expect("workspace");

        let escape = manager.replace_generated_react_sqlite_files(
            &manifest,
            &[GeneratedReactSqliteFile {
                relative_path: "../data/seed.sql".to_string(),
                contents: "escape".to_string(),
            }],
            &["../data/seed.sql".to_string()],
        );
        assert!(matches!(
            escape,
            Err(WorkspaceError::PathEscape(_)) | Err(WorkspaceError::InvalidManifest(_))
        ));

        let backslash = manager.replace_generated_react_sqlite_files(
            &manifest,
            &[GeneratedReactSqliteFile {
                relative_path: "data\\seed.sql".to_string(),
                contents: "bad".to_string(),
            }],
            &["data\\seed.sql".to_string()],
        );
        assert!(matches!(backslash, Err(WorkspaceError::InvalidManifest(_))));

        let wrong_root = manager.replace_generated_react_sqlite_files(
            &manifest,
            &[GeneratedReactSqliteFile {
                relative_path: "runtime/log.txt".to_string(),
                contents: "bad".to_string(),
            }],
            &["runtime/log.txt".to_string()],
        );
        assert!(matches!(
            wrong_root,
            Err(WorkspaceError::InvalidManifest(_))
        ));

        let mut extra_files = react_sqlite_test_files();
        extra_files.push(GeneratedReactSqliteFile {
            relative_path: "data/extra.sql".to_string(),
            contents: "extra".to_string(),
        });
        let extra = manager.replace_generated_react_sqlite_files(
            &manifest,
            &extra_files,
            &react_sqlite_allowed_files(),
        );
        assert!(matches!(extra, Err(WorkspaceError::InvalidManifest(_))));

        let mut missing_files = react_sqlite_test_files();
        missing_files.pop();
        let missing = manager.replace_generated_react_sqlite_files(
            &manifest,
            &missing_files,
            &react_sqlite_allowed_files(),
        );
        assert!(matches!(missing, Err(WorkspaceError::InvalidManifest(_))));
    }

    #[test]
    fn replaces_generated_canvas2d_files_and_removes_stale_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "Canvas Replace".to_string(),
                RuntimeKind::Canvas2d,
                &adapter,
            )
            .expect("workspace");
        let canvas_root = manifest.paths.generated.join("canvas");
        fs::write(canvas_root.join("stale.js"), "stale").expect("stale");

        let written = manager
            .replace_generated_canvas2d_files(
                &manifest,
                &canvas2d_test_files(),
                &canvas2d_allowed_files(),
            )
            .expect("replace canvas2d");

        assert_eq!(written.len(), canvas2d_allowed_files().len());
        assert!(canvas_root.join("index.html").exists());
        assert!(canvas_root.join("src/engine/loop.js").exists());
        assert!(canvas_root.join("src/game/levels.js").exists());
        assert!(canvas_root.join("assets").is_dir());
        assert!(!canvas_root.join("stale.js").exists());
    }

    #[test]
    fn rejects_generated_canvas2d_escape_backslash_extra_and_missing_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "Canvas Reject".to_string(),
                RuntimeKind::Canvas2d,
                &adapter,
            )
            .expect("workspace");

        let escape = manager.replace_generated_canvas2d_files(
            &manifest,
            &[GeneratedCanvas2dFile {
                relative_path: "../index.html".to_string(),
                contents: "escape".to_string(),
            }],
            &["../index.html".to_string()],
        );
        assert!(matches!(
            escape,
            Err(WorkspaceError::PathEscape(_)) | Err(WorkspaceError::InvalidManifest(_))
        ));

        let backslash = manager.replace_generated_canvas2d_files(
            &manifest,
            &[GeneratedCanvas2dFile {
                relative_path: "src\\main.js".to_string(),
                contents: "bad".to_string(),
            }],
            &["src\\main.js".to_string()],
        );
        assert!(matches!(backslash, Err(WorkspaceError::InvalidManifest(_))));

        let mut extra_files = canvas2d_test_files();
        extra_files.push(GeneratedCanvas2dFile {
            relative_path: "src/extra.js".to_string(),
            contents: "extra".to_string(),
        });
        let extra = manager.replace_generated_canvas2d_files(
            &manifest,
            &extra_files,
            &canvas2d_allowed_files(),
        );
        assert!(matches!(extra, Err(WorkspaceError::InvalidManifest(_))));

        let mut missing_files = canvas2d_test_files();
        missing_files.pop();
        let missing = manager.replace_generated_canvas2d_files(
            &manifest,
            &missing_files,
            &canvas2d_allowed_files(),
        );
        assert!(matches!(missing, Err(WorkspaceError::InvalidManifest(_))));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_generated_project_top_level_symlink_escape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "Markdown Symlink".to_string(),
                RuntimeKind::MarkdownKnowledge,
                &adapter,
            )
            .expect("workspace");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).expect("outside");
        fs::write(outside.join("sentinel.txt"), "keep").expect("sentinel");
        fs::remove_dir_all(manifest.paths.generated.join("react")).expect("remove react");
        std::os::unix::fs::symlink(&outside, manifest.paths.generated.join("react"))
            .expect("react symlink");

        let result = manager.replace_generated_markdown_knowledge_files(
            &manifest,
            &markdown_knowledge_project_files(),
            &markdown_knowledge_allowed_files(),
        );

        assert!(matches!(result, Err(WorkspaceError::PathEscape(_))));
        assert_eq!(
            fs::read_to_string(outside.join("sentinel.txt")).expect("sentinel"),
            "keep"
        );
        assert!(!outside.join("package.json").exists());
    }

    fn static_test_files() -> Vec<GeneratedStaticFile> {
        vec![
            GeneratedStaticFile {
                relative_path: "index.html".to_string(),
                contents: "index".to_string(),
            },
            GeneratedStaticFile {
                relative_path: "style.css".to_string(),
                contents: "style".to_string(),
            },
            GeneratedStaticFile {
                relative_path: "app.js".to_string(),
                contents: "script".to_string(),
            },
        ]
    }

    fn react_allowed_files() -> Vec<String> {
        [
            "package.json",
            "index.html",
            "vite.config.ts",
            "tsconfig.json",
            "src/main.tsx",
            "src/App.tsx",
            "src/components/TaskBoard.tsx",
            "src/styles/app.css",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect()
    }

    fn react_test_files() -> Vec<GeneratedReactFile> {
        react_allowed_files()
            .into_iter()
            .map(|relative_path| GeneratedReactFile {
                contents: format!("file: {relative_path}"),
                relative_path,
            })
            .collect()
    }

    fn react_sqlite_allowed_files() -> Vec<String> {
        [
            "react/package.json",
            "react/index.html",
            "react/vite.config.ts",
            "react/tsconfig.json",
            "react/src/main.tsx",
            "react/src/App.tsx",
            "react/src/components/CustomerManager.tsx",
            "react/src/styles/app.css",
            "react/server/index.ts",
            "react/server/db.ts",
            "react/server/routes/customers.ts",
            "data/schema.json",
            "data/migrations/001_create_customers.sql",
            "data/seed.sql",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect()
    }

    fn react_sqlite_test_files() -> Vec<GeneratedReactSqliteFile> {
        react_sqlite_allowed_files()
            .into_iter()
            .map(|relative_path| GeneratedReactSqliteFile {
                contents: if relative_path == "react/package.json" {
                    react_sqlite_test_package_json()
                } else {
                    format!("file: {relative_path}")
                },
                relative_path,
            })
            .collect()
    }

    fn react_sqlite_test_package_json() -> String {
        r#"{
  "scripts": {
    "dev": "vite --host 127.0.0.1"
  },
  "dependencies": {
    "@vitejs/plugin-react": "latest",
    "vite": "latest",
    "typescript": "latest",
    "react": "latest",
    "react-dom": "latest",
    "tsx": "latest"
  },
  "devDependencies": {
    "unused": "latest"
  }
}"#
        .to_string()
    }

    fn canvas2d_allowed_files() -> Vec<String> {
        [
            "index.html",
            "style.css",
            "src/main.js",
            "src/engine/loop.js",
            "src/engine/input.js",
            "src/engine/scene.js",
            "src/engine/collision.js",
            "src/engine/assets.js",
            "src/game/config.js",
            "src/game/player.js",
            "src/game/enemies.js",
            "src/game/levels.js",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect()
    }

    fn canvas2d_test_files() -> Vec<GeneratedCanvas2dFile> {
        canvas2d_allowed_files()
            .into_iter()
            .map(|relative_path| GeneratedCanvas2dFile {
                contents: format!("file: {relative_path}"),
                relative_path,
            })
            .collect()
    }

    fn markdown_knowledge_allowed_files() -> Vec<String> {
        [
            "markdown/index.json",
            "markdown/content/getting-started.md",
            "react/package.json",
            "react/index.html",
            "react/vite.config.ts",
            "react/tsconfig.json",
            "react/src/main.tsx",
            "react/src/App.tsx",
            "react/src/components/MarkdownKnowledgeApp.tsx",
            "react/src/styles/app.css",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect()
    }

    fn markdown_knowledge_project_files() -> Vec<GeneratedProjectFile> {
        markdown_knowledge_allowed_files()
            .into_iter()
            .map(|relative_path| GeneratedProjectFile {
                contents: format!("file: {relative_path}"),
                relative_path,
            })
            .collect()
    }
}
