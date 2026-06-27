use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackOsSupport {
    pub supported: bool,
    pub min_version: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackOsCompatibility {
    pub windows: PackOsSupport,
    pub macos: PackOsSupport,
    pub linux: PackOsSupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackCompatibility {
    pub client: String,
    pub os: PackOsCompatibility,
    pub arch: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIntegrity {
    pub sha256: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackCommandSpec {
    pub executable: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub allowed_network: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub kill_on_drop: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackStack {
    pub frontend: Option<String>,
    pub language: Option<String>,
    pub bundler: Option<String>,
    pub database: Option<String>,
    pub server: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackRuntime {
    pub kind: String,
    pub generated_root: String,
    pub entrypoint: String,
    pub bind: String,
    pub network: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackExecutor {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_toolchains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_top_level_dirs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preserve_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackSelection {
    pub software_type: String,
    pub reason: String,
    pub signals: Vec<String>,
    pub weight: i32,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackManifest {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub pack_type: String,
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub builtin: bool,
    pub compatibility: PackCompatibility,
    pub stack: RuntimePackStack,
    pub runtime: RuntimePackRuntime,
    pub executor: RuntimePackExecutor,
    pub prompt_envelope: String,
    pub selection: RuntimePackSelection,
    #[serde(default)]
    pub commands: HashMap<String, PackCommandSpec>,
    pub default_harness: Vec<String>,
    pub integrity: Option<PackIntegrity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareSpeciesMetadata {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessPackManifest {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub pack_type: String,
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub builtin: bool,
    pub compatibility: PackCompatibility,
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub software_species: Option<SoftwareSpeciesMetadata>,
    pub prompt_policy: String,
    pub integrity: Option<PackIntegrity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPackManifest {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub pack_type: String,
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub compatibility: PackCompatibility,
    pub capabilities: Vec<String>,
    pub entry: Option<PackCommandSpec>,
    pub integrity: Option<PackIntegrity>,
}
