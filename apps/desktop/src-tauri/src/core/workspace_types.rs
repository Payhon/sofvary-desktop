use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeKind {
    StaticHtml,
    ReactVite,
    ReactSqlite,
    AiAgentApp,
    Canvas2d,
    MarkdownKnowledge,
    DataTable,
    FileProcessor,
    DesktopWidget,
}

pub type WorkspaceMode = RuntimeKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeMode {
    Dev,
    Prod,
}

impl Default for RuntimeMode {
    fn default() -> Self {
        Self::Dev
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePaths {
    pub root: PathBuf,
    pub generated: PathBuf,
    pub generated_static: PathBuf,
    pub runtime: PathBuf,
    pub snapshots: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConstraints {
    pub boundary: PathBuf,
    pub allow_external_files: bool,
    pub allow_remote_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePreview {
    pub state: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppBoxManifest {
    pub app_id: String,
    pub name: String,
    pub mode: WorkspaceMode,
    pub created_at: String,
    pub updated_at: String,
    pub stack: Vec<String>,
    pub paths: WorkspacePaths,
    pub constraints: WorkspaceConstraints,
    pub preview: WorkspacePreview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SofvaryLockfile {
    pub client_version: String,
    pub runtime_packs: std::collections::HashMap<String, String>,
    pub harness_packs: std::collections::HashMap<String, String>,
    pub plugin_packs: std::collections::HashMap<String, String>,
    pub agent_adapter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub app_id: String,
    pub name: String,
    pub mode: WorkspaceMode,
    pub updated_at: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSummary {
    pub snapshot_id: String,
    pub created_at: String,
    pub path: PathBuf,
}
