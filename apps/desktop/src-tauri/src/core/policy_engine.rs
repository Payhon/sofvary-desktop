use crate::core::policy_types::{
    PolicyActionKind, PolicyAgentInstallRequest, PolicyAiProviderCallRequest,
    PolicyAiProviderKeyStoreRequest, PolicyAiProviderRebindRequest, PolicyAppReleaseRequest,
    PolicyApprovalSet, PolicyCapsuleImportRequest, PolicyCommandRequest, PolicyDecision,
    PolicyDecisionKind, PolicyExternalAgentProcessRequest, PolicyFileWriteRequest,
    PolicyPackInstallRequest, PolicyRuntimeEnvironmentInstallRequest, PolicyRuntimeStartRequest,
    PolicyWorkspaceLockfileUpdateRequest,
};
use crate::platform::{command_executable_name, is_protected_delete_target};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("policy denied {action:?}: {reason}")]
    Forbidden {
        action: PolicyActionKind,
        reason: String,
    },
    #[error("policy requires confirmation for {action:?}: {reason}")]
    RequiresConfirmation {
        action: PolicyActionKind,
        reason: String,
    },
}

#[derive(Debug, Default, Clone)]
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate_file_write(&self, _request: PolicyFileWriteRequest) -> PolicyDecision {
        let request = _request;
        let subject = request.target_path.display().to_string();
        let normalized_root = normalize_existing_path(&request.workspace_root);
        let normalized_target = normalize_maybe_new_path(&request.target_path);
        if !path_inside(&normalized_target, &normalized_root) {
            return decision(
                PolicyActionKind::AgentFileWrite,
                PolicyDecisionKind::Forbidden,
                "File write blocked",
                "The requested file write leaves the active workspace boundary.",
                vec!["Workspace boundary must contain every generated file write.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        let relative = normalized_target
            .strip_prefix(&normalized_root)
            .unwrap_or(normalized_target.as_path());
        if relative.starts_with("generated")
            || relative == Path::new("app.box.json")
            || relative == Path::new("sofvary.lock.json")
        {
            return decision(
                PolicyActionKind::AgentFileWrite,
                PolicyDecisionKind::Allowed,
                "File write allowed",
                "The target is inside the active workspace generated area or metadata lockfile.",
                vec![
                    "Current workspace generated files and required metadata are allowed."
                        .to_string(),
                ],
                Vec::new(),
                Some(subject),
            );
        }

        PolicyDecision {
            action: PolicyActionKind::AgentFileWrite,
            decision: PolicyDecisionKind::Forbidden,
            title: "File write blocked".to_string(),
            summary: "The target path is not allowed by the Phase 22 file policy.".to_string(),
            reasons: vec![
                "Agent writes are limited to generated files and Sofvary metadata.".to_string(),
            ],
            risks: vec![subject.clone()],
            subject: Some(subject),
        }
    }

    pub fn evaluate_command(&self, _request: PolicyCommandRequest) -> PolicyDecision {
        let request = _request;
        let subject = command_subject(&request);

        if has_global_install(&request.command.args) {
            return decision(
                PolicyActionKind::CommandExecution,
                PolicyDecisionKind::Forbidden,
                "Command blocked",
                "Global package installation is forbidden by the Phase 22 command policy.",
                vec!["Global installs can modify shared system or user tooling.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        if modifies_path(&request) {
            return decision(
                PolicyActionKind::CommandExecution,
                PolicyDecisionKind::Forbidden,
                "Command blocked",
                "Commands that modify PATH are forbidden.",
                vec!["Generated apps must not modify system PATH.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        if uses_remote_download_script(&request) {
            return decision(
                PolicyActionKind::CommandExecution,
                PolicyDecisionKind::Forbidden,
                "Command blocked",
                "Remote download scripts are forbidden.",
                vec!["Sofvary does not execute curl/wget or shell download pipelines.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        if binds_public_interface(&request) {
            return decision(
                PolicyActionKind::CommandExecution,
                PolicyDecisionKind::Forbidden,
                "Command blocked",
                "Binding generated app servers to non-loopback interfaces is forbidden.",
                vec!["Runtime servers must remain on 127.0.0.1.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        if deletes_system_directory(&request) {
            return decision(
                PolicyActionKind::CommandExecution,
                PolicyDecisionKind::Forbidden,
                "Command blocked",
                "Deleting system directories is forbidden.",
                vec!["Delete operations cannot target system directories.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        if request.command.allowed_network {
            return decision(
                PolicyActionKind::CommandExecution,
                PolicyDecisionKind::RequiresConfirmation,
                "Network command requires approval",
                "This command requests network access.",
                vec!["Network access is off by default.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        if is_allowed_local_runtime_command(&request) {
            return decision(
                PolicyActionKind::CommandExecution,
                PolicyDecisionKind::Allowed,
                "Command allowed",
                "The command matches Sofvary's local runtime command allowlist.",
                vec!["Current workspace local dev/build commands are allowed.".to_string()],
                Vec::new(),
                Some(subject),
            );
        }

        decision(
            PolicyActionKind::CommandExecution,
            PolicyDecisionKind::RequiresConfirmation,
            "Command requires approval",
            "This structured command is not on the default allowlist.",
            vec!["Unknown commands require explicit approval.".to_string()],
            vec![subject.clone()],
            Some(subject),
        )
    }

    pub fn evaluate_external_agent_process(
        &self,
        request: PolicyExternalAgentProcessRequest,
    ) -> PolicyDecision {
        let subject = format!(
            "{}:{}:{}",
            request.agent_id, request.transport, request.executable
        );
        decision(
            PolicyActionKind::ExternalAgentProcess,
            PolicyDecisionKind::RequiresConfirmation,
            "External agent requires approval",
            "Sofvary is about to start a configured coding agent process.",
            vec![
                "External coding agents may use their own model credentials, network access, and native tool policy.".to_string(),
                "Sofvary will still stage file output and validate workspace writes before preview.".to_string(),
            ],
            vec![format!(
                "{} via {} ({})",
                request.provider, request.transport, request.executable
            )],
            Some(subject),
        )
    }

    pub fn evaluate_agent_install(&self, request: PolicyAgentInstallRequest) -> PolicyDecision {
        decision(
            PolicyActionKind::AgentInstall,
            PolicyDecisionKind::RequiresConfirmation,
            "Agent install requires approval",
            "Sofvary is about to install or open setup instructions for a coding agent.",
            vec![
                "Sofvary-managed installs stay inside the application data directory.".to_string(),
                "External agents remain external processes and must be discovered, configured, and tested before use.".to_string(),
            ],
            vec![format!(
                "{} via {} ({})",
                request.label, request.install_method, request.subject
            )],
            Some(request.subject),
        )
    }

    pub fn evaluate_runtime_environment_install(
        &self,
        request: PolicyRuntimeEnvironmentInstallRequest,
    ) -> PolicyDecision {
        decision(
            PolicyActionKind::RuntimeEnvironmentInstall,
            PolicyDecisionKind::RequiresConfirmation,
            "Runtime environment install requires approval",
            "Sofvary is about to install a managed runtime environment into application data.",
            vec![
                "Managed runtime environments stay inside the Sofvary data directory.".to_string(),
                "Sofvary will verify artifact hashes before activating sidecar executables."
                    .to_string(),
                "This does not modify the system PATH or use a global package manager.".to_string(),
            ],
            vec![format!(
                "{} {} for {} ({})",
                request.kind, request.version, request.platform, request.sha256
            )],
            Some(request.subject),
        )
    }

    pub fn evaluate_dependency_install(&self, _request: PolicyCommandRequest) -> PolicyDecision {
        let request = _request;
        let subject = dependency_install_subject(&request);
        let command_decision = self.evaluate_command(request.clone());
        if command_decision.decision == PolicyDecisionKind::Forbidden {
            return PolicyDecision {
                action: PolicyActionKind::DependencyInstall,
                subject: Some(subject),
                ..command_decision
            };
        }

        let mut reasons =
            vec!["Dependency installation is a high-risk action even when offline.".to_string()];
        let mut risks = vec![subject.clone()];
        if request.command.allowed_network {
            reasons.push(
                "Network access is only used to hydrate the local dependency cache when offline install cannot proceed."
                    .to_string(),
            );
            risks.push(
                "Package registry network access may download workspace dependencies.".to_string(),
            );
        }

        decision(
            PolicyActionKind::DependencyInstall,
            PolicyDecisionKind::RequiresConfirmation,
            "Dependency install requires approval",
            "The runtime wants to install workspace dependencies.",
            reasons,
            risks,
            Some(subject),
        )
    }

    pub fn evaluate_runtime_start(&self, request: PolicyRuntimeStartRequest) -> PolicyDecision {
        let subject = format!("{} on {}", request.runtime_kind, request.bind);
        if !is_allowed_loopback_bind(&request.bind) {
            return decision(
                PolicyActionKind::RuntimeStart,
                PolicyDecisionKind::Forbidden,
                "Runtime start blocked",
                "Generated app runtimes must bind to 127.0.0.1.",
                vec!["Runtime previews must stay local-only.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }
        if request.network != "local-only" {
            return decision(
                PolicyActionKind::RuntimeStart,
                PolicyDecisionKind::RequiresConfirmation,
                "Runtime network access requires approval",
                "The runtime requests non-local network access.",
                vec!["Network access is disabled by default.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        decision(
            PolicyActionKind::RuntimeStart,
            PolicyDecisionKind::Allowed,
            "Runtime start allowed",
            "The runtime starts inside the current workspace and binds locally.",
            vec!["Local runtime preview is allowed.".to_string()],
            Vec::new(),
            Some(subject),
        )
    }

    pub fn evaluate_pack_install(&self, request: PolicyPackInstallRequest) -> PolicyDecision {
        let subject = pack_install_subject(
            request.app_id.as_deref(),
            &request.kind,
            &request.id,
            &request.version,
        );
        if request.kind == "plugin" || request.trust_level != "builtin" {
            return decision(
                PolicyActionKind::PackInstall,
                PolicyDecisionKind::RequiresConfirmation,
                "Pack install requires approval",
                "Installing a registry or plugin pack requires explicit approval.",
                vec![
                    "Network-distributed packs are not implicitly trusted in Phase 22.".to_string(),
                ],
                vec![subject.clone()],
                Some(subject),
            );
        }

        decision(
            PolicyActionKind::PackInstall,
            PolicyDecisionKind::Allowed,
            "Pack install allowed",
            "Built-in pack installation is allowed.",
            vec!["Built-in packs are bundled with the desktop client.".to_string()],
            Vec::new(),
            Some(subject),
        )
    }

    pub fn evaluate_plugin_enablement(&self, plugin_id: &str) -> PolicyDecision {
        decision(
            PolicyActionKind::PluginEnablement,
            PolicyDecisionKind::RequiresConfirmation,
            "Plugin enablement requires approval",
            "Enabling a plugin pack requires explicit approval.",
            vec!["Plugin execution is still out of scope for Phase 22.".to_string()],
            vec![plugin_id.to_string()],
            Some(plugin_id.to_string()),
        )
    }

    pub fn evaluate_app_release(&self, request: PolicyAppReleaseRequest) -> PolicyDecision {
        let subject = app_release_subject(
            &request.app_id,
            &request.target_platform,
            &request.output_dir,
        );
        if request.app_id.trim().is_empty()
            || request.app_name.trim().is_empty()
            || request.target_platform.trim().is_empty()
        {
            return decision(
                PolicyActionKind::AppRelease,
                PolicyDecisionKind::Forbidden,
                "App release blocked",
                "Publishing requires complete app, platform, and output metadata.",
                vec!["Release metadata must be explicit before files are written.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }
        if request.output_dir.as_os_str().is_empty() {
            return decision(
                PolicyActionKind::AppRelease,
                PolicyDecisionKind::Forbidden,
                "App release blocked",
                "Publishing requires a concrete output folder.",
                vec!["Release artifacts cannot be written to an implicit location.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        let mut reasons = vec![
            "Publishing reads the generated workspace and writes a distributable release package."
                .to_string(),
            "The release package stores a seed copy and runtime metadata, not Sofvary account or marketplace state."
                .to_string(),
        ];
        let mut risks = vec![
            format!("Output: {}", request.output_dir.display()),
            format!("Runtime: {}", request.runtime_kind),
        ];
        if request.include_ai_continuation {
            reasons.push(
                "AI continuation is white-label and requires the installed app user to configure their own provider credential."
                    .to_string(),
            );
            risks.push(
                "AI continuation metadata is included; raw provider secrets are not allowed."
                    .to_string(),
            );
        }
        if !request.plugin_packs.is_empty() {
            risks.push(format!(
                "Plugin metadata: {}",
                request.plugin_packs.join(", ")
            ));
        }

        decision(
            PolicyActionKind::AppRelease,
            PolicyDecisionKind::RequiresConfirmation,
            "App release requires approval",
            "Sofvary is about to create a local beta release package for this generated app.",
            reasons,
            risks,
            Some(subject),
        )
    }

    pub fn evaluate_workspace_lockfile_update(
        &self,
        request: PolicyWorkspaceLockfileUpdateRequest,
    ) -> PolicyDecision {
        let subject = workspace_lockfile_update_subject(
            &request.app_id,
            &request.kind,
            &request.id,
            &request.version,
        );
        decision(
            PolicyActionKind::WorkspaceLockfileUpdate,
            PolicyDecisionKind::RequiresConfirmation,
            "Workspace lockfile update requires approval",
            "Installing a registry pack into a workspace changes that workspace's exact pack lockfile.",
            vec![
                "Workspace runtime, harness, and plugin versions are reproducibility-critical."
                    .to_string(),
            ],
            vec![subject.clone()],
            Some(subject),
        )
    }

    #[allow(dead_code)]
    pub fn evaluate_ai_provider_key_store(
        &self,
        request: PolicyAiProviderKeyStoreRequest,
    ) -> PolicyDecision {
        let subject = ai_provider_profile_subject(
            &request.app_id,
            &request.profile_id,
            &request.provider_kind,
        );
        if request.app_id.trim().is_empty()
            || request.profile_id.trim().is_empty()
            || request.provider_kind.trim().is_empty()
        {
            return decision(
                PolicyActionKind::AiProviderKeyStore,
                PolicyDecisionKind::Forbidden,
                "AI provider key storage blocked",
                "AI provider key storage requires complete app, profile, and provider metadata.",
                vec!["Provider credentials must be scoped to a specific app profile.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }
        if !request.secure_store_available {
            return decision(
                PolicyActionKind::AiProviderKeyStore,
                PolicyDecisionKind::Forbidden,
                "AI provider key storage blocked",
                "Secure credential storage is not available on this platform session.",
                vec!["API keys must only be written to the platform secure store.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        decision(
            PolicyActionKind::AiProviderKeyStore,
            PolicyDecisionKind::RequiresConfirmation,
            "AI provider key storage requires approval",
            "Sofvary is about to store an AI provider key in platform secure storage.",
            vec![
                "The key will not be written into generated app files, logs, capsules, or provider binding metadata."
                    .to_string(),
            ],
            vec![subject.clone()],
            Some(subject),
        )
    }

    #[allow(dead_code)]
    pub fn evaluate_ai_provider_rebind(
        &self,
        request: PolicyAiProviderRebindRequest,
    ) -> PolicyDecision {
        let subject = format!(
            "{}:{}->{}",
            request.app_id, request.requirement_id, request.profile_id
        );
        if request.app_id.trim().is_empty()
            || request.requirement_id.trim().is_empty()
            || request.profile_id.trim().is_empty()
        {
            return decision(
                PolicyActionKind::AiProviderRebind,
                PolicyDecisionKind::Forbidden,
                "AI provider binding blocked",
                "AI provider rebinding requires complete app, requirement, and profile metadata.",
                vec![
                    "Generated apps may receive binding status only, never local provider ids or key references."
                        .to_string(),
                ],
                vec![subject.clone()],
                Some(subject),
            );
        }

        decision(
            PolicyActionKind::AiProviderRebind,
            PolicyDecisionKind::RequiresConfirmation,
            "AI provider binding requires approval",
            "Sofvary is about to bind this AI Agent App to a local provider profile.",
            vec![
                "Capsule exports will keep only provider requirements and reset imported apps to needs-provider-binding."
                    .to_string(),
            ],
            vec![subject.clone()],
            Some(subject),
        )
    }

    #[allow(dead_code)]
    pub fn evaluate_ai_provider_call(
        &self,
        request: PolicyAiProviderCallRequest,
    ) -> PolicyDecision {
        let subject = format!(
            "{}:{}:{}:{}",
            request.app_id, request.profile_id, request.provider_kind, request.capability
        );
        if request.app_id.trim().is_empty()
            || request.profile_id.trim().is_empty()
            || request.provider_kind.trim().is_empty()
            || request.capability.trim().is_empty()
        {
            return decision(
                PolicyActionKind::AiProviderCall,
                PolicyDecisionKind::Forbidden,
                "AI provider call blocked",
                "AI provider calls require complete app, profile, provider, and capability metadata.",
                vec!["Provider calls must be attributable to a local app binding.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }
        if !request.provider_binding_ready {
            return decision(
                PolicyActionKind::AiProviderCall,
                PolicyDecisionKind::Forbidden,
                "AI provider call blocked",
                "This AI Agent App does not have an approved provider binding.",
                vec!["Imported AI Agent Apps start as needs-provider-binding.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }
        if !is_allowed_loopback_bind(&request.gateway_bind) {
            return decision(
                PolicyActionKind::AiProviderCall,
                PolicyDecisionKind::Forbidden,
                "AI provider call blocked",
                "Generated AI Agent Apps may only call the Sofvary AI Gateway on 127.0.0.1.",
                vec![
                    "Direct provider network calls from generated apps are forbidden.".to_string(),
                ],
                vec![request.gateway_bind, subject.clone()],
                Some(subject),
            );
        }
        if request.requires_secret && !request.secure_store_available {
            return decision(
                PolicyActionKind::AiProviderCall,
                PolicyDecisionKind::Forbidden,
                "AI provider call blocked",
                "Secure credential storage is not available, so real AI provider calls are disabled.",
                vec![
                    "Provider keys must come from platform secure storage before a gateway adapter can call a remote provider."
                        .to_string(),
                ],
                vec![subject.clone()],
                Some(subject),
            );
        }
        if request.requires_secret && !request.secure_key_available {
            return decision(
                PolicyActionKind::AiProviderCall,
                PolicyDecisionKind::Forbidden,
                "AI provider call blocked",
                "The selected AI provider profile does not have a stored credential.",
                vec!["No token or API key is available for this provider binding.".to_string()],
                vec![subject.clone()],
                Some(subject),
            );
        }

        decision(
            PolicyActionKind::AiProviderCall,
            PolicyDecisionKind::RequiresConfirmation,
            "AI provider call requires approval",
            "Sofvary is about to send this request through the local AI Gateway.",
            vec![
                "Generated code talks only to the loopback gateway; provider credentials remain in secure storage."
                    .to_string(),
            ],
            vec![subject.clone()],
            Some(subject),
        )
    }

    pub fn evaluate_capsule_import(&self, request: PolicyCapsuleImportRequest) -> PolicyDecision {
        let mut risks = Vec::new();
        if request.network != "local-only" {
            return decision(
                PolicyActionKind::CapsuleImport,
                PolicyDecisionKind::Forbidden,
                "Capsule import blocked",
                "Capsules requesting non-local network access are blocked in Phase 22.",
                vec!["Imported app capsules must remain local-only.".to_string()],
                vec![request.network],
                Some(request.name),
            );
        }
        if !request.workspace_write.is_empty() {
            risks.push(format!(
                "Workspace write: {}",
                request.workspace_write.join(", ")
            ));
        }
        if !request.requested.is_empty() {
            risks.push(format!("Requested: {}", request.requested.join(", ")));
        }
        if !request.plugin_packs.is_empty() {
            risks.push(format!("Plugins: {}", request.plugin_packs.join(", ")));
        }

        decision(
            PolicyActionKind::CapsuleImport,
            PolicyDecisionKind::RequiresConfirmation,
            "Capsule import requires approval",
            "Importing an App Capsule creates a new local workspace from external package metadata.",
            vec!["Capsule imports require shell-owned permission review.".to_string()],
            risks,
            Some(request.name),
        )
    }

    pub fn enforce(
        &self,
        decision: PolicyDecision,
        _approvals: &PolicyApprovalSet,
    ) -> Result<(), PolicyError> {
        let approvals = _approvals;
        match decision.decision {
            PolicyDecisionKind::Allowed => Ok(()),
            PolicyDecisionKind::RequiresConfirmation => {
                if approvals.permits(decision.action, decision.subject.as_deref()) {
                    Ok(())
                } else {
                    Err(PolicyError::RequiresConfirmation {
                        action: decision.action,
                        reason: decision.summary,
                    })
                }
            }
            PolicyDecisionKind::Forbidden => Err(PolicyError::Forbidden {
                action: decision.action,
                reason: decision.summary,
            }),
        }
    }
}

pub fn pack_install_subject(app_id: Option<&str>, kind: &str, id: &str, version: &str) -> String {
    match app_id {
        Some(app_id) => format!("{app_id}:{kind}:{id}@{version}"),
        None => format!("{kind}:{id}@{version}"),
    }
}

pub fn workspace_lockfile_update_subject(
    app_id: &str,
    kind: &str,
    id: &str,
    version: &str,
) -> String {
    format!("{app_id}:{kind}:{id}@{version}")
}

pub fn app_release_subject(app_id: &str, platform: &str, output_dir: &Path) -> String {
    format!("{app_id}:{platform}:{}", output_dir.display())
}

#[allow(dead_code)]
pub fn ai_provider_profile_subject(app_id: &str, profile_id: &str, provider_kind: &str) -> String {
    format!("{app_id}:{profile_id}:{provider_kind}")
}

pub fn path_inside(_path: &Path, _root: &Path) -> bool {
    let path = normalize_path_lexically(_path);
    let root = normalize_path_lexically(_root);
    path == root || path.starts_with(root)
}

pub fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    path.canonicalize()
        .map(|canonical| normalize_path_lexically(&canonical))
        .unwrap_or_else(|_| normalize_path_lexically(path))
}

fn normalize_maybe_new_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return normalize_path_lexically(&canonical);
    }

    if let (Some(parent), Some(file_name)) = (path.parent(), path.file_name()) {
        if let Ok(canonical_parent) = parent.canonicalize() {
            return normalize_path_lexically(&canonical_parent.join(file_name));
        }
    }

    normalize_path_lexically(path)
}

pub fn command_subject(request: &PolicyCommandRequest) -> String {
    let mut parts = Vec::with_capacity(request.command.args.len() + 1);
    parts.push(request.command.executable.display().to_string());
    parts.extend(request.command.args.iter().cloned());
    parts.join(" ")
}

fn dependency_install_subject(request: &PolicyCommandRequest) -> String {
    let mut parts = Vec::with_capacity(request.command.args.len() + 1);
    parts.push(
        command_executable_name(&request.command.executable)
            .unwrap_or_else(|| request.command.executable.display().to_string()),
    );
    parts.extend(request.command.args.iter().cloned());
    parts.join(" ")
}

fn decision(
    action: PolicyActionKind,
    decision: PolicyDecisionKind,
    title: &str,
    summary: &str,
    reasons: Vec<String>,
    risks: Vec<String>,
    subject: Option<String>,
) -> PolicyDecision {
    PolicyDecision {
        action,
        decision,
        title: title.to_string(),
        summary: summary.to_string(),
        reasons,
        risks,
        subject,
    }
}

fn has_global_install(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "-g" || arg == "--global")
}

fn modifies_path(request: &PolicyCommandRequest) -> bool {
    request
        .command
        .env
        .keys()
        .any(|key| key.eq_ignore_ascii_case("PATH"))
        || request.command.args.iter().any(|arg| {
            arg.eq_ignore_ascii_case("PATH")
                || arg
                    .split_once('=')
                    .is_some_and(|(key, _)| key.eq_ignore_ascii_case("PATH"))
        })
}

fn uses_remote_download_script(request: &PolicyCommandRequest) -> bool {
    let executable = command_executable_name(&request.command.executable);
    let joined = request.command.args.join(" ").to_lowercase();
    matches!(executable.as_deref(), Some("curl" | "wget"))
        || ((joined.contains("curl ")
            || joined.contains("wget ")
            || joined.contains("irm ")
            || joined.contains("iwr "))
            && (joined.contains("http://") || joined.contains("https://")))
}

fn binds_public_interface(request: &PolicyCommandRequest) -> bool {
    if request
        .command
        .args
        .iter()
        .chain(request.command.env.values())
        .any(|value| value.contains("0.0.0.0") || value == "::" || value == "[::]")
    {
        return true;
    }

    for (index, arg) in request.command.args.iter().enumerate() {
        if matches!(arg.as_str(), "--host" | "--hostname" | "-H") {
            if let Some(host) = request.command.args.get(index + 1) {
                if !is_allowed_loopback_bind(host) {
                    return true;
                }
            }
        }
        for prefix in ["--host=", "--hostname="] {
            if let Some(host) = arg.strip_prefix(prefix) {
                if !is_allowed_loopback_bind(host) {
                    return true;
                }
            }
        }
    }

    request
        .command
        .env
        .iter()
        .any(|(key, value)| is_host_env_key(key) && !is_allowed_loopback_bind(value))
}

fn is_host_env_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("HOST") || key.to_ascii_uppercase().ends_with("_HOST")
}

fn is_allowed_loopback_bind(value: &str) -> bool {
    value.trim().trim_matches(|ch| ch == '"' || ch == '\'') == "127.0.0.1"
}

fn deletes_system_directory(request: &PolicyCommandRequest) -> bool {
    let executable = command_executable_name(&request.command.executable);
    let is_delete = matches!(executable.as_deref(), Some("rm" | "rmdir" | "del"));
    is_delete
        && request
            .command
            .args
            .iter()
            .any(|arg| is_protected_delete_target(arg))
}

fn is_allowed_local_runtime_command(request: &PolicyCommandRequest) -> bool {
    let executable = command_executable_name(&request.command.executable);
    if executable.as_deref() != Some("pnpm") {
        return false;
    }
    let args = request.command.args.as_slice();
    (args.len() >= 6
        && args[0] == "exec"
        && args[1] == "vite"
        && args[2] == "--host"
        && args[3] == "127.0.0.1"
        && args[4] == "--port")
        || (args == ["exec", "tsx", "server/index.ts"])
        || (args == ["run", "build"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::CommandSpec;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn policy_blocks_path_escape() {
        let engine = PolicyEngine::new();
        let workspace_root = PathBuf::from("/tmp/sofvary/app_1");

        let decision = engine.evaluate_file_write(PolicyFileWriteRequest {
            workspace_root,
            target_path: PathBuf::from("/tmp/sofvary/secret.txt"),
        });

        assert_eq!(decision.decision, PolicyDecisionKind::Forbidden);
    }

    #[test]
    fn policy_blocks_forbidden_command() {
        let engine = PolicyEngine::new();

        let decision = engine.evaluate_command(PolicyCommandRequest {
            name: "install".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("npm"),
                args: vec![
                    "install".to_string(),
                    "-g".to_string(),
                    "unsafe-tool".to_string(),
                ],
                cwd: PathBuf::from("/tmp/sofvary/app_1"),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(1_000),
                kill_on_drop: true,
            },
        });

        assert_eq!(decision.decision, PolicyDecisionKind::Forbidden);
    }

    #[test]
    fn policy_blocks_path_mutation_case_insensitively() {
        let engine = PolicyEngine::new();
        let mut env = HashMap::new();
        env.insert("Path".to_string(), "/tmp/bin".to_string());

        let decision = engine.evaluate_command(PolicyCommandRequest {
            name: "dev".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("pnpm"),
                args: vec!["exec".to_string(), "vite".to_string()],
                cwd: PathBuf::from("/tmp/sofvary/app_1"),
                env,
                allowed_network: false,
                timeout_ms: Some(1_000),
                kill_on_drop: true,
            },
        });

        assert_eq!(decision.decision, PolicyDecisionKind::Forbidden);
    }

    #[test]
    fn policy_blocks_public_bind_argument_forms() {
        let engine = PolicyEngine::new();

        let zero_bind = engine.evaluate_command(PolicyCommandRequest {
            name: "dev".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("pnpm"),
                args: vec![
                    "exec".to_string(),
                    "vite".to_string(),
                    "--host=0.0.0.0".to_string(),
                    "--port".to_string(),
                    "5173".to_string(),
                ],
                cwd: PathBuf::from("/tmp/sofvary/app_1"),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(1_000),
                kill_on_drop: true,
            },
        });
        assert_eq!(zero_bind.decision, PolicyDecisionKind::Forbidden);

        let lan_bind = engine.evaluate_command(PolicyCommandRequest {
            name: "dev".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("pnpm"),
                args: vec![
                    "exec".to_string(),
                    "vite".to_string(),
                    "--host".to_string(),
                    "192.168.1.10".to_string(),
                    "--port".to_string(),
                    "5173".to_string(),
                ],
                cwd: PathBuf::from("/tmp/sofvary/app_1"),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(1_000),
                kill_on_drop: true,
            },
        });
        assert_eq!(lan_bind.decision, PolicyDecisionKind::Forbidden);
    }

    #[test]
    fn policy_blocks_runtime_start_on_non_loopback_binds() {
        let engine = PolicyEngine::new();

        for bind in ["0.0.0.0", "localhost", "::1", "192.168.1.10"] {
            let decision = engine.evaluate_runtime_start(PolicyRuntimeStartRequest {
                workspace_root: PathBuf::from("/tmp/sofvary/app_1"),
                runtime_kind: "react-vite".to_string(),
                bind: bind.to_string(),
                network: "local-only".to_string(),
            });

            assert_eq!(decision.decision, PolicyDecisionKind::Forbidden);
        }

        let local = engine.evaluate_runtime_start(PolicyRuntimeStartRequest {
            workspace_root: PathBuf::from("/tmp/sofvary/app_1"),
            runtime_kind: "react-vite".to_string(),
            bind: "127.0.0.1".to_string(),
            network: "local-only".to_string(),
        });
        assert_eq!(local.decision, PolicyDecisionKind::Allowed);
    }

    #[test]
    fn policy_requires_confirmation_for_dependency_install() {
        let engine = PolicyEngine::new();

        let decision = engine.evaluate_dependency_install(PolicyCommandRequest {
            name: "install".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("pnpm"),
                args: vec!["install".to_string(), "--offline".to_string()],
                cwd: PathBuf::from("/tmp/sofvary/app_1/generated/react"),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(60_000),
                kill_on_drop: true,
            },
        });

        assert_eq!(decision.decision, PolicyDecisionKind::RequiresConfirmation);
    }

    #[test]
    fn dependency_install_subject_ignores_sidecar_absolute_path() {
        let engine = PolicyEngine::new();
        let preview = engine.evaluate_dependency_install(PolicyCommandRequest {
            name: "install".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("pnpm"),
                args: vec![
                    "install".to_string(),
                    "--offline".to_string(),
                    "--ignore-scripts".to_string(),
                ],
                cwd: PathBuf::from("generated/react"),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(60_000),
                kill_on_drop: true,
            },
        });
        let runtime = engine.evaluate_dependency_install(PolicyCommandRequest {
            name: "install".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("C:\\Sofvary\\sidecars\\windows-x64\\pnpm.cmd"),
                args: vec![
                    "install".to_string(),
                    "--offline".to_string(),
                    "--ignore-scripts".to_string(),
                ],
                cwd: PathBuf::from("C:\\Sofvary\\workspaces\\app\\generated\\react"),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(60_000),
                kill_on_drop: true,
            },
        });

        assert_eq!(preview.subject, runtime.subject);
        assert_eq!(
            runtime.subject.as_deref(),
            Some("pnpm install --offline --ignore-scripts")
        );
    }

    #[test]
    fn dependency_recovery_install_subject_includes_network_recovery_args() {
        let engine = PolicyEngine::new();
        let decision = engine.evaluate_dependency_install(PolicyCommandRequest {
            name: "install".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("C:\\Sofvary\\sidecars\\windows-x64\\pnpm.cmd"),
                args: vec![
                    "install".to_string(),
                    "--ignore-scripts".to_string(),
                    "--prefer-offline".to_string(),
                ],
                cwd: PathBuf::from("C:\\Sofvary\\workspaces\\app\\generated\\react"),
                env: HashMap::new(),
                allowed_network: true,
                timeout_ms: Some(120_000),
                kill_on_drop: true,
            },
        });

        assert_eq!(decision.decision, PolicyDecisionKind::RequiresConfirmation);
        assert_eq!(
            decision.subject.as_deref(),
            Some("pnpm install --ignore-scripts --prefer-offline")
        );
        assert!(decision
            .risks
            .iter()
            .any(|risk| risk.contains("Package registry network access")));
    }

    #[test]
    fn policy_requires_confirmation_for_external_agent_process() {
        let engine = PolicyEngine::new();

        let decision = engine.evaluate_external_agent_process(PolicyExternalAgentProcessRequest {
            agent_id: "codex".to_string(),
            provider: "codex".to_string(),
            transport: "acp".to_string(),
            executable: "/usr/local/bin/codex-acp".to_string(),
        });

        assert_eq!(decision.action, PolicyActionKind::ExternalAgentProcess);
        assert_eq!(decision.decision, PolicyDecisionKind::RequiresConfirmation);
        assert_eq!(
            decision.subject.as_deref(),
            Some("codex:acp:/usr/local/bin/codex-acp")
        );
    }

    #[test]
    fn policy_requires_confirmation_for_runtime_environment_install() {
        let engine = PolicyEngine::new();

        let decision =
            engine.evaluate_runtime_environment_install(PolicyRuntimeEnvironmentInstallRequest {
                kind: "nodejs".to_string(),
                version: "24.16.0".to_string(),
                platform: "windows-x64".to_string(),
                sha256: "abc123".to_string(),
                subject: "runtime-env:nodejs:24.16.0:windows-x64:abc123".to_string(),
            });

        assert_eq!(decision.action, PolicyActionKind::RuntimeEnvironmentInstall);
        assert_eq!(decision.decision, PolicyDecisionKind::RequiresConfirmation);
        assert_eq!(
            decision.subject.as_deref(),
            Some("runtime-env:nodejs:24.16.0:windows-x64:abc123")
        );
    }

    #[test]
    fn policy_blocks_ai_provider_key_store_without_secure_store() {
        let engine = PolicyEngine::new();

        let decision = engine.evaluate_ai_provider_key_store(PolicyAiProviderKeyStoreRequest {
            app_id: "app_alpha".to_string(),
            profile_id: "openai".to_string(),
            provider_kind: "openai".to_string(),
            secure_store_available: false,
        });

        assert_eq!(decision.action, PolicyActionKind::AiProviderKeyStore);
        assert_eq!(decision.decision, PolicyDecisionKind::Forbidden);
    }

    #[test]
    fn policy_requires_confirmation_for_ai_provider_rebind() {
        let engine = PolicyEngine::new();

        let decision = engine.evaluate_ai_provider_rebind(PolicyAiProviderRebindRequest {
            app_id: "app_alpha".to_string(),
            requirement_id: "openai-text-image-video".to_string(),
            profile_id: "primary".to_string(),
        });

        assert_eq!(decision.action, PolicyActionKind::AiProviderRebind);
        assert_eq!(decision.decision, PolicyDecisionKind::RequiresConfirmation);
        assert_eq!(
            decision.subject.as_deref(),
            Some("app_alpha:openai-text-image-video->primary")
        );
    }

    #[test]
    fn policy_blocks_ai_provider_call_without_binding_secret_or_loopback() {
        let engine = PolicyEngine::new();

        let base = PolicyAiProviderCallRequest {
            app_id: "app_alpha".to_string(),
            profile_id: "primary".to_string(),
            provider_kind: "openai".to_string(),
            capability: "text".to_string(),
            gateway_bind: "127.0.0.1".to_string(),
            provider_binding_ready: true,
            requires_secret: true,
            secure_store_available: true,
            secure_key_available: true,
        };

        let no_binding = engine.evaluate_ai_provider_call(PolicyAiProviderCallRequest {
            provider_binding_ready: false,
            ..base.clone()
        });
        assert_eq!(no_binding.decision, PolicyDecisionKind::Forbidden);

        let no_store = engine.evaluate_ai_provider_call(PolicyAiProviderCallRequest {
            secure_store_available: false,
            ..base.clone()
        });
        assert_eq!(no_store.decision, PolicyDecisionKind::Forbidden);

        let no_key = engine.evaluate_ai_provider_call(PolicyAiProviderCallRequest {
            secure_key_available: false,
            ..base.clone()
        });
        assert_eq!(no_key.decision, PolicyDecisionKind::Forbidden);

        let public_bind = engine.evaluate_ai_provider_call(PolicyAiProviderCallRequest {
            gateway_bind: "localhost".to_string(),
            ..base
        });
        assert_eq!(public_bind.decision, PolicyDecisionKind::Forbidden);
    }

    #[test]
    fn policy_requires_confirmation_for_ready_ai_provider_call() {
        let engine = PolicyEngine::new();

        let decision = engine.evaluate_ai_provider_call(PolicyAiProviderCallRequest {
            app_id: "app_alpha".to_string(),
            profile_id: "primary".to_string(),
            provider_kind: "openai".to_string(),
            capability: "image".to_string(),
            gateway_bind: "127.0.0.1".to_string(),
            provider_binding_ready: true,
            requires_secret: true,
            secure_store_available: true,
            secure_key_available: true,
        });

        assert_eq!(decision.action, PolicyActionKind::AiProviderCall);
        assert_eq!(decision.decision, PolicyDecisionKind::RequiresConfirmation);
    }

    #[test]
    fn policy_allows_windows_cmd_sidecar_runtime_command() {
        let engine = PolicyEngine::new();

        let decision = engine.evaluate_command(PolicyCommandRequest {
            name: "dev".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("C:\\Sofvary\\sidecars\\windows-x64\\pnpm.cmd"),
                args: vec![
                    "exec".to_string(),
                    "vite".to_string(),
                    "--host".to_string(),
                    "127.0.0.1".to_string(),
                    "--port".to_string(),
                    "5173".to_string(),
                ],
                cwd: PathBuf::from("C:\\Sofvary\\workspace\\generated\\react"),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(1_000),
                kill_on_drop: true,
            },
        });

        assert_eq!(decision.decision, PolicyDecisionKind::Allowed);
    }

    #[test]
    fn policy_blocks_platform_protected_delete_targets() {
        let engine = PolicyEngine::new();

        let decision = engine.evaluate_command(PolicyCommandRequest {
            name: "delete".to_string(),
            command: CommandSpec {
                executable: PathBuf::from("rmdir.exe"),
                args: vec!["C:/Windows/System32".to_string()],
                cwd: PathBuf::from("C:\\Sofvary\\workspace"),
                env: HashMap::new(),
                allowed_network: false,
                timeout_ms: Some(1_000),
                kill_on_drop: true,
            },
        });

        assert_eq!(decision.decision, PolicyDecisionKind::Forbidden);
    }

    #[test]
    fn policy_allows_static_generated_write() {
        let engine = PolicyEngine::new();
        let workspace_root = PathBuf::from("/tmp/sofvary/app_1");

        let decision = engine.evaluate_file_write(PolicyFileWriteRequest {
            workspace_root: workspace_root.clone(),
            target_path: workspace_root.join("generated/static/index.html"),
        });

        assert_eq!(decision.decision, PolicyDecisionKind::Allowed);
    }
}
