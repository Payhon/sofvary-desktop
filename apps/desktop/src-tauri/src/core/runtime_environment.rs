use crate::core::policy_engine::PolicyEngine;
use crate::core::policy_types::{PolicyApprovalSet, PolicyRuntimeEnvironmentInstallRequest};
use crate::platform::sidecar::platform_sidecar_dir;
use crate::platform::{
    current_adapter, ArchKind, CommandSpec, OsKind, PlatformAdapter, PlatformError, PlatformResult,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use chrono::Utc;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use thiserror::Error;
use zip::ZipArchive;

const PNPM_VERSION: &str = "10.12.3";
const PNPM_TARBALL_URL: &str = "https://registry.npmjs.org/pnpm/-/pnpm-10.12.3.tgz";
const PNPM_INTEGRITY: &str =
    "sha512-Rn3yxYYFYWVYCtbftUzqrZTFow+AiT697FpExapzwgWuSlu51e1ruE6nwkns54ZkK7tJ0GowffIY0D2kHDF0Fw==";
const MAX_NODE_ARTIFACT_BYTES: usize = 220 * 1024 * 1024;
const MAX_PNPM_TARBALL_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum RuntimeEnvironmentError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("http error: {0}")]
    Http(String),
    #[error("policy error: {0}")]
    Policy(#[from] crate::core::policy_engine::PolicyError),
    #[error("unsupported runtime environment: {0}")]
    Unsupported(String),
    #[error("invalid runtime environment artifact: {0}")]
    InvalidArtifact(String),
}

pub type RuntimeEnvironmentResult<T> = Result<T, RuntimeEnvironmentError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeEnvironmentKind {
    Nodejs,
    Python,
}

impl RuntimeEnvironmentKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Nodejs => "nodejs",
            Self::Python => "python",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeEnvironmentInstallState {
    Installed,
    NotInstalled,
    Installing,
    Failed,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeEnvironmentSource {
    Managed,
    ExternalPath,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentVersionOption {
    pub version: String,
    pub label: String,
    pub channel: String,
    pub recommended: bool,
    pub supported: bool,
    pub platform: String,
    pub artifact_url: String,
    pub sha256: String,
    pub pnpm_version: String,
    pub pnpm_artifact_url: String,
    pub pnpm_integrity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentCatalogItem {
    pub kind: RuntimeEnvironmentKind,
    pub label: String,
    pub description: String,
    pub required_tools: Vec<String>,
    pub supported: bool,
    pub versions: Vec<RuntimeEnvironmentVersionOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentToolStatus {
    pub name: String,
    pub ok: bool,
    pub version: Option<String>,
    pub executable: Option<PathBuf>,
    pub source: RuntimeEnvironmentSource,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentInstallRecord {
    pub kind: RuntimeEnvironmentKind,
    pub version: String,
    pub state: RuntimeEnvironmentInstallState,
    pub detail: String,
    pub checked_at: String,
    pub platform: String,
    pub sha256: String,
    #[serde(default)]
    pub install_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentStatus {
    pub catalog: RuntimeEnvironmentCatalogItem,
    pub active_version: Option<String>,
    pub install_state: RuntimeEnvironmentInstallState,
    pub detail: String,
    pub source: RuntimeEnvironmentSource,
    pub supported: bool,
    pub node: Option<RuntimeEnvironmentToolStatus>,
    pub pnpm: Option<RuntimeEnvironmentToolStatus>,
    pub last_install: Option<RuntimeEnvironmentInstallRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRuntimeEnvironmentInstallPayload {
    pub kind: RuntimeEnvironmentKind,
    pub version: String,
    #[serde(default)]
    pub policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveRuntimeEnvironmentPayload {
    pub kind: RuntimeEnvironmentKind,
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeEnvironmentRecordState {
    #[serde(default)]
    active_versions: Vec<RuntimeEnvironmentActiveVersion>,
    #[serde(default)]
    installs: Vec<RuntimeEnvironmentInstallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeEnvironmentActiveVersion {
    kind: RuntimeEnvironmentKind,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeToolchain {
    pub node: RuntimeEnvironmentToolStatus,
    pub pnpm: RuntimeEnvironmentToolStatus,
    pub source: RuntimeEnvironmentSource,
}

pub fn list_runtime_environment_catalog() -> Vec<RuntimeEnvironmentCatalogItem> {
    let adapter = current_adapter();
    list_runtime_environment_catalog_with_adapter(adapter.as_ref())
}

pub fn list_runtime_environment_catalog_with_adapter(
    adapter: &dyn PlatformAdapter,
) -> Vec<RuntimeEnvironmentCatalogItem> {
    let versions = nodejs_version_options(adapter);
    vec![RuntimeEnvironmentCatalogItem {
        kind: RuntimeEnvironmentKind::Nodejs,
        label: "Node.js Toolchain".to_string(),
        description:
            "Sofvary-managed Node.js plus pnpm for generated React/Vite runtimes and agents."
                .to_string(),
        required_tools: vec!["node".to_string(), "pnpm".to_string()],
        supported: versions.iter().any(|version| version.supported),
        versions,
    }]
}

pub fn get_runtime_environment_statuses() -> RuntimeEnvironmentResult<Vec<RuntimeEnvironmentStatus>>
{
    let adapter = current_adapter();
    get_runtime_environment_statuses_with_adapter(adapter.as_ref())
}

pub fn get_runtime_environment_statuses_with_adapter(
    adapter: &dyn PlatformAdapter,
) -> RuntimeEnvironmentResult<Vec<RuntimeEnvironmentStatus>> {
    let record_state = load_record_state(adapter)?;
    let catalog = list_runtime_environment_catalog_with_adapter(adapter);

    catalog
        .into_iter()
        .map(|item| status_for_catalog_item(adapter, item, &record_state))
        .collect()
}

pub fn start_runtime_environment_install(
    payload: StartRuntimeEnvironmentInstallPayload,
) -> RuntimeEnvironmentResult<RuntimeEnvironmentStatus> {
    let adapter = current_adapter();
    start_runtime_environment_install_with_adapter(payload, adapter.as_ref())
}

pub fn start_runtime_environment_install_with_adapter(
    payload: StartRuntimeEnvironmentInstallPayload,
    adapter: &dyn PlatformAdapter,
) -> RuntimeEnvironmentResult<RuntimeEnvironmentStatus> {
    let option = find_version_option(adapter, payload.kind, &payload.version)?;
    if !option.supported {
        return Err(RuntimeEnvironmentError::Unsupported(format!(
            "{} is not supported on {}",
            payload.version, option.platform
        )));
    }

    let subject = runtime_environment_install_subject(&option, payload.kind);
    let policy = PolicyEngine::new().evaluate_runtime_environment_install(
        PolicyRuntimeEnvironmentInstallRequest {
            kind: payload.kind.as_str().to_string(),
            version: payload.version.clone(),
            platform: option.platform.clone(),
            sha256: option.sha256.clone(),
            subject,
        },
    );
    PolicyEngine::new().enforce(policy, &payload.policy_approvals)?;

    save_install_record(
        adapter,
        RuntimeEnvironmentInstallRecord {
            kind: payload.kind,
            version: payload.version.clone(),
            state: RuntimeEnvironmentInstallState::Installing,
            detail: format!("Downloading {} {}.", option.label, option.version),
            checked_at: Utc::now().to_rfc3339(),
            platform: option.platform.clone(),
            sha256: option.sha256.clone(),
            install_path: None,
        },
    )?;

    let result = match payload.kind {
        RuntimeEnvironmentKind::Nodejs => install_nodejs_toolchain(adapter, &option),
        RuntimeEnvironmentKind::Python => Err(RuntimeEnvironmentError::Unsupported(
            "Python runtime environment management is reserved for a future phase.".to_string(),
        )),
    };

    match result {
        Ok(install_path) => {
            save_active_version(adapter, payload.kind, &payload.version)?;
            save_install_record(
                adapter,
                RuntimeEnvironmentInstallRecord {
                    kind: payload.kind,
                    version: payload.version.clone(),
                    state: RuntimeEnvironmentInstallState::Installed,
                    detail: format!(
                        "Installed {} into Sofvary runtime environments.",
                        option.label
                    ),
                    checked_at: Utc::now().to_rfc3339(),
                    platform: option.platform.clone(),
                    sha256: option.sha256.clone(),
                    install_path: Some(install_path),
                },
            )?;
        }
        Err(error) => {
            let detail = error.to_string();
            let _ = save_install_record(
                adapter,
                RuntimeEnvironmentInstallRecord {
                    kind: payload.kind,
                    version: payload.version.clone(),
                    state: RuntimeEnvironmentInstallState::Failed,
                    detail,
                    checked_at: Utc::now().to_rfc3339(),
                    platform: option.platform.clone(),
                    sha256: option.sha256.clone(),
                    install_path: None,
                },
            );
            return Err(error);
        }
    }

    get_runtime_environment_statuses_with_adapter(adapter)?
        .into_iter()
        .find(|status| status.catalog.kind == payload.kind)
        .ok_or_else(|| RuntimeEnvironmentError::Unsupported(payload.kind.as_str().to_string()))
}

pub fn set_active_runtime_environment_version(
    payload: SetActiveRuntimeEnvironmentPayload,
) -> RuntimeEnvironmentResult<RuntimeEnvironmentStatus> {
    let adapter = current_adapter();
    set_active_runtime_environment_version_with_adapter(payload, adapter.as_ref())
}

pub fn set_active_runtime_environment_version_with_adapter(
    payload: SetActiveRuntimeEnvironmentPayload,
    adapter: &dyn PlatformAdapter,
) -> RuntimeEnvironmentResult<RuntimeEnvironmentStatus> {
    let option = find_version_option(adapter, payload.kind, &payload.version)?;
    match payload.kind {
        RuntimeEnvironmentKind::Nodejs => {
            let install_path = nodejs_install_path(adapter, &option)?;
            if !install_path.is_dir() {
                return Err(RuntimeEnvironmentError::Unsupported(format!(
                    "Node.js {} is not installed in Sofvary data.",
                    payload.version
                )));
            }
            write_active_nodejs_sidecars(adapter, &install_path)?;
        }
        RuntimeEnvironmentKind::Python => {
            return Err(RuntimeEnvironmentError::Unsupported(
                "Python runtime environment management is reserved for a future phase.".to_string(),
            ));
        }
    }
    save_active_version(adapter, payload.kind, &payload.version)?;

    get_runtime_environment_statuses_with_adapter(adapter)?
        .into_iter()
        .find(|status| status.catalog.kind == payload.kind)
        .ok_or_else(|| RuntimeEnvironmentError::Unsupported(payload.kind.as_str().to_string()))
}

pub fn runtime_environment_install_subject_for(
    kind: RuntimeEnvironmentKind,
    version: &str,
) -> RuntimeEnvironmentResult<String> {
    let adapter = current_adapter();
    let option = find_version_option(adapter.as_ref(), kind, version)?;
    Ok(runtime_environment_install_subject(&option, kind))
}

pub fn resolve_node_toolchain_with_adapter(
    adapter: &dyn PlatformAdapter,
) -> RuntimeEnvironmentResult<NodeToolchain> {
    if let Ok(toolchain) = detect_node_toolchain(adapter, RuntimeEnvironmentSource::Managed) {
        return Ok(toolchain);
    }

    detect_node_toolchain(adapter, RuntimeEnvironmentSource::ExternalPath)
}

pub fn node_toolchain_available(adapter: &dyn PlatformAdapter) -> bool {
    resolve_node_toolchain_with_adapter(adapter).is_ok()
}

fn status_for_catalog_item(
    adapter: &dyn PlatformAdapter,
    catalog: RuntimeEnvironmentCatalogItem,
    record_state: &RuntimeEnvironmentRecordState,
) -> RuntimeEnvironmentResult<RuntimeEnvironmentStatus> {
    let active_version = active_version_for(record_state, catalog.kind);
    let last_install = latest_install_for(record_state, catalog.kind);

    if catalog.kind == RuntimeEnvironmentKind::Nodejs {
        let detected = resolve_node_toolchain_with_adapter(adapter);
        let (source, node, pnpm, install_state, detail) = match detected {
            Ok(toolchain) => (
                toolchain.source,
                Some(toolchain.node),
                Some(toolchain.pnpm),
                RuntimeEnvironmentInstallState::Installed,
                "Node.js toolchain is available.".to_string(),
            ),
            Err(error) => {
                let state = last_install
                    .as_ref()
                    .map(|record| record.state)
                    .filter(|state| {
                        matches!(
                            state,
                            RuntimeEnvironmentInstallState::Installing
                                | RuntimeEnvironmentInstallState::Failed
                        )
                    })
                    .unwrap_or(RuntimeEnvironmentInstallState::NotInstalled);
                (
                    RuntimeEnvironmentSource::Missing,
                    Some(tool_status_error("node", RuntimeEnvironmentSource::Missing, &error)),
                    Some(tool_status_error("pnpm", RuntimeEnvironmentSource::Missing, &error)),
                    state,
                    "Install a Sofvary-managed Node.js toolchain before running Node-backed runtimes.".to_string(),
                )
            }
        };
        return Ok(RuntimeEnvironmentStatus {
            supported: catalog.supported,
            catalog,
            active_version,
            install_state,
            detail,
            source,
            node,
            pnpm,
            last_install,
        });
    }

    Ok(RuntimeEnvironmentStatus {
        supported: false,
        catalog,
        active_version,
        install_state: RuntimeEnvironmentInstallState::Unsupported,
        detail: "Python runtime environment management is reserved for a future phase.".to_string(),
        source: RuntimeEnvironmentSource::Missing,
        node: None,
        pnpm: None,
        last_install,
    })
}

fn detect_node_toolchain(
    adapter: &dyn PlatformAdapter,
    source: RuntimeEnvironmentSource,
) -> RuntimeEnvironmentResult<NodeToolchain> {
    let node_executable = resolve_tool_executable(adapter, "node", source)?;
    let pnpm_executable = resolve_tool_executable(adapter, "pnpm", source)?;
    let node = probe_tool(adapter, "node", node_executable, source)?;
    let pnpm = probe_tool(adapter, "pnpm", pnpm_executable, source)?;
    Ok(NodeToolchain { node, pnpm, source })
}

fn resolve_tool_executable(
    adapter: &dyn PlatformAdapter,
    name: &str,
    source: RuntimeEnvironmentSource,
) -> RuntimeEnvironmentResult<PathBuf> {
    match source {
        RuntimeEnvironmentSource::Managed => Ok(adapter.resolve_sidecar_executable(name)?),
        RuntimeEnvironmentSource::ExternalPath => Ok(PathBuf::from(name)),
        RuntimeEnvironmentSource::Missing => Err(RuntimeEnvironmentError::Unsupported(format!(
            "{name} is missing"
        ))),
    }
}

fn probe_tool(
    adapter: &dyn PlatformAdapter,
    name: &str,
    executable: PathBuf,
    source: RuntimeEnvironmentSource,
) -> RuntimeEnvironmentResult<RuntimeEnvironmentToolStatus> {
    let output = adapter.run_process(CommandSpec {
        executable: executable.clone(),
        args: vec!["--version".to_string()],
        cwd: adapter.dirs()?.data_dir,
        env: HashMap::new(),
        allowed_network: false,
        timeout_ms: Some(10_000),
        kill_on_drop: true,
    })?;

    if output.status_code == Some(0) {
        return Ok(RuntimeEnvironmentToolStatus {
            name: name.to_string(),
            ok: true,
            version: Some(first_output_line(&output.stdout, &output.stderr)),
            executable: Some(executable),
            source,
            detail: "Detected.".to_string(),
        });
    }

    Err(RuntimeEnvironmentError::Unsupported(format!(
        "{name} --version failed: {}",
        summarize_command_output(&output.stderr, &output.stdout)
    )))
}

fn tool_status_error(
    name: &str,
    source: RuntimeEnvironmentSource,
    error: &RuntimeEnvironmentError,
) -> RuntimeEnvironmentToolStatus {
    RuntimeEnvironmentToolStatus {
        name: name.to_string(),
        ok: false,
        version: None,
        executable: None,
        source,
        detail: error.to_string(),
    }
}

fn install_nodejs_toolchain(
    adapter: &dyn PlatformAdapter,
    option: &RuntimeEnvironmentVersionOption,
) -> RuntimeEnvironmentResult<PathBuf> {
    let node_bytes = fetch_bytes(&option.artifact_url, MAX_NODE_ARTIFACT_BYTES)?;
    verify_sha256(&node_bytes, &option.sha256)?;

    let pnpm_bytes = fetch_bytes(&option.pnpm_artifact_url, MAX_PNPM_TARBALL_BYTES)?;
    verify_integrity(&pnpm_bytes, &option.pnpm_integrity)?;

    let install_path = nodejs_install_path(adapter, option)?;
    let temp_path = install_path.with_extension("tmp");
    if temp_path.exists() {
        fs::remove_dir_all(&temp_path)?;
    }
    if install_path.exists() {
        fs::remove_dir_all(&install_path)?;
    }
    fs::create_dir_all(&temp_path)?;

    let node_root = temp_path.join("node");
    fs::create_dir_all(&node_root)?;
    extract_node_artifact(adapter.os(), &node_bytes, &node_root)?;

    let pnpm_root = temp_path.join("pnpm");
    fs::create_dir_all(&pnpm_root)?;
    unpack_npm_package(&pnpm_bytes, &pnpm_root)?;

    fs::rename(&temp_path, &install_path)?;
    write_active_nodejs_sidecars(adapter, &install_path)?;
    Ok(install_path)
}

fn extract_node_artifact(
    os: OsKind,
    bytes: &[u8],
    output_dir: &Path,
) -> RuntimeEnvironmentResult<()> {
    match os {
        OsKind::Windows => extract_zip_root(bytes, output_dir),
        OsKind::Macos => extract_tar_gz_root(bytes, output_dir),
        OsKind::Linux => Err(RuntimeEnvironmentError::Unsupported(
            "Linux Node.js tar.xz extraction is reserved for a later cross-platform packaging phase."
                .to_string(),
        )),
    }
}

fn extract_zip_root(bytes: &[u8], output_dir: &Path) -> RuntimeEnvironmentResult<()> {
    let temp_dir = output_dir.join("extract");
    fs::create_dir_all(&temp_dir)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| RuntimeEnvironmentError::InvalidArtifact(error.to_string()))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| RuntimeEnvironmentError::InvalidArtifact(error.to_string()))?;
        let Some(enclosed) = file.enclosed_name().map(PathBuf::from) else {
            return Err(RuntimeEnvironmentError::InvalidArtifact(
                "zip entry escapes extraction directory".to_string(),
            ));
        };
        let target = temp_dir.join(enclosed);
        if file.is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = fs::File::create(&target)?;
            std::io::copy(&mut file, &mut output)?;
        }
    }

    promote_single_extracted_root(&temp_dir, output_dir)
}

fn extract_tar_gz_root(bytes: &[u8], output_dir: &Path) -> RuntimeEnvironmentResult<()> {
    let temp_dir = output_dir.join("extract");
    fs::create_dir_all(&temp_dir)?;
    let mut decoder = GzDecoder::new(bytes);
    let mut tar_bytes = Vec::new();
    decoder.read_to_end(&mut tar_bytes)?;
    unpack_restricted_tar(&tar_bytes, &temp_dir)?;
    promote_single_extracted_root(&temp_dir, output_dir)
}

fn promote_single_extracted_root(
    temp_dir: &Path,
    output_dir: &Path,
) -> RuntimeEnvironmentResult<()> {
    let roots = fs::read_dir(temp_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    let root = roots.iter().find(|path| path.is_dir()).ok_or_else(|| {
        RuntimeEnvironmentError::InvalidArtifact(
            "Node.js artifact does not contain a root directory".to_string(),
        )
    })?;

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        fs::rename(entry.path(), output_dir.join(entry.file_name()))?;
    }
    fs::remove_dir_all(temp_dir)?;
    Ok(())
}

fn unpack_npm_package(bytes: &[u8], output_dir: &Path) -> RuntimeEnvironmentResult<()> {
    let mut decoder = GzDecoder::new(bytes);
    let mut tar_bytes = Vec::new();
    decoder.read_to_end(&mut tar_bytes)?;
    let extract_dir = output_dir.join("extract");
    fs::create_dir_all(&extract_dir)?;
    unpack_restricted_tar(&tar_bytes, &extract_dir)?;
    let package_dir = extract_dir.join("package");
    if !package_dir.is_dir() {
        return Err(RuntimeEnvironmentError::InvalidArtifact(
            "pnpm tarball does not contain package/".to_string(),
        ));
    }
    for entry in fs::read_dir(&package_dir)? {
        let entry = entry?;
        fs::rename(entry.path(), output_dir.join(entry.file_name()))?;
    }
    fs::remove_dir_all(extract_dir)?;
    Ok(())
}

fn unpack_restricted_tar(bytes: &[u8], output_dir: &Path) -> RuntimeEnvironmentResult<()> {
    let mut offset = 0usize;
    while offset + 512 <= bytes.len() {
        let header = &bytes[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            break;
        }

        let relative_path = tar_path(header)?;
        let size = tar_size(header)?;
        let typeflag = header[156];
        let data_start = offset + 512;
        let data_end = data_start.checked_add(size).ok_or_else(|| {
            RuntimeEnvironmentError::InvalidArtifact("tar entry size overflow".to_string())
        })?;
        if data_end > bytes.len() {
            return Err(RuntimeEnvironmentError::InvalidArtifact(
                "tar entry exceeds archive length".to_string(),
            ));
        }

        let target = safe_tar_target(output_dir, &relative_path)?;
        match typeflag {
            b'0' | 0 => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(target, &bytes[data_start..data_end])?;
            }
            b'5' => {
                fs::create_dir_all(target)?;
            }
            b'2' | b'x' | b'g' => {
                // Node.js and npm tarballs can contain safe symlinks or pax metadata.
                // Sofvary only needs regular files and directories for managed sidecars.
            }
            _ => {
                return Err(RuntimeEnvironmentError::InvalidArtifact(format!(
                    "unsupported tar entry type for {}",
                    relative_path.display()
                )));
            }
        }

        offset = data_start + round_up_to_512(size);
    }
    Ok(())
}

fn tar_path(header: &[u8]) -> RuntimeEnvironmentResult<PathBuf> {
    let name = tar_string(&header[0..100]);
    let prefix = tar_string(&header[345..500]);
    let path = if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    };
    if path.is_empty() {
        return Err(RuntimeEnvironmentError::InvalidArtifact(
            "tar entry path is empty".to_string(),
        ));
    }
    Ok(PathBuf::from(path))
}

fn tar_size(header: &[u8]) -> RuntimeEnvironmentResult<usize> {
    let raw = tar_string(&header[124..136]);
    usize::from_str_radix(raw.trim(), 8).map_err(|error| {
        RuntimeEnvironmentError::InvalidArtifact(format!("invalid tar entry size: {error}"))
    })
}

fn tar_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn safe_tar_target(output_dir: &Path, relative_path: &Path) -> RuntimeEnvironmentResult<PathBuf> {
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(RuntimeEnvironmentError::InvalidArtifact(format!(
            "tar entry path escapes extraction directory: {}",
            relative_path.display()
        )));
    }
    Ok(output_dir.join(relative_path))
}

fn round_up_to_512(size: usize) -> usize {
    (size + 511) & !511
}

fn write_active_nodejs_sidecars(
    adapter: &dyn PlatformAdapter,
    install_path: &Path,
) -> RuntimeEnvironmentResult<()> {
    let dirs = adapter.dirs()?;
    let sidecar_dir = platform_sidecar_dir(&dirs, adapter.os(), adapter.arch());
    fs::create_dir_all(&sidecar_dir)?;

    for name in ["node.exe", "node", "pnpm.cmd", "pnpm.exe", "pnpm"] {
        let path = sidecar_dir.join(name);
        if path.exists() {
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }
    }

    let node_executable = node_executable_path(adapter.os(), install_path);
    let pnpm_cjs = install_path.join("pnpm").join("bin").join("pnpm.cjs");
    if !node_executable.is_file() {
        return Err(RuntimeEnvironmentError::InvalidArtifact(format!(
            "Node executable not found after install: {}",
            node_executable.display()
        )));
    }
    if !pnpm_cjs.is_file() {
        return Err(RuntimeEnvironmentError::InvalidArtifact(format!(
            "pnpm entrypoint not found after install: {}",
            pnpm_cjs.display()
        )));
    }
    make_executable(&node_executable)?;

    match adapter.os() {
        OsKind::Windows => {
            fs::copy(&node_executable, sidecar_dir.join("node.exe"))?;
            fs::write(
                sidecar_dir.join("pnpm.cmd"),
                format!(
                    "@echo off\r\n\"{}\" \"{}\" %*\r\n",
                    node_executable.display(),
                    pnpm_cjs.display()
                ),
            )?;
        }
        OsKind::Macos | OsKind::Linux => {
            let node_shim = sidecar_dir.join("node");
            let pnpm_shim = sidecar_dir.join("pnpm");
            fs::write(
                &node_shim,
                format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", node_executable.display()),
            )?;
            fs::write(
                &pnpm_shim,
                format!(
                    "#!/bin/sh\nexec \"{}\" \"{}\" \"$@\"\n",
                    node_executable.display(),
                    pnpm_cjs.display()
                ),
            )?;
            make_executable(&node_shim)?;
            make_executable(&pnpm_shim)?;
        }
    }

    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> RuntimeEnvironmentResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> RuntimeEnvironmentResult<()> {
    Ok(())
}

fn node_executable_path(os: OsKind, install_path: &Path) -> PathBuf {
    match os {
        OsKind::Windows => install_path.join("node").join("node.exe"),
        OsKind::Macos | OsKind::Linux => install_path.join("node").join("bin").join("node"),
    }
}

fn nodejs_install_path(
    adapter: &dyn PlatformAdapter,
    option: &RuntimeEnvironmentVersionOption,
) -> PlatformResult<PathBuf> {
    Ok(adapter
        .dirs()?
        .data_dir
        .join("runtime-environments")
        .join("nodejs")
        .join(&option.version)
        .join(&option.platform))
}

fn find_version_option(
    adapter: &dyn PlatformAdapter,
    kind: RuntimeEnvironmentKind,
    version: &str,
) -> RuntimeEnvironmentResult<RuntimeEnvironmentVersionOption> {
    list_runtime_environment_catalog_with_adapter(adapter)
        .into_iter()
        .find(|item| item.kind == kind)
        .and_then(|item| {
            item.versions
                .into_iter()
                .find(|item| item.version == version)
        })
        .ok_or_else(|| {
            RuntimeEnvironmentError::Unsupported(format!(
                "{} runtime environment version {version} is not in the Sofvary catalog",
                kind.as_str()
            ))
        })
}

fn runtime_environment_install_subject(
    option: &RuntimeEnvironmentVersionOption,
    kind: RuntimeEnvironmentKind,
) -> String {
    format!(
        "runtime-env:{}:{}:{}:{}",
        kind.as_str(),
        option.version,
        option.platform,
        option.sha256
    )
}

fn nodejs_version_options(adapter: &dyn PlatformAdapter) -> Vec<RuntimeEnvironmentVersionOption> {
    let platform = platform_slug(adapter.os(), adapter.arch());
    let entries = nodejs_artifact_entries()
        .into_iter()
        .filter(|entry| entry.platform == platform)
        .map(|entry| RuntimeEnvironmentVersionOption {
            version: entry.version.to_string(),
            label: format!("Node.js {}", entry.version),
            channel: entry.channel.to_string(),
            recommended: entry.recommended,
            supported: true,
            platform: entry.platform.to_string(),
            artifact_url: format!(
                "https://nodejs.org/dist/v{}/{}",
                entry.version, entry.file_name
            ),
            sha256: entry.sha256.to_string(),
            pnpm_version: PNPM_VERSION.to_string(),
            pnpm_artifact_url: PNPM_TARBALL_URL.to_string(),
            pnpm_integrity: PNPM_INTEGRITY.to_string(),
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return vec![
            unsupported_nodejs_option("24.16.0", "LTS", true, &platform),
            unsupported_nodejs_option("22.22.3", "Maintenance LTS", false, &platform),
        ];
    }

    entries
}

fn unsupported_nodejs_option(
    version: &str,
    channel: &str,
    recommended: bool,
    platform: &str,
) -> RuntimeEnvironmentVersionOption {
    RuntimeEnvironmentVersionOption {
        version: version.to_string(),
        label: format!("Node.js {version}"),
        channel: channel.to_string(),
        recommended,
        supported: false,
        platform: platform.to_string(),
        artifact_url: String::new(),
        sha256: String::new(),
        pnpm_version: PNPM_VERSION.to_string(),
        pnpm_artifact_url: PNPM_TARBALL_URL.to_string(),
        pnpm_integrity: PNPM_INTEGRITY.to_string(),
    }
}

#[derive(Debug, Clone, Copy)]
struct NodejsArtifactEntry {
    version: &'static str,
    channel: &'static str,
    recommended: bool,
    platform: &'static str,
    file_name: &'static str,
    sha256: &'static str,
}

fn nodejs_artifact_entries() -> Vec<NodejsArtifactEntry> {
    vec![
        NodejsArtifactEntry {
            version: "24.16.0",
            channel: "LTS",
            recommended: true,
            platform: "macos-arm64",
            file_name: "node-v24.16.0-darwin-arm64.tar.gz",
            sha256: "39189dab4eeb15706c424af0ac08a3044c9e48f7db12a7d77f6b7aafc7dd5df6",
        },
        NodejsArtifactEntry {
            version: "24.16.0",
            channel: "LTS",
            recommended: true,
            platform: "macos-x64",
            file_name: "node-v24.16.0-darwin-x64.tar.gz",
            sha256: "298b4c7b3cb80765c8703e42b90324a4ece3b6634947b89e769c3c980ab55185",
        },
        NodejsArtifactEntry {
            version: "24.16.0",
            channel: "LTS",
            recommended: true,
            platform: "windows-x64",
            file_name: "node-v24.16.0-win-x64.zip",
            sha256: "edaca9bd58ec8e92037dac4e877d52f6b8f430b81c18b57e264b4e2fb111cd56",
        },
        NodejsArtifactEntry {
            version: "24.16.0",
            channel: "LTS",
            recommended: true,
            platform: "windows-arm64",
            file_name: "node-v24.16.0-win-arm64.zip",
            sha256: "14834611d4c6b3c06054e7007732b90474c16e0b32f395e05b55a571ef71c6d2",
        },
        NodejsArtifactEntry {
            version: "22.22.3",
            channel: "Maintenance LTS",
            recommended: false,
            platform: "macos-arm64",
            file_name: "node-v22.22.3-darwin-arm64.tar.gz",
            sha256: "0da7ff74ef8611328c8212f17943368713a2ad953fb7d89a8c8a0eae87c23207",
        },
        NodejsArtifactEntry {
            version: "22.22.3",
            channel: "Maintenance LTS",
            recommended: false,
            platform: "macos-x64",
            file_name: "node-v22.22.3-darwin-x64.tar.gz",
            sha256: "45830ba752fa0d892c6dcd640946669801293cac820a33591ded40ac075198ec",
        },
        NodejsArtifactEntry {
            version: "22.22.3",
            channel: "Maintenance LTS",
            recommended: false,
            platform: "windows-x64",
            file_name: "node-v22.22.3-win-x64.zip",
            sha256: "6c8d54f635feff4df76c2ca80f45332eb2ff57d25226edce36592e51a177ee33",
        },
        NodejsArtifactEntry {
            version: "22.22.3",
            channel: "Maintenance LTS",
            recommended: false,
            platform: "windows-arm64",
            file_name: "node-v22.22.3-win-arm64.zip",
            sha256: "00be129a09e8872cd52d3bb8bba12412c5733d2224123a482a2dca4a6fbf2586",
        },
    ]
}

fn platform_slug(os: OsKind, arch: ArchKind) -> String {
    format!("{}-{}", os_slug(os), arch_slug(arch))
}

fn os_slug(os: OsKind) -> &'static str {
    match os {
        OsKind::Windows => "windows",
        OsKind::Macos => "macos",
        OsKind::Linux => "linux",
    }
}

fn arch_slug(arch: ArchKind) -> &'static str {
    match arch {
        ArchKind::X64 => "x64",
        ArchKind::Arm64 => "arm64",
        ArchKind::Unknown => "unknown",
    }
}

fn verify_sha256(bytes: &[u8], expected_hex: &str) -> RuntimeEnvironmentResult<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected_hex {
        return Err(RuntimeEnvironmentError::InvalidArtifact(
            "Node.js artifact sha256 check failed".to_string(),
        ));
    }
    Ok(())
}

fn verify_integrity(bytes: &[u8], integrity: &str) -> RuntimeEnvironmentResult<()> {
    let Some(encoded) = integrity
        .split_whitespace()
        .find_map(|token| token.strip_prefix("sha512-"))
    else {
        return Err(RuntimeEnvironmentError::InvalidArtifact(
            "pnpm integrity does not include sha512".to_string(),
        ));
    };
    let expected = BASE64_STANDARD
        .decode(encoded)
        .map_err(|error| RuntimeEnvironmentError::InvalidArtifact(error.to_string()))?;
    let actual = Sha512::digest(bytes);
    if expected.as_slice() != actual.as_slice() {
        return Err(RuntimeEnvironmentError::InvalidArtifact(
            "pnpm tarball integrity check failed".to_string(),
        ));
    }
    Ok(())
}

fn fetch_bytes(url: &str, max_bytes: usize) -> RuntimeEnvironmentResult<Vec<u8>> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| RuntimeEnvironmentError::Http(error.to_string()))?;
    response
        .body_mut()
        .with_config()
        .limit(max_bytes as u64)
        .read_to_vec()
        .map_err(|error| RuntimeEnvironmentError::Http(error.to_string()))
}

fn first_output_line(stdout: &str, stderr: &str) -> String {
    let source = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn summarize_command_output(stderr: &str, stdout: &str) -> String {
    let first_line = first_output_line(stdout, stderr);
    first_line.chars().take(360).collect()
}

fn active_version_for(
    state: &RuntimeEnvironmentRecordState,
    kind: RuntimeEnvironmentKind,
) -> Option<String> {
    state
        .active_versions
        .iter()
        .find(|active| active.kind == kind)
        .map(|active| active.version.clone())
}

fn latest_install_for(
    state: &RuntimeEnvironmentRecordState,
    kind: RuntimeEnvironmentKind,
) -> Option<RuntimeEnvironmentInstallRecord> {
    state
        .installs
        .iter()
        .rev()
        .find(|install| install.kind == kind)
        .cloned()
}

fn save_active_version(
    adapter: &dyn PlatformAdapter,
    kind: RuntimeEnvironmentKind,
    version: &str,
) -> RuntimeEnvironmentResult<()> {
    let mut state = load_record_state(adapter)?;
    state.active_versions.retain(|active| active.kind != kind);
    state.active_versions.push(RuntimeEnvironmentActiveVersion {
        kind,
        version: version.to_string(),
    });
    save_record_state(adapter, &state)
}

fn save_install_record(
    adapter: &dyn PlatformAdapter,
    record: RuntimeEnvironmentInstallRecord,
) -> RuntimeEnvironmentResult<()> {
    let mut state = load_record_state(adapter)?;
    state.installs.retain(|existing| {
        !(existing.kind == record.kind
            && existing.version == record.version
            && existing.platform == record.platform)
    });
    state.installs.push(record);
    save_record_state(adapter, &state)
}

fn load_record_state(
    adapter: &dyn PlatformAdapter,
) -> RuntimeEnvironmentResult<RuntimeEnvironmentRecordState> {
    let path = record_path(adapter)?;
    if !path.exists() {
        return Ok(RuntimeEnvironmentRecordState::default());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn save_record_state(
    adapter: &dyn PlatformAdapter,
    state: &RuntimeEnvironmentRecordState,
) -> RuntimeEnvironmentResult<()> {
    let path = record_path(adapter)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state)? + "\n")?;
    Ok(())
}

fn record_path(adapter: &dyn PlatformAdapter) -> PlatformResult<PathBuf> {
    Ok(adapter.dirs()?.data_dir.join("runtime-environments.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{PlatformDirs, ProcessHandle, ProcessOutput, WebviewProfile};
    use std::sync::{Arc, Mutex};

    #[test]
    fn catalog_filters_to_current_platform_and_recommends_lts() {
        let adapter = TestAdapter::new(OsKind::Windows, ArchKind::X64);
        let catalog = list_runtime_environment_catalog_with_adapter(&adapter);
        let node = catalog
            .into_iter()
            .find(|item| item.kind == RuntimeEnvironmentKind::Nodejs)
            .expect("node catalog");

        assert!(node.supported);
        assert_eq!(node.versions[0].platform, "windows-x64");
        assert!(node.versions.iter().any(|version| {
            version.version == "24.16.0" && version.recommended && version.channel == "LTS"
        }));
    }

    #[test]
    fn catalog_supports_macos_arm64_node_toolchain() {
        let adapter = TestAdapter::new(OsKind::Macos, ArchKind::Arm64);
        let catalog = list_runtime_environment_catalog_with_adapter(&adapter);
        let node = catalog
            .into_iter()
            .find(|item| item.kind == RuntimeEnvironmentKind::Nodejs)
            .expect("node catalog");

        assert!(node.supported);
        assert_eq!(node.versions[0].platform, "macos-arm64");
        assert_eq!(node.versions[0].version, "24.16.0");
        assert!(node.versions[0]
            .artifact_url
            .ends_with("node-v24.16.0-darwin-arm64.tar.gz"));
        assert_eq!(
            node.versions[0].sha256,
            "39189dab4eeb15706c424af0ac08a3044c9e48f7db12a7d77f6b7aafc7dd5df6"
        );
    }

    #[test]
    fn unsupported_platform_keeps_catalog_shape_without_install_support() {
        let adapter = TestAdapter::new(OsKind::Linux, ArchKind::X64);
        let catalog = list_runtime_environment_catalog_with_adapter(&adapter);
        let node = &catalog[0];

        assert!(!node.supported);
        assert!(node.versions.iter().all(|version| !version.supported));
    }

    #[test]
    fn runtime_environment_subject_is_exact_to_version_platform_and_hash() {
        let adapter = TestAdapter::new(OsKind::Windows, ArchKind::X64);
        let option = find_version_option(&adapter, RuntimeEnvironmentKind::Nodejs, "24.16.0")
            .expect("catalog option");

        assert_eq!(
            runtime_environment_install_subject(&option, RuntimeEnvironmentKind::Nodejs),
            "runtime-env:nodejs:24.16.0:windows-x64:edaca9bd58ec8e92037dac4e877d52f6b8f430b81c18b57e264b4e2fb111cd56"
        );
    }

    #[test]
    fn managed_toolchain_wins_over_external_path() {
        let adapter = TestAdapter::new(OsKind::Windows, ArchKind::X64).with_managed_sidecars();
        let toolchain = resolve_node_toolchain_with_adapter(&adapter).expect("toolchain");

        assert_eq!(toolchain.source, RuntimeEnvironmentSource::Managed);
        assert_eq!(
            toolchain.node.executable,
            Some(adapter.dirs.data_dir.join("sidecars/windows-x64/node.exe"))
        );
    }

    #[test]
    fn sha256_mismatch_is_rejected_before_activation() {
        let result = verify_sha256(
            b"not node",
            "edaca9bd58ec8e92037dac4e877d52f6b8f430b81c18b57e264b4e2fb111cd56",
        );

        assert!(matches!(
            result,
            Err(RuntimeEnvironmentError::InvalidArtifact(message))
                if message.contains("sha256")
        ));
    }

    #[derive(Clone)]
    struct TestAdapter {
        os: OsKind,
        arch: ArchKind,
        dirs: PlatformDirs,
        managed: bool,
        calls: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl TestAdapter {
        fn new(os: OsKind, arch: ArchKind) -> Self {
            let temp = tempfile::tempdir().expect("tempdir");
            let root = temp.path().to_path_buf();
            Self {
                os,
                arch,
                dirs: PlatformDirs {
                    data_dir: root.join("data"),
                    cache_dir: root.join("cache"),
                    config_dir: root.join("config"),
                },
                managed: false,
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_managed_sidecars(mut self) -> Self {
            self.managed = true;
            self
        }
    }

    impl PlatformAdapter for TestAdapter {
        fn os(&self) -> OsKind {
            self.os
        }

        fn arch(&self) -> ArchKind {
            self.arch
        }

        fn dirs(&self) -> PlatformResult<PlatformDirs> {
            Ok(self.dirs.clone())
        }

        fn normalize_path(&self, input: &str) -> PlatformResult<PathBuf> {
            Ok(PathBuf::from(input))
        }

        fn ensure_executable(&self, _path: &Path) -> PlatformResult<()> {
            Ok(())
        }

        fn resolve_sidecar_executable(&self, name: &str) -> PlatformResult<PathBuf> {
            if !self.managed {
                return Err(PlatformError::InvalidPath(format!("missing {name}")));
            }
            let file = match (name, self.os) {
                ("node", OsKind::Windows) => "node.exe",
                ("pnpm", OsKind::Windows) => "pnpm.cmd",
                ("node", _) => "node",
                ("pnpm", _) => "pnpm",
                _ => return Err(PlatformError::Unsupported(name.to_string())),
            };
            Ok(self
                .dirs
                .data_dir
                .join("sidecars")
                .join(platform_slug(self.os, self.arch))
                .join(file))
        }

        fn run_process(&self, spec: CommandSpec) -> PlatformResult<ProcessOutput> {
            self.calls
                .lock()
                .expect("calls")
                .push(spec.executable.clone());
            let executable = spec.executable.display().to_string();
            let version = if executable.contains("sidecars") {
                if executable.contains("pnpm") {
                    "10.12.3"
                } else {
                    "v24.16.0"
                }
            } else if executable.contains("pnpm") {
                "9.0.0"
            } else {
                "v22.0.0"
            };
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: format!("{version}\n"),
                stderr: String::new(),
            })
        }

        fn spawn_process(&self, _spec: CommandSpec) -> PlatformResult<ProcessHandle> {
            Err(PlatformError::Unsupported("spawn".to_string()))
        }

        fn kill_process_tree(&self, _pid: u32) -> PlatformResult<()> {
            Ok(())
        }

        fn allocate_local_port(&self) -> PlatformResult<u16> {
            Ok(3000)
        }

        fn open_external(&self, _url: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn reveal_path(&self, _path: &Path) -> PlatformResult<()> {
            Ok(())
        }

        fn register_protocol_handler(&self, _protocol: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn register_global_shortcut(&self, _accelerator: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn unregister_global_shortcut(&self, _accelerator: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn show_tray_or_menu_bar_item(&self) -> PlatformResult<()> {
            Ok(())
        }

        fn get_active_monitor_work_area(&self) -> PlatformResult<crate::platform::WorkArea> {
            Ok(crate::platform::WorkArea {
                x: 0,
                y: 0,
                width: 1200,
                height: 760,
            })
        }

        fn secure_store_set(&self, _key: &str, _value: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn secure_store_get(&self, _key: &str) -> PlatformResult<Option<String>> {
            Ok(None)
        }

        fn current_webview_profile(&self) -> WebviewProfile {
            WebviewProfile {
                engine: "test".to_string(),
                supports_transparency: true,
                notes: Vec::new(),
            }
        }
    }
}
