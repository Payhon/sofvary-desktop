use crate::core::agent_config::{
    AgentCommandConfig, AgentConfig, AgentConfigResult, AgentInstallSource, AgentProvider,
};
use crate::platform::{current_adapter, ArchKind, OsKind, PlatformAdapter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEV_AGENT_ADAPTER_DIR_ENV: &str = "SOFVARY_DEV_AGENT_ADAPTER_DIR";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredAgent {
    pub config: AgentConfig,
    pub detected: bool,
    pub status: String,
}

pub fn discover_agents() -> AgentConfigResult<Vec<DiscoveredAgent>> {
    let adapter = current_adapter();
    discover_agents_with_adapter(adapter.as_ref())
}

pub fn discover_agents_with_adapter(
    adapter: &dyn PlatformAdapter,
) -> AgentConfigResult<Vec<DiscoveredAgent>> {
    let dirs = adapter.dirs()?;
    let controlled_dir = dirs.data_dir.join("agent-adapters").join(format!(
        "{}-{}",
        os_slug(adapter.os()),
        arch_slug(adapter.arch())
    ));
    let dev_dir = env::var_os(DEV_AGENT_ADAPTER_DIR_ENV).map(PathBuf::from);

    Ok(provider_templates()
        .into_iter()
        .map(|template| {
            discover_template(
                template,
                &controlled_dir,
                dev_dir.as_deref(),
                adapter.os(),
                adapter.arch(),
            )
        })
        .collect())
}

fn discover_template(
    template: ProviderTemplate,
    controlled_dir: &Path,
    dev_dir: Option<&Path>,
    os: OsKind,
    arch: ArchKind,
) -> DiscoveredAgent {
    let acp = template
        .acp
        .as_ref()
        .and_then(|command| resolve_command(command, controlled_dir, dev_dir, os));
    let cli = template
        .cli
        .as_ref()
        .and_then(|command| resolve_command(command, controlled_dir, dev_dir, os))
        .filter(|command| command_config_is_usable(template.provider, command, os, arch));
    let detected = acp.is_some() || cli.is_some();
    let status = match (&acp, &cli) {
        (Some(acp), _) => format!("ACP available via {}", acp.executable.display()),
        (None, Some(cli)) => format!("CLI fallback available via {}", cli.executable.display()),
        (None, None) => "Not found on this machine".to_string(),
    };

    DiscoveredAgent {
        config: AgentConfig {
            id: template.id.to_string(),
            provider: template.provider,
            label: template.label.to_string(),
            enabled: detected,
            acp,
            cli,
            allow_cli_fallback: template.allow_cli_fallback,
            last_test: None,
        },
        detected,
        status,
    }
}

fn resolve_command(
    template: &CommandTemplate,
    controlled_dir: &Path,
    dev_dir: Option<&Path>,
    os: OsKind,
) -> Option<AgentCommandConfig> {
    let candidates = template
        .executables
        .iter()
        .flat_map(|executable| executable_candidates(executable, os))
        .collect::<Vec<_>>();

    if let Some(dev_dir) = dev_dir {
        if let Some(path) = first_existing(dev_dir, &candidates) {
            return Some(command_config(
                path,
                template,
                AgentInstallSource::DevOverride,
            ));
        }
    }

    if let Some(path) = first_existing(controlled_dir, &candidates) {
        return Some(command_config(path, template, AgentInstallSource::Bundled));
    }

    find_on_path(&candidates)
        .map(|path| command_config(path, template, AgentInstallSource::ExternalPath))
}

fn command_config(
    executable: PathBuf,
    template: &CommandTemplate,
    source: AgentInstallSource,
) -> AgentCommandConfig {
    AgentCommandConfig {
        executable,
        args: template.args.iter().map(|arg| arg.to_string()).collect(),
        env: HashMap::new(),
        source,
    }
}

fn first_existing(dir: &Path, candidates: &[String]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|candidate| dir.join(candidate))
        .find(|candidate| candidate.is_file())
}

fn find_on_path(candidates: &[String]) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .flat_map(|dir| candidates.iter().map(move |candidate| dir.join(candidate)))
        .find(|candidate| candidate.is_file())
}

fn command_config_is_usable(
    provider: AgentProvider,
    command: &AgentCommandConfig,
    os: OsKind,
    arch: ArchKind,
) -> bool {
    if provider != AgentProvider::Codex {
        return true;
    }
    codex_cli_vendor_binary_exists(&command.executable, os, arch).unwrap_or(true)
}

fn codex_cli_vendor_binary_exists(executable: &Path, os: OsKind, arch: ArchKind) -> Option<bool> {
    let (platform_package, target_triple, binary_name) = codex_platform_artifact(os, arch)?;
    let script_path = fs::canonicalize(executable).ok()?;
    if script_path.file_name().and_then(|name| name.to_str()) != Some("codex.js") {
        return Some(true);
    }
    let package_root = script_path.parent()?.parent()?;
    if package_root.file_name().and_then(|name| name.to_str()) != Some("codex")
        || package_root
            .parent()?
            .file_name()
            .and_then(|name| name.to_str())
            != Some("@openai")
    {
        return Some(true);
    }
    let local_binary = package_root
        .join("vendor")
        .join(target_triple)
        .join("codex")
        .join(binary_name);
    let optional_binary = package_root
        .join("node_modules")
        .join("@openai")
        .join(platform_package)
        .join("vendor")
        .join(target_triple)
        .join("codex")
        .join(binary_name);
    Some(local_binary.is_file() || optional_binary.is_file())
}

fn codex_platform_artifact(
    os: OsKind,
    arch: ArchKind,
) -> Option<(&'static str, &'static str, &'static str)> {
    match (os, arch) {
        (OsKind::Macos, ArchKind::Arm64) => {
            Some(("codex-darwin-arm64", "aarch64-apple-darwin", "codex"))
        }
        (OsKind::Macos, ArchKind::X64) => {
            Some(("codex-darwin-x64", "x86_64-apple-darwin", "codex"))
        }
        (OsKind::Linux, ArchKind::Arm64) => {
            Some(("codex-linux-arm64", "aarch64-unknown-linux-musl", "codex"))
        }
        (OsKind::Linux, ArchKind::X64) => {
            Some(("codex-linux-x64", "x86_64-unknown-linux-musl", "codex"))
        }
        (OsKind::Windows, ArchKind::Arm64) => {
            Some(("codex-win32-arm64", "aarch64-pc-windows-msvc", "codex.exe"))
        }
        (OsKind::Windows, ArchKind::X64) => {
            Some(("codex-win32-x64", "x86_64-pc-windows-msvc", "codex.exe"))
        }
        _ => None,
    }
}

fn executable_candidates(executable: &str, os: OsKind) -> Vec<String> {
    if os != OsKind::Windows {
        return vec![executable.to_string()];
    }

    if executable.ends_with(".exe") || executable.ends_with(".cmd") {
        vec![executable.to_string()]
    } else {
        vec![
            format!("{executable}.exe"),
            format!("{executable}.cmd"),
            executable.to_string(),
        ]
    }
}

struct ProviderTemplate {
    id: &'static str,
    provider: AgentProvider,
    label: &'static str,
    acp: Option<CommandTemplate>,
    cli: Option<CommandTemplate>,
    allow_cli_fallback: bool,
}

struct CommandTemplate {
    executables: &'static [&'static str],
    args: &'static [&'static str],
}

fn provider_templates() -> Vec<ProviderTemplate> {
    vec![
        ProviderTemplate {
            id: "codex",
            provider: AgentProvider::Codex,
            label: "Codex",
            acp: Some(CommandTemplate {
                executables: &["codex-acp"],
                args: &[],
            }),
            cli: Some(CommandTemplate {
                executables: &["codex"],
                args: &[
                    "exec",
                    "--skip-git-repo-check",
                    "--ephemeral",
                    "-c",
                    "model_reasoning_effort=\"medium\"",
                ],
            }),
            allow_cli_fallback: true,
        },
        ProviderTemplate {
            id: "claude-code",
            provider: AgentProvider::ClaudeCode,
            label: "Claude Code",
            acp: Some(CommandTemplate {
                executables: &["claude-agent-acp"],
                args: &[],
            }),
            cli: Some(CommandTemplate {
                executables: &["claude"],
                args: &["-p", "--output-format", "json"],
            }),
            allow_cli_fallback: false,
        },
        ProviderTemplate {
            id: "cursor",
            provider: AgentProvider::Cursor,
            label: "Cursor",
            acp: Some(CommandTemplate {
                executables: &["cursor"],
                args: &["agent", "--acp"],
            }),
            cli: None,
            allow_cli_fallback: false,
        },
        ProviderTemplate {
            id: "opencode",
            provider: AgentProvider::Opencode,
            label: "OpenCode",
            acp: Some(CommandTemplate {
                executables: &["opencode"],
                args: &["acp"],
            }),
            cli: Some(CommandTemplate {
                executables: &["opencode"],
                args: &["run", "--format", "json"],
            }),
            allow_cli_fallback: false,
        },
        ProviderTemplate {
            id: "kimi-code",
            provider: AgentProvider::KimiCode,
            label: "Kimi Code",
            acp: Some(CommandTemplate {
                executables: &["kimi", "kimi-code"],
                args: &["acp"],
            }),
            cli: Some(CommandTemplate {
                executables: &["kimi", "kimi-code"],
                args: &["-p", "--output-format", "text"],
            }),
            allow_cli_fallback: true,
        },
        ProviderTemplate {
            id: "qoder",
            provider: AgentProvider::Qoder,
            label: "Qoder",
            acp: Some(CommandTemplate {
                executables: &["qoder"],
                args: &["acp"],
            }),
            cli: None,
            allow_cli_fallback: false,
        },
        ProviderTemplate {
            id: "sofvary-pi",
            provider: AgentProvider::SofvaryPi,
            label: "Sofvary Pi",
            acp: None,
            cli: Some(CommandTemplate {
                executables: &["pi"],
                args: &["--mode", "rpc"],
            }),
            allow_cli_fallback: false,
        },
        ProviderTemplate {
            id: "deepseek-tui",
            provider: AgentProvider::DeepseekTui,
            label: "DeepSeek TUI",
            acp: Some(CommandTemplate {
                executables: &["codewhale", "deepseek", "deepseek-tui"],
                args: &["serve", "--acp"],
            }),
            cli: None,
            allow_cli_fallback: false,
        },
    ]
}

fn os_slug(os: OsKind) -> &'static str {
    match os {
        OsKind::Windows => "windows",
        OsKind::Macos => "macos",
        OsKind::Linux => "linux",
    }
}

fn arch_slug(arch: ArchKind) -> &'static str {
    match arch {
        ArchKind::X64 => "x64",
        ArchKind::Arm64 => "arm64",
        ArchKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn prefers_dev_adapter_over_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dev = temp.path().join("dev");
        fs::create_dir_all(&dev).expect("dev dir");
        let codex = dev.join("codex-acp");
        fs::write(&codex, b"adapter").expect("adapter");

        let template = ProviderTemplate {
            id: "codex",
            provider: AgentProvider::Codex,
            label: "Codex",
            acp: Some(CommandTemplate {
                executables: &["codex-acp"],
                args: &[],
            }),
            cli: None,
            allow_cli_fallback: false,
        };
        let discovered = discover_template(
            template,
            temp.path(),
            Some(&dev),
            OsKind::Macos,
            ArchKind::Arm64,
        );

        assert_eq!(
            discovered.config.acp.expect("acp").source,
            AgentInstallSource::DevOverride
        );
    }

    #[test]
    fn codex_cli_wrapper_without_vendor_binary_is_not_usable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let package_bin = temp.path().join("lib/node_modules/@openai/codex/bin");
        fs::create_dir_all(&package_bin).expect("package bin");
        let wrapper = package_bin.join("codex.js");
        fs::write(&wrapper, b"#!/usr/bin/env node\n").expect("wrapper");

        assert!(
            !codex_cli_vendor_binary_exists(&wrapper, OsKind::Macos, ArchKind::Arm64)
                .expect("codex path should be recognized")
        );
    }

    #[test]
    fn codex_cli_wrapper_with_vendor_binary_is_usable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let package_root = temp.path().join("lib/node_modules/@openai/codex");
        let package_bin = package_root.join("bin");
        let vendor_bin = package_root
            .join("node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/codex");
        fs::create_dir_all(&package_bin).expect("package bin");
        fs::create_dir_all(&vendor_bin).expect("vendor bin");
        let wrapper = package_bin.join("codex.js");
        let native = vendor_bin.join("codex");
        fs::write(&wrapper, b"#!/usr/bin/env node\n").expect("wrapper");
        fs::write(native, b"native").expect("native");

        assert!(
            codex_cli_vendor_binary_exists(&wrapper, OsKind::Macos, ArchKind::Arm64)
                .expect("codex path should be recognized")
        );
    }

    #[test]
    fn codex_cli_template_skips_git_repo_check_for_generated_workspaces() {
        let codex = provider_templates()
            .into_iter()
            .find(|template| template.id == "codex")
            .expect("codex template");
        let cli = codex.cli.expect("codex cli template");

        assert!(cli.args.contains(&"--skip-git-repo-check"));
    }
}
