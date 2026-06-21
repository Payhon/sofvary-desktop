use crate::platform::dirs::platform_dirs_for_app;
use crate::platform::process::{
    kill_process_tree, run_structured_process, spawn_structured_process,
};
use crate::platform::protocol::register_protocol_handler_stub;
use crate::platform::secure_store::{secure_store_get_stub, secure_store_set_stub};
use crate::platform::{
    allocate_loopback_port, current_arch, default_work_area, normalize_path_lexically, CommandSpec,
    OsKind, PlatformAdapter, PlatformDirs, PlatformError, PlatformResult, ProcessHandle,
    ProcessOutput, WebviewProfile, WorkArea,
};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub struct WindowsPlatformAdapter;

impl PlatformAdapter for WindowsPlatformAdapter {
    fn os(&self) -> OsKind {
        OsKind::Windows
    }

    fn arch(&self) -> crate::platform::ArchKind {
        current_arch()
    }

    fn dirs(&self) -> PlatformResult<PlatformDirs> {
        platform_dirs_for_app("Sofvary")
    }

    fn normalize_path(&self, input: &str) -> PlatformResult<PathBuf> {
        normalize_path_lexically(input)
    }

    fn ensure_executable(&self, path: &Path) -> PlatformResult<()> {
        if path.exists() {
            Ok(())
        } else {
            Err(PlatformError::InvalidPath(format!(
                "executable does not exist: {}",
                path.display()
            )))
        }
    }

    fn resolve_sidecar_executable(&self, name: &str) -> PlatformResult<PathBuf> {
        crate::platform::sidecar::resolve_sidecar_executable(name, self)
    }

    fn run_process(&self, spec: CommandSpec) -> PlatformResult<ProcessOutput> {
        run_structured_process(spec)
    }

    fn spawn_process(&self, spec: CommandSpec) -> PlatformResult<ProcessHandle> {
        spawn_structured_process(spec)
    }

    fn kill_process_tree(&self, pid: u32) -> PlatformResult<()> {
        kill_process_tree(pid)
    }

    fn allocate_local_port(&self) -> PlatformResult<u16> {
        allocate_loopback_port()
    }

    fn open_external(&self, url: &str) -> PlatformResult<()> {
        if url.trim().is_empty() {
            return Err(PlatformError::InvalidPath(
                "url cannot be empty".to_string(),
            ));
        }
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }

    fn reveal_path(&self, path: &Path) -> PlatformResult<()> {
        if !path.exists() {
            return Err(PlatformError::InvalidPath(format!(
                "path does not exist: {}",
                path.display()
            )));
        }
        Command::new("explorer")
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }

    fn register_protocol_handler(&self, protocol: &str) -> PlatformResult<()> {
        register_protocol_handler_stub(protocol)
    }

    fn register_global_shortcut(&self, accelerator: &str) -> PlatformResult<()> {
        if accelerator.trim().is_empty() {
            return Err(PlatformError::Unsupported(
                "accelerator cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn unregister_global_shortcut(&self, _accelerator: &str) -> PlatformResult<()> {
        Ok(())
    }

    fn show_tray_or_menu_bar_item(&self) -> PlatformResult<()> {
        Ok(())
    }

    fn get_active_monitor_work_area(&self) -> PlatformResult<WorkArea> {
        Ok(default_work_area())
    }

    fn secure_store_set(&self, key: &str, value: &str) -> PlatformResult<()> {
        secure_store_set_stub(key, value)
    }

    fn secure_store_get(&self, key: &str) -> PlatformResult<Option<String>> {
        secure_store_get_stub(key)
    }

    fn current_webview_profile(&self) -> WebviewProfile {
        WebviewProfile {
            engine: "webview2".to_string(),
            supports_transparency: true,
            notes: vec!["Windows-first adapter skeleton".to_string()],
        }
    }
}
