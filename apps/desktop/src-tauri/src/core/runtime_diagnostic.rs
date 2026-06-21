use crate::core::file_processor_runtime::FileProcessorRuntimeError;
use crate::core::react_project_runtime::ReactProjectRuntimeError;
use crate::core::react_sqlite_runtime::ReactSqliteRuntimeError;
use crate::core::react_vite_runtime::ReactViteRuntimeError;
use crate::core::runtime_dependency_install::{
    is_dependency_network_failure, is_dependency_version_resolution_failure,
    is_offline_dependency_cache_failure,
};
use crate::core::workspace_types::RuntimeKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const OUTPUT_TAIL_LIMIT: usize = 2400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDiagnosticStage {
    Install,
    Build,
    Api,
    DevServer,
    RuntimeStart,
    WorkspaceValidation,
    Policy,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDiagnosticCategory {
    Environment,
    GeneratedCode,
    Policy,
    RuntimeInfra,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDiagnosticRepairTarget {
    Agent,
    Sofvary,
    User,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnostic {
    pub runtime_kind: RuntimeKind,
    pub stage: RuntimeDiagnosticStage,
    #[serde(default)]
    pub command_name: Option<String>,
    #[serde(default)]
    pub status_code: Option<i32>,
    #[serde(default)]
    pub stdout_tail: Option<String>,
    #[serde(default)]
    pub stderr_tail: Option<String>,
    #[serde(default)]
    pub log_path: Option<PathBuf>,
    pub category: RuntimeDiagnosticCategory,
    pub repairable_by: RuntimeDiagnosticRepairTarget,
}

impl RuntimeDiagnostic {
    pub fn summary(&self) -> String {
        let stage = match self.stage {
            RuntimeDiagnosticStage::Install => "dependency install",
            RuntimeDiagnosticStage::Build => "build",
            RuntimeDiagnosticStage::Api => "local API",
            RuntimeDiagnosticStage::DevServer => "dev server",
            RuntimeDiagnosticStage::RuntimeStart => "runtime start",
            RuntimeDiagnosticStage::WorkspaceValidation => "workspace validation",
            RuntimeDiagnosticStage::Policy => "policy",
            RuntimeDiagnosticStage::Unknown => "runtime",
        };
        let actor = match self.repairable_by {
            RuntimeDiagnosticRepairTarget::Agent => "Agent can attempt a repair",
            RuntimeDiagnosticRepairTarget::Sofvary => "Sofvary environment setup is required",
            RuntimeDiagnosticRepairTarget::User => "user confirmation is required",
            RuntimeDiagnosticRepairTarget::None => "automatic repair is not available",
        };
        match (&self.command_name, self.status_code) {
            (Some(command), Some(status)) => {
                format!("{stage} command '{command}' failed with status {status}; {actor}")
            }
            (Some(command), None) => format!("{stage} command '{command}' failed; {actor}"),
            (None, _) => format!("{stage} failed; {actor}"),
        }
    }

    pub fn is_agent_repairable(&self) -> bool {
        self.repairable_by == RuntimeDiagnosticRepairTarget::Agent
    }
}

pub fn diagnostic_from_react_vite_error(
    runtime_kind: RuntimeKind,
    error: &ReactViteRuntimeError,
) -> RuntimeDiagnostic {
    match error {
        ReactViteRuntimeError::CommandFailed {
            name,
            status,
            stdout,
            stderr,
            log_path,
        } => diagnostic_from_command_failure(
            runtime_kind,
            name,
            *status,
            stdout,
            stderr,
            Some(log_path.clone()),
        ),
        ReactViteRuntimeError::DevServerNotReady(_) => {
            generated_code_diagnostic(runtime_kind, RuntimeDiagnosticStage::DevServer, None, None)
        }
        ReactViteRuntimeError::InvalidPromptEnvelope(message) => generated_code_diagnostic(
            runtime_kind,
            RuntimeDiagnosticStage::WorkspaceValidation,
            None,
            Some(message),
        ),
        ReactViteRuntimeError::Policy(error) => policy_diagnostic(runtime_kind, error.to_string()),
        ReactViteRuntimeError::MissingCommand { name, .. } => RuntimeDiagnostic {
            runtime_kind,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: Some(name.clone()),
            status_code: None,
            stdout_tail: None,
            stderr_tail: None,
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactViteRuntimeError::InvalidCommandSpec(message) => RuntimeDiagnostic {
            runtime_kind,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(message, OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactViteRuntimeError::Platform(error) => RuntimeDiagnostic {
            runtime_kind,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(&error.to_string(), OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::Environment,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactViteRuntimeError::Io(error) => RuntimeDiagnostic {
            runtime_kind,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(&error.to_string(), OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactViteRuntimeError::PathEscape => RuntimeDiagnostic {
            runtime_kind,
            stage: RuntimeDiagnosticStage::WorkspaceValidation,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some("runtime path escaped the workspace boundary".to_string()),
            log_path: None,
            category: RuntimeDiagnosticCategory::Policy,
            repairable_by: RuntimeDiagnosticRepairTarget::User,
        },
    }
}

pub fn diagnostic_from_react_sqlite_error(error: &ReactSqliteRuntimeError) -> RuntimeDiagnostic {
    match error {
        ReactSqliteRuntimeError::CommandFailed {
            name,
            status,
            stdout,
            stderr,
            log_path,
        } => diagnostic_from_command_failure(
            RuntimeKind::ReactSqlite,
            name,
            *status,
            stdout,
            stderr,
            Some(log_path.clone()),
        ),
        ReactSqliteRuntimeError::ApiNotReady(_) => generated_code_diagnostic(
            RuntimeKind::ReactSqlite,
            RuntimeDiagnosticStage::Api,
            None,
            None,
        ),
        ReactSqliteRuntimeError::DevServerNotReady(_) => generated_code_diagnostic(
            RuntimeKind::ReactSqlite,
            RuntimeDiagnosticStage::DevServer,
            None,
            None,
        ),
        ReactSqliteRuntimeError::InvalidPromptEnvelope(message) => generated_code_diagnostic(
            RuntimeKind::ReactSqlite,
            RuntimeDiagnosticStage::WorkspaceValidation,
            None,
            Some(message),
        ),
        ReactSqliteRuntimeError::Policy(error) => {
            policy_diagnostic(RuntimeKind::ReactSqlite, error.to_string())
        }
        ReactSqliteRuntimeError::MissingCommand { name, .. } => RuntimeDiagnostic {
            runtime_kind: RuntimeKind::ReactSqlite,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: Some(name.clone()),
            status_code: None,
            stdout_tail: None,
            stderr_tail: None,
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactSqliteRuntimeError::InvalidCommandSpec(message) => RuntimeDiagnostic {
            runtime_kind: RuntimeKind::ReactSqlite,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(message, OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactSqliteRuntimeError::Platform(error) => RuntimeDiagnostic {
            runtime_kind: RuntimeKind::ReactSqlite,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(&error.to_string(), OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::Environment,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactSqliteRuntimeError::Io(error) => RuntimeDiagnostic {
            runtime_kind: RuntimeKind::ReactSqlite,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(&error.to_string(), OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactSqliteRuntimeError::UnsupportedMode => RuntimeDiagnostic {
            runtime_kind: RuntimeKind::ReactSqlite,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some("React + SQLite currently supports dev mode only".to_string()),
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactSqliteRuntimeError::PathEscape => RuntimeDiagnostic {
            runtime_kind: RuntimeKind::ReactSqlite,
            stage: RuntimeDiagnosticStage::WorkspaceValidation,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some("runtime path escaped the workspace boundary".to_string()),
            log_path: None,
            category: RuntimeDiagnosticCategory::Policy,
            repairable_by: RuntimeDiagnosticRepairTarget::User,
        },
    }
}

pub fn diagnostic_from_react_project_error(
    runtime_kind: RuntimeKind,
    error: &ReactProjectRuntimeError,
) -> RuntimeDiagnostic {
    match error {
        ReactProjectRuntimeError::ReactVite(error) => {
            diagnostic_from_react_vite_error(runtime_kind, error)
        }
        ReactProjectRuntimeError::InvalidPromptEnvelope(message) => generated_code_diagnostic(
            runtime_kind,
            RuntimeDiagnosticStage::WorkspaceValidation,
            None,
            Some(message),
        ),
        ReactProjectRuntimeError::Io(error) => RuntimeDiagnostic {
            runtime_kind,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(&error.to_string(), OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        ReactProjectRuntimeError::PathEscape => RuntimeDiagnostic {
            runtime_kind,
            stage: RuntimeDiagnosticStage::WorkspaceValidation,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some("runtime path escaped the workspace boundary".to_string()),
            log_path: None,
            category: RuntimeDiagnosticCategory::Policy,
            repairable_by: RuntimeDiagnosticRepairTarget::User,
        },
    }
}

pub fn diagnostic_from_file_processor_error(
    error: &FileProcessorRuntimeError,
) -> RuntimeDiagnostic {
    match error {
        FileProcessorRuntimeError::ReactProject(error) => {
            diagnostic_from_react_project_error(RuntimeKind::FileProcessor, error)
        }
        FileProcessorRuntimeError::Io(error) => RuntimeDiagnostic {
            runtime_kind: RuntimeKind::FileProcessor,
            stage: RuntimeDiagnosticStage::RuntimeStart,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(&error.to_string(), OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::RuntimeInfra,
            repairable_by: RuntimeDiagnosticRepairTarget::Sofvary,
        },
        FileProcessorRuntimeError::UnselectedPath(_)
        | FileProcessorRuntimeError::MissingDryRun
        | FileProcessorRuntimeError::PathEscape(_)
        | FileProcessorRuntimeError::InvalidPlan(_) => RuntimeDiagnostic {
            runtime_kind: RuntimeKind::FileProcessor,
            stage: RuntimeDiagnosticStage::Policy,
            command_name: None,
            status_code: None,
            stdout_tail: None,
            stderr_tail: Some(tail(&error.to_string(), OUTPUT_TAIL_LIMIT)),
            log_path: None,
            category: RuntimeDiagnosticCategory::Policy,
            repairable_by: RuntimeDiagnosticRepairTarget::User,
        },
    }
}

pub fn diagnostic_from_command_failure(
    runtime_kind: RuntimeKind,
    command_name: &str,
    status_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    log_path: Option<PathBuf>,
) -> RuntimeDiagnostic {
    let stage = command_stage(command_name);
    let combined = format!("{stdout}\n{stderr}");
    let package_manifest_failure = contains_any(
        &combined,
        &[
            "package.json",
            "json.parse",
            "unexpected token",
            "invalid json",
            "manifest",
        ],
    );
    let toolchain_failure_text = contains_any(
        &combined,
        &[
            "sidecar",
            "No such file",
            "No such file or directory",
            "not recognized as",
            "ENOENT",
            "executable",
        ],
    );
    let install_environment_failure = command_name == "install"
        && !package_manifest_failure
        && !is_dependency_version_resolution_failure(stdout, stderr)
        && (is_offline_dependency_cache_failure(stdout, stderr)
            || is_dependency_network_failure(stdout, stderr)
            || toolchain_failure_text);

    let (category, repairable_by) = if install_environment_failure || toolchain_failure_text {
        (
            RuntimeDiagnosticCategory::Environment,
            RuntimeDiagnosticRepairTarget::Sofvary,
        )
    } else {
        (
            RuntimeDiagnosticCategory::GeneratedCode,
            RuntimeDiagnosticRepairTarget::Agent,
        )
    };

    RuntimeDiagnostic {
        runtime_kind,
        stage,
        command_name: Some(command_name.to_string()),
        status_code,
        stdout_tail: non_empty_tail(stdout),
        stderr_tail: non_empty_tail(stderr),
        log_path,
        category,
        repairable_by,
    }
}

fn generated_code_diagnostic(
    runtime_kind: RuntimeKind,
    stage: RuntimeDiagnosticStage,
    stdout: Option<&str>,
    stderr: Option<&str>,
) -> RuntimeDiagnostic {
    RuntimeDiagnostic {
        runtime_kind,
        stage,
        command_name: None,
        status_code: None,
        stdout_tail: stdout.and_then(non_empty_tail),
        stderr_tail: stderr.and_then(non_empty_tail),
        log_path: None,
        category: RuntimeDiagnosticCategory::GeneratedCode,
        repairable_by: RuntimeDiagnosticRepairTarget::Agent,
    }
}

fn policy_diagnostic(runtime_kind: RuntimeKind, message: String) -> RuntimeDiagnostic {
    RuntimeDiagnostic {
        runtime_kind,
        stage: RuntimeDiagnosticStage::Policy,
        command_name: None,
        status_code: None,
        stdout_tail: None,
        stderr_tail: Some(tail(&message, OUTPUT_TAIL_LIMIT)),
        log_path: None,
        category: RuntimeDiagnosticCategory::Policy,
        repairable_by: RuntimeDiagnosticRepairTarget::User,
    }
}

fn command_stage(command_name: &str) -> RuntimeDiagnosticStage {
    match command_name {
        "install" => RuntimeDiagnosticStage::Install,
        "build" => RuntimeDiagnosticStage::Build,
        "api" => RuntimeDiagnosticStage::Api,
        "dev" => RuntimeDiagnosticStage::DevServer,
        _ => RuntimeDiagnosticStage::Unknown,
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let value = value.to_ascii_lowercase();
    needles
        .iter()
        .any(|needle| value.contains(&needle.to_ascii_lowercase()))
}

fn non_empty_tail(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| tail(trimmed, OUTPUT_TAIL_LIMIT))
}

fn tail(value: &str, max_chars: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(max_chars);
    chars[start..].iter().collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_offline_failure_is_environment_not_agent_repairable() {
        let diagnostic = diagnostic_from_command_failure(
            RuntimeKind::ReactSqlite,
            "install",
            Some(1),
            "",
            "ERR_PNPM_NO_OFFLINE_TARBALL missing package in offline cache",
            None,
        );

        assert_eq!(diagnostic.stage, RuntimeDiagnosticStage::Install);
        assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Environment);
        assert_eq!(
            diagnostic.repairable_by,
            RuntimeDiagnosticRepairTarget::Sofvary
        );
        assert!(!diagnostic.is_agent_repairable());
    }

    #[test]
    fn install_package_manifest_failure_is_agent_repairable_generated_code() {
        let diagnostic = diagnostic_from_command_failure(
            RuntimeKind::ReactVite,
            "install",
            Some(1),
            "",
            "package.json: JSON.parse unexpected token",
            None,
        );

        assert_eq!(diagnostic.stage, RuntimeDiagnosticStage::Install);
        assert_eq!(
            diagnostic.category,
            RuntimeDiagnosticCategory::GeneratedCode
        );
        assert_eq!(
            diagnostic.repairable_by,
            RuntimeDiagnosticRepairTarget::Agent
        );
        assert!(diagnostic.is_agent_repairable());
    }

    #[test]
    fn install_dependency_version_resolution_failure_is_agent_repairable_generated_code() {
        let diagnostic = diagnostic_from_command_failure(
            RuntimeKind::ReactVite,
            "install",
            Some(1),
            "",
            "ERR_PNPM_NO_MATCHING_VERSION No matching version found for undici-types@~7.18.0",
            None,
        );

        assert_eq!(diagnostic.stage, RuntimeDiagnosticStage::Install);
        assert_eq!(
            diagnostic.category,
            RuntimeDiagnosticCategory::GeneratedCode
        );
        assert_eq!(
            diagnostic.repairable_by,
            RuntimeDiagnosticRepairTarget::Agent
        );
        assert!(diagnostic.is_agent_repairable());
    }

    #[test]
    fn install_network_fetch_failure_is_environment_not_agent_repairable() {
        let diagnostic = diagnostic_from_command_failure(
            RuntimeKind::ReactVite,
            "install",
            Some(1),
            "",
            "ERR_PNPM_META_FETCH_FAIL GET https://registry.example.test/react: EAI_AGAIN",
            None,
        );

        assert_eq!(diagnostic.stage, RuntimeDiagnosticStage::Install);
        assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Environment);
        assert_eq!(
            diagnostic.repairable_by,
            RuntimeDiagnosticRepairTarget::Sofvary
        );
        assert!(!diagnostic.is_agent_repairable());
    }

    #[test]
    fn build_failure_is_agent_repairable_generated_code() {
        let diagnostic = diagnostic_from_command_failure(
            RuntimeKind::ReactVite,
            "build",
            Some(2),
            "",
            "src/App.tsx: Expected closing tag",
            None,
        );

        assert_eq!(diagnostic.stage, RuntimeDiagnosticStage::Build);
        assert_eq!(
            diagnostic.category,
            RuntimeDiagnosticCategory::GeneratedCode
        );
        assert_eq!(
            diagnostic.repairable_by,
            RuntimeDiagnosticRepairTarget::Agent
        );
        assert!(diagnostic.is_agent_repairable());
    }

    #[test]
    fn api_readiness_failure_is_agent_repairable_generated_code() {
        let diagnostic = diagnostic_from_react_sqlite_error(&ReactSqliteRuntimeError::ApiNotReady(
            "http://127.0.0.1:3333/api/health".to_string(),
        ));

        assert_eq!(diagnostic.stage, RuntimeDiagnosticStage::Api);
        assert_eq!(
            diagnostic.category,
            RuntimeDiagnosticCategory::GeneratedCode
        );
        assert_eq!(
            diagnostic.repairable_by,
            RuntimeDiagnosticRepairTarget::Agent
        );
    }

    #[test]
    fn policy_failure_requires_user_confirmation() {
        let diagnostic = diagnostic_from_react_vite_error(
            RuntimeKind::ReactVite,
            &ReactViteRuntimeError::Policy(
                crate::core::policy_engine::PolicyError::RequiresConfirmation {
                    action: crate::core::policy_types::PolicyActionKind::DependencyInstall,
                    reason: "install".to_string(),
                },
            ),
        );

        assert_eq!(diagnostic.stage, RuntimeDiagnosticStage::Policy);
        assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Policy);
        assert_eq!(
            diagnostic.repairable_by,
            RuntimeDiagnosticRepairTarget::User
        );
    }
}
