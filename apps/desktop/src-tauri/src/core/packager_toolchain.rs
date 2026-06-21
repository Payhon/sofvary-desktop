use crate::core::policy_types::PolicyApprovalSet;
use crate::core::runtime_environment::{
    list_runtime_environment_catalog_with_adapter, start_runtime_environment_install_with_adapter,
    RuntimeEnvironmentError, RuntimeEnvironmentKind, StartRuntimeEnvironmentInstallPayload,
};
use crate::platform::{current_adapter, CommandSpec, OsKind, PlatformAdapter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackagerToolchainError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("platform error: {0}")]
    Platform(#[from] crate::platform::PlatformError),
    #[error("runtime environment error: {0}")]
    RuntimeEnvironment(#[from] RuntimeEnvironmentError),
    #[error("invalid packager toolchain request: {0}")]
    Invalid(String),
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
    #[serde(default)]
    pub policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Clone)]
struct ToolProbe {
    status: PackagerToolchainRequirementStatus,
    executable: Option<PathBuf>,
}

pub fn get_packager_toolchain_status() -> PackagerToolchainResult<PackagerToolchainStatus> {
    let adapter = current_adapter();
    get_packager_toolchain_status_with_adapter(adapter.as_ref())
}

pub fn get_packager_toolchain_status_with_adapter(
    adapter: &dyn PlatformAdapter,
) -> PackagerToolchainResult<PackagerToolchainStatus> {
    let platform = target_platform_for_os(adapter.os()).to_string();
    let probe_cwd = adapter.dirs()?.data_dir;
    let host_template_dir = published_host_template_dir();
    let node = managed_or_external_tool_status(
        adapter,
        PackagerToolchainRequirementKind::Node,
        "Node.js",
        "node",
        &["--version"],
        true,
        true,
        &probe_cwd,
    );
    let pnpm = managed_or_external_tool_status(
        adapter,
        PackagerToolchainRequirementKind::Pnpm,
        "pnpm",
        "pnpm",
        &["--version"],
        true,
        true,
        &probe_cwd,
    );
    let rustc = path_tool_status(
        adapter,
        PackagerToolchainRequirementKind::Rustc,
        "Rust compiler",
        "rustc",
        &["--version"],
        true,
        false,
        &probe_cwd,
    );
    let cargo = path_tool_status(
        adapter,
        PackagerToolchainRequirementKind::Cargo,
        "Cargo",
        "cargo",
        &["--version"],
        true,
        false,
        &probe_cwd,
    );
    let tauri_cli = match pnpm.executable.clone() {
        Some(pnpm_executable) => tool_status_for_executable(
            adapter,
            PackagerToolchainRequirementKind::TauriCli,
            "Tauri CLI",
            pnpm_executable,
            "pnpm",
            &["tauri", "--version"],
            true,
            false,
            &host_template_dir,
        ),
        None => ToolProbe {
            status: PackagerToolchainRequirementStatus {
                kind: PackagerToolchainRequirementKind::TauriCli,
                label: "Tauri CLI".to_string(),
                installed: false,
                required: true,
                installable: false,
                version: None,
                detail: "Install Node/pnpm first; Tauri CLI will be re-checked from the published host template.".to_string(),
            },
            executable: None,
        },
    };
    let requirements = vec![
        node.status,
        pnpm.status,
        rustc.status,
        cargo.status,
        tauri_cli.status,
    ];
    let ready = requirements
        .iter()
        .filter(|requirement| requirement.required)
        .all(|requirement| requirement.installed);
    let install_action_available = missing_installable_node_or_pnpm(&requirements);
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
    start_packager_toolchain_install_with_adapter(payload, adapter.as_ref())
}

pub fn start_packager_toolchain_install_with_adapter(
    payload: StartPackagerToolchainInstallPayload,
    adapter: &dyn PlatformAdapter,
) -> PackagerToolchainResult<PackagerToolchainStatus> {
    let current_platform = target_platform_for_os(adapter.os());
    if payload.target_platform != current_platform {
        return Err(PackagerToolchainError::Invalid(format!(
            "packager toolchain install only supports the current platform: {current_platform}"
        )));
    }

    let status = get_packager_toolchain_status_with_adapter(adapter)?;
    if !missing_installable_node_or_pnpm(&status.requirements) {
        return Ok(status);
    }

    let version = default_managed_nodejs_version(adapter)?;
    start_runtime_environment_install_with_adapter(
        StartRuntimeEnvironmentInstallPayload {
            kind: RuntimeEnvironmentKind::Nodejs,
            version,
            policy_approvals: payload.policy_approvals,
        },
        adapter,
    )?;

    get_packager_toolchain_status_with_adapter(adapter)
}

pub fn target_platform_for_os(os: OsKind) -> &'static str {
    match os {
        OsKind::Windows => "windows",
        OsKind::Macos => "macos",
        OsKind::Linux => "linux",
    }
}

fn managed_or_external_tool_status(
    adapter: &dyn PlatformAdapter,
    kind: PackagerToolchainRequirementKind,
    label: &str,
    name: &str,
    args: &[&str],
    required: bool,
    installable: bool,
    cwd: &Path,
) -> ToolProbe {
    if let Ok(executable) = adapter.resolve_sidecar_executable(name) {
        let probe = tool_status_for_executable(
            adapter,
            kind.clone(),
            label,
            executable,
            "Sofvary managed",
            args,
            required,
            installable,
            cwd,
        );
        if probe.status.installed {
            return probe;
        }
    }

    path_tool_status(adapter, kind, label, name, args, required, installable, cwd)
}

fn path_tool_status(
    adapter: &dyn PlatformAdapter,
    kind: PackagerToolchainRequirementKind,
    label: &str,
    command: &str,
    args: &[&str],
    required: bool,
    installable: bool,
    cwd: &Path,
) -> ToolProbe {
    tool_status_for_executable(
        adapter,
        kind,
        label,
        PathBuf::from(command),
        "external PATH",
        args,
        required,
        installable,
        cwd,
    )
}

fn tool_status_for_executable(
    adapter: &dyn PlatformAdapter,
    kind: PackagerToolchainRequirementKind,
    label: &str,
    executable: PathBuf,
    source: &str,
    args: &[&str],
    required: bool,
    installable: bool,
    cwd: &Path,
) -> ToolProbe {
    let output = adapter.run_process(CommandSpec {
        executable: executable.clone(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        cwd: cwd.to_path_buf(),
        env: HashMap::new(),
        allowed_network: false,
        timeout_ms: Some(10_000),
        kill_on_drop: true,
    });

    match output {
        Ok(output) if output.status_code == Some(0) => ToolProbe {
            status: PackagerToolchainRequirementStatus {
                kind,
                label: label.to_string(),
                installed: true,
                required,
                installable,
                version: Some(first_output_line(&output.stdout, &output.stderr)),
                detail: format!("{label} is available from {source}."),
            },
            executable: Some(executable),
        },
        Ok(output) => ToolProbe {
            status: PackagerToolchainRequirementStatus {
                kind,
                label: label.to_string(),
                installed: false,
                required,
                installable,
                version: None,
                detail: summarize_command_output(&output.stderr, &output.stdout),
            },
            executable: None,
        },
        Err(error) => ToolProbe {
            status: PackagerToolchainRequirementStatus {
                kind,
                label: label.to_string(),
                installed: false,
                required,
                installable,
                version: None,
                detail: error.to_string(),
            },
            executable: None,
        },
    }
}

fn missing_installable_node_or_pnpm(requirements: &[PackagerToolchainRequirementStatus]) -> bool {
    requirements.iter().any(|requirement| {
        matches!(
            requirement.kind,
            PackagerToolchainRequirementKind::Node | PackagerToolchainRequirementKind::Pnpm
        ) && requirement.required
            && !requirement.installed
            && requirement.installable
    })
}

fn default_managed_nodejs_version(
    adapter: &dyn PlatformAdapter,
) -> PackagerToolchainResult<String> {
    let catalog = list_runtime_environment_catalog_with_adapter(adapter)
        .into_iter()
        .find(|item| item.kind == RuntimeEnvironmentKind::Nodejs)
        .ok_or_else(|| {
            PackagerToolchainError::Invalid(
                "Node.js runtime environment catalog is not available.".to_string(),
            )
        })?;
    let version = catalog
        .versions
        .iter()
        .find(|version| version.recommended && version.supported)
        .or_else(|| catalog.versions.iter().find(|version| version.supported))
        .ok_or_else(|| {
            PackagerToolchainError::Invalid(
                "Sofvary-managed Node/pnpm is not available for this platform yet.".to_string(),
            )
        })?;
    Ok(version.version.clone())
}

fn published_host_template_dir() -> PathBuf {
    if let Ok(path) = std::env::var("SOFVARY_PUBLISHED_HOST_TEMPLATE_DIR") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("published-host")
}

fn first_output_line(stdout: &str, stderr: &str) -> String {
    let source = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn summarize_command_output(stderr: &str, stdout: &str) -> String {
    let first_line = first_output_line(stdout, stderr);
    first_line.chars().take(360).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{
        ArchKind, PlatformDirs, PlatformError, PlatformResult, ProcessHandle, ProcessOutput,
        WebviewProfile, WorkArea,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn maps_os_to_release_platform() {
        assert_eq!(target_platform_for_os(OsKind::Windows), "windows");
        assert_eq!(target_platform_for_os(OsKind::Macos), "macos");
        assert_eq!(target_platform_for_os(OsKind::Linux), "linux");
    }

    #[test]
    fn command_status_reports_missing_command_without_panic() {
        let adapter = TestAdapter::new(OsKind::Windows, ArchKind::X64);
        let status = path_tool_status(
            &adapter,
            PackagerToolchainRequirementKind::TauriCli,
            "Missing Tool",
            "definitely-not-a-sofvary-tool",
            &["--version"],
            true,
            false,
            &adapter.dirs.data_dir,
        );
        assert!(!status.status.installed);
        assert!(status.status.required);
    }

    #[test]
    fn managed_node_and_pnpm_are_used_for_packager_detection() {
        let adapter = TestAdapter::new(OsKind::Windows, ArchKind::X64).with_managed_sidecars();
        let status = get_packager_toolchain_status_with_adapter(&adapter).expect("status");

        assert!(status.ready);
        assert!(!status.install_action_available);
        let calls = adapter.calls.lock().expect("calls");
        assert!(calls.iter().any(|call| call
            .executable
            .ends_with("sidecars/windows-x64/pnpm.cmd")
            && call.args == ["tauri", "--version"]));
    }

    #[test]
    fn install_action_only_covers_managed_node_and_pnpm() {
        let adapter = TestAdapter::new(OsKind::Windows, ArchKind::X64)
            .with_external_node_pnpm()
            .without_tauri_cli();
        let status = get_packager_toolchain_status_with_adapter(&adapter).expect("status");

        assert!(!status.ready);
        assert!(!status.install_action_available);
        let tauri = status
            .requirements
            .iter()
            .find(|requirement| requirement.kind == PackagerToolchainRequirementKind::TauriCli)
            .expect("tauri status");
        assert!(!tauri.installable);
    }

    #[derive(Clone)]
    struct ProcessCall {
        executable: PathBuf,
        args: Vec<String>,
    }

    #[derive(Clone)]
    struct TestAdapter {
        os: OsKind,
        arch: ArchKind,
        dirs: PlatformDirs,
        managed: bool,
        external_node_pnpm: bool,
        tauri_cli: bool,
        calls: Arc<Mutex<Vec<ProcessCall>>>,
    }

    impl TestAdapter {
        fn new(os: OsKind, arch: ArchKind) -> Self {
            let temp = tempfile::tempdir().expect("tempdir");
            let root = temp.path().to_path_buf();
            Self {
                os,
                arch,
                dirs: PlatformDirs {
                    data_dir: root.join("data"),
                    cache_dir: root.join("cache"),
                    config_dir: root.join("config"),
                },
                managed: false,
                external_node_pnpm: false,
                tauri_cli: true,
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_managed_sidecars(mut self) -> Self {
            self.managed = true;
            self
        }

        fn with_external_node_pnpm(mut self) -> Self {
            self.external_node_pnpm = true;
            self
        }

        fn without_tauri_cli(mut self) -> Self {
            self.tauri_cli = false;
            self
        }

        fn platform_slug(&self) -> &'static str {
            match (self.os, self.arch) {
                (OsKind::Windows, ArchKind::X64) => "windows-x64",
                (OsKind::Windows, ArchKind::Arm64) => "windows-arm64",
                (OsKind::Macos, ArchKind::X64) => "macos-x64",
                (OsKind::Macos, ArchKind::Arm64) => "macos-arm64",
                (OsKind::Linux, ArchKind::X64) => "linux-x64",
                (OsKind::Linux, ArchKind::Arm64) => "linux-arm64",
                _ => "unknown-unknown",
            }
        }
    }

    impl PlatformAdapter for TestAdapter {
        fn os(&self) -> OsKind {
            self.os
        }

        fn arch(&self) -> ArchKind {
            self.arch
        }

        fn dirs(&self) -> PlatformResult<PlatformDirs> {
            Ok(self.dirs.clone())
        }

        fn normalize_path(&self, input: &str) -> PlatformResult<PathBuf> {
            Ok(PathBuf::from(input))
        }

        fn ensure_executable(&self, _path: &Path) -> PlatformResult<()> {
            Ok(())
        }

        fn resolve_sidecar_executable(&self, name: &str) -> PlatformResult<PathBuf> {
            if !self.managed {
                return Err(PlatformError::InvalidPath(format!("missing {name}")));
            }
            let file = match (name, self.os) {
                ("node", OsKind::Windows) => "node.exe",
                ("pnpm", OsKind::Windows) => "pnpm.cmd",
                ("node", _) => "node",
                ("pnpm", _) => "pnpm",
                _ => return Err(PlatformError::Unsupported(name.to_string())),
            };
            Ok(self
                .dirs
                .data_dir
                .join("sidecars")
                .join(self.platform_slug())
                .join(file))
        }

        fn run_process(&self, spec: CommandSpec) -> PlatformResult<ProcessOutput> {
            self.calls.lock().expect("calls").push(ProcessCall {
                executable: spec.executable.clone(),
                args: spec.args.clone(),
            });
            let executable = spec.executable.display().to_string();
            let executable_name = spec
                .executable
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let ok = if executable.contains("sidecars") {
                executable_name.contains("node") || executable_name.contains("pnpm")
            } else if matches!(executable_name, "node" | "pnpm") {
                self.external_node_pnpm
            } else if matches!(executable_name, "rustc" | "cargo") {
                true
            } else {
                false
            };
            if !ok {
                return Err(PlatformError::InvalidPath(format!(
                    "program not found: {executable}"
                )));
            }
            if spec.args == ["tauri", "--version"] && !self.tauri_cli {
                return Ok(ProcessOutput {
                    status_code: Some(1),
                    stdout: String::new(),
                    stderr: "Tauri CLI not found".to_string(),
                });
            }
            let stdout = if executable_name.contains("node") {
                "v24.16.0\n"
            } else if executable_name.contains("pnpm") {
                if spec.args == ["tauri", "--version"] {
                    "tauri-cli 2.9.5\n"
                } else {
                    "10.12.3\n"
                }
            } else if executable_name == "rustc" {
                "rustc 1.95.0\n"
            } else {
                "cargo 1.95.0\n"
            };
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: stdout.to_string(),
                stderr: String::new(),
            })
        }

        fn spawn_process(&self, _spec: CommandSpec) -> PlatformResult<ProcessHandle> {
            Err(PlatformError::Unsupported("spawn".to_string()))
        }

        fn kill_process_tree(&self, _pid: u32) -> PlatformResult<()> {
            Ok(())
        }

        fn allocate_local_port(&self) -> PlatformResult<u16> {
            Ok(3000)
        }

        fn open_external(&self, _url: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn reveal_path(&self, _path: &Path) -> PlatformResult<()> {
            Ok(())
        }

        fn register_protocol_handler(&self, _protocol: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn register_global_shortcut(&self, _accelerator: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn unregister_global_shortcut(&self, _accelerator: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn show_tray_or_menu_bar_item(&self) -> PlatformResult<()> {
            Ok(())
        }

        fn get_active_monitor_work_area(&self) -> PlatformResult<WorkArea> {
            Ok(WorkArea {
                x: 0,
                y: 0,
                width: 1200,
                height: 760,
            })
        }

        fn secure_store_set(&self, _key: &str, _value: &str) -> PlatformResult<()> {
            Ok(())
        }

        fn secure_store_get(&self, _key: &str) -> PlatformResult<Option<String>> {
            Ok(None)
        }

        fn current_webview_profile(&self) -> WebviewProfile {
            WebviewProfile {
                engine: "test".to_string(),
                supports_transparency: true,
                notes: Vec::new(),
            }
        }
    }
}
