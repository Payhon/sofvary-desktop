use crate::core::harness_engine::{PromptEnvelope, DESKTOP_WIDGET_ALLOWED_FILES};
use crate::core::pack_types::RuntimePackManifest;
use crate::core::policy_types::PolicyApprovalSet;
use crate::core::react_project_runtime::{
    ReactProjectRuntime, ReactProjectRuntimeError, ReactProjectRuntimeServer,
    ReactProjectRuntimeSpec,
};
use crate::core::workspace_types::{AppBoxManifest, RuntimeMode};

pub type DesktopWidgetRuntimeError = ReactProjectRuntimeError;
pub type DesktopWidgetRuntimeServer = ReactProjectRuntimeServer;

#[derive(Default)]
pub struct DesktopWidgetRuntime;

impl DesktopWidgetRuntime {
    pub fn new() -> Self {
        Self
    }

    #[allow(dead_code)]
    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), DesktopWidgetRuntimeError> {
        ReactProjectRuntime::new(desktop_widget_spec()).validate_prompt_envelope(envelope)
    }

    #[allow(dead_code)]
    pub fn start_workspace_with_envelope(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
    ) -> Result<DesktopWidgetRuntimeServer, DesktopWidgetRuntimeError> {
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
    ) -> Result<DesktopWidgetRuntimeServer, DesktopWidgetRuntimeError> {
        ReactProjectRuntime::new(desktop_widget_spec()).start_workspace_with_envelope_with_policy(
            manifest,
            envelope,
            runtime_pack,
            mode,
            approvals,
        )
    }
}

fn desktop_widget_spec() -> ReactProjectRuntimeSpec {
    ReactProjectRuntimeSpec {
        runtime_kind: "desktop-widget",
        generated_root: "generated",
        entrypoint: "react/src/main.tsx",
        output_format: "desktop-widget-project",
        allowed_files: &DESKTOP_WIDGET_ALLOWED_FILES,
        label: "Desktop Widget",
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
    fn desktop_widget_runtime_blocks_system_window_capabilities() {
        let envelope = test_envelope();
        for capability in [
            "transparent-window",
            "always-on-top-window",
            "tray-integration",
            "notification-plugin",
            "system-automation",
        ] {
            assert!(envelope
                .harness_policy
                .blocked_capabilities
                .contains(&capability.to_string()));
        }
        DesktopWidgetRuntime::new()
            .validate_prompt_envelope(&envelope)
            .expect("valid envelope");
    }

    fn test_envelope() -> PromptEnvelope {
        let allowed_files = DESKTOP_WIDGET_ALLOWED_FILES
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        PromptEnvelope {
            schema_version: "1.0".to_string(),
            envelope_id: "penv_widget_test".to_string(),
            created_at: "now".to_string(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: "sofvary.runtime.desktop-widget".to_string(),
                    version: "0.1.0".to_string(),
                },
                harness_packs: vec![PackReference {
                    id: "sofvary.harness.desktop-widget".to_string(),
                    version: "0.1.0".to_string(),
                }],
                runtime_kind: "desktop-widget".to_string(),
                generated_root: "generated".to_string(),
                entrypoint: "react/src/main.tsx".to_string(),
                bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
            },
            user_intent: "Build a pomodoro widget".to_string(),
            current_app_state: CurrentAppState {
                app_id: "app_test".to_string(),
                workspace_name: "Test".to_string(),
                mode: "create".to_string(),
                existing_files: Vec::new(),
                file_context: Vec::new(),
                preview_state: "empty".to_string(),
            },
            runtime_policy: RuntimePolicy {
                runtime_kind: "desktop-widget".to_string(),
                allowed_entrypoints: vec!["react/src/main.tsx".to_string()],
                allowed_server_bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
                package_install: false,
            },
            harness_policy: HarnessPolicy {
                system_instructions: Vec::new(),
                file_system_rules: Vec::new(),
                output_rules: Vec::new(),
                blocked_capabilities: vec![
                    "always-on-top-window".to_string(),
                    "notification-plugin".to_string(),
                    "system-automation".to_string(),
                    "transparent-window".to_string(),
                    "tray-integration".to_string(),
                ],
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
                format: "desktop-widget-project".to_string(),
                files: allowed_files,
                shell_ui_included: false,
            },
            acceptance_criteria: Vec::new(),
        }
    }
}
