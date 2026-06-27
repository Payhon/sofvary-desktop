use crate::core::pack_manager::{read_pack_resource_text, PackError, PackKind, PackManager};
use crate::core::pack_types::RuntimePackManifest;
use crate::core::policy_engine::{PolicyEngine, PolicyError};
use crate::core::policy_types::{PolicyApprovalSet, PolicyCapsuleImportRequest};
use crate::core::workspace_manager::{WorkspaceError, WorkspaceManager};
use crate::core::workspace_types::{
    AppBoxManifest, RuntimeKind, SofvaryLockfile, WorkspaceSummary,
};
use crate::platform::{current_adapter, PlatformAdapter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const CAPSULE_SCHEMA_VERSION: &str = "1.0";
const CAPSULE_TYPE: &str = "sofvary.app-capsule";
const CAPSULE_APP_VERSION: &str = "0.1.0";
const MAX_CAPSULE_ARCHIVE_ENTRIES: usize = 2048;
const MAX_CAPSULE_ENTRY_BYTES: u64 = 10 * 1024 * 1024;
const MAX_CAPSULE_TOTAL_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum AppCapsuleError {
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("pack compatibility error: {0}")]
    Pack(#[from] PackError),
    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),
    #[error("capsule is invalid: {0}")]
    InvalidCapsule(String),
    #[error("capsule export blocked by secret scanner: {0}")]
    SecretDetected(String),
}

pub type AppCapsuleResult<T> = Result<T, AppCapsuleError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportAppCapsulePayload {
    pub app_id: String,
    pub include_prompt_history: bool,
    pub output_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAppCapsulePayload {
    pub capsule_path: PathBuf,
    #[serde(default)]
    pub policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleManifest {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub capsule_type: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: AppCapsuleAuthor,
    pub created_at: String,
    pub runtime: AppCapsuleRuntime,
    pub harness: AppCapsuleHarness,
    pub plugins: AppCapsulePlugins,
    pub permissions: AppCapsulePermissions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_provider_requirements: Option<AppCapsuleAiProviderRequirements>,
    pub entry: AppCapsuleEntry,
    pub database: AppCapsuleDatabase,
    pub prompt: AppCapsulePrompt,
    pub artifacts: Vec<AppCapsuleArtifact>,
    pub lockfile: AppCapsuleLockfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleAuthor {
    pub id: Option<String>,
    pub name: Option<String>,
    pub profile_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleLockedPack {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleRuntime {
    pub kind: RuntimeKind,
    pub pack: AppCapsuleLockedPack,
    pub generated_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleHarness {
    pub packs: Vec<AppCapsuleLockedPack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsulePlugins {
    pub packs: Vec<AppCapsuleLockedPack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsulePermissions {
    pub network: String,
    pub requested: Vec<String>,
    pub filesystem: AppCapsuleFilesystemPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleFilesystemPermissions {
    pub read: Vec<String>,
    pub write: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleEntry {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleAiProviderRequirements {
    pub requirements: Vec<AppCapsuleAiProviderRequirement>,
    pub secrets_included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleAiProviderRequirement {
    pub provider: String,
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleDatabase {
    pub engine: String,
    pub include_data: bool,
    pub schema: Vec<String>,
    pub migrations: Vec<String>,
    pub seed: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub excluded_data: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsulePrompt {
    pub included: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_path: Option<String>,
    pub redacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleArtifact {
    pub path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCapsuleLockfile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportAppCapsuleResult {
    pub capsule_path: PathBuf,
    pub manifest: AppCapsuleManifest,
    pub checksums: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAppCapsuleResult {
    pub workspace: WorkspaceSummary,
    pub app_manifest: AppBoxManifest,
    pub capsule_manifest: AppCapsuleManifest,
}

pub fn export_app_capsule(
    manager: &WorkspaceManager,
    payload: ExportAppCapsulePayload,
) -> AppCapsuleResult<ExportAppCapsuleResult> {
    let adapter = current_adapter();
    export_app_capsule_with_adapter(manager, payload, adapter.as_ref())
}

pub fn import_app_capsule(
    manager: &WorkspaceManager,
    payload: ImportAppCapsulePayload,
) -> AppCapsuleResult<ImportAppCapsuleResult> {
    let adapter = current_adapter();
    import_app_capsule_with_adapter(manager, payload, adapter.as_ref())
}

pub fn inspect_app_capsule_bytes_with_adapter(
    bytes: &[u8],
    adapter: &dyn PlatformAdapter,
) -> AppCapsuleResult<AppCapsuleManifest> {
    let files = read_capsule_files_from_reader(Cursor::new(bytes))?;
    validate_capsule_file_set(&files, adapter)
}

pub fn export_app_capsule_with_adapter(
    manager: &WorkspaceManager,
    payload: ExportAppCapsulePayload,
    adapter: &dyn PlatformAdapter,
) -> AppCapsuleResult<ExportAppCapsuleResult> {
    let app_manifest = manager.get_workspace_with_adapter(payload.app_id, adapter)?;
    let lockfile = manager.read_lockfile_for_manifest(&app_manifest)?;
    let generated_root = app_manifest.paths.generated.clone();
    let runtime_pack = single_locked_pack(&lockfile.runtime_packs, "runtime")?;
    let pack_manager = PackManager::new_with_adapter(adapter)?;
    let resolved_runtime_pack = pack_manager
        .resolver()
        .resolve_runtime(&runtime_pack.id, &runtime_pack.version)?;

    let mut file_entries =
        collect_generated_entries(&generated_root, &resolved_runtime_pack.manifest)?;
    let lockfile_bytes = serde_json::to_vec_pretty(&lockfile)?;
    file_entries.push(CapsuleFileEntry {
        path: "sofvary.lock.json".to_string(),
        bytes: lockfile_bytes.clone(),
    });

    let prompt_history = app_manifest.paths.root.join("prompt.history.jsonl");
    let include_prompt_history = payload.include_prompt_history && prompt_history.exists();
    if include_prompt_history {
        let bytes = fs::read(&prompt_history)?;
        scan_for_secrets("prompt/history.jsonl", &bytes)?;
        file_entries.push(CapsuleFileEntry {
            path: "prompt/history.jsonl".to_string(),
            bytes,
        });
    }

    for entry in &file_entries {
        scan_for_secrets(&entry.path, &entry.bytes)?;
    }

    let generated_paths = file_entries
        .iter()
        .filter(|entry| entry.path.starts_with("source/generated/"))
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let runtime_kind = app_manifest.mode.clone();
    let database = database_metadata(&runtime_kind, &generated_paths);
    let harness_packs = locked_packs_from_map(&lockfile.harness_packs);
    let plugin_packs = locked_packs_from_map(&lockfile.plugin_packs);

    let mut manifest = AppCapsuleManifest {
        schema_version: CAPSULE_SCHEMA_VERSION.to_string(),
        capsule_type: CAPSULE_TYPE.to_string(),
        id: app_manifest.app_id.clone(),
        name: app_manifest.name.clone(),
        version: CAPSULE_APP_VERSION.to_string(),
        author: AppCapsuleAuthor {
            id: None,
            name: Some("local-user".to_string()),
            profile_url: None,
        },
        created_at: app_manifest.updated_at.clone(),
        runtime: AppCapsuleRuntime {
            kind: runtime_kind.clone(),
            pack: runtime_pack,
            generated_root: "source/generated".to_string(),
        },
        harness: AppCapsuleHarness {
            packs: harness_packs,
        },
        plugins: AppCapsulePlugins {
            packs: plugin_packs,
        },
        permissions: AppCapsulePermissions {
            network: "local-only".to_string(),
            requested: Vec::new(),
            filesystem: AppCapsuleFilesystemPermissions {
                read: vec!["source/generated".to_string()],
                write: Vec::new(),
            },
        },
        ai_provider_requirements: ai_provider_requirements_for_runtime(&runtime_kind),
        entry: AppCapsuleEntry {
            path: entry_path_for_runtime_pack(&resolved_runtime_pack.manifest)?,
            kind: entry_kind_for_runtime_pack(&resolved_runtime_pack.manifest),
        },
        database,
        prompt: AppCapsulePrompt {
            included: include_prompt_history,
            history_path: include_prompt_history.then(|| "prompt/history.jsonl".to_string()),
            redacted: !include_prompt_history,
        },
        artifacts: Vec::new(),
        lockfile: AppCapsuleLockfile {
            path: "sofvary.lock.json".to_string(),
            sha256: Some(sha256_hex(&lockfile_bytes)),
        },
    };

    file_entries.push(CapsuleFileEntry {
        path: "README.md".to_string(),
        bytes: readme_for_capsule(&manifest).into_bytes(),
    });
    file_entries.sort_by(|a, b| a.path.cmp(&b.path));

    let mut artifacts = artifacts_for_entries(&file_entries);
    artifacts.push(AppCapsuleArtifact {
        path: "manifest.json".to_string(),
        kind: "manifest".to_string(),
        sha256: None,
        size_bytes: None,
    });
    artifacts.push(AppCapsuleArtifact {
        path: "checksums.json".to_string(),
        kind: "checksum".to_string(),
        sha256: None,
        size_bytes: None,
    });
    artifacts.sort_by(|a, b| a.path.cmp(&b.path));
    manifest.artifacts = artifacts;
    validate_capsule_manifest(&manifest)?;

    file_entries.push(CapsuleFileEntry {
        path: "manifest.json".to_string(),
        bytes: serde_json::to_vec_pretty(&manifest)?,
    });
    file_entries.sort_by(|a, b| a.path.cmp(&b.path));
    let checksums = checksums_for_entries(&file_entries);
    let checksum_entry = CapsuleFileEntry {
        path: "checksums.json".to_string(),
        bytes: serde_json::to_vec_pretty(&checksums)?,
    };

    write_capsule_file(
        &payload.output_path,
        &["source/", "source/generated/", "screenshots/", "prompt/"],
        &file_entries,
        &checksum_entry,
    )?;

    Ok(ExportAppCapsuleResult {
        capsule_path: payload.output_path,
        manifest,
        checksums,
    })
}

pub fn import_app_capsule_with_adapter(
    manager: &WorkspaceManager,
    payload: ImportAppCapsulePayload,
    adapter: &dyn PlatformAdapter,
) -> AppCapsuleResult<ImportAppCapsuleResult> {
    let files = read_capsule_files(&payload.capsule_path)?;
    let capsule_manifest = validate_capsule_file_set(&files, adapter)?;
    let imported_lockfile: SofvaryLockfile =
        serde_json::from_slice(required_file(&files, &capsule_manifest.lockfile.path)?)?;
    let engine = PolicyEngine::new();
    engine.enforce(
        engine.evaluate_capsule_import(capsule_policy_request(&capsule_manifest)),
        &payload.policy_approvals,
    )?;

    let runtime_kind = runtime_kind_from_lockfile(&imported_lockfile)?;
    let imported = manager.create_workspace_for_runtime_with_adapter(
        capsule_manifest.name.clone(),
        runtime_kind,
        adapter,
    )?;

    if imported.paths.generated.exists() {
        fs::remove_dir_all(&imported.paths.generated)?;
    }
    fs::create_dir_all(&imported.paths.generated)?;

    for (path, bytes) in files
        .iter()
        .filter(|(path, _)| path.starts_with("source/generated/"))
    {
        let relative_path = path
            .strip_prefix("source/generated/")
            .ok_or_else(|| AppCapsuleError::InvalidCapsule(format!("invalid path: {path}")))?;
        if relative_path.is_empty() {
            continue;
        }
        validate_capsule_entry_path(relative_path)?;
        let target = manager.ensure_child(&imported.paths.generated, Path::new(relative_path))?;
        let parent = target.parent().ok_or_else(|| {
            AppCapsuleError::InvalidCapsule(format!("capsule path has no parent: {path}"))
        })?;
        fs::create_dir_all(parent)?;
        fs::write(target, bytes)?;
    }

    if let Some(prompt_bytes) = files.get("prompt/history.jsonl") {
        fs::write(
            imported.paths.root.join("prompt.history.jsonl"),
            prompt_bytes,
        )?;
    }
    fs::write(
        imported.paths.root.join("sofvary.lock.json"),
        serde_json::to_string_pretty(&imported_lockfile)? + "\n",
    )?;

    let workspace = WorkspaceSummary {
        app_id: imported.app_id.clone(),
        name: imported.name.clone(),
        mode: imported.mode.clone(),
        updated_at: imported.updated_at.clone(),
        root: imported.paths.root.clone(),
    };

    Ok(ImportAppCapsuleResult {
        workspace,
        app_manifest: imported,
        capsule_manifest,
    })
}

pub fn capsule_policy_request(manifest: &AppCapsuleManifest) -> PolicyCapsuleImportRequest {
    PolicyCapsuleImportRequest {
        name: manifest.name.clone(),
        network: manifest.permissions.network.clone(),
        workspace_read: manifest.permissions.filesystem.read.clone(),
        workspace_write: manifest.permissions.filesystem.write.clone(),
        requested: manifest.permissions.requested.clone(),
        plugin_packs: manifest
            .plugins
            .packs
            .iter()
            .map(|pack| format!("{}@{}", pack.id, pack.version))
            .collect(),
    }
}

#[derive(Debug, Clone)]
struct CapsuleFileEntry {
    path: String,
    bytes: Vec<u8>,
}

fn collect_generated_entries(
    generated_root: &Path,
    runtime_pack: &RuntimePackManifest,
) -> AppCapsuleResult<Vec<CapsuleFileEntry>> {
    if !generated_root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for capsule_path in allowed_capsule_generated_paths(runtime_pack)? {
        let relative_path = capsule_path
            .strip_prefix("source/generated/")
            .ok_or_else(|| {
                AppCapsuleError::InvalidCapsule(format!("invalid path: {capsule_path}"))
            })?;
        let path = generated_root.join(relative_path);
        if !path.exists() {
            continue;
        }
        let file_type = fs::symlink_metadata(&path)?.file_type();
        if !file_type.is_file() {
            continue;
        }
        validate_capsule_entry_path(&capsule_path)?;
        entries.push(CapsuleFileEntry {
            path: capsule_path,
            bytes: fs::read(path)?,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn scan_for_secrets(path: &str, bytes: &[u8]) -> AppCapsuleResult<()> {
    let text = String::from_utf8_lossy(bytes);
    let lower_text = text.to_lowercase();
    if lower_text.contains("-----begin ") && lower_text.contains(" private key-----") {
        return Err(AppCapsuleError::SecretDetected(format!(
            "{path} contains private key material"
        )));
    }

    for line in text.lines() {
        let lower = line.to_lowercase();
        for marker in [
            "api_key",
            "api key",
            "apikey",
            "access_token",
            "auth_token",
            "token",
            "password",
        ] {
            if !lower.contains(marker) {
                continue;
            }
            let Some(value) = secret_value_after_separator(line) else {
                continue;
            };
            if looks_like_secret_value(value) {
                return Err(AppCapsuleError::SecretDetected(format!(
                    "{path} contains a possible {marker}"
                )));
            }
        }
    }

    Ok(())
}

fn secret_value_after_separator(line: &str) -> Option<&str> {
    let separator_index = line.find('=').or_else(|| line.find(':'))?;
    Some(
        line[(separator_index + 1)..]
            .trim()
            .trim_matches(|c| matches!(c, '"' | '\'' | '`' | ';' | ',' | ' ')),
    )
}

fn looks_like_secret_value(value: &str) -> bool {
    let lower = value.to_lowercase();
    if matches!(
        lower.as_str(),
        "" | "true" | "false" | "null" | "undefined" | "demo" | "example" | "placeholder"
    ) {
        return false;
    }
    value.chars().filter(|c| !c.is_whitespace()).count() >= 16
}

fn ordered_map(map: &HashMap<String, String>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn locked_packs_from_map(map: &HashMap<String, String>) -> Vec<AppCapsuleLockedPack> {
    ordered_map(map)
        .into_iter()
        .map(|(id, version)| AppCapsuleLockedPack { id, version })
        .collect()
}

fn locked_pack_map(packs: &[AppCapsuleLockedPack]) -> BTreeMap<String, String> {
    packs
        .iter()
        .map(|pack| (pack.id.clone(), pack.version.clone()))
        .collect()
}

fn single_locked_pack(
    map: &HashMap<String, String>,
    label: &str,
) -> AppCapsuleResult<AppCapsuleLockedPack> {
    if map.len() != 1 {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "capsule requires exactly one {label} pack"
        )));
    }
    let (id, version) = map
        .iter()
        .next()
        .ok_or_else(|| AppCapsuleError::InvalidCapsule(format!("missing {label} pack")))?;
    Ok(AppCapsuleLockedPack {
        id: id.clone(),
        version: version.clone(),
    })
}

fn validate_manifest_matches_lockfile(
    manifest: &AppCapsuleManifest,
    lockfile: &SofvaryLockfile,
) -> AppCapsuleResult<()> {
    let runtime_pack = single_locked_pack(&lockfile.runtime_packs, "runtime")?;
    if manifest.runtime.pack != runtime_pack {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest runtime pack does not match lockfile".to_string(),
        ));
    }
    if locked_pack_map(&manifest.harness.packs) != ordered_map(&lockfile.harness_packs) {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest harness packs do not match lockfile".to_string(),
        ));
    }
    if locked_pack_map(&manifest.plugins.packs) != ordered_map(&lockfile.plugin_packs) {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest plugin packs do not match lockfile".to_string(),
        ));
    }
    if runtime_kind_from_lockfile(lockfile)? != manifest.runtime.kind {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest runtime kind does not match lockfile runtime pack".to_string(),
        ));
    }
    Ok(())
}

fn validate_local_pack_compatibility(
    lockfile: &SofvaryLockfile,
    adapter: &dyn PlatformAdapter,
) -> AppCapsuleResult<()> {
    let pack_manager = PackManager::new_with_adapter(adapter)?;
    for (id, version) in &lockfile.runtime_packs {
        pack_manager.resolver().resolve_runtime(id, version)?;
    }
    for (id, version) in &lockfile.harness_packs {
        pack_manager.resolver().resolve_harness(id, version)?;
    }
    for (id, version) in &lockfile.plugin_packs {
        pack_manager.resolver().resolve_plugin(id, version)?;
    }
    Ok(())
}

fn validate_pack_reference(pack: &AppCapsuleLockedPack) -> AppCapsuleResult<()> {
    validate_pack_id_like(&pack.id)?;
    validate_semver_like(&pack.version)
}

fn validate_pack_id_like(id: &str) -> AppCapsuleResult<()> {
    let parts = id.split('.').collect::<Vec<_>>();
    if parts.len() < 2 || parts.iter().any(|part| !is_pack_id_part(part)) {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "invalid pack id: {id}"
        )));
    }
    Ok(())
}

fn is_pack_id_part(part: &str) -> bool {
    let mut chars = part.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn validate_semver_like(version: &str) -> AppCapsuleResult<()> {
    let mut build_parts = version.split('+');
    let before_build = build_parts.next().unwrap_or_default();
    let build = build_parts.next();
    if build_parts.next().is_some()
        || build
            .map(|build| !valid_semver_identifier_list(build, false))
            .unwrap_or(false)
    {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "invalid semver version: {version}"
        )));
    }

    let mut prerelease_parts = before_build.split('-');
    let core = prerelease_parts.next().unwrap_or_default();
    let prerelease = prerelease_parts.next();
    if prerelease_parts.next().is_some()
        || prerelease
            .map(|prerelease| !valid_semver_identifier_list(prerelease, true))
            .unwrap_or(false)
    {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "invalid semver version: {version}"
        )));
    }

    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.chars().all(|ch| ch.is_ascii_digit())
        })
    {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "invalid semver version: {version}"
        )));
    }
    Ok(())
}

fn valid_semver_identifier_list(value: &str, reject_numeric_leading_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
                && (!reject_numeric_leading_zero
                    || !identifier.chars().all(|ch| ch.is_ascii_digit())
                    || identifier == "0"
                    || !identifier.starts_with('0'))
        })
}

fn validate_import_artifacts(
    manifest: &AppCapsuleManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> AppCapsuleResult<()> {
    if !files.contains_key(&manifest.entry.path) {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "manifest entry path is missing from capsule: {}",
            manifest.entry.path
        )));
    }

    let artifact_paths = manifest
        .artifacts
        .iter()
        .map(|artifact| artifact.path.clone())
        .collect::<BTreeSet<_>>();
    let file_paths = files.keys().cloned().collect::<BTreeSet<_>>();
    if artifact_paths != file_paths {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest artifacts do not match capsule files".to_string(),
        ));
    }

    for artifact in &manifest.artifacts {
        let bytes = required_file(files, &artifact.path)?;
        if let Some(expected) = &artifact.sha256 {
            let actual = sha256_hex(bytes);
            if &actual != expected {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "manifest artifact sha256 mismatch for {}",
                    artifact.path
                )));
            }
        }
        if let Some(expected) = artifact.size_bytes {
            if bytes.len() as u64 != expected {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "manifest artifact size mismatch for {}",
                    artifact.path
                )));
            }
        }
    }
    Ok(())
}

fn validate_importable_capsule_runtime(runtime: &str) -> AppCapsuleResult<()> {
    let requires_local_toolchain = PackManager::new()
        .and_then(|manager| manager.resolve_runtime_packs_by_kind(runtime))
        .map(|packs| {
            !packs
                .runtime
                .manifest
                .executor
                .required_toolchains
                .is_empty()
        })
        .unwrap_or(false);
    if requires_local_toolchain {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "capsule runtime '{runtime}' requires local sidecar execution and cannot be imported from untrusted capsules in this phase"
        )));
    }

    Ok(())
}

fn validate_generated_file_set_for_runtime(
    runtime_pack: &RuntimePackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> AppCapsuleResult<()> {
    let allowed = allowed_capsule_generated_paths(runtime_pack)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let generated_files = files
        .keys()
        .filter(|path| path.starts_with("source/generated/"))
        .cloned()
        .collect::<BTreeSet<_>>();

    let unexpected = generated_files
        .difference(&allowed)
        .cloned()
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "capsule contains generated files outside the {} allowlist: {}",
            runtime_pack.runtime.kind,
            unexpected.join(", ")
        )));
    }

    Ok(())
}

fn allowed_capsule_generated_paths(
    runtime_pack: &RuntimePackManifest,
) -> AppCapsuleResult<Vec<String>> {
    let prefix = generated_path_prefix_for_runtime_pack(runtime_pack)?;
    Ok(allowed_generated_files_for_runtime_pack(runtime_pack)?
        .into_iter()
        .map(|path| {
            if prefix.is_empty() {
                format!("source/generated/{path}")
            } else {
                format!("source/generated/{prefix}/{path}")
            }
        })
        .collect())
}

fn generated_path_prefix_for_runtime_pack(
    runtime_pack: &RuntimePackManifest,
) -> AppCapsuleResult<String> {
    let root = runtime_pack.runtime.generated_root.as_str();
    if root == "generated" {
        return Ok(String::new());
    }
    let prefix = root.strip_prefix("generated/").ok_or_else(|| {
        AppCapsuleError::InvalidCapsule(format!(
            "runtime generatedRoot must be generated or generated/<dir>, found {root}"
        ))
    })?;
    validate_capsule_entry_path(&format!("source/generated/{prefix}/placeholder"))?;
    Ok(prefix.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapsuleRuntimeEnvelopeConfig {
    allowed_files: Vec<String>,
}

fn allowed_generated_files_for_runtime_pack(
    runtime_pack: &RuntimePackManifest,
) -> AppCapsuleResult<Vec<String>> {
    let config: CapsuleRuntimeEnvelopeConfig = serde_json::from_str(&read_pack_resource_text(
        PackKind::Runtime,
        &runtime_pack.id,
        &runtime_pack.version,
        &runtime_pack.prompt_envelope,
    )?)?;
    if config.allowed_files.is_empty() {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "runtime pack {}@{} prompt envelope must declare allowedFiles",
            runtime_pack.id, runtime_pack.version
        )));
    }
    for path in &config.allowed_files {
        validate_capsule_entry_path(&format!("source/generated/{path}"))?;
    }
    Ok(config.allowed_files)
}

fn database_metadata(runtime: &str, generated_paths: &[String]) -> AppCapsuleDatabase {
    let mut schema = Vec::new();
    let mut migrations = Vec::new();
    let mut seed = Vec::new();

    for path in generated_paths {
        if !path.starts_with("source/generated/data/") {
            continue;
        }
        if path.contains("/migrations/") {
            migrations.push(path.clone());
        } else if path.contains("/seed/") || path.ends_with("/seed.sql") {
            seed.push(path.clone());
        } else if path.ends_with(".sql") || path.contains("/schema") {
            schema.push(path.clone());
        }
    }

    AppCapsuleDatabase {
        engine: if runtime == "react-sqlite" {
            "sqlite".to_string()
        } else {
            "none".to_string()
        },
        include_data: false,
        schema,
        migrations,
        seed,
        excluded_data: if runtime == "react-sqlite" {
            vec!["source/generated/data/app.sqlite".to_string()]
        } else {
            Vec::new()
        },
    }
}

fn ai_provider_requirements_for_runtime(runtime: &str) -> Option<AppCapsuleAiProviderRequirements> {
    if runtime != "ai-agent-app" {
        return None;
    }

    Some(AppCapsuleAiProviderRequirements {
        requirements: vec![
            AppCapsuleAiProviderRequirement {
                provider: "openai".to_string(),
                capabilities: vec!["text".to_string()],
                models: vec!["gpt-5.1".to_string()],
                credential_kind: Some("api-key".to_string()),
                required: Some(true),
                purpose: Some(
                    "Draft, edit, and summarize text through the local Sofvary AI Gateway."
                        .to_string(),
                ),
            },
            AppCapsuleAiProviderRequirement {
                provider: "openai".to_string(),
                capabilities: vec!["image".to_string()],
                models: vec!["gpt-image-1".to_string()],
                credential_kind: Some("api-key".to_string()),
                required: Some(false),
                purpose: Some(
                    "Create image artifacts through the local Sofvary AI Gateway.".to_string(),
                ),
            },
            AppCapsuleAiProviderRequirement {
                provider: "openai".to_string(),
                capabilities: vec!["video".to_string()],
                models: vec!["sora-2".to_string()],
                credential_kind: Some("api-key".to_string()),
                required: Some(false),
                purpose: Some(
                    "Create video artifacts through the local Sofvary AI Gateway.".to_string(),
                ),
            },
        ],
        secrets_included: false,
    })
}

fn artifacts_for_entries(entries: &[CapsuleFileEntry]) -> Vec<AppCapsuleArtifact> {
    entries
        .iter()
        .map(|entry| AppCapsuleArtifact {
            path: entry.path.clone(),
            kind: artifact_kind_for_path(&entry.path).to_string(),
            sha256: Some(sha256_hex(&entry.bytes)),
            size_bytes: Some(entry.bytes.len() as u64),
        })
        .collect()
}

fn artifact_kind_for_path(path: &str) -> &'static str {
    if path == "sofvary.lock.json" {
        "lockfile"
    } else if path == "README.md" {
        "readme"
    } else if path == "prompt/history.jsonl" {
        "prompt"
    } else if path.starts_with("screenshots/") {
        "screenshot"
    } else if path.starts_with("source/generated/data/migrations/") {
        "database-migration"
    } else if path.starts_with("source/generated/data/seed/") {
        "database-seed"
    } else if path.starts_with("source/generated/data/")
        && (path.ends_with(".sql") || path.contains("/schema"))
    {
        "database-schema"
    } else if path.starts_with("source/generated/") {
        "source"
    } else {
        "other"
    }
}

fn validate_capsule_manifest(manifest: &AppCapsuleManifest) -> AppCapsuleResult<()> {
    if manifest.schema_version != CAPSULE_SCHEMA_VERSION {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "unsupported schemaVersion '{}'",
            manifest.schema_version
        )));
    }
    if manifest.capsule_type != CAPSULE_TYPE {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "unsupported capsule type '{}'",
            manifest.capsule_type
        )));
    }
    if manifest.id.trim().is_empty() || manifest.name.trim().is_empty() {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest id and name are required".to_string(),
        ));
    }
    validate_semver_like(&manifest.version)?;
    if manifest.runtime.pack.id.trim().is_empty() || manifest.harness.packs.is_empty() {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest runtime and harness exact versions are required".to_string(),
        ));
    }
    validate_pack_reference(&manifest.runtime.pack)?;
    for pack in manifest
        .harness
        .packs
        .iter()
        .chain(manifest.plugins.packs.iter())
    {
        validate_pack_reference(pack)?;
    }
    if manifest.runtime.generated_root != "source/generated" {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest runtime generatedRoot must be source/generated".to_string(),
        ));
    }
    if manifest.permissions.network != "local-only" {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest permissions network must be local-only".to_string(),
        ));
    }
    validate_ai_provider_requirements(manifest)?;
    for path in manifest
        .permissions
        .filesystem
        .read
        .iter()
        .chain(manifest.permissions.filesystem.write.iter())
    {
        validate_manifest_capsule_path(path)?;
    }
    validate_manifest_capsule_path(&manifest.entry.path)?;
    if !matches!(
        manifest.entry.kind.as_str(),
        "html" | "react" | "node" | "static" | "widget"
    ) {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "unsupported entry kind: {}",
            manifest.entry.kind
        )));
    }
    validate_manifest_capsule_path(&manifest.runtime.generated_root)?;
    if !matches!(manifest.database.engine.as_str(), "none" | "sqlite") {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "unsupported database engine: {}",
            manifest.database.engine
        )));
    }
    for path in manifest
        .database
        .schema
        .iter()
        .chain(manifest.database.migrations.iter())
        .chain(manifest.database.seed.iter())
        .chain(manifest.database.excluded_data.iter())
    {
        validate_manifest_capsule_path(path)?;
    }
    if manifest.database.include_data {
        return Err(AppCapsuleError::InvalidCapsule(
            "capsule manifest must not include real database data in Phase 18".to_string(),
        ));
    }
    for artifact in &manifest.artifacts {
        validate_manifest_capsule_path(&artifact.path)?;
        match artifact.kind.as_str() {
            "manifest" if artifact.path != "manifest.json" => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "manifest artifact path must be manifest.json: {}",
                    artifact.path
                )));
            }
            "checksum" if artifact.path != "checksums.json" => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "checksum artifact path must be checksums.json: {}",
                    artifact.path
                )));
            }
            "readme" if artifact.path != "README.md" => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "readme artifact path must be README.md: {}",
                    artifact.path
                )));
            }
            "lockfile" if artifact.path != "sofvary.lock.json" => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "lockfile artifact path must be sofvary.lock.json: {}",
                    artifact.path
                )));
            }
            "source" if !artifact.path.starts_with("source/generated/") => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "source artifact path must stay under source/generated: {}",
                    artifact.path
                )));
            }
            "prompt" if artifact.path != "prompt/history.jsonl" => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "unsupported prompt artifact path: {}",
                    artifact.path
                )));
            }
            "screenshot" if !artifact.path.starts_with("screenshots/") => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "screenshot artifact path must stay under screenshots: {}",
                    artifact.path
                )));
            }
            "source" | "prompt" | "screenshot" | "readme" | "database-schema"
            | "database-migration" | "database-seed" | "lockfile" | "manifest" | "checksum"
            | "other" => {}
            other => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "unsupported artifact kind: {other}"
                )));
            }
        }
    }
    if let Some(path) = &manifest.prompt.history_path {
        validate_manifest_capsule_path(path)?;
        if path != "prompt/history.jsonl" {
            return Err(AppCapsuleError::InvalidCapsule(format!(
                "unsupported prompt history path: {path}"
            )));
        }
    }
    validate_manifest_capsule_path(&manifest.lockfile.path)?;
    if manifest.lockfile.path != "sofvary.lock.json" {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "unsupported lockfile path: {}",
            manifest.lockfile.path
        )));
    }
    Ok(())
}

fn validate_ai_provider_requirements(manifest: &AppCapsuleManifest) -> AppCapsuleResult<()> {
    let Some(requirements) = &manifest.ai_provider_requirements else {
        if manifest.runtime.kind == "ai-agent-app" {
            return Err(AppCapsuleError::InvalidCapsule(
                "AI Agent App capsule requires provider requirements metadata".to_string(),
            ));
        }
        return Ok(());
    };

    if requirements.secrets_included {
        return Err(AppCapsuleError::InvalidCapsule(
            "AI provider requirements must not include secrets".to_string(),
        ));
    }
    if requirements.requirements.is_empty() {
        return Err(AppCapsuleError::InvalidCapsule(
            "AI provider requirements must not be empty".to_string(),
        ));
    }

    for requirement in &requirements.requirements {
        validate_nonempty_plain_metadata("aiProviderRequirements.provider", &requirement.provider)?;
        validate_plain_metadata_list(
            "aiProviderRequirements.capabilities",
            &requirement.capabilities,
        )?;
        validate_plain_metadata_list("aiProviderRequirements.models", &requirement.models)?;
        if let Some(credential_kind) = &requirement.credential_kind {
            if !matches!(
                credential_kind.as_str(),
                "api-key" | "oauth" | "local-account" | "none"
            ) {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "unsupported AI credential kind: {credential_kind}"
                )));
            }
        }
        if let Some(purpose) = &requirement.purpose {
            validate_nonempty_plain_metadata("aiProviderRequirements.purpose", purpose)?;
        }
    }

    Ok(())
}

fn validate_plain_metadata_list(field: &str, values: &[String]) -> AppCapsuleResult<()> {
    for value in values {
        validate_nonempty_plain_metadata(field, value)?;
    }
    Ok(())
}

fn validate_nonempty_plain_metadata(field: &str, value: &str) -> AppCapsuleResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "{field} must not be empty"
        )));
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("secure-key-ref")
        || lower.contains("secure key ref")
        || lower.contains("provider-id")
        || lower.contains("provider id")
        || lower.contains("api key:")
        || lower.contains("api_key")
    {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "{field} must not include local bindings or secret references"
        )));
    }
    Ok(())
}

fn validate_prompt_consistency(
    manifest: &AppCapsuleManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> AppCapsuleResult<()> {
    let has_prompt = files.contains_key("prompt/history.jsonl");
    if manifest.prompt.included != has_prompt {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest prompt metadata does not match prompt/history.jsonl".to_string(),
        ));
    }
    if manifest.prompt.included
        && manifest.prompt.history_path.as_deref() != Some("prompt/history.jsonl")
    {
        return Err(AppCapsuleError::InvalidCapsule(
            "manifest prompt path must be prompt/history.jsonl".to_string(),
        ));
    }
    Ok(())
}

fn validate_capsule_entry_path(path: &str) -> AppCapsuleResult<()> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() || trimmed.contains('\\') {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "invalid capsule entry path: {path}"
        )));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "capsule entry path must be relative: {}",
            path.display()
        )));
    }
    let mut components = path.components();
    let Some(first_component) = components.next() else {
        return Err(AppCapsuleError::InvalidCapsule(
            "capsule entry path must not be empty".to_string(),
        ));
    };
    match first_component {
        Component::Normal(value) => {
            if value
                .to_str()
                .map(|value| value.contains(':'))
                .unwrap_or(true)
            {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "capsule entry path must not use a Windows drive or prefix: {}",
                    path.display()
                )));
            }
        }
        Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
            return Err(AppCapsuleError::InvalidCapsule(format!(
                "capsule entry path escapes archive root: {}",
                path.display()
            )));
        }
        Component::CurDir => {
            return Err(AppCapsuleError::InvalidCapsule(format!(
                "invalid capsule entry path: {}",
                path.display()
            )));
        }
    }
    for component in components {
        if !matches!(component, Component::Normal(_)) {
            return Err(AppCapsuleError::InvalidCapsule(format!(
                "capsule entry path escapes archive root: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn validate_manifest_capsule_path(path: &str) -> AppCapsuleResult<()> {
    normalized_capsule_entry_path(path).map(|_| ())
}

fn normalized_capsule_entry_path(path: &str) -> AppCapsuleResult<String> {
    validate_capsule_entry_path(path)?;
    let is_dir = path.ends_with('/');
    let trimmed = path.trim_end_matches('/');
    let mut parts = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str().ok_or_else(|| {
                    AppCapsuleError::InvalidCapsule(format!(
                        "capsule entry path is not valid UTF-8: {path}"
                    ))
                })?;
                parts.push(value.to_string());
            }
            _ => {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "capsule entry path escapes archive root: {path}"
                )));
            }
        }
    }
    let normalized = parts.join("/");
    if normalized != trimmed {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "capsule entry path is not normalized: {path}"
        )));
    }
    Ok(if is_dir {
        format!("{normalized}/")
    } else {
        normalized
    })
}

fn entry_path_for_runtime_pack(runtime_pack: &RuntimePackManifest) -> AppCapsuleResult<String> {
    let prefix = generated_path_prefix_for_runtime_pack(runtime_pack)?;
    let path = if prefix.is_empty() {
        format!("source/generated/{}", runtime_pack.runtime.entrypoint)
    } else {
        format!(
            "source/generated/{prefix}/{}",
            runtime_pack.runtime.entrypoint
        )
    };
    validate_capsule_entry_path(&path)?;
    Ok(path)
}

fn entry_kind_for_runtime_pack(runtime_pack: &RuntimePackManifest) -> String {
    match runtime_pack.executor.kind.as_str() {
        "static-html" | "canvas2d" => "html",
        "desktop-widget" => "widget",
        _ => "react",
    }
    .to_string()
}

fn runtime_kind_from_lockfile(lockfile: &SofvaryLockfile) -> AppCapsuleResult<RuntimeKind> {
    if lockfile.runtime_packs.len() != 1 {
        return Err(AppCapsuleError::InvalidCapsule(
            "capsule import requires exactly one runtime pack".to_string(),
        ));
    }
    let (runtime_id, runtime_version) = lockfile
        .runtime_packs
        .iter()
        .next()
        .ok_or_else(|| AppCapsuleError::InvalidCapsule("missing runtime pack".to_string()))?;
    let pack_manager = PackManager::new()?;
    let runtime = pack_manager
        .resolver()
        .resolve_runtime(runtime_id, runtime_version)?;
    Ok(runtime.manifest.runtime.kind)
}

fn checksums_for_entries(entries: &[CapsuleFileEntry]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|entry| (entry.path.clone(), sha256_hex(&entry.bytes)))
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_capsule_file(
    path: &Path,
    directories: &[&str],
    file_entries: &[CapsuleFileEntry],
    checksum_entry: &CapsuleFileEntry,
) -> AppCapsuleResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let file = fs::File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut written_dirs = HashSet::new();
    for directory in directories {
        validate_capsule_entry_path(directory)?;
        if written_dirs.insert(*directory) {
            zip.add_directory(*directory, options)?;
        }
    }
    for entry in file_entries.iter().chain(std::iter::once(checksum_entry)) {
        validate_capsule_entry_path(&entry.path)?;
        zip.start_file(&entry.path, options)?;
        zip.write_all(&entry.bytes)?;
    }
    zip.finish()?;
    Ok(())
}

fn read_capsule_files(path: &Path) -> AppCapsuleResult<BTreeMap<String, Vec<u8>>> {
    let file = fs::File::open(path)?;
    read_capsule_files_from_reader(file)
}

fn read_capsule_files_from_reader<R: Read + Seek>(
    reader: R,
) -> AppCapsuleResult<BTreeMap<String, Vec<u8>>> {
    let mut archive = ZipArchive::new(reader)?;
    if archive.len() > MAX_CAPSULE_ARCHIVE_ENTRIES {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "capsule contains too many zip entries: {} > {}",
            archive.len(),
            MAX_CAPSULE_ARCHIVE_ENTRIES
        )));
    }

    let mut files = BTreeMap::new();
    let mut seen_dirs = BTreeSet::new();
    let mut total_uncompressed_bytes = 0_u64;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        let normalized_name = normalized_capsule_entry_path(&name)?;
        if entry.is_dir() {
            if !seen_dirs.insert(normalized_name.clone()) {
                return Err(AppCapsuleError::InvalidCapsule(format!(
                    "duplicate capsule directory entry: {normalized_name}"
                )));
            }
            continue;
        }
        if files.contains_key(&normalized_name) || seen_dirs.contains(&normalized_name) {
            return Err(AppCapsuleError::InvalidCapsule(format!(
                "duplicate capsule entry: {normalized_name}"
            )));
        }
        if entry.size() > MAX_CAPSULE_ENTRY_BYTES {
            return Err(AppCapsuleError::InvalidCapsule(format!(
                "capsule entry exceeds the per-file limit of {MAX_CAPSULE_ENTRY_BYTES} bytes: {normalized_name}"
            )));
        }
        let bytes = read_capsule_entry_limited(&mut entry, &normalized_name)?;
        total_uncompressed_bytes = total_uncompressed_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| {
                AppCapsuleError::InvalidCapsule(
                    "capsule uncompressed size overflowed validation".to_string(),
                )
            })?;
        if total_uncompressed_bytes > MAX_CAPSULE_TOTAL_UNCOMPRESSED_BYTES {
            return Err(AppCapsuleError::InvalidCapsule(format!(
                "capsule exceeds the total uncompressed limit of {MAX_CAPSULE_TOTAL_UNCOMPRESSED_BYTES} bytes"
            )));
        }
        files.insert(normalized_name, bytes);
    }

    for required in ["manifest.json", "checksums.json", "README.md"] {
        required_file(&files, required)?;
    }
    if !files
        .keys()
        .any(|path| path.starts_with("source/generated/"))
        && !seen_dirs.contains("source/generated/")
    {
        return Err(AppCapsuleError::InvalidCapsule(
            "capsule must contain source/generated/".to_string(),
        ));
    }

    Ok(files)
}

fn read_capsule_entry_limited<R: Read>(reader: &mut R, path: &str) -> AppCapsuleResult<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_CAPSULE_ENTRY_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CAPSULE_ENTRY_BYTES {
        return Err(AppCapsuleError::InvalidCapsule(format!(
            "capsule entry exceeds the per-file limit of {MAX_CAPSULE_ENTRY_BYTES} bytes: {path}"
        )));
    }
    Ok(bytes)
}

fn validate_capsule_file_set(
    files: &BTreeMap<String, Vec<u8>>,
    adapter: &dyn PlatformAdapter,
) -> AppCapsuleResult<AppCapsuleManifest> {
    validate_checksums(files)?;

    let manifest_bytes = required_file(files, "manifest.json")?;
    let capsule_manifest: AppCapsuleManifest = serde_json::from_slice(manifest_bytes)?;
    validate_capsule_manifest(&capsule_manifest)?;
    validate_prompt_consistency(&capsule_manifest, files)?;

    let lockfile_bytes = required_file(files, &capsule_manifest.lockfile.path)?;
    if let Some(expected) = &capsule_manifest.lockfile.sha256 {
        let actual = sha256_hex(lockfile_bytes);
        if &actual != expected {
            return Err(AppCapsuleError::InvalidCapsule(
                "manifest lockfile sha256 does not match included lockfile".to_string(),
            ));
        }
    }
    let imported_lockfile: SofvaryLockfile = serde_json::from_slice(lockfile_bytes)?;
    validate_manifest_matches_lockfile(&capsule_manifest, &imported_lockfile)?;
    validate_import_artifacts(&capsule_manifest, files)?;
    validate_importable_capsule_runtime(&capsule_manifest.runtime.kind)?;
    let pack_manager = PackManager::new_with_adapter(adapter)?;
    let runtime_pack = pack_manager.resolver().resolve_runtime(
        &capsule_manifest.runtime.pack.id,
        &capsule_manifest.runtime.pack.version,
    )?;
    validate_generated_file_set_for_runtime(&runtime_pack.manifest, files)?;
    validate_local_pack_compatibility(&imported_lockfile, adapter)?;

    Ok(capsule_manifest)
}

fn required_file<'a>(
    files: &'a BTreeMap<String, Vec<u8>>,
    path: &str,
) -> AppCapsuleResult<&'a [u8]> {
    files
        .get(path)
        .map(Vec::as_slice)
        .ok_or_else(|| AppCapsuleError::InvalidCapsule(format!("missing capsule file: {path}")))
}

fn validate_checksums(files: &BTreeMap<String, Vec<u8>>) -> AppCapsuleResult<()> {
    let checksum_bytes = required_file(files, "checksums.json")?;
    let checksums: BTreeMap<String, String> = serde_json::from_slice(checksum_bytes)?;
    if checksums.contains_key("checksums.json") {
        return Err(AppCapsuleError::InvalidCapsule(
            "checksums.json must not include itself".to_string(),
        ));
    }

    let actual_paths = files
        .keys()
        .filter(|path| path.as_str() != "checksums.json")
        .cloned()
        .collect::<BTreeSet<_>>();
    let checksum_paths = checksums.keys().cloned().collect::<BTreeSet<_>>();
    if actual_paths != checksum_paths {
        return Err(AppCapsuleError::InvalidCapsule(
            "checksums.json paths do not match capsule files".to_string(),
        ));
    }

    for (path, expected) in checksums {
        let actual = sha256_hex(required_file(files, &path)?);
        if actual != expected {
            return Err(AppCapsuleError::InvalidCapsule(format!(
                "checksum mismatch for {path}"
            )));
        }
    }

    Ok(())
}

fn readme_for_capsule(manifest: &AppCapsuleManifest) -> String {
    format!(
        "# {}\n\nSofvary App Capsule exported for local import.\n\nRuntime packs: {}\nHarness packs: {}\n",
        manifest.name,
        format!("{}@{}", manifest.runtime.pack.id, manifest.runtime.pack.version),
        manifest
            .harness
            .packs
            .iter()
            .map(|pack| format!("{}@{}", pack.id, pack.version))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows::WindowsPlatformAdapter;
    use crate::platform::{
        ArchKind, CommandSpec, OsKind, PlatformDirs, PlatformResult, ProcessHandle, ProcessOutput,
        WebviewProfile, WorkArea,
    };
    use std::path::Path;
    use tempfile::TempDir;

    fn import_payload(capsule_path: PathBuf) -> ImportAppCapsulePayload {
        let subject = read_capsule_files(&capsule_path)
            .ok()
            .and_then(|files| {
                required_file(&files, "manifest.json")
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<AppCapsuleManifest>(bytes).ok())
            })
            .map(|manifest| manifest.name);

        ImportAppCapsulePayload {
            capsule_path,
            policy_approvals: crate::core::policy_types::PolicyApprovalSet {
                approved: vec![crate::core::policy_types::PolicyApprovalGrant {
                    action: crate::core::policy_types::PolicyActionKind::CapsuleImport,
                    subject,
                }],
            },
        }
    }

    struct TempAdapter {
        dirs: PlatformDirs,
    }

    impl crate::platform::PlatformAdapter for TempAdapter {
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

    fn temp_adapter(temp: &TempDir) -> TempAdapter {
        TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        }
    }

    fn zip_names(path: &Path) -> Vec<String> {
        let file = fs::File::open(path).expect("capsule");
        let mut archive = ZipArchive::new(file).expect("zip");
        let mut names = (0..archive.len())
            .map(|index| archive.by_index(index).expect("entry").name().to_string())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn write_static_sample(manifest: &AppBoxManifest) {
        fs::create_dir_all(manifest.paths.generated.join("static")).expect("static dir");
        fs::write(
            manifest.paths.generated.join("static/index.html"),
            "<!doctype html><div>hello</div>",
        )
        .expect("index");
    }

    fn write_generated_allowlist_sample(manifest: &AppBoxManifest, runtime_kind: &str) {
        let runtime_pack = runtime_pack_for_kind(runtime_kind);
        for capsule_path in allowed_capsule_generated_paths(&runtime_pack).expect("allowed files") {
            let relative_path = capsule_path
                .strip_prefix("source/generated/")
                .expect("generated path");
            let target = manifest.paths.generated.join(relative_path);
            fs::create_dir_all(target.parent().expect("parent")).expect("parent dir");
            fs::write(target, format!("file: {capsule_path}")).expect("file");
        }
    }

    fn runtime_pack_for_kind(runtime_kind: &str) -> RuntimePackManifest {
        crate::core::pack_manager::runtime_catalog_manifests()
            .expect("runtime catalog")
            .into_iter()
            .find(|manifest| manifest.runtime.kind == runtime_kind)
            .unwrap_or_else(|| panic!("runtime kind {runtime_kind} is available"))
    }

    fn rewrite_capsule_from_files(path: &Path, files: BTreeMap<String, Vec<u8>>) {
        let mut entries = Vec::new();
        for (path, bytes) in files {
            if path == "checksums.json" {
                continue;
            }
            entries.push(CapsuleFileEntry { path, bytes });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let checksums = checksums_for_entries(&entries);
        let checksum_entry = CapsuleFileEntry {
            path: "checksums.json".to_string(),
            bytes: serde_json::to_vec_pretty(&checksums).expect("checksums"),
        };
        write_capsule_file(
            path,
            &["source/", "source/generated/", "screenshots/", "prompt/"],
            &entries,
            &checksum_entry,
        )
        .expect("rewrite capsule");
    }

    fn exported_static_capsule(
        temp: &TempDir,
        manager: &WorkspaceManager,
        adapter: &TempAdapter,
        name: &str,
    ) -> (AppBoxManifest, PathBuf) {
        let source = manager
            .create_workspace_for_runtime_with_adapter(
                name.to_string(),
                "static-html".to_string(),
                adapter,
            )
            .expect("workspace");
        write_static_sample(&source);
        let capsule_path = temp.path().join(format!("{name}.sfcapsule"));
        export_app_capsule_with_adapter(
            manager,
            ExportAppCapsulePayload {
                app_id: source.app_id.clone(),
                include_prompt_history: false,
                output_path: capsule_path.clone(),
            },
            adapter,
        )
        .expect("export");
        (source, capsule_path)
    }

    #[test]
    fn export_structure_contains_required_files_and_checksums() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "Capsule Test".to_string(),
                "static-html".to_string(),
                &adapter,
            )
            .expect("workspace");
        write_static_sample(&manifest);
        fs::write(
            manifest.paths.root.join("prompt.history.jsonl"),
            "user prompt\n",
        )
        .expect("prompt");

        let capsule_path = temp.path().join("capsules").join("test.sfcapsule");
        let result = export_app_capsule_with_adapter(
            &manager,
            ExportAppCapsulePayload {
                app_id: manifest.app_id.clone(),
                include_prompt_history: true,
                output_path: capsule_path.clone(),
            },
            &adapter,
        )
        .expect("export");

        let names = zip_names(&capsule_path);
        assert!(names.contains(&"manifest.json".to_string()));
        assert!(names.contains(&"checksums.json".to_string()));
        assert!(names.contains(&"README.md".to_string()));
        assert!(names.contains(&"source/generated/static/index.html".to_string()));
        assert!(names.contains(&"prompt/history.jsonl".to_string()));
        assert!(names.contains(&"screenshots/".to_string()));
        assert_eq!(result.manifest.capsule_type, CAPSULE_TYPE);
        assert!(result
            .checksums
            .contains_key("source/generated/static/index.html"));
        assert!(!result.checksums.contains_key("checksums.json"));
    }

    #[test]
    fn export_only_includes_runtime_allowlisted_generated_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "Allowlisted Export".to_string(),
                "static-html".to_string(),
                &adapter,
            )
            .expect("workspace");
        write_static_sample(&manifest);
        fs::create_dir_all(manifest.paths.generated.join("static/nested")).expect("nested");
        fs::create_dir_all(manifest.paths.generated.join("node_modules/pkg"))
            .expect("node_modules");
        fs::write(
            manifest.paths.generated.join("static/nested/local.txt"),
            "local data",
        )
        .expect("local data");
        fs::write(
            manifest.paths.generated.join("node_modules/pkg/index.js"),
            "console.log('unexpected')",
        )
        .expect("node module");

        let capsule_path = temp.path().join("allowlist.sfcapsule");
        export_app_capsule_with_adapter(
            &manager,
            ExportAppCapsulePayload {
                app_id: manifest.app_id,
                include_prompt_history: false,
                output_path: capsule_path.clone(),
            },
            &adapter,
        )
        .expect("export");

        let names = zip_names(&capsule_path);
        assert!(names.contains(&"source/generated/static/index.html".to_string()));
        assert!(!names.contains(&"source/generated/static/nested/local.txt".to_string()));
        assert!(!names.contains(&"source/generated/node_modules/pkg/index.js".to_string()));
    }

    #[test]
    fn import_creates_new_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let source = manager
            .create_workspace_for_runtime_with_adapter(
                "Imported App".to_string(),
                "static-html".to_string(),
                &adapter,
            )
            .expect("workspace");
        write_static_sample(&source);
        let capsule_path = temp.path().join("import.sfcapsule");
        export_app_capsule_with_adapter(
            &manager,
            ExportAppCapsulePayload {
                app_id: source.app_id.clone(),
                include_prompt_history: false,
                output_path: capsule_path.clone(),
            },
            &adapter,
        )
        .expect("export");

        let imported =
            import_app_capsule_with_adapter(&manager, import_payload(capsule_path), &adapter)
                .expect("import");

        assert_ne!(imported.workspace.app_id, source.app_id);
        assert!(imported
            .app_manifest
            .paths
            .generated
            .join("static/index.html")
            .exists());
        let app_count = fs::read_dir(temp.path().join("data/apps"))
            .expect("apps root")
            .count();
        assert_eq!(app_count, 2);
    }

    #[test]
    fn import_rejects_sidecar_backed_runtime_capsules() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let source = manager
            .create_workspace_for_runtime_with_adapter(
                "Executable Capsule".to_string(),
                "react-vite".to_string(),
                &adapter,
            )
            .expect("workspace");
        write_generated_allowlist_sample(&source, "react-vite");
        let capsule_path = temp.path().join("react-vite.sfcapsule");
        export_app_capsule_with_adapter(
            &manager,
            ExportAppCapsulePayload {
                app_id: source.app_id,
                include_prompt_history: false,
                output_path: capsule_path.clone(),
            },
            &adapter,
        )
        .expect("export");

        let error =
            import_app_capsule_with_adapter(&manager, import_payload(capsule_path), &adapter)
                .expect_err("sidecar-backed capsule should fail");

        assert!(error
            .to_string()
            .contains("requires local sidecar execution"));
    }

    #[test]
    fn import_rejects_wrong_exact_runtime_pack_version() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let (_source, capsule_path) =
            exported_static_capsule(&temp, &manager, &adapter, "wrong-version");
        let mut files = read_capsule_files(&capsule_path).expect("files");
        let mut manifest: AppCapsuleManifest =
            serde_json::from_slice(required_file(&files, "manifest.json").expect("manifest"))
                .expect("manifest json");
        let mut lockfile: SofvaryLockfile =
            serde_json::from_slice(required_file(&files, "sofvary.lock.json").expect("lockfile"))
                .expect("lockfile json");
        lockfile
            .runtime_packs
            .insert(manifest.runtime.pack.id.clone(), "9.9.9".to_string());
        manifest.runtime.pack.version = "9.9.9".to_string();
        let lockfile_bytes = serde_json::to_vec_pretty(&lockfile).expect("lockfile bytes");
        manifest.lockfile.sha256 = Some(sha256_hex(&lockfile_bytes));
        for artifact in &mut manifest.artifacts {
            if artifact.path == "sofvary.lock.json" {
                artifact.sha256 = Some(sha256_hex(&lockfile_bytes));
                artifact.size_bytes = Some(lockfile_bytes.len() as u64);
            }
        }
        files.insert("sofvary.lock.json".to_string(), lockfile_bytes);
        files.insert(
            "manifest.json".to_string(),
            serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
        );
        rewrite_capsule_from_files(&capsule_path, files);

        let error =
            import_app_capsule_with_adapter(&manager, import_payload(capsule_path), &adapter)
                .expect_err("wrong version should fail");
        assert!(error.to_string().contains("missing runtime pack"));
    }

    #[test]
    fn import_rejects_missing_entry_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let (_source, capsule_path) =
            exported_static_capsule(&temp, &manager, &adapter, "missing-entry");
        let mut files = read_capsule_files(&capsule_path).expect("files");
        let mut manifest: AppCapsuleManifest =
            serde_json::from_slice(required_file(&files, "manifest.json").expect("manifest"))
                .expect("manifest json");
        manifest.entry.path = "source/generated/static/missing.html".to_string();
        files.insert(
            "manifest.json".to_string(),
            serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
        );
        rewrite_capsule_from_files(&capsule_path, files);

        let error =
            import_app_capsule_with_adapter(&manager, import_payload(capsule_path), &adapter)
                .expect_err("missing entry should fail");
        assert!(error.to_string().contains("entry path is missing"));
    }

    #[test]
    fn import_rejects_files_not_declared_as_artifacts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let (_source, capsule_path) =
            exported_static_capsule(&temp, &manager, &adapter, "extra-artifact");
        let mut files = read_capsule_files(&capsule_path).expect("files");
        files.insert(
            "source/generated/static/extra.html".to_string(),
            b"extra".to_vec(),
        );
        rewrite_capsule_from_files(&capsule_path, files);

        let error =
            import_app_capsule_with_adapter(&manager, import_payload(capsule_path), &adapter)
                .expect_err("undeclared file should fail");
        assert!(error
            .to_string()
            .contains("artifacts do not match capsule files"));
    }

    #[test]
    fn import_rejects_declared_generated_files_outside_runtime_allowlist() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let (_source, capsule_path) =
            exported_static_capsule(&temp, &manager, &adapter, "declared-extra");
        let mut files = read_capsule_files(&capsule_path).expect("files");
        let mut manifest: AppCapsuleManifest =
            serde_json::from_slice(required_file(&files, "manifest.json").expect("manifest"))
                .expect("manifest json");
        let bytes = b"declared but not allowed".to_vec();
        let path = "source/generated/static/extra.html".to_string();
        manifest.artifacts.push(AppCapsuleArtifact {
            path: path.clone(),
            kind: "source".to_string(),
            sha256: Some(sha256_hex(&bytes)),
            size_bytes: Some(bytes.len() as u64),
        });
        manifest.artifacts.sort_by(|a, b| a.path.cmp(&b.path));
        files.insert(path, bytes);
        files.insert(
            "manifest.json".to_string(),
            serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
        );
        rewrite_capsule_from_files(&capsule_path, files);

        let error =
            import_app_capsule_with_adapter(&manager, import_payload(capsule_path), &adapter)
                .expect_err("declared extra generated file should fail");

        assert!(error
            .to_string()
            .contains("outside the static-html allowlist"));
    }

    #[test]
    fn checksum_mismatch_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let source = manager
            .create_workspace_for_runtime_with_adapter(
                "Checksum".to_string(),
                "static-html".to_string(),
                &adapter,
            )
            .expect("workspace");
        write_static_sample(&source);
        let lockfile = manager
            .read_lockfile_for_manifest(&source)
            .expect("lockfile");
        let lockfile_bytes = serde_json::to_vec_pretty(&lockfile).expect("lockfile bytes");
        let manifest = AppCapsuleManifest {
            schema_version: CAPSULE_SCHEMA_VERSION.to_string(),
            capsule_type: CAPSULE_TYPE.to_string(),
            id: source.app_id.clone(),
            name: source.name.clone(),
            version: CAPSULE_APP_VERSION.to_string(),
            author: AppCapsuleAuthor {
                id: None,
                name: Some("local-user".to_string()),
                profile_url: None,
            },
            created_at: source.updated_at.clone(),
            runtime: AppCapsuleRuntime {
                kind: "static-html".to_string(),
                pack: single_locked_pack(&lockfile.runtime_packs, "runtime").expect("runtime"),
                generated_root: "source/generated".to_string(),
            },
            harness: AppCapsuleHarness {
                packs: locked_packs_from_map(&lockfile.harness_packs),
            },
            plugins: AppCapsulePlugins { packs: Vec::new() },
            permissions: AppCapsulePermissions {
                network: "local-only".to_string(),
                requested: Vec::new(),
                filesystem: AppCapsuleFilesystemPermissions {
                    read: vec!["source/generated".to_string()],
                    write: Vec::new(),
                },
            },
            ai_provider_requirements: None,
            entry: AppCapsuleEntry {
                path: "source/generated/static/index.html".to_string(),
                kind: "html".to_string(),
            },
            database: AppCapsuleDatabase {
                engine: "none".to_string(),
                include_data: false,
                schema: Vec::new(),
                migrations: Vec::new(),
                seed: Vec::new(),
                excluded_data: Vec::new(),
            },
            prompt: AppCapsulePrompt {
                included: false,
                history_path: None,
                redacted: true,
            },
            artifacts: Vec::new(),
            lockfile: AppCapsuleLockfile {
                path: "sofvary.lock.json".to_string(),
                sha256: Some(sha256_hex(&lockfile_bytes)),
            },
        };
        let entries = vec![
            CapsuleFileEntry {
                path: "manifest.json".to_string(),
                bytes: serde_json::to_vec_pretty(&manifest).expect("manifest"),
            },
            CapsuleFileEntry {
                path: "README.md".to_string(),
                bytes: b"readme".to_vec(),
            },
            CapsuleFileEntry {
                path: "source/generated/static/index.html".to_string(),
                bytes: b"real content".to_vec(),
            },
            CapsuleFileEntry {
                path: "sofvary.lock.json".to_string(),
                bytes: lockfile_bytes,
            },
        ];
        let checksum_entry = CapsuleFileEntry {
            path: "checksums.json".to_string(),
            bytes: serde_json::to_vec_pretty(&BTreeMap::from([
                ("manifest.json".to_string(), sha256_hex(&entries[0].bytes)),
                ("README.md".to_string(), sha256_hex(&entries[1].bytes)),
                (
                    "source/generated/static/index.html".to_string(),
                    "bad".to_string(),
                ),
                (
                    "sofvary.lock.json".to_string(),
                    sha256_hex(&entries[3].bytes),
                ),
            ]))
            .expect("checksums"),
        };
        let capsule_path = temp.path().join("bad-checksum.sfcapsule");
        write_capsule_file(
            &capsule_path,
            &["source/", "source/generated/", "screenshots/"],
            &entries,
            &checksum_entry,
        )
        .expect("write capsule");

        let error =
            import_app_capsule_with_adapter(&manager, import_payload(capsule_path), &adapter)
                .expect_err("checksum mismatch");
        assert!(error.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn secret_scanner_blocks_basic_secret_text() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "Secret".to_string(),
                "static-html".to_string(),
                &adapter,
            )
            .expect("workspace");
        fs::create_dir_all(manifest.paths.generated.join("static")).expect("static dir");
        fs::write(
            manifest.paths.generated.join("static/index.html"),
            format!(
                "const api_key = \"{}\";",
                ["sk", "_live_very_secret_value"].concat()
            ),
        )
        .expect("secret");

        let error = export_app_capsule_with_adapter(
            &manager,
            ExportAppCapsulePayload {
                app_id: manifest.app_id,
                include_prompt_history: false,
                output_path: temp.path().join("secret.sfcapsule"),
            },
            &adapter,
        )
        .expect_err("secret should block export");
        assert!(error.to_string().contains("secret scanner"));
        let private_key_sample = [
            "-----BEGIN OPENSSH ",
            "PRIVATE KEY-----\nabc\n-----END OPENSSH ",
            "PRIVATE KEY-----",
        ]
        .concat();
        assert!(scan_for_secrets("private.pem", private_key_sample.as_bytes()).is_err());
        assert!(scan_for_secrets("env", b"password = \"very_secret_password_value\"").is_err());
        assert!(scan_for_secrets("env", b"token: \"very_secret_token_value\"").is_err());
    }

    #[test]
    fn sqlite_app_database_is_excluded_but_schema_migrations_and_seed_are_retained() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_for_runtime_with_adapter(
                "SQLite".to_string(),
                "react-sqlite".to_string(),
                &adapter,
            )
            .expect("workspace");
        write_generated_allowlist_sample(&manifest, "react-sqlite");
        fs::write(manifest.paths.generated.join("data/app.sqlite"), b"real db").expect("db");
        fs::write(
            manifest.paths.generated.join("data/local-cache.json"),
            "{\"token\":\"not exported\"}",
        )
        .expect("extra data");

        let capsule_path = temp.path().join("sqlite.sfcapsule");
        let result = export_app_capsule_with_adapter(
            &manager,
            ExportAppCapsulePayload {
                app_id: manifest.app_id,
                include_prompt_history: false,
                output_path: capsule_path.clone(),
            },
            &adapter,
        )
        .expect("export");

        let names = zip_names(&capsule_path);
        assert!(!names.contains(&"source/generated/data/app.sqlite".to_string()));
        assert!(!names.contains(&"source/generated/data/local-cache.json".to_string()));
        assert!(names.contains(&"source/generated/data/schema.json".to_string()));
        assert!(names
            .contains(&"source/generated/data/migrations/001_create_customers.sql".to_string()));
        assert!(names.contains(&"source/generated/data/seed.sql".to_string()));
        assert!(result
            .manifest
            .database
            .schema
            .contains(&"source/generated/data/schema.json".to_string()));
        assert!(result
            .manifest
            .database
            .migrations
            .contains(&"source/generated/data/migrations/001_create_customers.sql".to_string()));
        assert!(result
            .manifest
            .database
            .seed
            .contains(&"source/generated/data/seed.sql".to_string()));
    }

    #[test]
    fn path_traversal_entry_is_blocked() {
        let temp = tempfile::tempdir().expect("tempdir");
        let capsule_path = temp.path().join("traversal.sfcapsule");
        let file = fs::File::create(&capsule_path).expect("file");
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("../evil.txt", options).expect("entry");
        zip.write_all(b"evil").expect("write");
        zip.finish().expect("finish");

        let adapter = temp_adapter(&temp);
        let manager = WorkspaceManager::new();
        let error =
            import_app_capsule_with_adapter(&manager, import_payload(capsule_path), &adapter)
                .expect_err("traversal should fail");
        assert!(error.to_string().contains("escapes archive root"));
    }

    #[test]
    fn capsule_rejects_too_many_zip_entries() {
        let temp = tempfile::tempdir().expect("tempdir");
        let capsule_path = temp.path().join("too-many.sfcapsule");
        let file = fs::File::create(&capsule_path).expect("file");
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for index in 0..=MAX_CAPSULE_ARCHIVE_ENTRIES {
            zip.start_file(format!("source/generated/static/{index}.txt"), options)
                .expect("entry");
            zip.write_all(b"x").expect("write");
        }
        zip.finish().expect("finish");

        let error = read_capsule_files(&capsule_path).expect_err("entry count should fail");
        assert!(error.to_string().contains("too many zip entries"));
    }

    #[test]
    fn capsule_rejects_oversized_zip_entry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let capsule_path = temp.path().join("oversized-entry.sfcapsule");
        let file = fs::File::create(&capsule_path).expect("file");
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("source/generated/static/big.bin", options)
            .expect("entry");
        zip.write_all(&vec![0; (MAX_CAPSULE_ENTRY_BYTES + 1) as usize])
            .expect("write");
        zip.finish().expect("finish");

        let error = read_capsule_files(&capsule_path).expect_err("entry size should fail");
        assert!(error.to_string().contains("per-file limit"));
    }

    #[test]
    fn non_normalized_capsule_entry_is_blocked() {
        let temp = tempfile::tempdir().expect("tempdir");
        let capsule_path = temp.path().join("non-normalized.sfcapsule");
        let file = fs::File::create(&capsule_path).expect("file");
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("source//generated/static/index.html", options)
            .expect("entry");
        zip.write_all(b"html").expect("write");
        zip.finish().expect("finish");

        let error = read_capsule_files(&capsule_path).expect_err("non-normalized path should fail");
        assert!(error.to_string().contains("not normalized"));
    }

    #[test]
    fn semver_manifest_version_rejects_empty_prerelease_or_build_metadata() {
        assert!(validate_semver_like("1.2.3-alpha.1+build.5").is_ok());
        assert!(validate_semver_like("1.2.3-").is_err());
        assert!(validate_semver_like("1.2.3+").is_err());
        assert!(validate_semver_like("1.2.3-01").is_err());
    }

    #[test]
    fn manifest_metadata_paths_must_be_normalized() {
        let mut manifest = minimal_capsule_manifest_for_validation();
        manifest.permissions.filesystem.read = vec!["source//generated".to_string()];

        let error = validate_capsule_manifest(&manifest).expect_err("non-normalized manifest path");
        assert!(error.to_string().contains("not normalized"));
    }

    #[test]
    fn ai_agent_capsule_requires_provider_requirements_metadata() {
        let mut manifest = minimal_capsule_manifest_for_validation();
        manifest.runtime.kind = "ai-agent-app".to_string();
        manifest.runtime.pack.id = "test.runtime.ai-agent-app".to_string();

        let missing =
            validate_capsule_manifest(&manifest).expect_err("missing provider requirements");
        assert!(missing.to_string().contains("provider requirements"));

        manifest.ai_provider_requirements = Some(AppCapsuleAiProviderRequirements {
            requirements: vec![AppCapsuleAiProviderRequirement {
                provider: "openai".to_string(),
                capabilities: vec!["text".to_string(), "image".to_string(), "video".to_string()],
                models: vec!["gpt-5".to_string(), "gpt-image-1".to_string()],
                credential_kind: Some("api-key".to_string()),
                required: Some(true),
                purpose: Some("Runtime binding metadata only".to_string()),
            }],
            secrets_included: false,
        });

        validate_capsule_manifest(&manifest).expect("valid provider requirements");
    }

    #[test]
    fn ai_agent_capsule_rejects_secret_included_requirements() {
        let mut manifest = minimal_capsule_manifest_for_validation();
        manifest.runtime.kind = "ai-agent-app".to_string();
        manifest.runtime.pack.id = "test.runtime.ai-agent-app".to_string();
        manifest.ai_provider_requirements = Some(AppCapsuleAiProviderRequirements {
            requirements: vec![AppCapsuleAiProviderRequirement {
                provider: "openai".to_string(),
                capabilities: vec!["text".to_string()],
                models: vec!["gpt-5".to_string()],
                credential_kind: Some("api-key".to_string()),
                required: Some(true),
                purpose: Some("Runtime binding metadata only".to_string()),
            }],
            secrets_included: true,
        });

        let error = validate_capsule_manifest(&manifest).expect_err("secret export blocked");
        assert!(error.to_string().contains("must not include secrets"));
    }

    fn minimal_capsule_manifest_for_validation() -> AppCapsuleManifest {
        let runtime_pack = AppCapsuleLockedPack {
            id: "sofvary.runtime.static-html".to_string(),
            version: "0.1.0".to_string(),
        };
        AppCapsuleManifest {
            schema_version: CAPSULE_SCHEMA_VERSION.to_string(),
            capsule_type: CAPSULE_TYPE.to_string(),
            id: "app-test".to_string(),
            name: "Test App".to_string(),
            version: "0.1.0".to_string(),
            author: AppCapsuleAuthor {
                id: None,
                name: Some("local-user".to_string()),
                profile_url: None,
            },
            created_at: "2026-06-05T00:00:00Z".to_string(),
            runtime: AppCapsuleRuntime {
                kind: "static-html".to_string(),
                pack: runtime_pack,
                generated_root: "source/generated".to_string(),
            },
            harness: AppCapsuleHarness {
                packs: vec![AppCapsuleLockedPack {
                    id: "sofvary.harness.static-html".to_string(),
                    version: "0.1.0".to_string(),
                }],
            },
            plugins: AppCapsulePlugins { packs: Vec::new() },
            permissions: AppCapsulePermissions {
                network: "local-only".to_string(),
                requested: Vec::new(),
                filesystem: AppCapsuleFilesystemPermissions {
                    read: vec!["source/generated".to_string()],
                    write: Vec::new(),
                },
            },
            ai_provider_requirements: None,
            entry: AppCapsuleEntry {
                path: "source/generated/static/index.html".to_string(),
                kind: "html".to_string(),
            },
            database: AppCapsuleDatabase {
                engine: "none".to_string(),
                include_data: false,
                schema: Vec::new(),
                migrations: Vec::new(),
                seed: Vec::new(),
                excluded_data: Vec::new(),
            },
            prompt: AppCapsulePrompt {
                included: false,
                history_path: None,
                redacted: true,
            },
            artifacts: vec![
                AppCapsuleArtifact {
                    path: "manifest.json".to_string(),
                    kind: "manifest".to_string(),
                    sha256: None,
                    size_bytes: None,
                },
                AppCapsuleArtifact {
                    path: "checksums.json".to_string(),
                    kind: "checksum".to_string(),
                    sha256: None,
                    size_bytes: None,
                },
                AppCapsuleArtifact {
                    path: "README.md".to_string(),
                    kind: "readme".to_string(),
                    sha256: None,
                    size_bytes: None,
                },
                AppCapsuleArtifact {
                    path: "sofvary.lock.json".to_string(),
                    kind: "lockfile".to_string(),
                    sha256: None,
                    size_bytes: None,
                },
                AppCapsuleArtifact {
                    path: "source/generated/static/index.html".to_string(),
                    kind: "source".to_string(),
                    sha256: None,
                    size_bytes: None,
                },
            ],
            lockfile: AppCapsuleLockfile {
                path: "sofvary.lock.json".to_string(),
                sha256: None,
            },
        }
    }
}
