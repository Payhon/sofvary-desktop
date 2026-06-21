use crate::core::app_capsule::{
    import_app_capsule_with_adapter, inspect_app_capsule_bytes_with_adapter, AppCapsuleError,
    AppCapsuleManifest, ImportAppCapsulePayload, ImportAppCapsuleResult,
};
use crate::core::cloud_config::sofvary_api_base_url_from_env;
use crate::core::pack_registry::RegistryArtifactMetadata;
use crate::core::policy_types::PolicyApprovalSet;
use crate::core::runtime_manager::{RuntimeManager, RuntimeManagerError, RuntimePreview};
use crate::core::workspace_manager::{WorkspaceError, WorkspaceManager};
use crate::core::workspace_types::RuntimeMode;
use crate::platform::{current_adapter, PlatformAdapter, PlatformError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

const MAX_CAPSULE_BYTES: u64 = 50 * 1024 * 1024;
const ACCOUNT_REFRESH_TOKEN_KEY: &str = "sofvary.account.refresh_token";

#[derive(Debug, Error)]
pub enum DeepLinkInstallError {
    #[error("invalid deep link: {0}")]
    InvalidDeepLink(String),
    #[error("registry response is invalid: {0}")]
    InvalidRegistryResponse(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("app capsule error: {0}")]
    AppCapsule(#[from] AppCapsuleError),
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("runtime error: {0}")]
    Runtime(#[from] RuntimeManagerError),
    #[error("capsule sha256 mismatch: expected {expected}, got {actual}")]
    Sha256Mismatch { expected: String, actual: String },
}

pub type DeepLinkInstallResultType<T> = Result<T, DeepLinkInstallError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkInstallPayload {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmDeepLinkInstallPayload {
    pub url: String,
    pub confirmed: bool,
    pub mode: Option<RuntimeMode>,
    #[serde(default)]
    pub policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallAppDeepLink {
    pub app_id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPermissionSummary {
    pub workspace_read: Vec<String>,
    pub workspace_write: Vec<String>,
    pub local_database: String,
    pub network: String,
    pub device_access: String,
    pub system_access: String,
    pub requested: Vec<String>,
    pub plugin_packs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryAppMetadata {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryAppVersionMetadata {
    pub id: String,
    pub app_id: String,
    pub version: String,
    pub artifact_id: String,
    pub artifact: RegistryArtifactMetadata,
    pub notes: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryAppResolveResponse {
    pub app: RegistryAppMetadata,
    pub version: RegistryAppVersionMetadata,
    pub artifact: RegistryArtifactMetadata,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkInstallPreflight {
    pub request: InstallAppDeepLink,
    pub app: RegistryAppMetadata,
    pub version: RegistryAppVersionMetadata,
    pub artifact: RegistryArtifactMetadata,
    pub permission_summary: InstallPermissionSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkInstallResult {
    pub request: InstallAppDeepLink,
    pub app: RegistryAppMetadata,
    pub version: RegistryAppVersionMetadata,
    pub artifact: RegistryArtifactMetadata,
    pub permission_summary: InstallPermissionSummary,
    pub import_result: ImportAppCapsuleResult,
    pub preview: RuntimePreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactDownloadUrlResponse {
    artifact: RegistryArtifactMetadata,
    download: RegistryDownloadMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryDownloadMetadata {
    method: String,
    url: String,
    expires_at: String,
    headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryAuthRefreshResponse {
    tokens: RegistryAuthTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryAuthTokens {
    access_token: String,
    refresh_token: String,
}

pub struct DeepLinkInstaller {
    base_url: String,
}

struct DownloadedCapsule {
    request: InstallAppDeepLink,
    app: RegistryAppMetadata,
    version: RegistryAppVersionMetadata,
    artifact: RegistryArtifactMetadata,
    permission_summary: InstallPermissionSummary,
    bytes: Vec<u8>,
}

impl DeepLinkInstaller {
    pub fn from_env() -> Self {
        Self::new(sofvary_api_base_url_from_env())
    }

    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn prepare_deep_link_install(
        &self,
        raw_url: &str,
        adapter: &dyn PlatformAdapter,
    ) -> DeepLinkInstallResultType<DeepLinkInstallPreflight> {
        let capsule = self.resolve_and_download_capsule(raw_url, adapter)?;
        Ok(DeepLinkInstallPreflight {
            request: capsule.request,
            app: capsule.app,
            version: capsule.version,
            artifact: capsule.artifact,
            permission_summary: capsule.permission_summary,
        })
    }

    pub fn install_app_from_deep_link(
        &self,
        workspace_manager: &WorkspaceManager,
        runtime_manager: &RuntimeManager,
        raw_url: &str,
        runtime_mode: RuntimeMode,
        policy_approvals: &PolicyApprovalSet,
        adapter: &dyn PlatformAdapter,
    ) -> DeepLinkInstallResultType<DeepLinkInstallResult> {
        let capsule = self.resolve_and_download_capsule(raw_url, adapter)?;
        let capsule_path = write_capsule_to_cache(adapter, &capsule)?;
        let import_result = match import_app_capsule_with_adapter(
            workspace_manager,
            ImportAppCapsulePayload {
                capsule_path: capsule_path.clone(),
                policy_approvals: policy_approvals.clone(),
            },
            adapter,
        ) {
            Ok(result) => result,
            Err(error) => {
                let _ = fs::remove_file(&capsule_path);
                return Err(error.into());
            }
        };
        let _ = fs::remove_file(&capsule_path);
        let preview = runtime_manager.preview_existing_workspace_with_adapter(
            import_result.workspace.app_id.clone(),
            runtime_mode,
            workspace_manager,
            adapter,
            policy_approvals,
        )?;

        Ok(DeepLinkInstallResult {
            request: capsule.request,
            app: capsule.app,
            version: capsule.version,
            artifact: capsule.artifact,
            permission_summary: capsule.permission_summary,
            import_result,
            preview,
        })
    }

    fn resolve_and_download_capsule(
        &self,
        raw_url: &str,
        adapter: &dyn PlatformAdapter,
    ) -> DeepLinkInstallResultType<DownloadedCapsule> {
        let request = parse_install_app_deep_link(raw_url)?;
        let resolved = self.resolve_app_capsule(&request)?;
        validate_resolved_capsule(&request, &resolved)?;

        let download = self.get_download_url(&resolved.artifact.id)?;
        validate_download_metadata(&self.base_url, &resolved.artifact, &download)?;

        let bytes = self.get_bytes(
            &download.download.url,
            &download.download.headers,
            resolved.artifact.size_bytes,
        )?;
        if bytes.len() as u64 != resolved.artifact.size_bytes {
            return Err(DeepLinkInstallError::InvalidRegistryResponse(format!(
                "downloaded capsule size did not match registry metadata for {}",
                resolved.artifact.id
            )));
        }
        verify_sha256(&bytes, &resolved.artifact.sha256)?;
        let manifest = inspect_app_capsule_bytes_with_adapter(&bytes, adapter)?;
        let permission_summary = permission_summary_from_manifest(&manifest);

        Ok(DownloadedCapsule {
            request,
            app: resolved.app,
            version: resolved.version,
            artifact: resolved.artifact,
            permission_summary,
            bytes,
        })
    }

    fn resolve_app_capsule(
        &self,
        request: &InstallAppDeepLink,
    ) -> DeepLinkInstallResultType<RegistryAppResolveResponse> {
        let url = format!(
            "{}/v1/registry/apps/resolve?id={}&version={}",
            self.base_url,
            encode_component(&request.app_id),
            encode_component(&request.version)
        );
        self.get_json(&url)
    }

    fn get_download_url(
        &self,
        artifact_id: &str,
    ) -> DeepLinkInstallResultType<ArtifactDownloadUrlResponse> {
        let url = format!(
            "{}/v1/artifacts/{}/download-url",
            self.base_url,
            encode_component(artifact_id)
        );
        self.get_json(&url)
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> DeepLinkInstallResultType<T> {
        let mut request = ureq::get(url);
        if let Some(authorization) = self.authorization_header()? {
            request = request.header("Authorization", &authorization);
        }
        let mut response = request
            .call()
            .map_err(|error| DeepLinkInstallError::Http(error.to_string()))?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| DeepLinkInstallError::Http(error.to_string()))?;
        Ok(serde_json::from_str(&text)?)
    }

    fn get_bytes(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        expected_size_bytes: u64,
    ) -> DeepLinkInstallResultType<Vec<u8>> {
        let mut request = ureq::get(url);
        if let Some(authorization) = self.authorization_header()? {
            request = request.header("Authorization", &authorization);
        }
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let mut response = request
            .call()
            .map_err(|error| DeepLinkInstallError::Http(error.to_string()))?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(expected_size_bytes.saturating_add(1))
            .read_to_vec()
            .map_err(|error| {
                DeepLinkInstallError::InvalidRegistryResponse(format!(
                    "capsule download exceeded registry sizeBytes or could not be read: {error}"
                ))
            })?;
        if bytes.len() as u64 > MAX_CAPSULE_BYTES {
            return Err(DeepLinkInstallError::InvalidRegistryResponse(format!(
                "capsule exceeds the Phase 20 size limit of {MAX_CAPSULE_BYTES} bytes"
            )));
        }
        Ok(bytes)
    }

    fn authorization_header(&self) -> DeepLinkInstallResultType<Option<String>> {
        let adapter = current_adapter();
        let Some(refresh_token) = adapter
            .secure_store_get(ACCOUNT_REFRESH_TOKEN_KEY)
            .map_err(DeepLinkInstallError::Platform)?
        else {
            return Ok(None);
        };
        if refresh_token.trim().is_empty() {
            return Ok(None);
        }

        let url = format!("{}/v1/auth/refresh", self.base_url);
        let mut response = ureq::post(&url)
            .content_type("application/json")
            .send(json!({ "refreshToken": refresh_token }).to_string())
            .map_err(|error| DeepLinkInstallError::Http(error.to_string()))?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| DeepLinkInstallError::Http(error.to_string()))?;
        let refreshed: RegistryAuthRefreshResponse = serde_json::from_str(&text)?;
        adapter
            .secure_store_set(ACCOUNT_REFRESH_TOKEN_KEY, &refreshed.tokens.refresh_token)
            .map_err(DeepLinkInstallError::Platform)?;
        Ok(Some(format!("Bearer {}", refreshed.tokens.access_token)))
    }
}

pub fn prepare_deep_link_install(
    payload: DeepLinkInstallPayload,
) -> DeepLinkInstallResultType<DeepLinkInstallPreflight> {
    let adapter = current_adapter();
    DeepLinkInstaller::from_env().prepare_deep_link_install(&payload.url, adapter.as_ref())
}

pub fn install_app_from_deep_link(
    workspace_manager: &WorkspaceManager,
    runtime_manager: &RuntimeManager,
    payload: ConfirmDeepLinkInstallPayload,
) -> DeepLinkInstallResultType<DeepLinkInstallResult> {
    if !payload.confirmed {
        return Err(DeepLinkInstallError::InvalidDeepLink(
            "deep link install requires explicit confirmation".to_string(),
        ));
    }
    let adapter = current_adapter();
    DeepLinkInstaller::from_env().install_app_from_deep_link(
        workspace_manager,
        runtime_manager,
        &payload.url,
        payload.mode.unwrap_or_default(),
        &payload.policy_approvals,
        adapter.as_ref(),
    )
}

pub fn parse_install_app_deep_link(raw_url: &str) -> DeepLinkInstallResultType<InstallAppDeepLink> {
    if raw_url.trim() != raw_url || raw_url.is_empty() {
        return Err(invalid_link("link must not contain surrounding whitespace"));
    }
    if raw_url.contains('#') {
        return Err(invalid_link("fragment is not allowed"));
    }
    let rest = raw_url
        .strip_prefix("sofvary://")
        .ok_or_else(|| invalid_link("scheme must be sofvary"))?;
    let (target, query) = rest
        .split_once('?')
        .ok_or_else(|| invalid_link("query string is required"))?;
    if target != "install/app" {
        return Err(invalid_link("target must be install/app"));
    }

    let mut values = BTreeMap::new();
    for part in query.split('&') {
        if part.is_empty() {
            return Err(invalid_link("query parameters must not be empty"));
        }
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| invalid_link("query parameters must use key=value"))?;
        if key != "id" && key != "version" {
            return Err(invalid_link("only id and version parameters are allowed"));
        }
        if values.contains_key(key) {
            return Err(invalid_link("duplicate query parameter"));
        }
        values.insert(key.to_string(), percent_decode_component(value)?);
    }

    if values.len() != 2 {
        return Err(invalid_link("id and version parameters are required"));
    }
    let app_id = values
        .remove("id")
        .ok_or_else(|| invalid_link("id is required"))?;
    let version = values
        .remove("version")
        .ok_or_else(|| invalid_link("version is required"))?;
    validate_app_id(&app_id)?;
    validate_semver_like(&version)?;

    Ok(InstallAppDeepLink { app_id, version })
}

pub fn permission_summary_from_manifest(manifest: &AppCapsuleManifest) -> InstallPermissionSummary {
    InstallPermissionSummary {
        workspace_read: manifest.permissions.filesystem.read.clone(),
        workspace_write: manifest.permissions.filesystem.write.clone(),
        local_database: format!(
            "{}; includeData={}; schema={}, migrations={}, seed={}, excluded={}",
            manifest.database.engine,
            manifest.database.include_data,
            manifest.database.schema.len(),
            manifest.database.migrations.len(),
            manifest.database.seed.len(),
            manifest.database.excluded_data.len()
        ),
        network: manifest.permissions.network.clone(),
        device_access: "not granted in Phase 20".to_string(),
        system_access: "not granted in Phase 20".to_string(),
        requested: manifest.permissions.requested.clone(),
        plugin_packs: manifest
            .plugins
            .packs
            .iter()
            .map(|pack| format!("{}@{}", pack.id, pack.version))
            .collect(),
    }
}

fn validate_resolved_capsule(
    request: &InstallAppDeepLink,
    resolved: &RegistryAppResolveResponse,
) -> DeepLinkInstallResultType<()> {
    if resolved.app.id != request.app_id
        || resolved.version.app_id != request.app_id
        || resolved.version.version != request.version
    {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "resolved app metadata does not match the deep link".to_string(),
        ));
    }
    if resolved.version.artifact_id != resolved.artifact.id
        || resolved.version.artifact.id != resolved.artifact.id
    {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "resolved app version artifact metadata is inconsistent".to_string(),
        ));
    }
    validate_capsule_artifact(&resolved.artifact)?;
    validate_capsule_artifact(&resolved.version.artifact)?;
    if resolved.version.artifact.sha256.to_lowercase() != resolved.artifact.sha256.to_lowercase() {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "app version artifact sha256 does not match resolved artifact".to_string(),
        ));
    }
    if resolved.version.artifact.kind != resolved.artifact.kind
        || resolved.version.artifact.size_bytes != resolved.artifact.size_bytes
    {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "app version artifact metadata does not match resolved artifact".to_string(),
        ));
    }
    Ok(())
}

fn validate_download_metadata(
    base_url: &str,
    expected: &RegistryArtifactMetadata,
    download: &ArtifactDownloadUrlResponse,
) -> DeepLinkInstallResultType<()> {
    if download.download.method != "GET" {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "capsule download method must be GET".to_string(),
        ));
    }
    if download.artifact.id != expected.id
        || download.artifact.kind != expected.kind
        || download.artifact.sha256.to_lowercase() != expected.sha256.to_lowercase()
        || download.artifact.size_bytes != expected.size_bytes
    {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "download artifact metadata does not match resolved app version".to_string(),
        ));
    }
    validate_capsule_artifact(&download.artifact)?;
    validate_same_origin_download_url(base_url, &download.download.url)
}

fn validate_capsule_artifact(artifact: &RegistryArtifactMetadata) -> DeepLinkInstallResultType<()> {
    if artifact.kind != "app-capsule" {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "app install requires app-capsule artifact".to_string(),
        ));
    }
    if artifact.status != "uploaded" {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(format!(
            "artifact '{}' is not uploaded",
            artifact.id
        )));
    }
    if !is_sha256(&artifact.sha256) {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "artifact sha256 must be 64 hex characters".to_string(),
        ));
    }
    if artifact.size_bytes == 0 || artifact.size_bytes > MAX_CAPSULE_BYTES {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(format!(
            "artifact size must be between 1 and {MAX_CAPSULE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_same_origin_download_url(
    base_url: &str,
    download_url: &str,
) -> DeepLinkInstallResultType<()> {
    let base_origin = url_origin(base_url)?;
    let download_origin = url_origin(download_url)?;
    if base_origin != download_origin {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "Phase 20 capsule downloads must come from the configured Sofvary Registry origin"
                .to_string(),
        ));
    }
    Ok(())
}

fn write_capsule_to_cache(
    adapter: &dyn PlatformAdapter,
    capsule: &DownloadedCapsule,
) -> DeepLinkInstallResultType<PathBuf> {
    let cache_dir = adapter.dirs()?.cache_dir.join("registry-capsules");
    fs::create_dir_all(&cache_dir)?;
    let file_name = format!(
        "{}-{}-{}.sfcapsule",
        sanitize_file_segment(&capsule.request.app_id),
        sanitize_file_segment(&capsule.request.version),
        &capsule.artifact.sha256[..12]
    );
    let path = cache_dir.join(file_name);
    fs::write(&path, &capsule.bytes)?;
    Ok(path)
}

fn verify_sha256(bytes: &[u8], expected: &str) -> DeepLinkInstallResultType<()> {
    if !is_sha256(expected) {
        return Err(DeepLinkInstallError::InvalidRegistryResponse(
            "artifact sha256 must be 64 hex characters".to_string(),
        ));
    }
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected.to_lowercase() {
        return Err(DeepLinkInstallError::Sha256Mismatch {
            expected: expected.to_lowercase(),
            actual,
        });
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn validate_app_id(value: &str) -> DeepLinkInstallResultType<()> {
    if value.is_empty() || value.len() > 96 || value.contains("..") {
        return Err(invalid_link("app id is invalid"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(invalid_link("app id contains unsupported characters"));
    }
    Ok(())
}

fn validate_semver_like(version: &str) -> DeepLinkInstallResultType<()> {
    let mut build_parts = version.split('+');
    let before_build = build_parts.next().unwrap_or_default();
    let build = build_parts.next();
    if build_parts.next().is_some()
        || build
            .map(|build| !valid_semver_identifier_list(build, false))
            .unwrap_or(false)
    {
        return Err(invalid_link("version must be an exact SemVer value"));
    }

    let mut prerelease_parts = before_build.split('-');
    let core = prerelease_parts.next().unwrap_or_default();
    let prerelease = prerelease_parts.next();
    if prerelease_parts.next().is_some()
        || prerelease
            .map(|prerelease| !valid_semver_identifier_list(prerelease, true))
            .unwrap_or(false)
    {
        return Err(invalid_link("version must be an exact SemVer value"));
    }

    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.chars().all(|ch| ch.is_ascii_digit())
        })
    {
        return Err(invalid_link("version must be an exact SemVer value"));
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

fn percent_decode_component(value: &str) -> DeepLinkInstallResultType<String> {
    let mut bytes = Vec::new();
    let raw = value.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            if index + 2 >= raw.len() {
                return Err(invalid_link("percent encoding is incomplete"));
            }
            let hi = hex_value(raw[index + 1])?;
            let lo = hex_value(raw[index + 2])?;
            bytes.push((hi << 4) | lo);
            index += 3;
        } else {
            bytes.push(raw[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).map_err(|_| invalid_link("query value must be UTF-8"))
}

fn hex_value(byte: u8) -> DeepLinkInstallResultType<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(invalid_link("percent encoding is invalid")),
    }
}

fn encode_component(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect::<Vec<_>>(),
        })
        .collect()
}

fn url_origin(value: &str) -> DeepLinkInstallResultType<String> {
    let (scheme, rest) = value
        .split_once("://")
        .ok_or_else(|| invalid_registry_url("url must include a scheme"))?;
    if scheme != "http" && scheme != "https" {
        return Err(invalid_registry_url("url scheme must be http or https"));
    }
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err(invalid_registry_url("url authority is invalid"));
    }
    Ok(format!("{scheme}://{authority}"))
}

fn sanitize_file_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn invalid_link(message: &str) -> DeepLinkInstallError {
    DeepLinkInstallError::InvalidDeepLink(message.to_string())
}

fn invalid_registry_url(message: &str) -> DeepLinkInstallError {
    DeepLinkInstallError::InvalidRegistryResponse(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app_capsule::{export_app_capsule_with_adapter, ExportAppCapsulePayload};
    use crate::core::workspace_types::{AppBoxManifest, RuntimeKind};
    use crate::platform::macos::MacosPlatformAdapter;
    use crate::platform::{
        ArchKind, CommandSpec, OsKind, PlatformDirs, PlatformResult, ProcessHandle, ProcessOutput,
        WebviewProfile, WorkArea,
    };
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::thread;
    use tempfile::TempDir;

    fn capsule_import_approval(name: &str) -> PolicyApprovalSet {
        PolicyApprovalSet {
            approved: vec![crate::core::policy_types::PolicyApprovalGrant {
                action: crate::core::policy_types::PolicyActionKind::CapsuleImport,
                subject: Some(name.to_string()),
            }],
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
            MacosPlatformAdapter.normalize_path(input)
        }

        fn ensure_executable(&self, path: &Path) -> PlatformResult<()> {
            MacosPlatformAdapter.ensure_executable(path)
        }

        fn resolve_sidecar_executable(&self, name: &str) -> PlatformResult<PathBuf> {
            MacosPlatformAdapter.resolve_sidecar_executable(name)
        }

        fn run_process(&self, spec: CommandSpec) -> PlatformResult<ProcessOutput> {
            MacosPlatformAdapter.run_process(spec)
        }

        fn spawn_process(&self, spec: CommandSpec) -> PlatformResult<ProcessHandle> {
            MacosPlatformAdapter.spawn_process(spec)
        }

        fn kill_process_tree(&self, pid: u32) -> PlatformResult<()> {
            MacosPlatformAdapter.kill_process_tree(pid)
        }

        fn allocate_local_port(&self) -> PlatformResult<u16> {
            MacosPlatformAdapter.allocate_local_port()
        }

        fn open_external(&self, url: &str) -> PlatformResult<()> {
            MacosPlatformAdapter.open_external(url)
        }

        fn reveal_path(&self, _path: &Path) -> PlatformResult<()> {
            Ok(())
        }

        fn register_protocol_handler(&self, protocol: &str) -> PlatformResult<()> {
            MacosPlatformAdapter.register_protocol_handler(protocol)
        }

        fn register_global_shortcut(&self, accelerator: &str) -> PlatformResult<()> {
            MacosPlatformAdapter.register_global_shortcut(accelerator)
        }

        fn unregister_global_shortcut(&self, accelerator: &str) -> PlatformResult<()> {
            MacosPlatformAdapter.unregister_global_shortcut(accelerator)
        }

        fn show_tray_or_menu_bar_item(&self) -> PlatformResult<()> {
            MacosPlatformAdapter.show_tray_or_menu_bar_item()
        }

        fn get_active_monitor_work_area(&self) -> PlatformResult<WorkArea> {
            MacosPlatformAdapter.get_active_monitor_work_area()
        }

        fn secure_store_set(&self, key: &str, value: &str) -> PlatformResult<()> {
            MacosPlatformAdapter.secure_store_set(key, value)
        }

        fn secure_store_get(&self, key: &str) -> PlatformResult<Option<String>> {
            MacosPlatformAdapter.secure_store_get(key)
        }

        fn current_webview_profile(&self) -> WebviewProfile {
            MacosPlatformAdapter.current_webview_profile()
        }
    }

    #[test]
    fn parses_valid_install_app_link() {
        let parsed = parse_install_app_deep_link(
            "sofvary://install/app?id=app-intent-notes&version=1.2.3-alpha.1%2Bbuild.5",
        )
        .expect("valid link");

        assert_eq!(parsed.app_id, "app-intent-notes");
        assert_eq!(parsed.version, "1.2.3-alpha.1+build.5");
    }

    #[test]
    fn rejects_external_or_wrong_target_links() {
        for link in [
            "https://install/app?id=app&version=1.0.0",
            "sofvary://open/app?id=app&version=1.0.0",
            "sofvary://install/pack?id=app&version=1.0.0",
            "sofvary://install/app?id=app&version=1.0.0#fragment",
        ] {
            assert!(parse_install_app_deep_link(link).is_err(), "{link}");
        }
    }

    #[test]
    fn rejects_external_url_and_duplicate_query_parameters() {
        for link in [
            "sofvary://install/app?id=app&version=1.0.0&url=https%3A%2F%2Fevil.test%2Fx",
            "sofvary://install/app?id=app&id=other&version=1.0.0",
            "sofvary://install/app?id=app&version=1.0.0&artifactId=artifact_1",
            "sofvary://install/app?id=app&version=1.0.0&workspace=app_existing",
        ] {
            assert!(parse_install_app_deep_link(link).is_err(), "{link}");
        }
    }

    #[test]
    fn rejects_encoded_path_traversal_and_invalid_semver() {
        for link in [
            "sofvary://install/app?id=app%2Fname&version=1.0.0",
            "sofvary://install/app?id=app..name&version=1.0.0",
            "sofvary://install/app?id=app&version=latest",
            "sofvary://install/app?id=app&version=1.0",
            "sofvary://install/app?id=app&version=01.0.0",
            "sofvary://install/app?id=app&version=1.0.0-01",
        ] {
            assert!(parse_install_app_deep_link(link).is_err(), "{link}");
        }
    }

    #[test]
    fn rejects_cross_origin_download_urls() {
        assert!(validate_same_origin_download_url(
            "http://127.0.0.1:4710",
            "http://127.0.0.1:4710/v1/artifacts/a/mock-download",
        )
        .is_ok());
        assert!(validate_same_origin_download_url(
            "http://127.0.0.1:4710",
            "file:///tmp/capsule.sfcapsule",
        )
        .is_err());
        assert!(validate_same_origin_download_url(
            "http://127.0.0.1:4710",
            "http://evil.test/capsule.sfcapsule",
        )
        .is_err());
    }

    #[test]
    fn rejects_mismatched_capsule_artifact_metadata() {
        let artifact = RegistryArtifactMetadata {
            id: "artifact_capsule_1".to_string(),
            kind: "app-capsule".to_string(),
            file_name: "intent-notes.sfcapsule".to_string(),
            content_type: "application/vnd.sofvary.app-capsule".to_string(),
            size_bytes: 128,
            sha256: "a".repeat(64),
            storage_key: "app-capsule/artifact_capsule_1/intent-notes.sfcapsule".to_string(),
            status: "uploaded".to_string(),
            created_at: "2026-06-06T00:00:00.000Z".to_string(),
            signature: None,
        };
        let request = InstallAppDeepLink {
            app_id: "app-intent-notes".to_string(),
            version: "0.2.0".to_string(),
        };
        let mut resolved = RegistryAppResolveResponse {
            app: RegistryAppMetadata {
                id: request.app_id.clone(),
                name: "Intent Notes".to_string(),
                summary: "Mock capsule app".to_string(),
                visibility: "public".to_string(),
            },
            version: RegistryAppVersionMetadata {
                id: "app_version_1".to_string(),
                app_id: request.app_id.clone(),
                version: request.version.clone(),
                artifact_id: artifact.id.clone(),
                artifact: artifact.clone(),
                notes: "Mocked registry capsule".to_string(),
                created_at: "2026-06-06T00:00:00.000Z".to_string(),
            },
            artifact,
        };

        resolved.version.artifact.size_bytes += 1;
        assert!(validate_resolved_capsule(&request, &resolved).is_err());
    }

    #[test]
    fn rejects_download_metadata_size_mismatch() {
        let artifact = RegistryArtifactMetadata {
            id: "artifact_capsule_1".to_string(),
            kind: "app-capsule".to_string(),
            file_name: "intent-notes.sfcapsule".to_string(),
            content_type: "application/vnd.sofvary.app-capsule".to_string(),
            size_bytes: 128,
            sha256: "a".repeat(64),
            storage_key: "app-capsule/artifact_capsule_1/intent-notes.sfcapsule".to_string(),
            status: "uploaded".to_string(),
            created_at: "2026-06-06T00:00:00.000Z".to_string(),
            signature: None,
        };
        let mut download_artifact = artifact.clone();
        download_artifact.size_bytes += 1;
        let download = ArtifactDownloadUrlResponse {
            artifact: download_artifact,
            download: RegistryDownloadMetadata {
                method: "GET".to_string(),
                url: "http://127.0.0.1:4710/v1/artifacts/a/mock-download".to_string(),
                expires_at: "2026-06-06T00:05:00.000Z".to_string(),
                headers: HashMap::new(),
            },
        };

        assert!(validate_download_metadata("http://127.0.0.1:4710", &artifact, &download).is_err());
    }

    #[test]
    fn installs_capsule_from_mocked_registry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = temp_adapter(&temp);
        let workspace_manager = WorkspaceManager::new();
        let source = workspace_manager
            .create_workspace_for_runtime_with_adapter(
                "Mock Registry Capsule".to_string(),
                RuntimeKind::StaticHtml,
                &adapter,
            )
            .expect("workspace");
        write_static_sample(&source);

        let capsule_path = temp.path().join("mock-registry.sfcapsule");
        export_app_capsule_with_adapter(
            &workspace_manager,
            ExportAppCapsulePayload {
                app_id: source.app_id.clone(),
                include_prompt_history: false,
                output_path: capsule_path.clone(),
            },
            &adapter,
        )
        .expect("export capsule");
        let capsule_bytes = fs::read(&capsule_path).expect("capsule bytes");
        let capsule_sha256 = format!("{:x}", Sha256::digest(&capsule_bytes));
        let (base_url, server) = start_mock_registry(capsule_bytes, capsule_sha256.clone(), 3);

        let runtime_manager = RuntimeManager::new();
        let result = DeepLinkInstaller::new(base_url).install_app_from_deep_link(
            &workspace_manager,
            &runtime_manager,
            "sofvary://install/app?id=app-intent-notes&version=0.2.0",
            RuntimeMode::Dev,
            &capsule_import_approval("Mock Registry Capsule"),
            &adapter,
        );
        server.join().expect("mock registry server");
        let result = result.expect("install result");

        assert_eq!(result.request.app_id, "app-intent-notes");
        assert_eq!(result.version.version, "0.2.0");
        assert_eq!(result.artifact.kind, "app-capsule");
        assert_eq!(result.artifact.sha256, capsule_sha256);
        assert_ne!(result.import_result.workspace.app_id, source.app_id);
        assert!(result
            .import_result
            .app_manifest
            .paths
            .generated
            .join("static/index.html")
            .exists());
        assert!(result.preview.preview_url.starts_with("http://127.0.0.1:"));
        assert!(result
            .preview
            .logs
            .iter()
            .any(|line| line.contains("Agent Gateway was not run")));
        assert!(result
            .permission_summary
            .workspace_read
            .contains(&"source/generated".to_string()));
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

    fn write_static_sample(manifest: &AppBoxManifest) {
        let static_root = manifest.paths.generated.join("static");
        fs::create_dir_all(&static_root).expect("static root");
        fs::write(
            static_root.join("index.html"),
            "<!doctype html><link rel=\"stylesheet\" href=\"./style.css\"><div id=\"app\">Mock capsule</div><script src=\"./app.js\"></script>",
        )
        .expect("index");
        fs::write(
            static_root.join("style.css"),
            "body { font-family: sans-serif; }",
        )
        .expect("style");
        fs::write(
            static_root.join("app.js"),
            "document.body.dataset.ready = 'true';",
        )
        .expect("script");
    }

    fn start_mock_registry(
        capsule_bytes: Vec<u8>,
        capsule_sha256: String,
        expected_requests: usize,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listener");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let server_base_url = base_url.clone();
        let handle = thread::spawn(move || {
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().expect("accept");
                let request = read_request(&mut stream);
                if request.starts_with("GET /v1/registry/apps/resolve?")
                    && request.contains("id=app-intent-notes")
                    && request.contains("version=0.2.0")
                {
                    let artifact = json!({
                        "id": "artifact_capsule_1",
                        "kind": "app-capsule",
                        "fileName": "intent-notes.sfcapsule",
                        "contentType": "application/vnd.sofvary.app-capsule",
                        "sizeBytes": capsule_bytes.len(),
                        "sha256": capsule_sha256,
                        "storageKey": "app-capsule/artifact_capsule_1/intent-notes.sfcapsule",
                        "status": "uploaded",
                        "createdAt": "2026-06-06T00:00:00.000Z",
                        "signature": null,
                    });
                    let body = json!({
                        "app": {
                            "id": "app-intent-notes",
                            "name": "Intent Notes",
                            "summary": "Mock capsule app",
                            "visibility": "public",
                        },
                        "version": {
                            "id": "app_version_1",
                            "appId": "app-intent-notes",
                            "version": "0.2.0",
                            "artifactId": "artifact_capsule_1",
                            "artifact": artifact.clone(),
                            "notes": "Mocked registry capsule",
                            "createdAt": "2026-06-06T00:00:00.000Z",
                        },
                        "artifact": artifact,
                    });
                    write_response(
                        &mut stream,
                        "200 OK",
                        "application/json",
                        serde_json::to_vec(&body).expect("json"),
                    );
                    continue;
                }

                if request.starts_with("GET /v1/artifacts/artifact_capsule_1/download-url") {
                    let body = json!({
                        "artifact": {
                            "id": "artifact_capsule_1",
                            "kind": "app-capsule",
                            "fileName": "intent-notes.sfcapsule",
                            "contentType": "application/vnd.sofvary.app-capsule",
                            "sizeBytes": capsule_bytes.len(),
                            "sha256": capsule_sha256,
                            "storageKey": "app-capsule/artifact_capsule_1/intent-notes.sfcapsule",
                            "status": "uploaded",
                            "createdAt": "2026-06-06T00:00:00.000Z",
                            "signature": null,
                        },
                        "download": {
                            "method": "GET",
                            "url": format!("{server_base_url}/v1/artifacts/artifact_capsule_1/mock-download"),
                            "expiresAt": "2026-06-06T00:05:00.000Z",
                            "headers": {},
                        },
                    });
                    write_response(
                        &mut stream,
                        "200 OK",
                        "application/json",
                        serde_json::to_vec(&body).expect("json"),
                    );
                    continue;
                }

                if request.starts_with("GET /v1/artifacts/artifact_capsule_1/mock-download") {
                    write_response(
                        &mut stream,
                        "200 OK",
                        "application/vnd.sofvary.app-capsule",
                        capsule_bytes.clone(),
                    );
                    continue;
                }

                write_response(
                    &mut stream,
                    "404 Not Found",
                    "application/json",
                    br#"{"error":"not found"}"#.to_vec(),
                );
            }
        });
        (base_url, handle)
    }

    fn read_request(stream: &mut TcpStream) -> String {
        let mut buffer = [0; 4096];
        let mut bytes = Vec::new();
        loop {
            let read = stream.read(&mut buffer).expect("read");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") || bytes.len() > 16_384 {
                break;
            }
        }
        String::from_utf8_lossy(&bytes).to_string()
    }

    fn write_response(stream: &mut TcpStream, status: &str, content_type: &str, body: Vec<u8>) {
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .expect("headers");
        stream.write_all(&body).expect("body");
    }
}
