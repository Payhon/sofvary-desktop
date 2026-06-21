use crate::core::harness_engine::{PromptEnvelope, AI_AGENT_APP_ALLOWED_FILES};
use crate::core::pack_types::RuntimePackManifest;
use crate::core::policy_types::PolicyApprovalSet;
use crate::core::react_project_runtime::{
    ReactProjectRuntime, ReactProjectRuntimeError, ReactProjectRuntimeServer,
    ReactProjectRuntimeSpec,
};
use crate::core::workspace_types::{AppBoxManifest, RuntimeMode};

pub type AiAgentAppRuntimeError = ReactProjectRuntimeError;
pub type AiAgentAppRuntimeServer = ReactProjectRuntimeServer;

#[derive(Default)]
pub struct AiAgentAppRuntime;

impl AiAgentAppRuntime {
    pub fn new() -> Self {
        Self
    }

    #[allow(dead_code)]
    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), AiAgentAppRuntimeError> {
        ReactProjectRuntime::new(ai_agent_app_spec()).validate_prompt_envelope(envelope)
    }

    #[allow(dead_code)]
    pub fn start_workspace_with_envelope(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
    ) -> Result<AiAgentAppRuntimeServer, AiAgentAppRuntimeError> {
        self.start_workspace_with_envelope_with_policy(
            manifest,
            envelope,
            runtime_pack,
            mode,
            &PolicyApprovalSet::default(),
        )
    }

    pub fn start_workspace_with_envelope_with_policy(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
        approvals: &PolicyApprovalSet,
    ) -> Result<AiAgentAppRuntimeServer, AiAgentAppRuntimeError> {
        ReactProjectRuntime::new(ai_agent_app_spec()).start_workspace_with_envelope_with_policy(
            manifest,
            envelope,
            runtime_pack,
            mode,
            approvals,
        )
    }
}

fn ai_agent_app_spec() -> ReactProjectRuntimeSpec {
    ReactProjectRuntimeSpec {
        runtime_kind: "ai-agent-app",
        generated_root: "generated",
        entrypoint: "react/src/main.tsx",
        output_format: "ai-agent-app-project",
        allowed_files: &AI_AGENT_APP_ALLOWED_FILES,
        label: "AI Agent App",
    }
}
