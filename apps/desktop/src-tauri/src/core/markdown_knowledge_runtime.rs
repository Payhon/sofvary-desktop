use crate::core::harness_engine::{PromptEnvelope, MARKDOWN_KNOWLEDGE_ALLOWED_FILES};
use crate::core::pack_types::RuntimePackManifest;
use crate::core::policy_types::PolicyApprovalSet;
use crate::core::react_project_runtime::{
    ReactProjectRuntime, ReactProjectRuntimeError, ReactProjectRuntimeServer,
    ReactProjectRuntimeSpec,
};
use crate::core::workspace_types::{AppBoxManifest, RuntimeMode};

pub type MarkdownKnowledgeRuntimeError = ReactProjectRuntimeError;
pub type MarkdownKnowledgeRuntimeServer = ReactProjectRuntimeServer;

#[derive(Default)]
pub struct MarkdownKnowledgeRuntime;

impl MarkdownKnowledgeRuntime {
    pub fn new() -> Self {
        Self
    }

    #[allow(dead_code)]
    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), MarkdownKnowledgeRuntimeError> {
        ReactProjectRuntime::new(markdown_knowledge_spec()).validate_prompt_envelope(envelope)
    }

    #[allow(dead_code)]
    pub fn start_workspace_with_envelope(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
    ) -> Result<MarkdownKnowledgeRuntimeServer, MarkdownKnowledgeRuntimeError> {
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
    ) -> Result<MarkdownKnowledgeRuntimeServer, MarkdownKnowledgeRuntimeError> {
        ReactProjectRuntime::new(markdown_knowledge_spec())
            .start_workspace_with_envelope_with_policy(
                manifest,
                envelope,
                runtime_pack,
                mode,
                approvals,
            )
    }
}

fn markdown_knowledge_spec() -> ReactProjectRuntimeSpec {
    ReactProjectRuntimeSpec {
        runtime_kind: "markdown-knowledge",
        generated_root: "generated",
        entrypoint: "react/src/main.tsx",
        output_format: "markdown-knowledge-project",
        allowed_files: &MARKDOWN_KNOWLEDGE_ALLOWED_FILES,
        label: "Markdown Knowledge",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness_engine::{
        BoxRuntimeContext, CommandPolicy, CurrentAppState, FileSystemPolicy, HarnessPolicy,
        OutputContract, PackReference, RuntimePolicy,
    };

    #[test]
    fn markdown_knowledge_runtime_accepts_valid_envelope() {
        MarkdownKnowledgeRuntime::new()
            .validate_prompt_envelope(&test_envelope())
            .expect("valid envelope");
    }

    #[test]
    fn markdown_knowledge_runtime_rejects_non_markdown_kind() {
        let mut envelope = test_envelope();
        envelope.runtime_policy.runtime_kind = "data-table".to_string();

        let result = MarkdownKnowledgeRuntime::new().validate_prompt_envelope(&envelope);

        assert!(matches!(
            result,
            Err(MarkdownKnowledgeRuntimeError::InvalidPromptEnvelope(_))
        ));
    }

    fn test_envelope() -> PromptEnvelope {
        let allowed_files = MARKDOWN_KNOWLEDGE_ALLOWED_FILES
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        PromptEnvelope {
            schema_version: "1.0".to_string(),
            envelope_id: "penv_markdown_test".to_string(),
            created_at: "now".to_string(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: "sofvary.runtime.markdown-knowledge".to_string(),
                    version: "0.1.0".to_string(),
                },
                harness_packs: vec![PackReference {
                    id: "sofvary.harness.markdown-knowledge".to_string(),
                    version: "0.1.0".to_string(),
                }],
                runtime_kind: "markdown-knowledge".to_string(),
                generated_root: "generated".to_string(),
                entrypoint: "react/src/main.tsx".to_string(),
                bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
            },
            user_intent: "Build a reading notes app".to_string(),
            current_app_state: CurrentAppState {
                app_id: "app_test".to_string(),
                workspace_name: "Test".to_string(),
                mode: "create".to_string(),
                existing_files: Vec::new(),
                file_context: Vec::new(),
                preview_state: "empty".to_string(),
            },
            runtime_policy: RuntimePolicy {
                runtime_kind: "markdown-knowledge".to_string(),
                allowed_entrypoints: vec!["react/src/main.tsx".to_string()],
                allowed_server_bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
                package_install: false,
            },
            harness_policy: HarnessPolicy {
                system_instructions: Vec::new(),
                file_system_rules: Vec::new(),
                output_rules: Vec::new(),
                blocked_capabilities: Vec::new(),
            },
            file_system_policy: FileSystemPolicy {
                root: "generated".to_string(),
                allowed_files: allowed_files.clone(),
                allow_external_files: false,
                allow_path_traversal: false,
            },
            command_policy: CommandPolicy {
                allow_shell: false,
                allow_package_install: false,
                allow_global_install: false,
                allowed_commands: Vec::new(),
            },
            output_contract: OutputContract {
                format: "markdown-knowledge-project".to_string(),
                files: allowed_files,
                shell_ui_included: false,
            },
            acceptance_criteria: Vec::new(),
        }
    }
}
