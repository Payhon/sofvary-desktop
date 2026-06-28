use crate::platform::{current_adapter, PlatformAdapter, PlatformError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmProviderConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("llm provider config not found: {0}")]
    NotFound(String),
    #[error("llm provider config is invalid: {0}")]
    Invalid(String),
}

pub type LlmProviderConfigResult<T> = Result<T, LlmProviderConfigError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LlmProviderKind {
    Openai,
    Anthropic,
    Openrouter,
    Deepseek,
    Google,
    Groq,
    Xai,
    KimiCoding,
    Ollama,
    OpenaiCompatible,
}

impl LlmProviderKind {
    pub fn as_pi_provider(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Openrouter => "openrouter",
            Self::Deepseek => "deepseek",
            Self::Google => "google",
            Self::Groq => "groq",
            Self::Xai => "xai",
            Self::KimiCoding => "kimi-coding",
            Self::Ollama => "ollama",
            Self::OpenaiCompatible => "openai-compatible",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderTestRecord {
    pub ok: bool,
    pub checked_at: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderConfig {
    pub provider_id: String,
    pub label: String,
    pub kind: LlmProviderKind,
    #[serde(default)]
    pub base_url: Option<String>,
    pub model: String,
    #[serde(default)]
    pub api_key_ref: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub last_test: Option<LlmProviderTestRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderConfigState {
    #[serde(default)]
    pub default_provider_id: Option<String>,
    #[serde(default)]
    pub providers: Vec<LlmProviderConfig>,
}

impl LlmProviderConfigState {
    pub fn with_default(mut self) -> Self {
        if self.default_provider_id.as_ref().is_some_and(|id| {
            self.providers
                .iter()
                .any(|provider| provider.provider_id == *id && provider.enabled)
        }) {
            return self;
        }
        self.default_provider_id = self
            .providers
            .iter()
            .find(|provider| provider.enabled)
            .map(|provider| provider.provider_id.clone());
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertLlmProviderPayload {
    pub config: LlmProviderConfig,
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LlmProviderConfigStore;

impl LlmProviderConfigStore {
    pub fn new() -> Self {
        Self
    }

    pub fn load(&self) -> LlmProviderConfigResult<LlmProviderConfigState> {
        let adapter = current_adapter();
        self.load_with_adapter(adapter.as_ref())
    }

    pub fn load_with_adapter(
        &self,
        adapter: &dyn PlatformAdapter,
    ) -> LlmProviderConfigResult<LlmProviderConfigState> {
        let path = config_path(adapter)?;
        if !path.exists() {
            return Ok(LlmProviderConfigState::default());
        }
        let bytes = fs::read(path)?;
        Ok(
            serde_json::from_slice::<LlmProviderConfigState>(strip_utf8_bom(&bytes))?
                .with_default(),
        )
    }

    pub fn save(&self, state: &LlmProviderConfigState) -> LlmProviderConfigResult<()> {
        let adapter = current_adapter();
        self.save_with_adapter(state, adapter.as_ref())
    }

    pub fn save_with_adapter(
        &self,
        state: &LlmProviderConfigState,
        adapter: &dyn PlatformAdapter,
    ) -> LlmProviderConfigResult<()> {
        let path = config_path(adapter)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            path,
            serde_json::to_string_pretty(&state.clone().with_default())? + "\n",
        )?;
        Ok(())
    }

    pub fn upsert(
        &self,
        payload: UpsertLlmProviderPayload,
    ) -> LlmProviderConfigResult<LlmProviderConfigState> {
        let adapter = current_adapter();
        let mut config = payload.config;
        validate_config(&config)?;
        if let Some(api_key) = payload.api_key {
            if !api_key.trim().is_empty() {
                let key_ref = secure_key_ref(&config.provider_id);
                adapter.secure_store_set(&key_ref, &api_key)?;
                config.api_key_ref = Some(key_ref);
            }
        }
        let mut state = self.load_with_adapter(adapter.as_ref())?;
        if let Some(existing) = state
            .providers
            .iter_mut()
            .find(|provider| provider.provider_id == config.provider_id)
        {
            *existing = config;
        } else {
            state.providers.push(config);
        }
        state = state.with_default();
        self.save_with_adapter(&state, adapter.as_ref())?;
        Ok(state)
    }

    pub fn delete(&self, provider_id: &str) -> LlmProviderConfigResult<LlmProviderConfigState> {
        let mut state = self.load()?;
        let original_len = state.providers.len();
        state
            .providers
            .retain(|provider| provider.provider_id != provider_id);
        if state.providers.len() == original_len {
            return Err(LlmProviderConfigError::NotFound(provider_id.to_string()));
        }
        if state.default_provider_id.as_deref() == Some(provider_id) {
            state.default_provider_id = None;
        }
        state = state.with_default();
        self.save(&state)?;
        Ok(state)
    }

    pub fn set_default(
        &self,
        provider_id: &str,
    ) -> LlmProviderConfigResult<LlmProviderConfigState> {
        let mut state = self.load()?;
        let provider = state
            .providers
            .iter()
            .find(|provider| provider.provider_id == provider_id)
            .ok_or_else(|| LlmProviderConfigError::NotFound(provider_id.to_string()))?;
        if !provider.enabled {
            return Err(LlmProviderConfigError::Invalid(format!(
                "provider is disabled: {provider_id}"
            )));
        }
        state.default_provider_id = Some(provider_id.to_string());
        self.save(&state)?;
        Ok(state)
    }

    pub fn record_test(
        &self,
        provider_id: &str,
        record: LlmProviderTestRecord,
    ) -> LlmProviderConfigResult<LlmProviderConfigState> {
        let mut state = self.load()?;
        let provider = state
            .providers
            .iter_mut()
            .find(|provider| provider.provider_id == provider_id)
            .ok_or_else(|| LlmProviderConfigError::NotFound(provider_id.to_string()))?;
        provider.last_test = Some(record);
        self.save(&state)?;
        Ok(state)
    }

    pub fn resolve_default(&self) -> LlmProviderConfigResult<Option<LlmProviderConfig>> {
        let state = self.load()?;
        let Some(id) = state.default_provider_id else {
            return Ok(None);
        };
        Ok(state
            .providers
            .into_iter()
            .find(|provider| provider.provider_id == id && provider.enabled))
    }

    pub fn resolve_enabled(&self, provider_id: &str) -> LlmProviderConfigResult<LlmProviderConfig> {
        let state = self.load()?;
        let provider = state
            .providers
            .into_iter()
            .find(|provider| provider.provider_id == provider_id)
            .ok_or_else(|| LlmProviderConfigError::NotFound(provider_id.to_string()))?;
        if !provider.enabled {
            return Err(LlmProviderConfigError::Invalid(format!(
                "provider is disabled: {provider_id}"
            )));
        }
        Ok(provider)
    }
}

pub fn fresh_llm_test_record(ok: bool, detail: impl Into<String>) -> LlmProviderTestRecord {
    LlmProviderTestRecord {
        ok,
        checked_at: Utc::now().to_rfc3339(),
        detail: detail.into(),
    }
}

pub fn secure_key_ref(provider_id: &str) -> String {
    format!("sofvary.llm-provider.{provider_id}.api-key")
}

fn validate_config(config: &LlmProviderConfig) -> LlmProviderConfigResult<()> {
    if config.provider_id.trim().is_empty() {
        return Err(LlmProviderConfigError::Invalid(
            "provider id cannot be empty".to_string(),
        ));
    }
    if config.label.trim().is_empty() {
        return Err(LlmProviderConfigError::Invalid(
            "provider label cannot be empty".to_string(),
        ));
    }
    if config.model.trim().is_empty() {
        return Err(LlmProviderConfigError::Invalid(
            "provider model cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn config_path(adapter: &dyn PlatformAdapter) -> LlmProviderConfigResult<PathBuf> {
    Ok(adapter.dirs()?.config_dir.join("llm-providers.json"))
}

fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_key_ref_does_not_contain_secret_value() {
        assert_eq!(
            secure_key_ref("openai"),
            "sofvary.llm-provider.openai.api-key"
        );
    }

    #[test]
    fn strips_utf8_bom_before_json_parse() {
        assert_eq!(
            strip_utf8_bom(b"\xEF\xBB\xBF{\"providers\":[]}"),
            b"{\"providers\":[]}"
        );
        assert_eq!(strip_utf8_bom(b"{\"providers\":[]}"), b"{\"providers\":[]}");
    }
}
