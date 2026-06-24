use crate::core::agent_catalog::discover_agents_with_adapter;
use crate::core::agent_config::{
    fresh_test_record, AgentCommandConfig, AgentConfig, AgentConfigError, AgentConfigStore,
    AgentInstallSource, AgentProvider, AgentTestRecord, AgentTransportKind,
};
use crate::core::policy_engine::PolicyEngine;
use crate::core::policy_types::{PolicyAgentInstallRequest, PolicyApprovalSet};
use crate::core::runtime_environment::{
    node_toolchain_available, resolve_node_toolchain_with_adapter, NodeToolchain,
};
use crate::platform::{
    current_adapter, ArchKind, CommandSpec, OsKind, PlatformAdapter, PlatformError, PlatformResult,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use chrono::Utc;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

const PI_NPM_PACKAGE: &str = "@earendil-works/pi-coding-agent";
const PI_NPM_METADATA_URL: &str =
    "https://registry.npmjs.org/@earendil-works%2Fpi-coding-agent/latest";
const MAX_PI_TARBALL_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum AgentInstallError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("agent config error: {0}")]
    AgentConfig(#[from] AgentConfigError),
    #[error("http error: {0}")]
    Http(String),
    #[error("policy error: {0}")]
    Policy(#[from] crate::core::policy_engine::PolicyError),
    #[error("unsupported agent install: {0}")]
    Unsupported(String),
    #[error("invalid agent install artifact: {0}")]
    InvalidArtifact(String),
}

pub type AgentInstallResult<T> = Result<T, AgentInstallError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentInstallCapability {
    Managed,
    ManualDownload,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentInstallStateKind {
    Installed,
    NotInstalled,
    Installing,
    Failed,
    Manual,
    NeedsRuntime,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstallCommandTemplate {
    pub executable: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstallCatalogItem {
    pub id: String,
    pub label: String,
    pub icon_key: String,
    pub provider: AgentProvider,
    pub docs_url: String,
    pub install_capability: AgentInstallCapability,
    pub recommended: bool,
    pub managed: bool,
    pub supported: bool,
    pub detect_commands: Vec<String>,
    pub acp: Option<AgentInstallCommandTemplate>,
    pub cli: Option<AgentInstallCommandTemplate>,
    pub version_command: Option<AgentInstallCommandTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstallRecord {
    pub agent_id: String,
    pub state: AgentInstallStateKind,
    pub detail: String,
    pub checked_at: String,
    #[serde(default)]
    pub install_method: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub executable: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentInstallRecordState {
    #[serde(default)]
    records: Vec<AgentInstallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstallStatus {
    pub catalog: AgentInstallCatalogItem,
    #[serde(default)]
    pub configured: Option<AgentConfig>,
    pub detected: bool,
    #[serde(default)]
    pub source: Option<AgentInstallSource>,
    #[serde(default)]
    pub executable: Option<PathBuf>,
    #[serde(default)]
    pub version: Option<String>,
    pub install_state: AgentInstallStateKind,
    pub detail: String,
    #[serde(default)]
    pub last_test: Option<AgentTestRecord>,
    #[serde(default)]
    pub last_install: Option<AgentInstallRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAgentInstallPayload {
    pub agent_id: String,
    #[serde(default)]
    pub policy_approvals: PolicyApprovalSet,
}

pub fn list_agent_install_catalog() -> Vec<AgentInstallCatalogItem> {
    let adapter = current_adapter();
    list_agent_install_catalog_with_adapter(adapter.as_ref())
}

pub fn list_agent_install_catalog_with_adapter(
    adapter: &dyn PlatformAdapter,
) -> Vec<AgentInstallCatalogItem> {
    install_catalog()
        .into_iter()
        .map(|mut item| {
            item.supported = supported_on_current_platform(adapter, &item);
            item
        })
        .collect()
}

pub fn get_agent_install_statuses(
    agent_store: &AgentConfigStore,
) -> AgentInstallResult<Vec<AgentInstallStatus>> {
    let adapter = current_adapter();
    get_agent_install_statuses_with_adapter(agent_store, adapter.as_ref())
}

pub fn get_agent_install_statuses_with_adapter(
    agent_store: &AgentConfigStore,
    adapter: &dyn PlatformAdapter,
) -> AgentInstallResult<Vec<AgentInstallStatus>> {
    let catalog = list_agent_install_catalog_with_adapter(adapter);
    let discovered = discover_agents_with_adapter(adapter)?;
    let config_state = agent_store.load_with_adapter(adapter)?;
    let record_state = load_record_state(adapter)?;

    Ok(catalog
        .into_iter()
        .map(|item| {
            let discovered_agent = discovered
                .iter()
                .find(|agent| agent.config.id == item.id && agent.detected);
            let configured = config_state
                .agents
                .iter()
                .find(|agent| agent.id == item.id)
                .cloned();
            let last_install = record_state
                .records
                .iter()
                .find(|record| record.agent_id == item.id)
                .cloned();
            let command = discovered_agent.and_then(|agent| {
                agent
                    .config
                    .acp
                    .as_ref()
                    .or(agent.config.cli.as_ref())
                    .cloned()
            });
            let detected = command.is_some();
            let last_test = configured
                .as_ref()
                .and_then(|agent| agent.last_test.clone());
            let (install_state, mut detail) =
                install_state_for_item(adapter, &item, detected, last_install.as_ref());
            if item.id == "codex"
                && discovered_agent
                    .and_then(|agent| agent.config.acp.as_ref())
                    .is_some()
                && discovered_agent
                    .and_then(|agent| agent.config.cli.as_ref())
                    .is_none()
            {
                detail = "已发现 Codex ACP；Codex CLI fallback 未发现或不可用。".to_string();
            }

            AgentInstallStatus {
                catalog: item,
                configured,
                detected,
                source: command.as_ref().map(|command| command.source),
                executable: command.map(|command| command.executable),
                version: last_install
                    .as_ref()
                    .and_then(|record| record.version.clone()),
                install_state,
                detail,
                last_test,
                last_install,
            }
        })
        .collect())
}

pub fn start_agent_install(
    agent_store: &AgentConfigStore,
    payload: StartAgentInstallPayload,
) -> AgentInstallResult<AgentInstallStatus> {
    let adapter = current_adapter();
    start_agent_install_with_adapter(agent_store, payload, adapter.as_ref())
}

pub fn start_agent_install_with_adapter(
    agent_store: &AgentConfigStore,
    payload: StartAgentInstallPayload,
    adapter: &dyn PlatformAdapter,
) -> AgentInstallResult<AgentInstallStatus> {
    let item = list_agent_install_catalog_with_adapter(adapter)
        .into_iter()
        .find(|item| item.id == payload.agent_id)
        .ok_or_else(|| AgentInstallError::Unsupported(payload.agent_id.clone()))?;
    if !item.supported {
        return Err(AgentInstallError::Unsupported(format!(
            "{} is not supported on this platform",
            item.label
        )));
    }

    let subject = agent_install_subject(&item);
    let policy = PolicyEngine::new().evaluate_agent_install(PolicyAgentInstallRequest {
        agent_id: item.id.clone(),
        label: item.label.clone(),
        install_method: install_method(&item).to_string(),
        subject,
    });
    PolicyEngine::new().enforce(policy, &payload.policy_approvals)?;

    if item.id == "sofvary-pi" {
        install_sofvary_pi(agent_store, adapter)?;
    } else {
        adapter.open_external(&item.docs_url)?;
        save_record(
            adapter,
            AgentInstallRecord {
                agent_id: item.id.clone(),
                state: AgentInstallStateKind::Manual,
                detail: format!("Opened official install page for {}.", item.label),
                checked_at: Utc::now().to_rfc3339(),
                install_method: Some("manual-download".to_string()),
                version: None,
                executable: None,
            },
        )?;
    }

    get_agent_install_statuses_with_adapter(agent_store, adapter)?
        .into_iter()
        .find(|status| status.catalog.id == item.id)
        .ok_or_else(|| AgentInstallError::Unsupported(item.id))
}

pub fn open_agent_install_page(agent_id: &str) -> AgentInstallResult<()> {
    let adapter = current_adapter();
    let item = list_agent_install_catalog_with_adapter(adapter.as_ref())
        .into_iter()
        .find(|item| item.id == agent_id)
        .ok_or_else(|| AgentInstallError::Unsupported(agent_id.to_string()))?;
    adapter.open_external(&item.docs_url)?;
    Ok(())
}

pub fn agent_install_subject_for_id(agent_id: &str) -> AgentInstallResult<String> {
    let adapter = current_adapter();
    let item = list_agent_install_catalog_with_adapter(adapter.as_ref())
        .into_iter()
        .find(|item| item.id == agent_id)
        .ok_or_else(|| AgentInstallError::Unsupported(agent_id.to_string()))?;
    Ok(agent_install_subject(&item))
}

pub fn agent_install_subject(item: &AgentInstallCatalogItem) -> String {
    let command_or_url = if item.id == "sofvary-pi" {
        PI_NPM_METADATA_URL
    } else {
        item.docs_url.as_str()
    };
    format!("{}:{}:{}", item.id, install_method(item), command_or_url)
}

fn install_method(item: &AgentInstallCatalogItem) -> &'static str {
    match item.install_capability {
        AgentInstallCapability::Managed => "managed-npm",
        AgentInstallCapability::ManualDownload | AgentInstallCapability::Unavailable => {
            "manual-download"
        }
    }
}

fn install_sofvary_pi(
    agent_store: &AgentConfigStore,
    adapter: &dyn PlatformAdapter,
) -> AgentInstallResult<()> {
    let toolchain = ensure_node_toolchain_available(adapter)?;
    save_record(
        adapter,
        AgentInstallRecord {
            agent_id: "sofvary-pi".to_string(),
            state: AgentInstallStateKind::Installing,
            detail: format!("Downloading {PI_NPM_PACKAGE}."),
            checked_at: Utc::now().to_rfc3339(),
            install_method: Some("managed-npm".to_string()),
            version: None,
            executable: None,
        },
    )?;

    let metadata = fetch_npm_metadata(PI_NPM_METADATA_URL)?;
    let bytes = fetch_bytes(&metadata.dist.tarball, MAX_PI_TARBALL_BYTES)?;
    verify_integrity(&bytes, &metadata.dist.integrity)?;

    let controlled_dir = controlled_adapter_dir(adapter)?;
    fs::create_dir_all(&controlled_dir)?;
    let package_dir = controlled_dir.join("pi-package");
    let extract_dir = controlled_dir.join("pi-package.tmp");
    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir)?;
    }
    fs::create_dir_all(&extract_dir)?;
    unpack_npm_package(&bytes, &extract_dir, &package_dir)?;
    install_package_dependencies(adapter, &package_dir, &toolchain)?;
    let bin_path = resolve_package_bin(&package_dir)?;
    let shim = write_pi_shim(
        adapter.os(),
        &controlled_dir,
        &package_dir,
        &bin_path,
        toolchain.node.executable.as_ref(),
    )?;
    let startup_detail = verify_pi_shim_starts(adapter, &shim)?;

    let previous = agent_store
        .load_with_adapter(adapter)?
        .agents
        .into_iter()
        .find(|agent| agent.id == "sofvary-pi");
    let mut config = pi_agent_config(previous, shim.clone());
    config.last_test = Some(fresh_test_record(
        true,
        AgentTransportKind::PiRpc,
        startup_detail,
    ));
    let mut state = agent_store.load_with_adapter(adapter)?;
    if let Some(existing) = state.agents.iter_mut().find(|agent| agent.id == config.id) {
        *existing = config;
    } else {
        state.agents.push(config);
    }
    agent_store.save_with_adapter(&state, adapter)?;

    save_record(
        adapter,
        AgentInstallRecord {
            agent_id: "sofvary-pi".to_string(),
            state: AgentInstallStateKind::Installed,
            detail: format!(
                "Installed {PI_NPM_PACKAGE} {} into Sofvary data.",
                metadata.version
            ),
            checked_at: Utc::now().to_rfc3339(),
            install_method: Some("managed-npm".to_string()),
            version: Some(metadata.version),
            executable: Some(shim),
        },
    )?;
    Ok(())
}

fn ensure_node_toolchain_available(
    adapter: &dyn PlatformAdapter,
) -> AgentInstallResult<NodeToolchain> {
    resolve_node_toolchain_with_adapter(adapter).map_err(|error| {
        AgentInstallError::Unsupported(format!(
            "Sofvary Pi requires the Node.js Toolchain from Settings: {error}"
        ))
    })
}

fn install_package_dependencies(
    adapter: &dyn PlatformAdapter,
    package_dir: &Path,
    toolchain: &NodeToolchain,
) -> AgentInstallResult<()> {
    let pnpm = toolchain
        .pnpm
        .executable
        .clone()
        .unwrap_or_else(|| PathBuf::from("pnpm"));
    let output = adapter.run_process(CommandSpec {
        executable: pnpm,
        args: pi_dependency_install_args(),
        cwd: package_dir.to_path_buf(),
        env: HashMap::new(),
        allowed_network: false,
        timeout_ms: Some(180_000),
        kill_on_drop: true,
    });
    match output {
        Ok(output) if output.status_code == Some(0) => Ok(()),
        Ok(output) => Err(AgentInstallError::InvalidArtifact(format!(
            "pnpm install for {PI_NPM_PACKAGE} failed with {:?}: {}",
            output.status_code,
            summarize_command_output(&output.stderr, &output.stdout)
        ))),
        Err(error) => Err(AgentInstallError::Unsupported(format!(
            "Sofvary Pi requires pnpm from the Node.js Toolchain to install package dependencies: {error}"
        ))),
    }
}

fn pi_dependency_install_args() -> Vec<String> {
    vec![
        "install".to_string(),
        "--prod".to_string(),
        "--ignore-scripts".to_string(),
    ]
}

fn verify_pi_shim_starts(adapter: &dyn PlatformAdapter, shim: &Path) -> AgentInstallResult<String> {
    let output = adapter.run_process(CommandSpec {
        executable: shim.to_path_buf(),
        args: vec!["--version".to_string()],
        cwd: adapter.dirs()?.data_dir,
        env: HashMap::new(),
        allowed_network: false,
        timeout_ms: Some(15_000),
        kill_on_drop: true,
    });
    match output {
        Ok(output) if output.status_code == Some(0) => {
            let version = summarize_command_output(&output.stdout, &output.stderr);
            Ok(format!("Pi command startup check succeeded: {version}"))
        }
        Ok(output) => Err(AgentInstallError::InvalidArtifact(format!(
            "Pi command startup check failed with {:?}: {}",
            output.status_code,
            summarize_command_output(&output.stderr, &output.stdout)
        ))),
        Err(error) => Err(AgentInstallError::InvalidArtifact(format!(
            "Pi command startup check failed: {error}"
        ))),
    }
}

fn summarize_command_output(stderr: &str, stdout: &str) -> String {
    let source = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    let first_line = source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("process failed without output");
    let mut output = String::new();
    for (index, ch) in first_line.chars().enumerate() {
        if index >= 360 {
            output.push_str("...");
            return output;
        }
        output.push(ch);
    }
    output
}

#[derive(Debug, Deserialize)]
struct NpmMetadata {
    version: String,
    dist: NpmDist,
}

#[derive(Debug, Deserialize)]
struct NpmDist {
    tarball: String,
    integrity: String,
}

fn fetch_npm_metadata(url: &str) -> AgentInstallResult<NpmMetadata> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| AgentInstallError::Http(error.to_string()))?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|error| AgentInstallError::Http(error.to_string()))?;
    Ok(serde_json::from_str(&text)?)
}

fn fetch_bytes(url: &str, max_bytes: usize) -> AgentInstallResult<Vec<u8>> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| AgentInstallError::Http(error.to_string()))?;
    response
        .body_mut()
        .with_config()
        .limit(max_bytes as u64)
        .read_to_vec()
        .map_err(|error| AgentInstallError::Http(error.to_string()))
}

fn verify_integrity(bytes: &[u8], integrity: &str) -> AgentInstallResult<()> {
    let Some(encoded) = integrity
        .split_whitespace()
        .find_map(|token| token.strip_prefix("sha512-"))
    else {
        return Err(AgentInstallError::InvalidArtifact(
            "npm dist.integrity does not include sha512".to_string(),
        ));
    };
    let expected = BASE64_STANDARD
        .decode(encoded)
        .map_err(|error| AgentInstallError::InvalidArtifact(error.to_string()))?;
    let actual = Sha512::digest(bytes);
    if expected.as_slice() != actual.as_slice() {
        return Err(AgentInstallError::InvalidArtifact(
            "npm tarball integrity check failed".to_string(),
        ));
    }
    Ok(())
}

fn unpack_npm_package(
    bytes: &[u8],
    extract_dir: &Path,
    package_dir: &Path,
) -> AgentInstallResult<()> {
    let mut decoder = GzDecoder::new(bytes);
    let mut tar_bytes = Vec::new();
    decoder.read_to_end(&mut tar_bytes)?;
    unpack_restricted_tar(&tar_bytes, extract_dir)?;

    let npm_package_dir = extract_dir.join("package");
    if !npm_package_dir.is_dir() {
        return Err(AgentInstallError::InvalidArtifact(
            "npm tarball does not contain package/".to_string(),
        ));
    }
    if package_dir.exists() {
        fs::remove_dir_all(package_dir)?;
    }
    fs::rename(&npm_package_dir, package_dir)?;
    if extract_dir.exists() {
        fs::remove_dir_all(extract_dir)?;
    }
    Ok(())
}

fn unpack_restricted_tar(bytes: &[u8], output_dir: &Path) -> AgentInstallResult<()> {
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
            AgentInstallError::InvalidArtifact("tar entry size overflow".to_string())
        })?;
        if data_end > bytes.len() {
            return Err(AgentInstallError::InvalidArtifact(
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
            _ => {
                return Err(AgentInstallError::InvalidArtifact(format!(
                    "unsupported tar entry type for {}",
                    relative_path.display()
                )));
            }
        }

        offset = data_start + round_up_to_512(size);
    }
    Ok(())
}

fn tar_path(header: &[u8]) -> AgentInstallResult<PathBuf> {
    let name = tar_string(&header[0..100]);
    let prefix = tar_string(&header[345..500]);
    let path = if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    };
    if path.is_empty() {
        return Err(AgentInstallError::InvalidArtifact(
            "tar entry path is empty".to_string(),
        ));
    }
    Ok(PathBuf::from(path))
}

fn tar_size(header: &[u8]) -> AgentInstallResult<usize> {
    let raw = tar_string(&header[124..136]);
    usize::from_str_radix(raw.trim(), 8)
        .map_err(|error| AgentInstallError::InvalidArtifact(error.to_string()))
}

fn tar_string(field: &[u8]) -> String {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).trim().to_string()
}

fn safe_tar_target(output_dir: &Path, relative_path: &Path) -> AgentInstallResult<PathBuf> {
    if relative_path.is_absolute() || !relative_path.starts_with("package") {
        return Err(AgentInstallError::InvalidArtifact(format!(
            "tar entry leaves npm package root: {}",
            relative_path.display()
        )));
    }
    if relative_path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(AgentInstallError::InvalidArtifact(format!(
            "tar entry uses parent traversal: {}",
            relative_path.display()
        )));
    }
    Ok(output_dir.join(relative_path))
}

fn round_up_to_512(size: usize) -> usize {
    (size + 511) & !511
}

fn resolve_package_bin(package_dir: &Path) -> AgentInstallResult<PathBuf> {
    let package_json = package_dir.join("package.json");
    let value: Value = serde_json::from_slice(&fs::read(&package_json)?)?;
    let bin = value
        .get("bin")
        .and_then(|bin| {
            bin.as_str().map(str::to_string).or_else(|| {
                bin.as_object().and_then(|map| {
                    map.get("pi")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| map.values().find_map(Value::as_str).map(str::to_string))
                })
            })
        })
        .ok_or_else(|| {
            AgentInstallError::InvalidArtifact(
                "package.json does not declare a pi executable bin".to_string(),
            )
        })?;
    let bin_path = package_dir.join(bin);
    if !bin_path.is_file() {
        return Err(AgentInstallError::InvalidArtifact(format!(
            "declared pi executable does not exist: {}",
            bin_path.display()
        )));
    }
    Ok(bin_path)
}

fn write_pi_shim(
    os: OsKind,
    controlled_dir: &Path,
    package_dir: &Path,
    bin_path: &Path,
    node_executable: Option<&PathBuf>,
) -> AgentInstallResult<PathBuf> {
    let shim = if os == OsKind::Windows {
        controlled_dir.join("pi.cmd")
    } else {
        controlled_dir.join("pi")
    };
    let node = node_executable
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "node".to_string());
    let content = if os == OsKind::Windows {
        format!(
            "@echo off\r\n\"{}\" \"{}\" %*\r\n",
            node,
            bin_path.display()
        )
    } else {
        format!(
            "#!/bin/sh\nexec \"{}\" \"{}\" \"$@\"\n",
            node,
            bin_path.display()
        )
    };
    fs::write(&shim, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&shim)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shim, permissions)?;
    }
    if !package_dir.is_dir() {
        return Err(AgentInstallError::InvalidArtifact(
            "Pi package directory was not created".to_string(),
        ));
    }
    Ok(shim)
}

fn pi_agent_config(previous: Option<AgentConfig>, executable: PathBuf) -> AgentConfig {
    let previous_enabled = previous.as_ref().map(|agent| agent.enabled).unwrap_or(true);
    let previous_last_test = previous.and_then(|agent| agent.last_test);
    AgentConfig {
        id: "sofvary-pi".to_string(),
        provider: AgentProvider::SofvaryPi,
        label: "Sofvary Pi".to_string(),
        enabled: previous_enabled,
        acp: None,
        cli: Some(AgentCommandConfig {
            executable,
            args: vec!["--mode".to_string(), "rpc".to_string()],
            env: HashMap::new(),
            source: AgentInstallSource::Bundled,
        }),
        allow_cli_fallback: false,
        default_interaction_mode: None,
        last_test: previous_last_test,
    }
}

fn install_state_for_item(
    adapter: &dyn PlatformAdapter,
    item: &AgentInstallCatalogItem,
    detected: bool,
    last_install: Option<&AgentInstallRecord>,
) -> (AgentInstallStateKind, String) {
    if !item.supported {
        return (
            AgentInstallStateKind::Unsupported,
            "当前系统或 CPU 架构暂不支持。".to_string(),
        );
    }
    if detected {
        return (
            AgentInstallStateKind::Installed,
            "已在本机发现可用命令。".to_string(),
        );
    }
    if item.id == "sofvary-pi" && !node_available(adapter) {
        return (
            AgentInstallStateKind::NeedsRuntime,
            "需要先安装 Node.js。".to_string(),
        );
    }
    if let Some(record) = last_install {
        if matches!(
            record.state,
            AgentInstallStateKind::Failed
                | AgentInstallStateKind::Installing
                | AgentInstallStateKind::Manual
                | AgentInstallStateKind::NeedsRuntime
        ) {
            return (record.state, record.detail.clone());
        }
    }
    if item.install_capability == AgentInstallCapability::ManualDownload {
        return (
            AgentInstallStateKind::Manual,
            "需要通过官方安装页安装，安装后刷新发现。".to_string(),
        );
    }
    (
        AgentInstallStateKind::NotInstalled,
        "未在 Sofvary 受控目录或 PATH 中发现。".to_string(),
    )
}

fn node_available(adapter: &dyn PlatformAdapter) -> bool {
    node_toolchain_available(adapter)
}

fn save_record(
    adapter: &dyn PlatformAdapter,
    record: AgentInstallRecord,
) -> AgentInstallResult<()> {
    let mut state = load_record_state(adapter)?;
    state
        .records
        .retain(|existing| existing.agent_id != record.agent_id);
    state.records.push(record);
    save_record_state(adapter, &state)
}

fn load_record_state(adapter: &dyn PlatformAdapter) -> AgentInstallResult<AgentInstallRecordState> {
    let path = record_path(adapter)?;
    if !path.exists() {
        return Ok(AgentInstallRecordState::default());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn save_record_state(
    adapter: &dyn PlatformAdapter,
    state: &AgentInstallRecordState,
) -> AgentInstallResult<()> {
    let path = record_path(adapter)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state)? + "\n")?;
    Ok(())
}

fn record_path(adapter: &dyn PlatformAdapter) -> PlatformResult<PathBuf> {
    Ok(adapter.dirs()?.data_dir.join("agent-installs.json"))
}

fn controlled_adapter_dir(adapter: &dyn PlatformAdapter) -> PlatformResult<PathBuf> {
    Ok(adapter
        .dirs()?
        .data_dir
        .join("agent-adapters")
        .join(format!(
            "{}-{}",
            os_slug(adapter.os()),
            arch_slug(adapter.arch())
        )))
}

fn supported_on_current_platform(
    adapter: &dyn PlatformAdapter,
    item: &AgentInstallCatalogItem,
) -> bool {
    item.supported
        && matches!(
            adapter.os(),
            OsKind::Windows | OsKind::Macos | OsKind::Linux
        )
        && adapter.arch() != ArchKind::Unknown
}

fn install_catalog() -> Vec<AgentInstallCatalogItem> {
    vec![
        AgentInstallCatalogItem {
            id: "sofvary-pi".to_string(),
            label: "Sofvary Pi".to_string(),
            icon_key: "sofvary-pi".to_string(),
            provider: AgentProvider::SofvaryPi,
            docs_url: "https://github.com/earendil-works/pi".to_string(),
            install_capability: AgentInstallCapability::Managed,
            recommended: true,
            managed: true,
            supported: true,
            detect_commands: vec!["pi".to_string()],
            acp: None,
            cli: Some(command_template("pi", &["--mode", "rpc"])),
            version_command: Some(command_template("pi", &["--version"])),
        },
        AgentInstallCatalogItem {
            id: "codex".to_string(),
            label: "Codex".to_string(),
            icon_key: "codex".to_string(),
            provider: AgentProvider::Codex,
            docs_url: "https://developers.openai.com/codex/cli".to_string(),
            install_capability: AgentInstallCapability::ManualDownload,
            recommended: false,
            managed: false,
            supported: true,
            detect_commands: vec!["codex-acp".to_string(), "codex".to_string()],
            acp: Some(command_template("codex-acp", &[])),
            cli: Some(command_template(
                "codex",
                &[
                    "exec",
                    "--skip-git-repo-check",
                    "--ephemeral",
                    "-c",
                    "model_reasoning_effort=\"medium\"",
                ],
            )),
            version_command: Some(command_template("codex", &["--version"])),
        },
        AgentInstallCatalogItem {
            id: "claude-code".to_string(),
            label: "Claude Code".to_string(),
            icon_key: "claude-code".to_string(),
            provider: AgentProvider::ClaudeCode,
            docs_url: "https://docs.anthropic.com/en/docs/claude-code/setup".to_string(),
            install_capability: AgentInstallCapability::ManualDownload,
            recommended: false,
            managed: false,
            supported: true,
            detect_commands: vec!["claude-agent-acp".to_string(), "claude".to_string()],
            acp: Some(command_template("claude-agent-acp", &[])),
            cli: Some(command_template(
                "claude",
                &["-p", "--output-format", "json"],
            )),
            version_command: Some(command_template("claude", &["--version"])),
        },
        AgentInstallCatalogItem {
            id: "cursor".to_string(),
            label: "Cursor".to_string(),
            icon_key: "cursor".to_string(),
            provider: AgentProvider::Cursor,
            docs_url: "https://docs.cursor.com/en/cli/overview".to_string(),
            install_capability: AgentInstallCapability::ManualDownload,
            recommended: false,
            managed: false,
            supported: true,
            detect_commands: vec!["cursor".to_string()],
            acp: Some(command_template("cursor", &["agent", "--acp"])),
            cli: None,
            version_command: Some(command_template("cursor", &["--version"])),
        },
        AgentInstallCatalogItem {
            id: "opencode".to_string(),
            label: "OpenCode".to_string(),
            icon_key: "opencode".to_string(),
            provider: AgentProvider::Opencode,
            docs_url: "https://opencode.ai/docs/".to_string(),
            install_capability: AgentInstallCapability::ManualDownload,
            recommended: false,
            managed: false,
            supported: true,
            detect_commands: vec!["opencode".to_string()],
            acp: Some(command_template("opencode", &["acp"])),
            cli: Some(command_template("opencode", &["run", "--format", "json"])),
            version_command: Some(command_template("opencode", &["--version"])),
        },
        AgentInstallCatalogItem {
            id: "kimi-code".to_string(),
            label: "Kimi Code".to_string(),
            icon_key: "kimi-code".to_string(),
            provider: AgentProvider::KimiCode,
            docs_url: "https://github.com/MoonshotAI/kimi-code".to_string(),
            install_capability: AgentInstallCapability::ManualDownload,
            recommended: false,
            managed: false,
            supported: true,
            detect_commands: vec!["kimi".to_string(), "kimi-code".to_string()],
            acp: Some(command_template("kimi", &["acp"])),
            cli: Some(command_template("kimi", &["run", "--format", "json"])),
            version_command: Some(command_template("kimi", &["--version"])),
        },
        AgentInstallCatalogItem {
            id: "qoder".to_string(),
            label: "Qoder".to_string(),
            icon_key: "qoder".to_string(),
            provider: AgentProvider::Qoder,
            docs_url: "https://docs.qoder.com/".to_string(),
            install_capability: AgentInstallCapability::ManualDownload,
            recommended: false,
            managed: false,
            supported: true,
            detect_commands: vec!["qoder".to_string()],
            acp: Some(command_template("qoder", &["acp"])),
            cli: None,
            version_command: Some(command_template("qoder", &["--version"])),
        },
        AgentInstallCatalogItem {
            id: "deepseek-tui".to_string(),
            label: "DeepSeek TUI".to_string(),
            icon_key: "deepseek-tui".to_string(),
            provider: AgentProvider::DeepseekTui,
            docs_url: "https://github.com/Hmbown/DeepSeek-TUI".to_string(),
            install_capability: AgentInstallCapability::ManualDownload,
            recommended: false,
            managed: false,
            supported: true,
            detect_commands: vec![
                "codewhale".to_string(),
                "deepseek".to_string(),
                "deepseek-tui".to_string(),
            ],
            acp: Some(command_template("codewhale", &["serve", "--acp"])),
            cli: None,
            version_command: Some(command_template("codewhale", &["--version"])),
        },
    ]
}

fn command_template(executable: &str, args: &[&str]) -> AgentInstallCommandTemplate {
    AgentInstallCommandTemplate {
        executable: executable.to_string(),
        args: args.iter().map(|arg| arg.to_string()).collect(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_common_agents() {
        let ids = install_catalog()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();

        for id in [
            "sofvary-pi",
            "codex",
            "claude-code",
            "cursor",
            "opencode",
            "kimi-code",
            "qoder",
            "deepseek-tui",
        ] {
            assert!(ids.contains(&id.to_string()), "missing {id}");
        }
    }

    #[test]
    fn install_subject_is_exact_to_method_and_target() {
        let item = install_catalog()
            .into_iter()
            .find(|item| item.id == "codex")
            .expect("codex catalog item");

        assert_eq!(
            agent_install_subject(&item),
            "codex:manual-download:https://developers.openai.com/codex/cli"
        );
    }

    #[test]
    fn verifies_sha512_integrity() {
        let bytes = b"pi";
        let digest = Sha512::digest(bytes);
        let integrity = format!("sha512-{}", BASE64_STANDARD.encode(digest));

        verify_integrity(bytes, &integrity).expect("integrity should pass");
        assert!(verify_integrity(b"other", &integrity).is_err());
    }

    #[test]
    fn pi_agent_config_uses_pi_rpc_command() {
        let config = pi_agent_config(None, PathBuf::from("/tmp/pi"));

        assert_eq!(config.provider, AgentProvider::SofvaryPi);
        assert_eq!(
            config.cli.expect("pi cli").args,
            ["--mode".to_string(), "rpc".to_string()]
        );
    }

    #[test]
    fn pi_dependency_install_args_are_pnpm_10_compatible() {
        let args = pi_dependency_install_args();

        assert_eq!(args, ["install", "--prod", "--ignore-scripts"]);
        assert!(!args.iter().any(|arg| arg == "--no-fund"));
    }

    #[test]
    fn deepseek_catalog_includes_codewhale_compatibility() {
        let item = install_catalog()
            .into_iter()
            .find(|item| item.id == "deepseek-tui")
            .expect("deepseek catalog item");

        assert!(item.detect_commands.contains(&"codewhale".to_string()));
        assert!(item.detect_commands.contains(&"deepseek".to_string()));
        assert!(item.detect_commands.contains(&"deepseek-tui".to_string()));
    }
}
