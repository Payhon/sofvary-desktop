use crate::platform::CommandSpec;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyActionKind {
    AgentFileWrite,
    ExternalAgentProcess,
    CommandExecution,
    DependencyInstall,
    RuntimeStart,
    CapsuleImport,
    PackInstall,
    AgentInstall,
    RuntimeEnvironmentInstall,
    WorkspaceLockfileUpdate,
    PluginEnablement,
    AiProviderKeyStore,
    AiProviderRebind,
    AiProviderCall,
    AppRelease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyDecisionKind {
    Allowed,
    RequiresConfirmation,
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDecision {
    pub action: PolicyActionKind,
    pub decision: PolicyDecisionKind,
    pub title: String,
    pub summary: String,
    pub reasons: Vec<String>,
    pub risks: Vec<String>,
    pub subject: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyApprovalSet {
    #[serde(default)]
    pub approved: Vec<PolicyApprovalGrant>,
}

impl PolicyApprovalSet {
    pub fn permits(&self, action: PolicyActionKind, subject: Option<&str>) -> bool {
        self.approved.iter().any(|grant| {
            grant.action == action
                && match (&grant.subject, subject) {
                    (None, None) => true,
                    (None, Some(_)) => false,
                    (Some(expected), Some(actual)) => expected == actual,
                    (Some(_), None) => false,
                }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyApprovalGrant {
    pub action: PolicyActionKind,
    #[serde(default)]
    pub subject: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyCommandRequest {
    pub name: String,
    pub command: CommandSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyExternalAgentProcessRequest {
    pub agent_id: String,
    pub provider: String,
    pub transport: String,
    pub executable: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyAgentInstallRequest {
    pub agent_id: String,
    pub label: String,
    pub install_method: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRuntimeEnvironmentInstallRequest {
    pub kind: String,
    pub version: String,
    pub platform: String,
    pub sha256: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyFileWriteRequest {
    pub workspace_root: PathBuf,
    pub target_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRuntimeStartRequest {
    pub workspace_root: PathBuf,
    pub runtime_kind: String,
    pub bind: String,
    pub network: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPackInstallRequest {
    #[serde(default)]
    pub app_id: Option<String>,
    pub kind: String,
    pub id: String,
    pub version: String,
    pub trust_level: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyWorkspaceLockfileUpdateRequest {
    pub app_id: String,
    pub kind: String,
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PolicyAiProviderKeyStoreRequest {
    pub app_id: String,
    pub profile_id: String,
    pub provider_kind: String,
    pub secure_store_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PolicyAiProviderRebindRequest {
    pub app_id: String,
    pub requirement_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PolicyAiProviderCallRequest {
    pub app_id: String,
    pub profile_id: String,
    pub provider_kind: String,
    pub capability: String,
    pub gateway_bind: String,
    pub provider_binding_ready: bool,
    pub requires_secret: bool,
    pub secure_store_available: bool,
    pub secure_key_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyCapsuleImportRequest {
    pub name: String,
    pub network: String,
    pub workspace_read: Vec<String>,
    pub workspace_write: Vec<String>,
    pub requested: Vec<String>,
    pub plugin_packs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyAppReleaseRequest {
    pub app_id: String,
    pub app_name: String,
    pub target_platform: String,
    pub output_dir: PathBuf,
    pub include_ai_continuation: bool,
    pub runtime_kind: String,
    pub plugin_packs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{PolicyActionKind, PolicyApprovalGrant, PolicyApprovalSet};

    #[test]
    fn approval_subject_must_match_exactly() {
        let approvals = PolicyApprovalSet {
            approved: vec![PolicyApprovalGrant {
                action: PolicyActionKind::DependencyInstall,
                subject: Some("pnpm install --offline".to_string()),
            }],
        };

        assert!(approvals.permits(
            PolicyActionKind::DependencyInstall,
            Some("pnpm install --offline")
        ));
        assert!(!approvals.permits(
            PolicyActionKind::DependencyInstall,
            Some("pnpm install --global")
        ));
        assert!(!approvals.permits(PolicyActionKind::DependencyInstall, None));
    }

    #[test]
    fn empty_subject_grant_does_not_wildcard_subject_actions() {
        let approvals = PolicyApprovalSet {
            approved: vec![PolicyApprovalGrant {
                action: PolicyActionKind::CapsuleImport,
                subject: None,
            }],
        };

        assert!(approvals.permits(PolicyActionKind::CapsuleImport, None));
        assert!(!approvals.permits(PolicyActionKind::CapsuleImport, Some("Shared CRM")));
    }
}
