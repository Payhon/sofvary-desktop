use crate::core::cloud_config::sofvary_api_base_url_from_env;
use crate::core::pack_manager::{InstalledPackSummary, PackError, PackKind, PackManager};
use crate::core::policy_engine::{PolicyEngine, PolicyError};
use crate::core::policy_types::{PolicyApprovalSet, PolicyPackInstallRequest};
use crate::core::workspace_manager::{WorkspaceError, WorkspaceManager};
use crate::platform::current_adapter;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_PACK_ARTIFACT_BYTES: u64 = 50 * 1024 * 1024;
const ACCOUNT_REFRESH_TOKEN_KEY: &str = "sofvary.account.refresh_token";

#[derive(Debug, Error)]
pub enum PackRegistryError {
    #[error("pack error: {0}")]
    Pack(#[from] PackError),
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),
    #[error("http error: {0}")]
    Http(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("registry response is invalid: {0}")]
    InvalidRegistryResponse(String),
    #[error("artifact sha256 mismatch: expected {expected}, got {actual}")]
    Sha256Mismatch { expected: String, actual: String },
}

pub type PackRegistryResult<T> = Result<T, PackRegistryError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveRegistryPackPayload {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRegistryPackPayload {
    pub id: String,
    pub version: String,
    pub app_id: Option<String>,
    #[serde(default)]
    pub policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRegistryPackResult {
    pub pack: InstalledPackSummary,
    pub installed: bool,
    pub lockfile_updated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryResolveResponse {
    pub pack: RegistryPackMetadata,
    pub version: RegistryPackVersionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPackMetadata {
    pub id: String,
    #[serde(rename = "type")]
    pub pack_type: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPackVersionMetadata {
    pub id: String,
    pub pack_id: String,
    pub version: String,
    pub manifest: serde_json::Value,
    pub artifact_id: String,
    pub artifact: RegistryArtifactMetadata,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryArtifactMetadata {
    pub id: String,
    pub kind: String,
    pub file_name: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub storage_key: String,
    pub status: String,
    pub created_at: String,
    pub signature: Option<String>,
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
    headers: std::collections::HashMap<String, String>,
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

pub struct PackRegistryInstaller {
    base_url: String,
}

impl PackRegistryInstaller {
    pub fn from_env() -> Self {
        Self::new(sofvary_api_base_url_from_env())
    }

    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn resolve_pack(
        &self,
        id: &str,
        version: &str,
    ) -> PackRegistryResult<RegistryResolveResponse> {
        let url = format!(
            "{}/v1/registry/resolve?id={}&version={}",
            self.base_url,
            encode_component(id),
            encode_component(version)
        );
        self.get_json(&url)
    }

    pub fn install_pack(
        &self,
        pack_manager: &PackManager,
        workspace_manager: &WorkspaceManager,
        payload: InstallRegistryPackPayload,
    ) -> PackRegistryResult<InstallRegistryPackResult> {
        let resolved = self.resolve_pack(&payload.id, &payload.version)?;
        let kind = PackKind::from_manifest_type(&resolved.pack.pack_type)?;
        validate_artifact_for_pack(kind, &resolved.version.artifact)?;
        let engine = PolicyEngine::new();
        engine.enforce(
            engine.evaluate_pack_install(PolicyPackInstallRequest {
                app_id: payload.app_id.clone(),
                kind: kind.to_string(),
                id: payload.id.clone(),
                version: payload.version.clone(),
                trust_level: "registry".to_string(),
            }),
            &payload.policy_approvals,
        )?;

        let download = self.get_download_url(&resolved.version.artifact.id)?;
        if download.download.method != "GET" {
            return Err(PackRegistryError::InvalidRegistryResponse(
                "artifact download method must be GET".to_string(),
            ));
        }
        if download.artifact.id != resolved.version.artifact.id
            || download.artifact.sha256.to_lowercase()
                != resolved.version.artifact.sha256.to_lowercase()
            || download.artifact.size_bytes != resolved.version.artifact.size_bytes
            || download.artifact.kind != resolved.version.artifact.kind
        {
            return Err(PackRegistryError::InvalidRegistryResponse(
                "download artifact metadata does not match resolved pack version".to_string(),
            ));
        }
        validate_artifact_size(&resolved.version.artifact)?;
        validate_download_url_same_origin(&self.base_url, &download.download.url)?;

        let bytes = self.get_bytes(&download.download.url, &download.download.headers)?;
        validate_downloaded_size(&bytes, &resolved.version.artifact)?;
        verify_sha256(&bytes, &resolved.version.artifact.sha256)?;
        let pack =
            pack_manager.install_pack_archive(kind, &payload.id, &payload.version, &bytes)?;

        let mut lockfile_updated = false;
        if let Some(app_id) = payload.app_id {
            workspace_manager.update_lockfile_pack_with_policy(
                app_id,
                &pack.kind,
                pack.id.clone(),
                pack.version.clone(),
                crate::platform::current_adapter().as_ref(),
                &payload.policy_approvals,
            )?;
            lockfile_updated = true;
        }

        Ok(InstallRegistryPackResult {
            pack,
            installed: true,
            lockfile_updated,
        })
    }

    fn get_download_url(
        &self,
        artifact_id: &str,
    ) -> PackRegistryResult<ArtifactDownloadUrlResponse> {
        let url = format!(
            "{}/v1/artifacts/{}/download-url",
            self.base_url,
            encode_component(artifact_id)
        );
        self.get_json(&url)
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> PackRegistryResult<T> {
        let mut request = ureq::get(url);
        if let Some(authorization) = self.authorization_header()? {
            request = request.header("Authorization", &authorization);
        }
        let mut response = request
            .call()
            .map_err(|error| PackRegistryError::Http(error.to_string()))?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| PackRegistryError::Http(error.to_string()))?;
        Ok(serde_json::from_str(&text)?)
    }

    fn get_bytes(
        &self,
        url: &str,
        headers: &std::collections::HashMap<String, String>,
    ) -> PackRegistryResult<Vec<u8>> {
        let mut request = ureq::get(url);
        if let Some(authorization) = self.authorization_header()? {
            request = request.header("Authorization", &authorization);
        }
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let mut response = request
            .call()
            .map_err(|error| PackRegistryError::Http(error.to_string()))?;
        response
            .body_mut()
            .with_config()
            .limit(MAX_PACK_ARTIFACT_BYTES)
            .read_to_vec()
            .map_err(|error| PackRegistryError::Http(error.to_string()))
    }

    fn authorization_header(&self) -> PackRegistryResult<Option<String>> {
        let adapter = current_adapter();
        let Some(refresh_token) = adapter
            .secure_store_get(ACCOUNT_REFRESH_TOKEN_KEY)
            .map_err(|error| PackRegistryError::Http(error.to_string()))?
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
            .map_err(|error| PackRegistryError::Http(error.to_string()))?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| PackRegistryError::Http(error.to_string()))?;
        let refreshed: RegistryAuthRefreshResponse = serde_json::from_str(&text)?;
        adapter
            .secure_store_set(ACCOUNT_REFRESH_TOKEN_KEY, &refreshed.tokens.refresh_token)
            .map_err(|error| PackRegistryError::Http(error.to_string()))?;
        Ok(Some(format!("Bearer {}", refreshed.tokens.access_token)))
    }
}

fn validate_artifact_for_pack(
    kind: PackKind,
    artifact: &RegistryArtifactMetadata,
) -> PackRegistryResult<()> {
    let expected = match kind {
        PackKind::Runtime => "runtime-pack",
        PackKind::Harness => "harness-pack",
        PackKind::Plugin => "plugin-pack",
    };
    if artifact.kind != expected {
        return Err(PackRegistryError::InvalidRegistryResponse(format!(
            "{} artifact required for {} pack",
            expected, kind
        )));
    }
    if artifact.status != "uploaded" {
        return Err(PackRegistryError::InvalidRegistryResponse(format!(
            "artifact '{}' is not uploaded",
            artifact.id
        )));
    }
    if !is_sha256(&artifact.sha256) {
        return Err(PackRegistryError::InvalidRegistryResponse(
            "artifact sha256 must be 64 hex characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_artifact_size(artifact: &RegistryArtifactMetadata) -> PackRegistryResult<()> {
    if artifact.size_bytes == 0 {
        return Err(PackRegistryError::InvalidRegistryResponse(
            "artifact sizeBytes must be greater than zero".to_string(),
        ));
    }
    if artifact.size_bytes > MAX_PACK_ARTIFACT_BYTES {
        return Err(PackRegistryError::InvalidRegistryResponse(format!(
            "artifact '{}' exceeds the Phase 23 pack size limit of {} bytes",
            artifact.id, MAX_PACK_ARTIFACT_BYTES
        )));
    }
    Ok(())
}

fn validate_downloaded_size(
    bytes: &[u8],
    artifact: &RegistryArtifactMetadata,
) -> PackRegistryResult<()> {
    if bytes.len() as u64 != artifact.size_bytes {
        return Err(PackRegistryError::InvalidRegistryResponse(format!(
            "downloaded artifact size did not match registry metadata for {}",
            artifact.id
        )));
    }
    Ok(())
}

fn validate_download_url_same_origin(
    registry_base_url: &str,
    download_url: &str,
) -> PackRegistryResult<()> {
    let registry_origin = http_origin(registry_base_url)?;
    let download_origin = http_origin(download_url)?;
    if registry_origin != download_origin {
        return Err(PackRegistryError::InvalidRegistryResponse(
            "artifact download URL must use the configured Sofvary Registry origin".to_string(),
        ));
    }
    Ok(())
}

fn http_origin(url: &str) -> PackRegistryResult<String> {
    let trimmed = url.trim();
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return Err(PackRegistryError::InvalidRegistryResponse(format!(
            "invalid URL: {url}"
        )));
    };
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https") {
        return Err(PackRegistryError::InvalidRegistryResponse(format!(
            "unsupported download URL scheme: {scheme}"
        )));
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if authority.is_empty() || authority.contains('@') {
        return Err(PackRegistryError::InvalidRegistryResponse(format!(
            "invalid URL authority: {url}"
        )));
    }
    Ok(format!("{scheme}://{authority}"))
}

fn verify_sha256(bytes: &[u8], expected: &str) -> PackRegistryResult<()> {
    if !is_sha256(expected) {
        return Err(PackRegistryError::InvalidRegistryResponse(
            "artifact sha256 must be 64 hex characters".to_string(),
        ));
    }
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected.to_lowercase() {
        return Err(PackRegistryError::Sha256Mismatch {
            expected: expected.to_lowercase(),
            actual,
        });
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::policy_engine::pack_install_subject;
    use crate::core::policy_types::{PolicyActionKind, PolicyApprovalGrant};
    use crate::platform::macos::MacosPlatformAdapter;
    use crate::platform::{
        ArchKind, CommandSpec, OsKind, PlatformAdapter, PlatformDirs, PlatformResult,
        ProcessHandle, ProcessOutput, WebviewProfile, WorkArea,
    };
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::thread;

    #[test]
    fn verify_sha256_rejects_mismatch() {
        let result = verify_sha256(b"pack", &"a".repeat(64));

        assert!(matches!(
            result,
            Err(PackRegistryError::Sha256Mismatch { .. })
        ));
    }

    #[test]
    fn validates_uploaded_artifact_kind_for_pack() {
        let artifact = RegistryArtifactMetadata {
            id: "artifact_1".to_string(),
            kind: "runtime-pack".to_string(),
            file_name: "runtime.zip".to_string(),
            content_type: "application/zip".to_string(),
            size_bytes: 1,
            sha256: "a".repeat(64),
            storage_key: "runtime-pack/artifact_1/runtime.zip".to_string(),
            status: "uploaded".to_string(),
            created_at: "now".to_string(),
            signature: None,
        };

        assert!(validate_artifact_for_pack(PackKind::Runtime, &artifact).is_ok());
        assert!(validate_artifact_for_pack(PackKind::Harness, &artifact).is_err());
    }

    #[test]
    fn download_url_must_match_registry_origin() {
        assert!(validate_download_url_same_origin(
            "http://127.0.0.1:4710",
            "http://127.0.0.1:4710/v1/artifacts/a/mock-download"
        )
        .is_ok());

        assert!(validate_download_url_same_origin(
            "http://127.0.0.1:4710",
            "http://evil.test/v1/artifacts/a/mock-download"
        )
        .is_err());
        assert!(
            validate_download_url_same_origin("http://127.0.0.1:4710", "file:///tmp/pack.zip")
                .is_err()
        );
        assert!(validate_download_url_same_origin(
            "http://127.0.0.1:4710",
            "ftp://127.0.0.1:4710/pack.zip"
        )
        .is_err());
    }

    #[test]
    fn validates_artifact_size_limits_and_downloaded_size() {
        let mut artifact = RegistryArtifactMetadata {
            id: "artifact_1".to_string(),
            kind: "runtime-pack".to_string(),
            file_name: "runtime.zip".to_string(),
            content_type: "application/zip".to_string(),
            size_bytes: 4,
            sha256: "a".repeat(64),
            storage_key: "runtime-pack/artifact_1/runtime.zip".to_string(),
            status: "uploaded".to_string(),
            created_at: "now".to_string(),
            signature: None,
        };

        assert!(validate_artifact_size(&artifact).is_ok());
        assert!(validate_downloaded_size(b"pack", &artifact).is_ok());
        assert!(validate_downloaded_size(b"packs", &artifact).is_err());

        artifact.size_bytes = 0;
        assert!(validate_artifact_size(&artifact).is_err());
        artifact.size_bytes = MAX_PACK_ARTIFACT_BYTES + 1;
        assert!(validate_artifact_size(&artifact).is_err());
    }

    #[test]
    fn registry_install_requires_app_scoped_pack_install_approval() {
        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = TempAdapter {
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let pack_manager = PackManager::new_with_adapter(&adapter).expect("pack manager");
        let workspace_manager = WorkspaceManager::new();
        let (base_url, server) = start_resolve_only_registry();

        let result = PackRegistryInstaller::new(base_url).install_pack(
            &pack_manager,
            &workspace_manager,
            InstallRegistryPackPayload {
                id: "sofvary.runtime.remote-test".to_string(),
                version: "0.2.0".to_string(),
                app_id: Some("app_policy_target".to_string()),
                policy_approvals: PolicyApprovalSet {
                    approved: vec![PolicyApprovalGrant {
                        action: PolicyActionKind::PackInstall,
                        subject: Some(pack_install_subject(
                            None,
                            "runtime",
                            "sofvary.runtime.remote-test",
                            "0.2.0",
                        )),
                    }],
                },
            },
        );

        server.join().expect("mock registry server");
        assert!(matches!(
            result,
            Err(PackRegistryError::Policy(
                PolicyError::RequiresConfirmation { .. }
            ))
        ));
    }

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
            Ok(())
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

    fn start_resolve_only_registry() -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listener");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_request(&mut stream);
            if request.starts_with("GET /v1/registry/resolve?")
                && request.contains("id=sofvary.runtime.remote-test")
                && request.contains("version=0.2.0")
            {
                let artifact = json!({
                    "id": "artifact_runtime_1",
                    "kind": "runtime-pack",
                    "fileName": "runtime.zip",
                    "contentType": "application/zip",
                    "sizeBytes": 1,
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "storageKey": "runtime-pack/artifact_runtime_1/runtime.zip",
                    "status": "uploaded",
                    "createdAt": "2026-06-07T00:00:00.000Z",
                    "signature": null,
                });
                let body = json!({
                    "pack": {
                        "id": "sofvary.runtime.remote-test",
                        "type": "sofvary.runtime-pack",
                        "name": "Remote Test Runtime",
                        "summary": "Mock runtime pack",
                        "description": "Mock runtime pack",
                        "createdAt": "2026-06-07T00:00:00.000Z",
                        "updatedAt": "2026-06-07T00:00:00.000Z",
                    },
                    "version": {
                        "id": "pack_version_1",
                        "packId": "sofvary.runtime.remote-test",
                        "version": "0.2.0",
                        "manifest": {},
                        "artifactId": "artifact_runtime_1",
                        "artifact": artifact,
                        "createdAt": "2026-06-07T00:00:00.000Z",
                    },
                });
                write_response(
                    &mut stream,
                    "200 OK",
                    "application/json",
                    serde_json::to_vec(&body).expect("json"),
                );
            } else {
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
