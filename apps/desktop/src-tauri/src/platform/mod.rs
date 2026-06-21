pub mod dirs;
pub mod host_shell;
pub mod linux;
pub mod macos;
pub mod path_policy;
pub mod process;
pub mod protocol;
pub mod secure_store;
pub mod sidecar;
pub mod stdio;
pub mod types;
pub mod windows;

use std::path::{Component, Path, PathBuf};

pub use path_policy::{command_executable_name, is_protected_delete_target};
pub use types::{
    ArchKind, CommandSpec, OsKind, PlatformDirs, PlatformError, PlatformResult, ProcessHandle,
    ProcessOutput, WebviewProfile, WorkArea,
};

// The platform boundary is intentionally ahead of the current MVP wiring.
#[allow(dead_code)]
pub trait PlatformAdapter: Send + Sync {
    fn os(&self) -> OsKind;
    fn arch(&self) -> ArchKind;
    fn dirs(&self) -> PlatformResult<PlatformDirs>;
    fn normalize_path(&self, input: &str) -> PlatformResult<PathBuf>;
    fn ensure_executable(&self, path: &Path) -> PlatformResult<()>;
    fn resolve_sidecar_executable(&self, name: &str) -> PlatformResult<PathBuf>;
    fn run_process(&self, spec: CommandSpec) -> PlatformResult<ProcessOutput>;
    fn spawn_process(&self, spec: CommandSpec) -> PlatformResult<ProcessHandle>;
    fn kill_process_tree(&self, pid: u32) -> PlatformResult<()>;
    fn allocate_local_port(&self) -> PlatformResult<u16>;
    fn open_external(&self, url: &str) -> PlatformResult<()>;
    fn reveal_path(&self, path: &Path) -> PlatformResult<()>;
    fn register_protocol_handler(&self, protocol: &str) -> PlatformResult<()>;
    fn register_global_shortcut(&self, accelerator: &str) -> PlatformResult<()>;
    fn unregister_global_shortcut(&self, accelerator: &str) -> PlatformResult<()>;
    fn show_tray_or_menu_bar_item(&self) -> PlatformResult<()>;
    fn get_active_monitor_work_area(&self) -> PlatformResult<WorkArea>;
    fn secure_store_set(&self, key: &str, value: &str) -> PlatformResult<()>;
    fn secure_store_get(&self, key: &str) -> PlatformResult<Option<String>>;
    fn current_webview_profile(&self) -> WebviewProfile;
}

pub fn current_adapter() -> Box<dyn PlatformAdapter> {
    #[cfg(target_os = "windows")]
    {
        return Box::new(windows::WindowsPlatformAdapter);
    }

    #[cfg(target_os = "macos")]
    {
        return Box::new(macos::MacosPlatformAdapter);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return Box::new(linux::LinuxPlatformAdapter);
    }

    #[allow(unreachable_code)]
    Box::new(linux::LinuxPlatformAdapter)
}

pub fn current_arch() -> ArchKind {
    match std::env::consts::ARCH {
        "x86_64" | "x64" => ArchKind::X64,
        "aarch64" | "arm64" => ArchKind::Arm64,
        _ => ArchKind::Unknown,
    }
}

#[allow(dead_code)]
pub(crate) fn normalize_path_lexically(input: &str) -> PlatformResult<PathBuf> {
    if input.trim().is_empty() {
        return Err(PlatformError::InvalidPath(
            "path cannot be empty".to_string(),
        ));
    }

    let input_path = PathBuf::from(input);
    let absolute = if input_path.is_absolute() {
        input_path
    } else {
        std::env::current_dir()?.join(input_path)
    };

    let mut normalized = PathBuf::new();
    for component in absolute.components() {
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

    Ok(normalized)
}

pub(crate) fn allocate_loopback_port() -> PlatformResult<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

#[allow(dead_code)]
pub(crate) fn default_work_area() -> WorkArea {
    WorkArea {
        x: 0,
        y: 0,
        width: 1200,
        height: 760,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_current_adapter() {
        let adapter = current_adapter();
        assert_eq!(adapter.arch(), current_arch());
    }

    #[test]
    fn normalizes_relative_path() {
        let normalized = normalize_path_lexically("./a/../b").expect("path should normalize");
        assert!(normalized.ends_with("b"));
    }

    #[test]
    fn allocates_loopback_port() {
        let port = allocate_loopback_port().expect("port should allocate");
        assert!(port > 0);
    }

    #[test]
    fn rejects_unsupported_sidecar_names() {
        assert!(matches!(
            sidecar::resolve_sidecar_executable("bash", current_adapter().as_ref()),
            Err(PlatformError::Unsupported(_))
        ));
    }
}
