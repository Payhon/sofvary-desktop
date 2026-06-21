use crate::platform::{command_executable_name, CommandSpec};
use std::collections::HashMap;
use std::path::PathBuf;

pub const DEPENDENCY_INSTALL_PRIMARY_ARGS: &[&str] = &["install", "--offline", "--ignore-scripts"];
pub const DEPENDENCY_INSTALL_RECOVERY_ARGS: &[&str] =
    &["install", "--ignore-scripts", "--prefer-offline"];

const RECOVERY_INSTALL_TIMEOUT_MS: u64 = 120_000;

pub fn dependency_install_recovery_spec(spec: &CommandSpec) -> Option<CommandSpec> {
    let executable = command_executable_name(&spec.executable).unwrap_or_default();
    if executable != "pnpm" || !is_offline_pnpm_install_spec(spec) {
        return None;
    }

    let mut args = spec
        .args
        .iter()
        .filter(|arg| arg.as_str() != "--offline")
        .cloned()
        .collect::<Vec<_>>();
    let prefer_offline = DEPENDENCY_INSTALL_RECOVERY_ARGS
        .last()
        .copied()
        .unwrap_or("--prefer-offline");
    if !args.iter().any(|arg| arg == prefer_offline) {
        args.push(prefer_offline.to_string());
    }

    Some(CommandSpec {
        executable: spec.executable.clone(),
        args,
        cwd: spec.cwd.clone(),
        env: spec.env.clone(),
        allowed_network: true,
        timeout_ms: Some(
            spec.timeout_ms
                .unwrap_or(RECOVERY_INSTALL_TIMEOUT_MS)
                .max(RECOVERY_INSTALL_TIMEOUT_MS),
        ),
        kill_on_drop: spec.kill_on_drop,
    })
}

pub fn dependency_install_policy_preview_specs() -> Vec<CommandSpec> {
    let primary = CommandSpec {
        executable: PathBuf::from("pnpm"),
        args: DEPENDENCY_INSTALL_PRIMARY_ARGS
            .iter()
            .map(|arg| arg.to_string())
            .collect(),
        cwd: PathBuf::from("generated/react"),
        env: HashMap::new(),
        allowed_network: false,
        timeout_ms: Some(60_000),
        kill_on_drop: true,
    };
    let recovery = dependency_install_recovery_spec(&primary)
        .expect("primary dependency install spec should have a recovery form");
    vec![primary, recovery]
}

pub fn is_offline_dependency_cache_failure(stdout: &str, stderr: &str) -> bool {
    let combined = diagnostic_text(stdout, stderr);
    contains_any(
        &combined,
        &[
            "ERR_PNPM_NO_OFFLINE_TARBALL",
            "ERR_PNPM_NO_OFFLINE_META",
            "ERR_PNPM_OFFLINE",
            "cannot download it in offline mode",
            "missing from the store",
            "is missing from the store",
            "offline mode",
        ],
    )
}

pub fn is_dependency_version_resolution_failure(stdout: &str, stderr: &str) -> bool {
    let combined = diagnostic_text(stdout, stderr);
    contains_any(
        &combined,
        &[
            "ERR_PNPM_NO_MATCHING_VERSION",
            "No matching version found",
            "No matching version found for",
            "not found in the npm registry",
        ],
    )
}

pub fn is_dependency_network_failure(stdout: &str, stderr: &str) -> bool {
    let combined = diagnostic_text(stdout, stderr);
    contains_any(
        &combined,
        &[
            "ERR_PNPM_META_FETCH_FAIL",
            "ECONNRESET",
            "ENOTFOUND",
            "ETIMEDOUT",
            "EAI_AGAIN",
            "network timeout",
            "fetch failed",
            "getaddrinfo",
        ],
    )
}

fn is_offline_pnpm_install_spec(spec: &CommandSpec) -> bool {
    spec.args.iter().any(|arg| arg == "install") && spec.args.iter().any(|arg| arg == "--offline")
}

fn diagnostic_text(stdout: &str, stderr: &str) -> String {
    format!("{stdout}\n{stderr}")
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let value = value.to_ascii_lowercase();
    needles
        .iter()
        .any(|needle| value.contains(&needle.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_spec_removes_offline_and_requests_policy_network() {
        let spec = CommandSpec {
            executable: PathBuf::from("pnpm"),
            args: DEPENDENCY_INSTALL_PRIMARY_ARGS
                .iter()
                .map(|arg| arg.to_string())
                .collect(),
            cwd: PathBuf::from("generated/react"),
            env: HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(60_000),
            kill_on_drop: true,
        };

        let recovery = dependency_install_recovery_spec(&spec).expect("recovery spec");

        assert_eq!(
            recovery.args,
            ["install", "--ignore-scripts", "--prefer-offline"]
        );
        assert!(recovery.allowed_network);
        assert_eq!(recovery.timeout_ms, Some(120_000));
    }

    #[test]
    fn classifies_offline_cache_failure_without_confusing_version_resolution() {
        let stderr = "ERR_PNPM_NO_OFFLINE_META Failed to resolve lightningcss@>=1.32.0 <2.0.0-0 in package mirror";

        assert!(is_offline_dependency_cache_failure("", stderr));
        assert!(!is_dependency_version_resolution_failure("", stderr));
    }

    #[test]
    fn classifies_dependency_version_resolution_as_generated_contract_issue() {
        let stderr =
            "ERR_PNPM_NO_MATCHING_VERSION No matching version found for undici-types@~7.18.0";

        assert!(is_dependency_version_resolution_failure("", stderr));
        assert!(!is_offline_dependency_cache_failure("", stderr));
    }

    #[test]
    fn classifies_dependency_network_failure_as_environment_issue() {
        let stderr = "ERR_PNPM_META_FETCH_FAIL GET https://registry.example.test/react: EAI_AGAIN";

        assert!(is_dependency_network_failure("", stderr));
        assert!(!is_dependency_version_resolution_failure("", stderr));
    }
}
