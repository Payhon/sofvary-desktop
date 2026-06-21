use crate::platform::{current_adapter, OsKind, PlatformAdapter};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackagerToolchainError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("platform error: {0}")]
    Platform(#[from] crate::platform::PlatformError),
}

pub type PackagerToolchainResult<T> = Result<T, PackagerToolchainError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PackagerToolchainRequirementKind {
    Node,
    Pnpm,
    Rustc,
    Cargo,
    TauriCli,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackagerToolchainRequirementStatus {
    pub kind: PackagerToolchainRequirementKind,
    pub label: String,
    pub installed: bool,
    pub required: bool,
    pub installable: bool,
    pub version: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackagerToolchainStatus {
    pub platform: String,
    pub ready: bool,
    pub beta: bool,
    pub install_action_available: bool,
    pub requirements: Vec<PackagerToolchainRequirementStatus>,
    pub detail: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPackagerToolchainInstallPayload {
    pub target_platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PackagerToolchainInstallIntent {
    target_platform: String,
    created_at: String,
    note: String,
}

pub fn get_packager_toolchain_status() -> PackagerToolchainResult<PackagerToolchainStatus> {
    let adapter = current_adapter();
    get_packager_toolchain_status_with_adapter(adapter.as_ref())
}

pub fn get_packager_toolchain_status_with_adapter(
    adapter: &dyn PlatformAdapter,
) -> PackagerToolchainResult<PackagerToolchainStatus> {
    let platform = target_platform_for_os(adapter.os()).to_string();
    let requirements = vec![
        command_status(
            PackagerToolchainRequirementKind::Node,
            "Node.js",
            "node",
            &["--version"],
            true,
            true,
        ),
        command_status(
            PackagerToolchainRequirementKind::Pnpm,
            "pnpm",
            "pnpm",
            &["--version"],
            true,
            true,
        ),
        command_status(
            PackagerToolchainRequirementKind::Rustc,
            "Rust compiler",
            "rustc",
            &["--version"],
            true,
            false,
        ),
        command_status(
            PackagerToolchainRequirementKind::Cargo,
            "Cargo",
            "cargo",
            &["--version"],
            true,
            false,
        ),
        command_status(
            PackagerToolchainRequirementKind::TauriCli,
            "Tauri CLI",
            "pnpm",
            &["exec", "tauri", "--version"],
            true,
            true,
        ),
    ];
    let ready = requirements
        .iter()
        .filter(|requirement| requirement.required)
        .all(|requirement| requirement.installed);
    let install_action_available = requirements.iter().any(|requirement| {
        requirement.required && !requirement.installed && requirement.installable
    });
    let detail = if ready {
        "Current platform packager toolchain is ready for unsigned beta packaging.".to_string()
    } else if install_action_available {
        "Install Sofvary-managed Node/pnpm support, then re-check Rust and Tauri CLI availability."
            .to_string()
    } else {
        "Install missing Rust/Cargo/Tauri prerequisites before creating installer artifacts."
            .to_string()
    };

    Ok(PackagerToolchainStatus {
        platform,
        ready,
        beta: true,
        install_action_available,
        requirements,
        detail,
    })
}

pub fn start_packager_toolchain_install(
    payload: StartPackagerToolchainInstallPayload,
) -> PackagerToolchainResult<PackagerToolchainStatus> {
    let adapter = current_adapter();
    let dirs = adapter.dirs()?;
    let root = dirs.data_dir.join("packager-toolchain");
    fs::create_dir_all(&root)?;
    let intent = PackagerToolchainInstallIntent {
        target_platform: payload.target_platform,
        created_at: Utc::now().to_rfc3339(),
        note: "Sofvary packager setup is local-only in this beta. Use Runtime Environment install for managed Node/pnpm; Rust/Cargo remain explicit prerequisites."
            .to_string(),
    };
    fs::write(
        root.join("install-intent.json"),
        serde_json::to_string_pretty(&intent)? + "\n",
    )?;

    get_packager_toolchain_status_with_adapter(adapter.as_ref())
}

pub fn target_platform_for_os(os: OsKind) -> &'static str {
    match os {
        OsKind::Windows => "windows",
        OsKind::Macos => "macos",
        OsKind::Linux => "linux",
    }
}

fn command_status(
    kind: PackagerToolchainRequirementKind,
    label: &str,
    command: &str,
    args: &[&str],
    required: bool,
    installable: bool,
) -> PackagerToolchainRequirementStatus {
    match Command::new(command).args(args).output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            PackagerToolchainRequirementStatus {
                kind,
                label: label.to_string(),
                installed: true,
                required,
                installable,
                detail: format!("{label} is available."),
                version,
            }
        }
        Ok(output) => PackagerToolchainRequirementStatus {
            kind,
            label: label.to_string(),
            installed: false,
            required,
            installable,
            version: None,
            detail: String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("command exited unsuccessfully")
                .to_string(),
        },
        Err(error) => PackagerToolchainRequirementStatus {
            kind,
            label: label.to_string(),
            installed: false,
            required,
            installable,
            version: None,
            detail: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_os_to_release_platform() {
        assert_eq!(target_platform_for_os(OsKind::Windows), "windows");
        assert_eq!(target_platform_for_os(OsKind::Macos), "macos");
        assert_eq!(target_platform_for_os(OsKind::Linux), "linux");
    }

    #[test]
    fn command_status_reports_missing_command_without_panic() {
        let status = command_status(
            PackagerToolchainRequirementKind::TauriCli,
            "Missing Tool",
            "definitely-not-a-sofvary-tool",
            &["--version"],
            true,
            false,
        );
        assert!(!status.installed);
        assert!(status.required);
    }
}
