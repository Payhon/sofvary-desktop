#![allow(dead_code)]

// Phase boundary: AI Gateway schemas and validation ship before runtime routing is wired.
use crate::core::ai_provider_config::{
    AiCapability, AiProviderBinding, AiProviderConfigError, AiProviderProfile,
    AppAiProviderProfiles,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AiGatewayError {
    #[error("AI gateway endpoint must bind to loopback only, got {0}")]
    NonLoopbackGateway(String),
    #[error("AI gateway request is invalid: {0}")]
    InvalidRequest(String),
    #[error("AI provider binding failed: {0}")]
    ProviderBinding(#[from] AiProviderConfigError),
}

pub type AiGatewayResult<T> = Result<T, AiGatewayError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiGatewayScheme {
    Http,
    Https,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopbackGatewayEndpoint {
    pub scheme: AiGatewayScheme,
    pub host: String,
    pub port: u16,
}

impl LoopbackGatewayEndpoint {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            scheme: AiGatewayScheme::Http,
            host: host.into(),
            port,
        }
    }

    pub fn validate(&self) -> AiGatewayResult<()> {
        if self.port == 0 {
            return Err(AiGatewayError::InvalidRequest(
                "gateway port cannot be 0".to_string(),
            ));
        }
        if !is_loopback_host(&self.host) {
            return Err(AiGatewayError::NonLoopbackGateway(self.host.clone()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGatewayRequestContext {
    pub request_id: String,
    pub workspace_id: String,
    pub gateway: LoopbackGatewayEndpoint,
    pub provider_binding: AiProviderBinding,
}

impl AiGatewayRequestContext {
    pub fn validate<'a>(
        &self,
        capability: AiCapability,
        provider_profiles: &'a AppAiProviderProfiles,
    ) -> AiGatewayResult<&'a AiProviderProfile> {
        validate_nonempty_id("request_id", &self.request_id)?;
        validate_nonempty_id("workspace_id", &self.workspace_id)?;
        self.gateway.validate()?;

        if self.provider_binding.capability != capability {
            return Err(AiGatewayError::InvalidRequest(format!(
                "request kind requires {:?}, binding requested {:?}",
                capability, self.provider_binding.capability
            )));
        }

        provider_profiles
            .resolve_binding(&self.provider_binding)
            .map_err(AiGatewayError::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiTextMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTextMessage {
    pub role: AiTextMessageRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTextJobRequest {
    pub context: AiGatewayRequestContext,
    pub messages: Vec<AiTextMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
}

impl AiTextJobRequest {
    pub fn validate<'a>(
        &self,
        provider_profiles: &'a AppAiProviderProfiles,
    ) -> AiGatewayResult<&'a AiProviderProfile> {
        let profile = self
            .context
            .validate(AiCapability::Text, provider_profiles)?;
        if self.messages.is_empty() {
            return Err(AiGatewayError::InvalidRequest(
                "text request requires at least one message".to_string(),
            ));
        }
        if self
            .messages
            .iter()
            .all(|message| message.content.trim().is_empty())
        {
            return Err(AiGatewayError::InvalidRequest(
                "text request messages cannot all be empty".to_string(),
            ));
        }
        if let Some(temperature) = self.temperature {
            if !(0.0..=2.0).contains(&temperature) {
                return Err(AiGatewayError::InvalidRequest(
                    "temperature must be between 0 and 2".to_string(),
                ));
            }
        }
        Ok(profile)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiImageFormat {
    Png,
    Jpeg,
    Webp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiImageSize {
    Square1024,
    Portrait1024x1536,
    Landscape1536x1024,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiImageJobRequest {
    pub context: AiGatewayRequestContext,
    pub prompt: String,
    pub size: AiImageSize,
    pub format: AiImageFormat,
    pub count: u8,
}

impl AiImageJobRequest {
    pub fn validate<'a>(
        &self,
        provider_profiles: &'a AppAiProviderProfiles,
    ) -> AiGatewayResult<&'a AiProviderProfile> {
        let profile = self
            .context
            .validate(AiCapability::Image, provider_profiles)?;
        validate_prompt("image prompt", &self.prompt)?;
        if !(1..=4).contains(&self.count) {
            return Err(AiGatewayError::InvalidRequest(
                "image request count must be between 1 and 4".to_string(),
            ));
        }
        Ok(profile)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiVideoAspectRatio {
    Landscape16x9,
    Portrait9x16,
    Square1x1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiVideoJobRequest {
    pub context: AiGatewayRequestContext,
    pub prompt: String,
    pub aspect_ratio: AiVideoAspectRatio,
    pub duration_seconds: u16,
}

impl AiVideoJobRequest {
    pub fn validate<'a>(
        &self,
        provider_profiles: &'a AppAiProviderProfiles,
    ) -> AiGatewayResult<&'a AiProviderProfile> {
        let profile = self
            .context
            .validate(AiCapability::Video, provider_profiles)?;
        validate_prompt("video prompt", &self.prompt)?;
        if !(1..=120).contains(&self.duration_seconds) {
            return Err(AiGatewayError::InvalidRequest(
                "video duration must be between 1 and 120 seconds".to_string(),
            ));
        }
        Ok(profile)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "jobKind", rename_all = "camelCase")]
pub enum AiGatewayJobRequest {
    Text(AiTextJobRequest),
    Image(AiImageJobRequest),
    Video(AiVideoJobRequest),
}

impl AiGatewayJobRequest {
    pub fn capability(&self) -> AiCapability {
        match self {
            Self::Text(_) => AiCapability::Text,
            Self::Image(_) => AiCapability::Image,
            Self::Video(_) => AiCapability::Video,
        }
    }

    pub fn validate<'a>(
        &self,
        provider_profiles: &'a AppAiProviderProfiles,
    ) -> AiGatewayResult<&'a AiProviderProfile> {
        match self {
            Self::Text(request) => request.validate(provider_profiles),
            Self::Image(request) => request.validate(provider_profiles),
            Self::Video(request) => request.validate(provider_profiles),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiJobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiErrorInfo {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTextJobResponse {
    pub request_id: String,
    pub job_id: String,
    pub status: AiJobStatus,
    pub provider_binding: AiProviderBinding,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub usage: Option<AiTokenUsage>,
    #[serde(default)]
    pub error: Option<AiErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiImageAsset {
    pub uri: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiImageJobResponse {
    pub request_id: String,
    pub job_id: String,
    pub status: AiJobStatus,
    pub provider_binding: AiProviderBinding,
    #[serde(default)]
    pub images: Vec<AiImageAsset>,
    #[serde(default)]
    pub error: Option<AiErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiVideoAsset {
    pub uri: String,
    pub mime_type: String,
    pub duration_seconds: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiVideoJobResponse {
    pub request_id: String,
    pub job_id: String,
    pub status: AiJobStatus,
    pub provider_binding: AiProviderBinding,
    #[serde(default)]
    pub videos: Vec<AiVideoAsset>,
    #[serde(default)]
    pub error: Option<AiErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "jobKind", rename_all = "camelCase")]
pub enum AiGatewayJobResponse {
    Text(AiTextJobResponse),
    Image(AiImageJobResponse),
    Video(AiVideoJobResponse),
}

pub fn is_loopback_host(host: &str) -> bool {
    let normalized = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "localhost" | "127.0.0.1" | "::1" | "0:0:0:0:0:0:0:1"
    )
}

fn validate_nonempty_id(name: &str, value: &str) -> AiGatewayResult<()> {
    if value.trim().is_empty() {
        return Err(AiGatewayError::InvalidRequest(format!(
            "{name} cannot be empty"
        )));
    }
    Ok(())
}

fn validate_prompt(name: &str, value: &str) -> AiGatewayResult<()> {
    if value.trim().is_empty() {
        return Err(AiGatewayError::InvalidRequest(format!(
            "{name} cannot be empty"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ai_provider_config::{
        secure_key_ref_for_app_profile_api_key, AiModelBinding, AiProviderEndpoint, AiProviderKind,
    };

    fn provider_profiles(capability: AiCapability) -> AppAiProviderProfiles {
        AppAiProviderProfiles {
            app_id: "app_alpha".to_string(),
            default_profile_id: Some("primary".to_string()),
            profiles: vec![AiProviderProfile {
                app_id: "app_alpha".to_string(),
                profile_id: "primary".to_string(),
                label: "Primary".to_string(),
                provider: AiProviderKind::Openai,
                enabled: true,
                endpoint: AiProviderEndpoint {
                    base_url: None,
                    organization_id: None,
                },
                secure_key_ref: Some(
                    secure_key_ref_for_app_profile_api_key("app_alpha", "primary").unwrap(),
                ),
                model_bindings: vec![AiModelBinding {
                    capability,
                    model: "model-1".to_string(),
                    enabled: true,
                }],
            }],
        }
    }

    fn context(capability: AiCapability) -> AiGatewayRequestContext {
        AiGatewayRequestContext {
            request_id: "req_1".to_string(),
            workspace_id: "workspace_1".to_string(),
            gateway: LoopbackGatewayEndpoint::new("127.0.0.1", 49152),
            provider_binding: AiProviderBinding {
                app_id: "app_alpha".to_string(),
                profile_id: "primary".to_string(),
                capability,
            },
        }
    }

    #[test]
    fn loopback_gateway_accepts_localhost_and_rejects_public_binds() {
        assert!(LoopbackGatewayEndpoint::new("localhost", 3000)
            .validate()
            .is_ok());
        assert!(LoopbackGatewayEndpoint::new("::1", 3000).validate().is_ok());
        assert!(matches!(
            LoopbackGatewayEndpoint::new("0.0.0.0", 3000).validate(),
            Err(AiGatewayError::NonLoopbackGateway(_))
        ));
        assert!(matches!(
            LoopbackGatewayEndpoint::new("192.168.1.2", 3000).validate(),
            Err(AiGatewayError::NonLoopbackGateway(_))
        ));
    }

    #[test]
    fn text_request_requires_bound_text_provider() {
        let request = AiTextJobRequest {
            context: context(AiCapability::Text),
            messages: vec![AiTextMessage {
                role: AiTextMessageRole::User,
                content: "Build a timer".to_string(),
            }],
            temperature: Some(0.5),
            max_output_tokens: Some(500),
        };

        assert!(request
            .validate(&provider_profiles(AiCapability::Text))
            .is_ok());
        assert!(matches!(
            request.validate(&provider_profiles(AiCapability::Image)),
            Err(AiGatewayError::ProviderBinding(
                AiProviderConfigError::MissingModelBinding(AiCapability::Text)
            ))
        ));
    }

    #[test]
    fn job_kind_must_match_provider_binding_capability() {
        let request = AiGatewayJobRequest::Image(AiImageJobRequest {
            context: context(AiCapability::Text),
            prompt: "A dashboard".to_string(),
            size: AiImageSize::Square1024,
            format: AiImageFormat::Png,
            count: 1,
        });

        assert!(matches!(
            request.validate(&provider_profiles(AiCapability::Image)),
            Err(AiGatewayError::InvalidRequest(_))
        ));
    }

    #[test]
    fn video_request_limits_duration() {
        let request = AiVideoJobRequest {
            context: context(AiCapability::Video),
            prompt: "Show the app loading".to_string(),
            aspect_ratio: AiVideoAspectRatio::Landscape16x9,
            duration_seconds: 121,
        };

        assert!(matches!(
            request.validate(&provider_profiles(AiCapability::Video)),
            Err(AiGatewayError::InvalidRequest(_))
        ));
    }

    #[test]
    fn serialized_gateway_request_does_not_contain_secret_fields() {
        let request = AiGatewayJobRequest::Text(AiTextJobRequest {
            context: context(AiCapability::Text),
            messages: vec![AiTextMessage {
                role: AiTextMessageRole::User,
                content: "Hello".to_string(),
            }],
            temperature: None,
            max_output_tokens: None,
        });

        let encoded = serde_json::to_string(&request).unwrap();
        assert!(!encoded.contains("apiKey"));
        assert!(!encoded.contains("rawSecret"));
        assert!(!encoded.contains("token"));
    }
}
