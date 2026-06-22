use crate::core::runtime_manager::{RuntimePreview, RuntimePreviewIssue};
use crate::core::workspace_types::{RuntimeKind, RuntimeMode};
use crate::platform::{current_adapter, PlatformAdapter, PlatformError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum BuildThreadError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("build thread not found: {0}")]
    NotFound(String),
    #[error("build thread is invalid: {0}")]
    Invalid(String),
}

pub type BuildThreadResult<T> = Result<T, BuildThreadError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuildThreadStatus {
    Queued,
    Planning,
    Building,
    Repairing,
    Previewing,
    PreviewBlocked,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuildThreadEntryKind {
    User,
    Assistant,
    AgentEvent,
    Tool,
    File,
    System,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildThreadSummary {
    pub id: String,
    pub title: String,
    pub status: BuildThreadStatus,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
    pub runtime_kind: RuntimeKind,
    pub runtime_mode: RuntimeMode,
    pub agent_id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub preview: Option<RuntimePreview>,
    #[serde(default)]
    pub preview_issue: Option<RuntimePreviewIssue>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildThreadEntry {
    pub id: String,
    pub thread_id: String,
    pub timestamp: String,
    pub kind: BuildThreadEntryKind,
    pub content: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildThreadDetail {
    pub summary: BuildThreadSummary,
    #[serde(default)]
    pub entries: Vec<BuildThreadEntry>,
}

#[derive(Debug, Clone)]
pub struct CreateBuildThreadRequest {
    pub title: String,
    pub prompt: String,
    pub runtime_kind: RuntimeKind,
    pub runtime_mode: RuntimeMode,
    pub agent_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct BuildThreadUpdate {
    pub status: Option<BuildThreadStatus>,
    pub workspace_id: Option<Option<String>>,
    pub app_id: Option<Option<String>>,
    pub preview: Option<Option<RuntimePreview>>,
    pub preview_issue: Option<Option<RuntimePreviewIssue>>,
    pub error: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BuildThreadStore;

impl BuildThreadStore {
    pub fn new() -> Self {
        Self
    }

    pub fn create(
        &self,
        request: CreateBuildThreadRequest,
    ) -> BuildThreadResult<BuildThreadDetail> {
        let adapter = current_adapter();
        self.create_with_adapter(adapter.as_ref(), request)
    }

    pub fn create_with_adapter(
        &self,
        adapter: &dyn PlatformAdapter,
        request: CreateBuildThreadRequest,
    ) -> BuildThreadResult<BuildThreadDetail> {
        let now = Utc::now().to_rfc3339();
        let id = format!("build_{}", Uuid::new_v4());
        let summary = BuildThreadSummary {
            id: id.clone(),
            title: request.title,
            status: BuildThreadStatus::Queued,
            workspace_id: None,
            app_id: None,
            runtime_kind: request.runtime_kind,
            runtime_mode: request.runtime_mode,
            agent_id: request.agent_id,
            created_at: now.clone(),
            updated_at: now.clone(),
            preview: None,
            preview_issue: None,
            error: None,
        };
        let entry = self.entry(
            &id,
            BuildThreadEntryKind::User,
            request.prompt,
            serde_json::json!({ "source": "create" }),
        );
        let detail = BuildThreadDetail {
            summary,
            entries: vec![entry],
        };
        self.save_with_adapter(adapter, &detail)?;
        Ok(detail)
    }

    pub fn list(&self) -> BuildThreadResult<Vec<BuildThreadSummary>> {
        let adapter = current_adapter();
        self.list_with_adapter(adapter.as_ref())
    }

    pub fn list_with_adapter(
        &self,
        adapter: &dyn PlatformAdapter,
    ) -> BuildThreadResult<Vec<BuildThreadSummary>> {
        let dir = threads_dir(adapter)?;
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut summaries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(detail) = read_detail(&path) else {
                continue;
            };
            summaries.push(detail.summary);
        }
        summaries.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(summaries)
    }

    pub fn get(&self, thread_id: &str) -> BuildThreadResult<BuildThreadDetail> {
        let adapter = current_adapter();
        let path = thread_path(adapter.as_ref(), thread_id)?;
        if !path.exists() {
            return Err(BuildThreadError::NotFound(thread_id.to_string()));
        }
        read_detail(&path)
    }

    pub fn delete(&self, thread_id: &str) -> BuildThreadResult<()> {
        let adapter = current_adapter();
        self.delete_with_adapter(adapter.as_ref(), thread_id)
    }

    pub fn delete_with_adapter(
        &self,
        adapter: &dyn PlatformAdapter,
        thread_id: &str,
    ) -> BuildThreadResult<()> {
        let path = thread_path(adapter, thread_id)?;
        if !path.exists() {
            return Err(BuildThreadError::NotFound(thread_id.to_string()));
        }
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn append_entry(
        &self,
        thread_id: &str,
        kind: BuildThreadEntryKind,
        content: impl Into<String>,
        metadata: Value,
    ) -> BuildThreadResult<BuildThreadEntry> {
        let mut detail = self.get(thread_id)?;
        let entry = self.entry(thread_id, kind, content, metadata);
        detail.summary.updated_at = entry.timestamp.clone();
        detail.entries.push(entry.clone());
        self.save(&detail)?;
        Ok(entry)
    }

    pub fn update(
        &self,
        thread_id: &str,
        update: BuildThreadUpdate,
    ) -> BuildThreadResult<BuildThreadSummary> {
        let adapter = current_adapter();
        self.update_with_adapter(adapter.as_ref(), thread_id, update)
    }

    pub fn update_with_adapter(
        &self,
        adapter: &dyn PlatformAdapter,
        thread_id: &str,
        update: BuildThreadUpdate,
    ) -> BuildThreadResult<BuildThreadSummary> {
        let path = thread_path(adapter, thread_id)?;
        if !path.exists() {
            return Err(BuildThreadError::NotFound(thread_id.to_string()));
        }
        let mut detail = read_detail(&path)?;
        if let Some(status) = update.status {
            if detail.summary.status == BuildThreadStatus::Canceled
                && status != BuildThreadStatus::Canceled
            {
                return Err(BuildThreadError::Invalid(
                    "canceled build thread cannot move to another status".to_string(),
                ));
            }
            detail.summary.status = status;
        }
        if let Some(workspace_id) = update.workspace_id {
            detail.summary.workspace_id = workspace_id;
        }
        if let Some(app_id) = update.app_id {
            detail.summary.app_id = app_id;
        }
        if let Some(preview) = update.preview {
            detail.summary.preview = preview;
        }
        if let Some(preview_issue) = update.preview_issue {
            detail.summary.preview_issue = preview_issue;
        }
        if let Some(error) = update.error {
            detail.summary.error = error;
        }
        detail.summary.updated_at = Utc::now().to_rfc3339();
        self.save_with_adapter(adapter, &detail)?;
        Ok(detail.summary)
    }

    pub fn delete_for_app(&self, app_id: &str) -> BuildThreadResult<usize> {
        let adapter = current_adapter();
        self.delete_for_app_with_adapter(adapter.as_ref(), app_id)
    }

    pub fn delete_for_app_with_adapter(
        &self,
        adapter: &dyn PlatformAdapter,
        app_id: &str,
    ) -> BuildThreadResult<usize> {
        if app_id.trim().is_empty()
            || app_id.contains('/')
            || app_id.contains('\\')
            || app_id.contains("..")
        {
            return Err(BuildThreadError::Invalid(
                "app id must be a single path segment".to_string(),
            ));
        }

        let dir = threads_dir(adapter)?;
        if !dir.exists() {
            return Ok(0);
        }

        let mut removed = 0;
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(detail) = read_detail(&path) else {
                continue;
            };
            if detail.summary.app_id.as_deref() == Some(app_id)
                || detail.summary.workspace_id.as_deref() == Some(app_id)
            {
                fs::remove_file(path)?;
                removed += 1;
            }
        }

        Ok(removed)
    }

    pub fn save(&self, detail: &BuildThreadDetail) -> BuildThreadResult<()> {
        let adapter = current_adapter();
        self.save_with_adapter(adapter.as_ref(), detail)
    }

    pub fn save_with_adapter(
        &self,
        adapter: &dyn PlatformAdapter,
        detail: &BuildThreadDetail,
    ) -> BuildThreadResult<()> {
        let path = thread_path(adapter, &detail.summary.id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(detail)? + "\n")?;
        Ok(())
    }

    fn entry(
        &self,
        thread_id: &str,
        kind: BuildThreadEntryKind,
        content: impl Into<String>,
        metadata: Value,
    ) -> BuildThreadEntry {
        BuildThreadEntry {
            id: format!("entry_{}", Uuid::new_v4()),
            thread_id: thread_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            kind,
            content: content.into(),
            metadata,
        }
    }
}

pub fn build_thread_title(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return "Untitled build".to_string();
    }
    trimmed
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(80)
        .collect()
}

fn read_detail(path: &PathBuf) -> BuildThreadResult<BuildThreadDetail> {
    Ok(serde_json::from_slice::<BuildThreadDetail>(&fs::read(
        path,
    )?)?)
}

fn threads_dir(adapter: &dyn PlatformAdapter) -> BuildThreadResult<PathBuf> {
    Ok(adapter.dirs()?.data_dir.join("build-threads"))
}

fn thread_path(adapter: &dyn PlatformAdapter, thread_id: &str) -> BuildThreadResult<PathBuf> {
    if thread_id.trim().is_empty()
        || thread_id.contains('/')
        || thread_id.contains('\\')
        || thread_id.contains("..")
    {
        return Err(BuildThreadError::Invalid(
            "thread id must be a single path segment".to_string(),
        ));
    }
    Ok(threads_dir(adapter)?.join(format!("{thread_id}.json")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_thread_title_uses_prompt_prefix() {
        assert_eq!(
            build_thread_title("create a local countdown timer app"),
            "create a local countdown timer app"
        );
        assert_eq!(build_thread_title("  "), "Untitled build");
    }

    #[test]
    fn deletes_threads_for_app_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            data_dir: temp.path().join("data"),
        };
        let store = BuildThreadStore::new();
        let mut target = store
            .create_with_adapter(
                &adapter,
                CreateBuildThreadRequest {
                    title: "Target".to_string(),
                    prompt: "target".to_string(),
                    runtime_kind: RuntimeKind::StaticHtml,
                    runtime_mode: RuntimeMode::Dev,
                    agent_id: "agent".to_string(),
                },
            )
            .expect("target");
        target.summary.app_id = Some("app_target".to_string());
        store
            .save_with_adapter(&adapter, &target)
            .expect("save target");

        let other = store
            .create_with_adapter(
                &adapter,
                CreateBuildThreadRequest {
                    title: "Other".to_string(),
                    prompt: "other".to_string(),
                    runtime_kind: RuntimeKind::StaticHtml,
                    runtime_mode: RuntimeMode::Dev,
                    agent_id: "agent".to_string(),
                },
            )
            .expect("other");

        assert_eq!(
            store
                .delete_for_app_with_adapter(&adapter, "app_target")
                .expect("delete"),
            1
        );
        let remaining = store.list_with_adapter(&adapter).expect("remaining");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, other.summary.id);
    }

    #[test]
    fn deletes_single_thread_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            data_dir: temp.path().join("data"),
        };
        let store = BuildThreadStore::new();
        let target = store
            .create_with_adapter(
                &adapter,
                CreateBuildThreadRequest {
                    title: "Target".to_string(),
                    prompt: "target".to_string(),
                    runtime_kind: RuntimeKind::StaticHtml,
                    runtime_mode: RuntimeMode::Dev,
                    agent_id: "agent".to_string(),
                },
            )
            .expect("target");
        let other = store
            .create_with_adapter(
                &adapter,
                CreateBuildThreadRequest {
                    title: "Other".to_string(),
                    prompt: "other".to_string(),
                    runtime_kind: RuntimeKind::StaticHtml,
                    runtime_mode: RuntimeMode::Dev,
                    agent_id: "agent".to_string(),
                },
            )
            .expect("other");

        store
            .delete_with_adapter(&adapter, &target.summary.id)
            .expect("delete target");

        let remaining = store.list_with_adapter(&adapter).expect("remaining");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, other.summary.id);
        assert!(matches!(
            store.delete_with_adapter(&adapter, &target.summary.id),
            Err(BuildThreadError::NotFound(_))
        ));
    }

    #[test]
    fn canceled_thread_rejects_later_status_updates() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            data_dir: temp.path().join("data"),
        };
        let store = BuildThreadStore::new();
        let thread = store
            .create_with_adapter(
                &adapter,
                CreateBuildThreadRequest {
                    title: "Target".to_string(),
                    prompt: "target".to_string(),
                    runtime_kind: RuntimeKind::StaticHtml,
                    runtime_mode: RuntimeMode::Dev,
                    agent_id: "agent".to_string(),
                },
            )
            .expect("target");

        store
            .update_with_adapter(
                &adapter,
                &thread.summary.id,
                BuildThreadUpdate {
                    status: Some(BuildThreadStatus::Canceled),
                    ..BuildThreadUpdate::default()
                },
            )
            .expect("cancel");

        assert!(matches!(
            store.update_with_adapter(
                &adapter,
                &thread.summary.id,
                BuildThreadUpdate {
                    status: Some(BuildThreadStatus::Building),
                    ..BuildThreadUpdate::default()
                },
            ),
            Err(BuildThreadError::Invalid(_))
        ));

        let detail =
            read_detail(&thread_path(&adapter, &thread.summary.id).expect("path")).expect("detail");
        assert_eq!(detail.summary.status, BuildThreadStatus::Canceled);
    }

    struct TempAdapter {
        data_dir: PathBuf,
    }

    impl PlatformAdapter for TempAdapter {
        fn os(&self) -> crate::platform::OsKind {
            crate::platform::OsKind::Macos
        }

        fn arch(&self) -> crate::platform::ArchKind {
            crate::platform::ArchKind::Arm64
        }

        fn dirs(&self) -> crate::platform::PlatformResult<crate::platform::PlatformDirs> {
            Ok(crate::platform::PlatformDirs {
                data_dir: self.data_dir.clone(),
                cache_dir: self.data_dir.join("cache"),
                config_dir: self.data_dir.join("config"),
            })
        }

        fn normalize_path(&self, input: &str) -> crate::platform::PlatformResult<PathBuf> {
            Ok(PathBuf::from(input))
        }

        fn ensure_executable(
            &self,
            _path: &std::path::Path,
        ) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn resolve_sidecar_executable(
            &self,
            name: &str,
        ) -> crate::platform::PlatformResult<PathBuf> {
            Ok(PathBuf::from(name))
        }

        fn run_process(
            &self,
            _spec: crate::platform::CommandSpec,
        ) -> crate::platform::PlatformResult<crate::platform::ProcessOutput> {
            unimplemented!("not needed for build thread tests")
        }

        fn spawn_process(
            &self,
            _spec: crate::platform::CommandSpec,
        ) -> crate::platform::PlatformResult<crate::platform::ProcessHandle> {
            unimplemented!("not needed for build thread tests")
        }

        fn kill_process_tree(&self, _pid: u32) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn allocate_local_port(&self) -> crate::platform::PlatformResult<u16> {
            Ok(0)
        }

        fn open_external(&self, _url: &str) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn reveal_path(&self, _path: &std::path::Path) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn register_protocol_handler(
            &self,
            _protocol: &str,
        ) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn register_global_shortcut(
            &self,
            _accelerator: &str,
        ) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn unregister_global_shortcut(
            &self,
            _accelerator: &str,
        ) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn show_tray_or_menu_bar_item(&self) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn get_active_monitor_work_area(
            &self,
        ) -> crate::platform::PlatformResult<crate::platform::WorkArea> {
            Ok(crate::platform::WorkArea {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            })
        }

        fn secure_store_set(
            &self,
            _key: &str,
            _value: &str,
        ) -> crate::platform::PlatformResult<()> {
            Ok(())
        }

        fn secure_store_get(&self, _key: &str) -> crate::platform::PlatformResult<Option<String>> {
            Ok(None)
        }

        fn current_webview_profile(&self) -> crate::platform::WebviewProfile {
            crate::platform::WebviewProfile {
                engine: "test".to_string(),
                supports_transparency: true,
                notes: Vec::new(),
            }
        }
    }
}
