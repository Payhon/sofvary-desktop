#![allow(dead_code)]

// Phase boundary: provider profile metadata exists before the shell exposes provider binding UI.
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AiProviderConfigError {
    #[error("AI provider profile is invalid: {0}")]
    InvalidProfile(String),
    #[error("AI provider binding is invalid: {0}")]
    InvalidBinding(String),
    #[error("AI provider profile not found: {0}")]
    ProfileNotFound(String),
    #[error("AI provider profile is disabled: {0}")]
    ProfileDisabled(String),
    #[error("AI provider profile is missing a model binding for {0:?}")]
    MissingModelBinding(AiCapability),
    #[error("AI provider profile requires a secure key reference: {0}")]
    MissingSecureKeyRef(String),
}

pub type AiProviderConfigResult<T> = Result<T, AiProviderConfigError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiCapability {
    Text,
    Image,
    Video,
}

impl AiCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiProviderKind {
    Openai,
    Anthropic,
    Openrouter,
    Deepseek,
    Google,
    Groq,
    Xai,
    Kimi,
    Ollama,
    OpenaiCompatible,
    Mock,
}

impl AiProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Openrouter => "openrouter",
            Self::Deepseek => "deepseek",
            Self::Google => "google",
            Self::Groq => "groq",
            Self::Xai => "xai",
            Self::Kimi => "kimi",
            Self::Ollama => "ollama",
            Self::OpenaiCompatible => "openai-compatible",
            Self::Mock => "mock",
        }
    }

    pub fn requires_secure_key_ref(self) -> bool {
        !matches!(self, Self::Ollama | Self::Mock)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecureKeyRef(String);

impl SecureKeyRef {
    pub fn new(value: impl Into<String>) -> AiProviderConfigResult<Self> {
        let value = value.into();
        validate_ref_value(&value)?;
        Ok(Self(value))
    }

    pub fn for_app_profile_api_key(app_id: &str, profile_id: &str) -> AiProviderConfigResult<Self> {
        validate_ref_segment("app_id", app_id)?;
        validate_ref_segment("profile_id", profile_id)?;
        Self::new(format!(
            "sofvary.ai-provider.apps.{app_id}.profiles.{profile_id}.api-key"
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecureKeyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecureKeyRef").field(&self.0).finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RawSecret(String);

impl RawSecret {
    pub fn new(value: impl Into<String>) -> AiProviderConfigResult<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AiProviderConfigError::InvalidProfile(
                "raw secret cannot be empty".to_string(),
            ));
        }
        Ok(Self(value))
    }

    pub fn expose_for_secure_store(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RawSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RawSecret([redacted])")
    }
}

impl<'de> Deserialize<'de> for RawSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        RawSecret::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModelBinding {
    pub capability: AiCapability,
    pub model: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl AiModelBinding {
    pub fn validate(&self) -> AiProviderConfigResult<()> {
        if self.enabled && self.model.trim().is_empty() {
            return Err(AiProviderConfigError::InvalidProfile(format!(
                "model binding for {:?} cannot be empty",
                self.capability
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderEndpoint {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
}

impl AiProviderEndpoint {
    pub fn validate(&self) -> AiProviderConfigResult<()> {
        if let Some(base_url) = &self.base_url {
            let trimmed = base_url.trim();
            if trimmed.is_empty() {
                return Err(AiProviderConfigError::InvalidProfile(
                    "base url cannot be empty when present".to_string(),
                ));
            }
            if trimmed.contains(char::is_whitespace) {
                return Err(AiProviderConfigError::InvalidProfile(
                    "base url cannot contain whitespace".to_string(),
                ));
            }
        }

        if self
            .organization_id
            .as_ref()
            .is_some_and(|organization_id| organization_id.trim().is_empty())
        {
            return Err(AiProviderConfigError::InvalidProfile(
                "organization id cannot be empty when present".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderProfile {
    pub app_id: String,
    pub profile_id: String,
    pub label: String,
    pub provider: AiProviderKind,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub endpoint: AiProviderEndpoint,
    #[serde(default)]
    pub secure_key_ref: Option<SecureKeyRef>,
    #[serde(default)]
    pub model_bindings: Vec<AiModelBinding>,
}

impl AiProviderProfile {
    pub fn validate(&self) -> AiProviderConfigResult<()> {
        validate_profile_id("app_id", &self.app_id)?;
        validate_profile_id("profile_id", &self.profile_id)?;
        if self.label.trim().is_empty() {
            return Err(AiProviderConfigError::InvalidProfile(
                "label cannot be empty".to_string(),
            ));
        }
        self.endpoint.validate()?;

        if self.enabled && self.provider.requires_secure_key_ref() && self.secure_key_ref.is_none()
        {
            return Err(AiProviderConfigError::MissingSecureKeyRef(format!(
                "{}:{}",
                self.app_id, self.profile_id
            )));
        }

        let mut seen = HashSet::new();
        for binding in &self.model_bindings {
            binding.validate()?;
            if binding.enabled && !seen.insert(binding.capability) {
                return Err(AiProviderConfigError::InvalidProfile(format!(
                    "duplicate enabled model binding for {:?}",
                    binding.capability
                )));
            }
        }

        Ok(())
    }

    pub fn model_for(&self, capability: AiCapability) -> Option<&str> {
        self.model_bindings
            .iter()
            .find(|binding| binding.enabled && binding.capability == capability)
            .map(|binding| binding.model.as_str())
    }

    pub fn validate_for_capability(
        &self,
        capability: AiCapability,
    ) -> AiProviderConfigResult<&str> {
        self.validate()?;
        if !self.enabled {
            return Err(AiProviderConfigError::ProfileDisabled(
                self.profile_id.clone(),
            ));
        }
        self.model_for(capability)
            .ok_or(AiProviderConfigError::MissingModelBinding(capability))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderBinding {
    pub app_id: String,
    pub profile_id: String,
    pub capability: AiCapability,
}

impl AiProviderBinding {
    pub fn validate_shape(&self) -> AiProviderConfigResult<()> {
        validate_profile_id("app_id", &self.app_id)?;
        validate_profile_id("profile_id", &self.profile_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppAiProviderProfiles {
    pub app_id: String,
    #[serde(default)]
    pub default_profile_id: Option<String>,
    #[serde(default)]
    pub profiles: Vec<AiProviderProfile>,
}

impl AppAiProviderProfiles {
    pub fn validate(&self) -> AiProviderConfigResult<()> {
        validate_profile_id("app_id", &self.app_id)?;
        let mut ids = HashSet::new();
        for profile in &self.profiles {
            profile.validate()?;
            if profile.app_id != self.app_id {
                return Err(AiProviderConfigError::InvalidProfile(format!(
                    "profile {} belongs to app {}, not {}",
                    profile.profile_id, profile.app_id, self.app_id
                )));
            }
            if !ids.insert(profile.profile_id.as_str()) {
                return Err(AiProviderConfigError::InvalidProfile(format!(
                    "duplicate profile id: {}",
                    profile.profile_id
                )));
            }
        }

        if let Some(default_profile_id) = &self.default_profile_id {
            if !self
                .profiles
                .iter()
                .any(|profile| profile.profile_id == *default_profile_id && profile.enabled)
            {
                return Err(AiProviderConfigError::InvalidProfile(format!(
                    "default profile is missing or disabled: {default_profile_id}"
                )));
            }
        }

        Ok(())
    }

    pub fn resolve_binding(
        &self,
        binding: &AiProviderBinding,
    ) -> AiProviderConfigResult<&AiProviderProfile> {
        self.validate()?;
        binding.validate_shape()?;
        if binding.app_id != self.app_id {
            return Err(AiProviderConfigError::InvalidBinding(format!(
                "binding app {} does not match profile set app {}",
                binding.app_id, self.app_id
            )));
        }

        let profile = self
            .profiles
            .iter()
            .find(|profile| profile.profile_id == binding.profile_id)
            .ok_or_else(|| AiProviderConfigError::ProfileNotFound(binding.profile_id.clone()))?;
        profile.validate_for_capability(binding.capability)?;
        Ok(profile)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderProfileUpsert {
    pub profile: AiProviderProfile,
    #[serde(default)]
    pub raw_secret: Option<RawSecret>,
}

pub fn secure_key_ref_for_app_profile_api_key(
    app_id: &str,
    profile_id: &str,
) -> AiProviderConfigResult<SecureKeyRef> {
    SecureKeyRef::for_app_profile_api_key(app_id, profile_id)
}

fn validate_ref_value(value: &str) -> AiProviderConfigResult<()> {
    if value.trim() != value || value.is_empty() {
        return Err(AiProviderConfigError::InvalidProfile(
            "secure key ref cannot be empty or padded".to_string(),
        ));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(AiProviderConfigError::InvalidProfile(
            "secure key ref contains unsupported characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_ref_segment(name: &str, value: &str) -> AiProviderConfigResult<()> {
    if value.trim() != value || value.is_empty() {
        return Err(AiProviderConfigError::InvalidProfile(format!(
            "{name} cannot be empty or padded"
        )));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(AiProviderConfigError::InvalidProfile(format!(
            "{name} contains unsupported characters"
        )));
    }
    Ok(())
}

fn validate_profile_id(name: &str, value: &str) -> AiProviderConfigResult<()> {
    validate_ref_segment(name, value)
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(provider: AiProviderKind) -> AiProviderProfile {
        AiProviderProfile {
            app_id: "app_alpha".to_string(),
            profile_id: "primary".to_string(),
            label: "Primary".to_string(),
            provider,
            enabled: true,
            endpoint: AiProviderEndpoint {
                base_url: None,
                organization_id: None,
            },
            secure_key_ref: Some(
                secure_key_ref_for_app_profile_api_key("app_alpha", "primary").unwrap(),
            ),
            model_bindings: vec![AiModelBinding {
                capability: AiCapability::Text,
                model: "gpt-5-mini".to_string(),
                enabled: true,
            }],
        }
    }

    #[test]
    fn secure_key_ref_is_scoped_to_app_and_profile() {
        let key_ref = secure_key_ref_for_app_profile_api_key("app_alpha", "primary").unwrap();

        assert_eq!(
            key_ref.as_str(),
            "sofvary.ai-provider.apps.app_alpha.profiles.primary.api-key"
        );
    }

    #[test]
    fn secure_key_ref_rejects_path_like_segments() {
        assert!(secure_key_ref_for_app_profile_api_key("../escape", "primary").is_err());
        assert!(secure_key_ref_for_app_profile_api_key("app_alpha", "primary/key").is_err());
    }

    #[test]
    fn raw_secret_debug_is_redacted_and_not_serializable() {
        let secret = RawSecret::new("sk-real-secret").unwrap();

        assert_eq!(format!("{secret:?}"), "RawSecret([redacted])");
        assert_eq!(secret.expose_for_secure_store(), "sk-real-secret");
    }

    #[test]
    fn profile_serializes_key_ref_but_not_raw_secret() {
        let profile = profile(AiProviderKind::Openai);
        let encoded = serde_json::to_string(&profile).unwrap();

        assert!(encoded.contains("secureKeyRef"));
        assert!(encoded.contains("sofvary.ai-provider.apps.app_alpha.profiles.primary.api-key"));
        assert!(!encoded.contains("sk-real-secret"));
        assert!(!encoded.contains("rawSecret"));
    }

    #[test]
    fn enabled_remote_provider_requires_secure_key_ref() {
        let mut profile = profile(AiProviderKind::Openai);
        profile.secure_key_ref = None;

        assert!(matches!(
            profile.validate(),
            Err(AiProviderConfigError::MissingSecureKeyRef(_))
        ));
    }

    #[test]
    fn mock_provider_can_be_keyless() {
        let mut profile = profile(AiProviderKind::Mock);
        profile.secure_key_ref = None;

        assert!(profile.validate().is_ok());
    }

    #[test]
    fn provider_binding_must_match_app_scope_and_capability() {
        let profile_set = AppAiProviderProfiles {
            app_id: "app_alpha".to_string(),
            default_profile_id: Some("primary".to_string()),
            profiles: vec![profile(AiProviderKind::Openai)],
        };

        let good = AiProviderBinding {
            app_id: "app_alpha".to_string(),
            profile_id: "primary".to_string(),
            capability: AiCapability::Text,
        };
        let wrong_app = AiProviderBinding {
            app_id: "app_beta".to_string(),
            ..good.clone()
        };
        let wrong_capability = AiProviderBinding {
            capability: AiCapability::Image,
            ..good.clone()
        };

        assert!(profile_set.resolve_binding(&good).is_ok());
        assert!(matches!(
            profile_set.resolve_binding(&wrong_app),
            Err(AiProviderConfigError::InvalidBinding(_))
        ));
        assert!(matches!(
            profile_set.resolve_binding(&wrong_capability),
            Err(AiProviderConfigError::MissingModelBinding(
                AiCapability::Image
            ))
        ));
    }
}
