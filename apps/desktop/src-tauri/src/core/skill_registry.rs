use crate::core::cloud_config::sofvary_api_base_url_from_env;
use crate::core::pack_registry::RegistryArtifactMetadata;
use crate::platform::current_adapter;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

const MAX_SKILL_ARTIFACT_BYTES: u64 = 20 * 1024 * 1024;
const ACCOUNT_REFRESH_TOKEN_KEY: &str = "sofvary.account.refresh_token";

#[derive(Debug, Error)]
pub enum SkillRegistryError {
    #[error("http error: {0}")]
    Http(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("platform error: {0}")]
    Platform(String),
    #[error("registry response is invalid: {0}")]
    InvalidRegistryResponse(String),
    #[error("skill artifact sha256 mismatch: expected {expected}, got {actual}")]
    Sha256Mismatch { expected: String, actual: String },
}

pub type SkillRegistryResult<T> = Result<T, SkillRegistryError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRegistrySkillPayload {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSkillSummary {
    pub id: String,
    pub version: String,
    pub cache_path: PathBuf,
    pub sha256: String,
    pub executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistrySkillResolveResponse {
    skill: RegistrySkillMetadata,
    version: RegistrySkillVersionMetadata,
    artifact: RegistryArtifactMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistrySkillMetadata {
    id: String,
    name: String,
    slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistrySkillVersionMetadata {
    id: String,
    skill_id: String,
    version: String,
    manifest: serde_json::Value,
    artifact_id: String,
    artifact: RegistryArtifactMetadata,
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

pub struct SkillRegistryInstaller {
    base_url: String,
}

impl SkillRegistryInstaller {
    pub fn from_env() -> Self {
        Self::new(sofvary_api_base_url_from_env())
    }

    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn install_skill(
        &self,
        payload: InstallRegistrySkillPayload,
    ) -> SkillRegistryResult<InstalledSkillSummary> {
        let resolved = self.resolve_skill(&payload.id, &payload.version)?;
        validate_skill_artifact(&resolved.artifact)?;
        if resolved.version.artifact_id != resolved.artifact.id
            || resolved.version.artifact.sha256.to_lowercase()
                != resolved.artifact.sha256.to_lowercase()
        {
            return Err(SkillRegistryError::InvalidRegistryResponse(
                "skill version artifact metadata is inconsistent".to_string(),
            ));
        }
        let download = self.get_download_url(&resolved.artifact.id)?;
        validate_download_metadata(&self.base_url, &resolved.artifact, &download)?;
        let bytes = self.get_bytes(&download.download.url, &download.download.headers)?;
        if bytes.len() as u64 != resolved.artifact.size_bytes {
            return Err(SkillRegistryError::InvalidRegistryResponse(
                "downloaded skill size did not match registry metadata".to_string(),
            ));
        }
        verify_sha256(&bytes, &resolved.artifact.sha256)?;

        let adapter = current_adapter();
        let cache_dir = adapter
            .dirs()
            .map_err(|error| SkillRegistryError::Platform(error.to_string()))?
            .cache_dir
            .join("skills")
            .join(sanitize_path_segment(&resolved.skill.slug))
            .join(&resolved.version.version);
        fs::create_dir_all(&cache_dir)?;
        fs::write(cache_dir.join("skill-pack.zip"), &bytes)?;
        fs::write(
            cache_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&resolved.version.manifest)?,
        )?;

        Ok(InstalledSkillSummary {
            id: resolved.skill.id,
            version: resolved.version.version,
            cache_path: cache_dir,
            sha256: resolved.artifact.sha256,
            executable: false,
        })
    }

    fn resolve_skill(
        &self,
        id: &str,
        version: &str,
    ) -> SkillRegistryResult<RegistrySkillResolveResponse> {
        let url = format!(
            "{}/v1/registry/skills/resolve?id={}&version={}",
            self.base_url,
            encode_component(id),
            encode_component(version)
        );
        self.get_json(&url)
    }

    fn get_download_url(
        &self,
        artifact_id: &str,
    ) -> SkillRegistryResult<ArtifactDownloadUrlResponse> {
        let url = format!(
            "{}/v1/artifacts/{}/download-url",
            self.base_url,
            encode_component(artifact_id)
        );
        self.get_json(&url)
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> SkillRegistryResult<T> {
        let mut request = ureq::get(url);
        if let Some(authorization) = self.authorization_header()? {
            request = request.header("Authorization", &authorization);
        }
        let mut response = request
            .call()
            .map_err(|error| SkillRegistryError::Http(error.to_string()))?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| SkillRegistryError::Http(error.to_string()))?;
        Ok(serde_json::from_str(&text)?)
    }

    fn get_bytes(
        &self,
        url: &str,
        headers: &std::collections::HashMap<String, String>,
    ) -> SkillRegistryResult<Vec<u8>> {
        let mut request = ureq::get(url);
        if let Some(authorization) = self.authorization_header()? {
            request = request.header("Authorization", &authorization);
        }
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let mut response = request
            .call()
            .map_err(|error| SkillRegistryError::Http(error.to_string()))?;
        response
            .body_mut()
            .with_config()
            .limit(MAX_SKILL_ARTIFACT_BYTES)
            .read_to_vec()
            .map_err(|error| SkillRegistryError::Http(error.to_string()))
    }

    fn authorization_header(&self) -> SkillRegistryResult<Option<String>> {
        let adapter = current_adapter();
        let Some(refresh_token) = adapter
            .secure_store_get(ACCOUNT_REFRESH_TOKEN_KEY)
            .map_err(|error| SkillRegistryError::Platform(error.to_string()))?
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
            .map_err(|error| SkillRegistryError::Http(error.to_string()))?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| SkillRegistryError::Http(error.to_string()))?;
        let refreshed: RegistryAuthRefreshResponse = serde_json::from_str(&text)?;
        adapter
            .secure_store_set(ACCOUNT_REFRESH_TOKEN_KEY, &refreshed.tokens.refresh_token)
            .map_err(|error| SkillRegistryError::Platform(error.to_string()))?;
        Ok(Some(format!("Bearer {}", refreshed.tokens.access_token)))
    }
}

fn validate_skill_artifact(artifact: &RegistryArtifactMetadata) -> SkillRegistryResult<()> {
    if artifact.kind != "skill-pack" {
        return Err(SkillRegistryError::InvalidRegistryResponse(
            "skill install requires skill-pack artifact".to_string(),
        ));
    }
    if artifact.status != "uploaded" {
        return Err(SkillRegistryError::InvalidRegistryResponse(format!(
            "artifact '{}' is not uploaded",
            artifact.id
        )));
    }
    if artifact.size_bytes == 0 || artifact.size_bytes > MAX_SKILL_ARTIFACT_BYTES {
        return Err(SkillRegistryError::InvalidRegistryResponse(format!(
            "skill artifact size must be between 1 and {MAX_SKILL_ARTIFACT_BYTES} bytes"
        )));
    }
    if !is_sha256(&artifact.sha256) {
        return Err(SkillRegistryError::InvalidRegistryResponse(
            "artifact sha256 must be 64 hex characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_download_metadata(
    base_url: &str,
    expected: &RegistryArtifactMetadata,
    download: &ArtifactDownloadUrlResponse,
) -> SkillRegistryResult<()> {
    if download.download.method != "GET" {
        return Err(SkillRegistryError::InvalidRegistryResponse(
            "skill download method must be GET".to_string(),
        ));
    }
    if download.artifact.id != expected.id
        || download.artifact.kind != expected.kind
        || download.artifact.sha256.to_lowercase() != expected.sha256.to_lowercase()
        || download.artifact.size_bytes != expected.size_bytes
    {
        return Err(SkillRegistryError::InvalidRegistryResponse(
            "download artifact metadata does not match resolved skill version".to_string(),
        ));
    }
    validate_same_origin(base_url, &download.download.url)
}

fn validate_same_origin(base_url: &str, download_url: &str) -> SkillRegistryResult<()> {
    let base_origin = url_origin(base_url)?;
    let download_origin = url_origin(download_url)?;
    if base_origin != download_origin {
        return Err(SkillRegistryError::InvalidRegistryResponse(
            "skill downloads must come from the configured Sofvary Registry origin".to_string(),
        ));
    }
    Ok(())
}

fn verify_sha256(bytes: &[u8], expected: &str) -> SkillRegistryResult<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected.to_lowercase() {
        return Err(SkillRegistryError::Sha256Mismatch {
            expected: expected.to_lowercase(),
            actual,
        });
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn url_origin(value: &str) -> SkillRegistryResult<String> {
    let (scheme, rest) = value.split_once("://").ok_or_else(|| {
        SkillRegistryError::InvalidRegistryResponse("url must include a scheme".to_string())
    })?;
    if scheme != "http" && scheme != "https" {
        return Err(SkillRegistryError::InvalidRegistryResponse(
            "url scheme must be http or https".to_string(),
        ));
    }
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err(SkillRegistryError::InvalidRegistryResponse(
            "url authority is invalid".to_string(),
        ));
    }
    Ok(format!("{scheme}://{authority}"))
}

fn sanitize_path_segment(value: &str) -> String {
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
