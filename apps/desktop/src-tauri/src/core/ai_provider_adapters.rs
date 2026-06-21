#![allow(dead_code)]

// Phase boundary: provider adapter contracts are validated now and invoked in a later gateway pass.
use crate::core::ai_gateway::{
    AiGatewayError, AiGatewayResult, AiImageAsset, AiImageJobRequest, AiImageJobResponse,
    AiJobStatus, AiTextJobRequest, AiTextJobResponse, AiTokenUsage, AiVideoAsset,
    AiVideoJobRequest, AiVideoJobResponse,
};
use crate::core::ai_provider_config::{
    AiCapability, AiProviderBinding, AiProviderKind, AiProviderProfile,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AiProviderAdapterError {
    #[error("adapter does not support {0:?}")]
    UnsupportedCapability(AiCapability),
    #[error("gateway validation failed: {0}")]
    Gateway(#[from] AiGatewayError),
    #[error("provider profile is invalid: {0}")]
    ProviderProfile(String),
}

pub type AiProviderAdapterResult<T> = Result<T, AiProviderAdapterError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderAdapterInfo {
    pub adapter_id: String,
    pub provider: AiProviderKind,
    pub capabilities: Vec<AiCapability>,
}

pub trait AiProviderAdapter: Send + Sync {
    fn info(&self) -> AiProviderAdapterInfo;

    fn supports(&self, capability: AiCapability) -> bool {
        self.info().capabilities.contains(&capability)
    }

    fn generate_text(
        &self,
        request: &AiTextJobRequest,
        profile: &AiProviderProfile,
    ) -> AiProviderAdapterResult<AiTextJobResponse>;

    fn generate_image(
        &self,
        request: &AiImageJobRequest,
        profile: &AiProviderProfile,
    ) -> AiProviderAdapterResult<AiImageJobResponse>;

    fn generate_video(
        &self,
        request: &AiVideoJobRequest,
        profile: &AiProviderProfile,
    ) -> AiProviderAdapterResult<AiVideoJobResponse>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockAiProviderAdapter {
    info: AiProviderAdapterInfo,
}

impl MockAiProviderAdapter {
    pub fn new(adapter_id: impl Into<String>, capabilities: Vec<AiCapability>) -> Self {
        Self {
            info: AiProviderAdapterInfo {
                adapter_id: adapter_id.into(),
                provider: AiProviderKind::Mock,
                capabilities,
            },
        }
    }

    pub fn all_capabilities() -> Self {
        Self::new(
            "mock-ai-provider",
            vec![AiCapability::Text, AiCapability::Image, AiCapability::Video],
        )
    }

    fn ensure_capability(
        &self,
        binding: &AiProviderBinding,
        profile: &AiProviderProfile,
        capability: AiCapability,
    ) -> AiProviderAdapterResult<()> {
        if !self.info.capabilities.contains(&capability) {
            return Err(AiProviderAdapterError::UnsupportedCapability(capability));
        }
        if binding.app_id != profile.app_id
            || binding.profile_id != profile.profile_id
            || binding.capability != capability
        {
            return Err(AiProviderAdapterError::ProviderProfile(format!(
                "request binding {}:{}:{:?} does not match profile {}:{}:{:?}",
                binding.app_id,
                binding.profile_id,
                binding.capability,
                profile.app_id,
                profile.profile_id,
                capability
            )));
        }
        profile
            .validate_for_capability(capability)
            .map_err(|error| AiProviderAdapterError::ProviderProfile(error.to_string()))?;
        Ok(())
    }
}

impl Default for MockAiProviderAdapter {
    fn default() -> Self {
        Self::all_capabilities()
    }
}

impl AiProviderAdapter for MockAiProviderAdapter {
    fn info(&self) -> AiProviderAdapterInfo {
        self.info.clone()
    }

    fn generate_text(
        &self,
        request: &AiTextJobRequest,
        profile: &AiProviderProfile,
    ) -> AiProviderAdapterResult<AiTextJobResponse> {
        self.ensure_capability(
            &request.context.provider_binding,
            profile,
            AiCapability::Text,
        )?;
        let joined_prompt = request
            .messages
            .iter()
            .map(|message| message.content.trim())
            .filter(|content| !content.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let text = if joined_prompt.is_empty() {
            "Mock response.".to_string()
        } else {
            format!("Mock response for: {joined_prompt}")
        };

        Ok(AiTextJobResponse {
            request_id: request.context.request_id.clone(),
            job_id: format!("mock-text-{}", request.context.request_id),
            status: AiJobStatus::Succeeded,
            provider_binding: request.context.provider_binding.clone(),
            text: Some(text),
            usage: Some(AiTokenUsage {
                input_tokens: request.messages.len() as u32,
                output_tokens: 4,
                total_tokens: request.messages.len() as u32 + 4,
            }),
            error: None,
        })
    }

    fn generate_image(
        &self,
        request: &AiImageJobRequest,
        profile: &AiProviderProfile,
    ) -> AiProviderAdapterResult<AiImageJobResponse> {
        self.ensure_capability(
            &request.context.provider_binding,
            profile,
            AiCapability::Image,
        )?;
        let (width, height) = match request.size {
            crate::core::ai_gateway::AiImageSize::Square1024 => (1024, 1024),
            crate::core::ai_gateway::AiImageSize::Portrait1024x1536 => (1024, 1536),
            crate::core::ai_gateway::AiImageSize::Landscape1536x1024 => (1536, 1024),
        };
        let mime_type = match request.format {
            crate::core::ai_gateway::AiImageFormat::Png => "image/png",
            crate::core::ai_gateway::AiImageFormat::Jpeg => "image/jpeg",
            crate::core::ai_gateway::AiImageFormat::Webp => "image/webp",
        };

        Ok(AiImageJobResponse {
            request_id: request.context.request_id.clone(),
            job_id: format!("mock-image-{}", request.context.request_id),
            status: AiJobStatus::Succeeded,
            provider_binding: request.context.provider_binding.clone(),
            images: (0..request.count)
                .map(|index| AiImageAsset {
                    uri: format!("mock://image/{}/{}", request.context.request_id, index),
                    mime_type: mime_type.to_string(),
                    width,
                    height,
                })
                .collect(),
            error: None,
        })
    }

    fn generate_video(
        &self,
        request: &AiVideoJobRequest,
        profile: &AiProviderProfile,
    ) -> AiProviderAdapterResult<AiVideoJobResponse> {
        self.ensure_capability(
            &request.context.provider_binding,
            profile,
            AiCapability::Video,
        )?;

        Ok(AiVideoJobResponse {
            request_id: request.context.request_id.clone(),
            job_id: format!("mock-video-{}", request.context.request_id),
            status: AiJobStatus::Succeeded,
            provider_binding: request.context.provider_binding.clone(),
            videos: vec![AiVideoAsset {
                uri: format!("mock://video/{}", request.context.request_id),
                mime_type: "video/mp4".to_string(),
                duration_seconds: request.duration_seconds,
            }],
            error: None,
        })
    }
}

pub fn map_gateway_validation<T>(result: AiGatewayResult<T>) -> AiProviderAdapterResult<T> {
    result.map_err(AiProviderAdapterError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ai_gateway::{
        AiGatewayRequestContext, AiImageFormat, AiImageJobRequest, AiImageSize, AiTextMessage,
        AiTextMessageRole, AiVideoAspectRatio, LoopbackGatewayEndpoint,
    };
    use crate::core::ai_provider_config::{AiModelBinding, AiProviderBinding, AiProviderEndpoint};

    fn profile(capability: AiCapability) -> AiProviderProfile {
        AiProviderProfile {
            app_id: "app_alpha".to_string(),
            profile_id: "mock".to_string(),
            label: "Mock".to_string(),
            provider: AiProviderKind::Mock,
            enabled: true,
            endpoint: AiProviderEndpoint {
                base_url: None,
                organization_id: None,
            },
            secure_key_ref: None,
            model_bindings: vec![AiModelBinding {
                capability,
                model: format!("mock-{}", capability.as_str()),
                enabled: true,
            }],
        }
    }

    fn context(capability: AiCapability) -> AiGatewayRequestContext {
        AiGatewayRequestContext {
            request_id: "req_1".to_string(),
            workspace_id: "workspace_1".to_string(),
            gateway: LoopbackGatewayEndpoint::new("127.0.0.1", 3030),
            provider_binding: AiProviderBinding {
                app_id: "app_alpha".to_string(),
                profile_id: "mock".to_string(),
                capability,
            },
        }
    }

    #[test]
    fn mock_text_adapter_returns_deterministic_response() {
        let adapter = MockAiProviderAdapter::all_capabilities();
        let request = AiTextJobRequest {
            context: context(AiCapability::Text),
            messages: vec![AiTextMessage {
                role: AiTextMessageRole::User,
                content: "Build a clock".to_string(),
            }],
            temperature: None,
            max_output_tokens: None,
        };

        let response = adapter
            .generate_text(&request, &profile(AiCapability::Text))
            .unwrap();

        assert_eq!(response.status, AiJobStatus::Succeeded);
        assert_eq!(response.job_id, "mock-text-req_1");
        assert!(response.text.unwrap().contains("Build a clock"));
    }

    #[test]
    fn mock_image_adapter_respects_count_and_size() {
        let adapter = MockAiProviderAdapter::all_capabilities();
        let request = AiImageJobRequest {
            context: context(AiCapability::Image),
            prompt: "A compact app preview".to_string(),
            size: AiImageSize::Landscape1536x1024,
            format: AiImageFormat::Webp,
            count: 2,
        };

        let response = adapter
            .generate_image(&request, &profile(AiCapability::Image))
            .unwrap();

        assert_eq!(response.images.len(), 2);
        assert_eq!(response.images[0].width, 1536);
        assert_eq!(response.images[0].mime_type, "image/webp");
    }

    #[test]
    fn mock_video_adapter_uses_requested_duration() {
        let adapter = MockAiProviderAdapter::all_capabilities();
        let request = AiVideoJobRequest {
            context: context(AiCapability::Video),
            prompt: "A short demo".to_string(),
            aspect_ratio: AiVideoAspectRatio::Landscape16x9,
            duration_seconds: 6,
        };

        let response = adapter
            .generate_video(&request, &profile(AiCapability::Video))
            .unwrap();

        assert_eq!(response.videos[0].duration_seconds, 6);
        assert_eq!(response.videos[0].uri, "mock://video/req_1");
    }

    #[test]
    fn mock_adapter_rejects_unsupported_capability() {
        let adapter = MockAiProviderAdapter::new("text-only", vec![AiCapability::Text]);
        let request = AiImageJobRequest {
            context: context(AiCapability::Image),
            prompt: "A compact app preview".to_string(),
            size: AiImageSize::Square1024,
            format: AiImageFormat::Png,
            count: 1,
        };

        assert!(matches!(
            adapter.generate_image(&request, &profile(AiCapability::Image)),
            Err(AiProviderAdapterError::UnsupportedCapability(
                AiCapability::Image
            ))
        ));
    }

    #[test]
    fn mock_adapter_requires_request_binding_to_match_profile() {
        let adapter = MockAiProviderAdapter::all_capabilities();
        let mut request = AiTextJobRequest {
            context: context(AiCapability::Text),
            messages: vec![AiTextMessage {
                role: AiTextMessageRole::User,
                content: "Hello".to_string(),
            }],
            temperature: None,
            max_output_tokens: None,
        };
        request.context.provider_binding.profile_id = "other".to_string();

        assert!(matches!(
            adapter.generate_text(&request, &profile(AiCapability::Text)),
            Err(AiProviderAdapterError::ProviderProfile(_))
        ));
    }

    #[test]
    fn adapter_response_serialization_does_not_expose_secret_fields() {
        let adapter = MockAiProviderAdapter::all_capabilities();
        let request = AiTextJobRequest {
            context: context(AiCapability::Text),
            messages: vec![AiTextMessage {
                role: AiTextMessageRole::User,
                content: "Hello".to_string(),
            }],
            temperature: None,
            max_output_tokens: None,
        };

        let response = adapter
            .generate_text(&request, &profile(AiCapability::Text))
            .unwrap();
        let encoded = serde_json::to_string(&response).unwrap();

        assert!(!encoded.contains("apiKey"));
        assert!(!encoded.contains("rawSecret"));
        assert!(!encoded.contains("token"));
    }
}
