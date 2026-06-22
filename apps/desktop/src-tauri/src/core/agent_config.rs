use crate::platform::{current_adapter, PlatformAdapter, PlatformError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("agent config not found: {0}")]
    NotFound(String),
    #[error("agent config is disabled: {0}")]
    Disabled(String),
    #[error("agent config is invalid: {0}")]
    Invalid(String),
}

pub type AgentConfigResult<T> = Result<T, AgentConfigError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentProvider {
    Codex,
    ClaudeCode,
    Cursor,
    Opencode,
    KimiCode,
    Qoder,
    DeepseekTui,
    SofvaryPi,
    Custom,
}

impl AgentProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
            Self::Opencode => "opencode",
            Self::KimiCode => "kimi-code",
            Self::Qoder => "qoder",
            Self::DeepseekTui => "deepseek-tui",
            Self::SofvaryPi => "sofvary-pi",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentTransportKind {
    Acp,
    Cli,
    PiRpc,
    WorkspaceHandoff,
}

impl AgentTransportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Acp => "acp",
            Self::Cli => "cli",
            Self::PiRpc => "pi-rpc",
            Self::WorkspaceHandoff => "workspace-handoff",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentInteractionMode {
    PiNative,
    ThirdPartyManaged,
    WorkspaceHandoff,
}

impl Default for AgentInteractionMode {
    fn default() -> Self {
        Self::PiNative
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentInstallSource {
    Bundled,
    DevOverride,
    ExternalPath,
    Missing,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommandConfig {
    pub executable: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub source: AgentInstallSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTestRecord {
    pub ok: bool,
    pub transport: AgentTransportKind,
    pub checked_at: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    pub id: String,
    pub provider: AgentProvider,
    pub label: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub acp: Option<AgentCommandConfig>,
    #[serde(default)]
    pub cli: Option<AgentCommandConfig>,
    #[serde(default)]
    pub allow_cli_fallback: bool,
    #[serde(default)]
    pub default_interaction_mode: Option<AgentInteractionMode>,
    #[serde(default)]
    pub last_test: Option<AgentTestRecord>,
}

impl AgentConfig {
    pub fn is_ready(&self) -> bool {
        self.enabled
            && (self.acp.is_some()
                || (self.provider == AgentProvider::SofvaryPi && self.cli.is_some())
                || (self.allow_cli_fallback && self.cli.is_some()))
    }

    pub fn effective_interaction_mode(&self) -> AgentInteractionMode {
        self.default_interaction_mode
            .unwrap_or_else(|| default_interaction_mode_for_provider(self.provider))
    }
}

pub fn default_interaction_mode_for_provider(provider: AgentProvider) -> AgentInteractionMode {
    if provider == AgentProvider::SofvaryPi {
        AgentInteractionMode::PiNative
    } else {
        AgentInteractionMode::ThirdPartyManaged
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfigState {
    #[serde(default)]
    pub default_agent_id: Option<String>,
    #[serde(default)]
    pub agents: Vec<AgentConfig>,
}

impl AgentConfigState {
    pub fn with_default(mut self) -> Self {
        if self.default_agent_id.as_ref().is_some_and(|id| {
            self.agents
                .iter()
                .any(|agent| agent.id == *id && agent.is_ready())
        }) {
            return self;
        }

        self.default_agent_id = self
            .agents
            .iter()
            .find(|agent| agent.is_ready())
            .map(|agent| agent.id.clone());
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentConfigStore;

impl AgentConfigStore {
    pub fn new() -> Self {
        Self
    }

    pub fn load(&self) -> AgentConfigResult<AgentConfigState> {
        let adapter = current_adapter();
        self.load_with_adapter(adapter.as_ref())
    }

    pub fn save(&self, state: &AgentConfigState) -> AgentConfigResult<()> {
        let adapter = current_adapter();
        self.save_with_adapter(state, adapter.as_ref())
    }

    pub fn load_with_adapter(
        &self,
        adapter: &dyn PlatformAdapter,
    ) -> AgentConfigResult<AgentConfigState> {
        let path = config_path(adapter)?;
        if !path.exists() {
            return Ok(AgentConfigState::default());
        }
        let bytes = fs::read(path)?;
        Ok(serde_json::from_slice::<AgentConfigState>(strip_utf8_bom(&bytes))?.with_default())
    }

    pub fn save_with_adapter(
        &self,
        state: &AgentConfigState,
        adapter: &dyn PlatformAdapter,
    ) -> AgentConfigResult<()> {
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

    pub fn upsert(&self, config: AgentConfig) -> AgentConfigResult<AgentConfigState> {
        validate_config(&config)?;
        let mut state = self.load()?;
        if let Some(existing) = state.agents.iter_mut().find(|agent| agent.id == config.id) {
            *existing = config;
        } else {
            state.agents.push(config);
        }
        state = state.with_default();
        self.save(&state)?;
        Ok(state)
    }

    pub fn delete(&self, agent_id: &str) -> AgentConfigResult<AgentConfigState> {
        let mut state = self.load()?;
        let original_len = state.agents.len();
        state.agents.retain(|agent| agent.id != agent_id);
        if state.agents.len() == original_len {
            return Err(AgentConfigError::NotFound(agent_id.to_string()));
        }
        if state.default_agent_id.as_deref() == Some(agent_id) {
            state.default_agent_id = None;
        }
        state = state.with_default();
        self.save(&state)?;
        Ok(state)
    }

    pub fn set_default(&self, agent_id: &str) -> AgentConfigResult<AgentConfigState> {
        let mut state = self.load()?;
        let config = state
            .agents
            .iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| AgentConfigError::NotFound(agent_id.to_string()))?;
        if !config.is_ready() {
            return Err(AgentConfigError::Disabled(agent_id.to_string()));
        }
        state.default_agent_id = Some(agent_id.to_string());
        self.save(&state)?;
        Ok(state)
    }

    pub fn resolve_agent(&self, agent_id: Option<&str>) -> AgentConfigResult<AgentConfig> {
        let state = self.load()?;
        resolve_agent_from_state(&state, agent_id)
    }

    pub fn record_test(
        &self,
        agent_id: &str,
        record: AgentTestRecord,
    ) -> AgentConfigResult<AgentConfigState> {
        let mut state = self.load()?;
        let config = state
            .agents
            .iter_mut()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| AgentConfigError::NotFound(agent_id.to_string()))?;
        config.last_test = Some(record);
        self.save(&state)?;
        Ok(state)
    }
}

pub fn resolve_agent_from_state(
    state: &AgentConfigState,
    agent_id: Option<&str>,
) -> AgentConfigResult<AgentConfig> {
    let id = agent_id
        .map(str::to_string)
        .or_else(|| state.default_agent_id.clone())
        .ok_or_else(|| {
            AgentConfigError::Invalid("no default coding agent is configured".to_string())
        })?;

    let config = state
        .agents
        .iter()
        .find(|agent| agent.id == id)
        .cloned()
        .ok_or_else(|| AgentConfigError::NotFound(id.clone()))?;
    if !config.is_ready() {
        return Err(AgentConfigError::Disabled(id));
    }
    Ok(config)
}

pub fn fresh_test_record(
    ok: bool,
    transport: AgentTransportKind,
    detail: impl Into<String>,
) -> AgentTestRecord {
    AgentTestRecord {
        ok,
        transport,
        checked_at: Utc::now().to_rfc3339(),
        detail: detail.into(),
    }
}

fn config_path(adapter: &dyn PlatformAdapter) -> AgentConfigResult<PathBuf> {
    Ok(adapter.dirs()?.config_dir.join("agents.json"))
}

fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

fn validate_config(config: &AgentConfig) -> AgentConfigResult<()> {
    if config.id.trim().is_empty() {
        return Err(AgentConfigError::Invalid(
            "agent id cannot be empty".to_string(),
        ));
    }
    if config.label.trim().is_empty() {
        return Err(AgentConfigError::Invalid(
            "agent label cannot be empty".to_string(),
        ));
    }
    if config.provider == AgentProvider::Custom && config.acp.is_none() && config.cli.is_none() {
        return Err(AgentConfigError::Invalid(
            "custom agent requires at least one command".to_string(),
        ));
    }
    Ok(())
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_selects_first_ready_default() {
        let state = AgentConfigState {
            default_agent_id: None,
            agents: vec![
                AgentConfig {
                    id: "missing".to_string(),
                    provider: AgentProvider::Codex,
                    label: "Missing".to_string(),
                    enabled: true,
                    acp: None,
                    cli: None,
                    allow_cli_fallback: false,
                    default_interaction_mode: None,
                    last_test: None,
                },
                AgentConfig {
                    id: "opencode".to_string(),
                    provider: AgentProvider::Opencode,
                    label: "OpenCode".to_string(),
                    enabled: true,
                    acp: Some(AgentCommandConfig {
                        executable: PathBuf::from("/bin/opencode"),
                        args: vec!["acp".to_string()],
                        env: HashMap::new(),
                        source: AgentInstallSource::ExternalPath,
                    }),
                    cli: None,
                    allow_cli_fallback: false,
                    default_interaction_mode: Some(AgentInteractionMode::WorkspaceHandoff),
                    last_test: None,
                },
            ],
        }
        .with_default();

        assert_eq!(state.default_agent_id.as_deref(), Some("opencode"));
        assert_eq!(
            state.agents[1].effective_interaction_mode(),
            AgentInteractionMode::WorkspaceHandoff
        );
    }

    #[test]
    fn strips_utf8_bom_before_json_parse() {
        assert_eq!(
            strip_utf8_bom(b"\xEF\xBB\xBF{\"agents\":[]}"),
            b"{\"agents\":[]}"
        );
        assert_eq!(strip_utf8_bom(b"{\"agents\":[]}"), b"{\"agents\":[]}");
    }
}
