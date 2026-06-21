use crate::platform::{
    ArchKind, OsKind, PlatformAdapter, PlatformDirs, PlatformError, PlatformResult,
};
use std::path::{Path, PathBuf};

pub const DEV_SIDECAR_DIR_ENV: &str = "SOFVARY_DEV_SIDECAR_DIR";

pub fn resolve_sidecar_executable(
    name: &str,
    adapter: &dyn PlatformAdapter,
) -> PlatformResult<PathBuf> {
    let sidecar = SidecarName::parse(name)?;
    let dirs = adapter.dirs()?;
    resolve_sidecar_in_dirs(sidecar, adapter.os(), adapter.arch(), &dirs)
}

pub(crate) fn resolve_sidecar_in_dirs(
    sidecar: SidecarName,
    os: OsKind,
    arch: ArchKind,
    dirs: &PlatformDirs,
) -> PlatformResult<PathBuf> {
    let candidates = sidecar_file_candidates(sidecar, os);
    let search_dirs = sidecar_search_dirs(os, arch, dirs);

    for dir in &search_dirs {
        for file_name in &candidates {
            let candidate = dir.join(file_name);
            if candidate.is_file() {
                ensure_sidecar_is_executable(sidecar, &candidate)?;
                return Ok(candidate);
            }
        }
    }

    Err(PlatformError::InvalidPath(format!(
        "sidecar executable '{}' was not found in controlled Sofvary sidecar dirs: {}",
        sidecar.as_str(),
        search_dirs
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

pub(crate) fn platform_sidecar_dir(dirs: &PlatformDirs, os: OsKind, arch: ArchKind) -> PathBuf {
    dirs.data_dir
        .join("sidecars")
        .join(format!("{}-{}", os_slug(os), arch_slug(arch)))
}

fn sidecar_search_dirs(os: OsKind, arch: ArchKind, dirs: &PlatformDirs) -> Vec<PathBuf> {
    let mut search_dirs = Vec::new();
    if let Some(dev_dir) = std::env::var_os(DEV_SIDECAR_DIR_ENV) {
        if !dev_dir.is_empty() {
            search_dirs.push(PathBuf::from(dev_dir));
        }
    }
    search_dirs.push(platform_sidecar_dir(dirs, os, arch));
    search_dirs
}

fn sidecar_file_candidates(sidecar: SidecarName, os: OsKind) -> Vec<&'static str> {
    match (sidecar, os) {
        (SidecarName::Node, OsKind::Windows) => vec!["node.exe"],
        (SidecarName::Pnpm, OsKind::Windows) => vec!["pnpm.exe", "pnpm.cmd", "pnpm"],
        (SidecarName::Node, _) => vec!["node"],
        (SidecarName::Pnpm, _) => vec!["pnpm"],
    }
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

#[cfg(unix)]
fn ensure_sidecar_is_executable(sidecar: SidecarName, candidate: &Path) -> PlatformResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = candidate.metadata()?;
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(PlatformError::InvalidPath(format!(
            "sidecar executable '{}' is not executable: {}",
            sidecar.as_str(),
            candidate.display()
        )));
    }

    Ok(())
}

#[cfg(not(unix))]
fn ensure_sidecar_is_executable(_sidecar: SidecarName, _candidate: &Path) -> PlatformResult<()> {
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidecarName {
    Node,
    Pnpm,
}

impl SidecarName {
    fn parse(name: &str) -> PlatformResult<Self> {
        match name {
            "node" => Ok(Self::Node),
            "pnpm" => Ok(Self::Pnpm),
            other => Err(PlatformError::Unsupported(format!(
                "unsupported sidecar executable: {other}"
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Pnpm => "pnpm",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_dirs(root: &Path) -> PlatformDirs {
        PlatformDirs {
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            config_dir: root.join("config"),
        }
    }

    #[test]
    fn adapter_resolves_sidecar_from_controlled_dev_dir() {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var_os(DEV_SIDECAR_DIR_ENV);
        let temp = tempfile::tempdir().expect("tempdir");
        let sidecar_path = temp.path().join("pnpm");
        std::fs::write(&sidecar_path, b"test pnpm sidecar").expect("write sidecar");
        make_test_executable(&sidecar_path);
        std::env::set_var(DEV_SIDECAR_DIR_ENV, temp.path());

        let adapter = crate::platform::current_adapter();
        let resolved = adapter
            .resolve_sidecar_executable("pnpm")
            .expect("controlled pnpm sidecar");

        assert_eq!(resolved, sidecar_path);
        restore_env(previous);
    }

    #[test]
    fn missing_sidecar_fails_clearly() {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var_os(DEV_SIDECAR_DIR_ENV);
        let temp = tempfile::tempdir().expect("tempdir");
        std::env::set_var(DEV_SIDECAR_DIR_ENV, temp.path());

        let result = resolve_sidecar_in_dirs(
            SidecarName::Pnpm,
            OsKind::Macos,
            ArchKind::Arm64,
            &test_dirs(temp.path()),
        );

        assert!(matches!(
            result,
            Err(PlatformError::InvalidPath(message))
                if message.contains("sidecar executable 'pnpm'")
                    && message.contains(temp.path().to_string_lossy().as_ref())
        ));
        restore_env(previous);
    }

    #[test]
    fn unsupported_sidecar_name_fails_before_path_lookup() {
        assert!(matches!(
            SidecarName::parse("bash"),
            Err(PlatformError::Unsupported(message))
                if message.contains("unsupported sidecar executable")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn non_executable_sidecar_file_fails_clearly() {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var_os(DEV_SIDECAR_DIR_ENV);
        std::env::remove_var(DEV_SIDECAR_DIR_ENV);
        let temp = tempfile::tempdir().expect("tempdir");
        let dirs = test_dirs(temp.path());
        let sidecar_dir = platform_sidecar_dir(&dirs, OsKind::Macos, ArchKind::Arm64);
        std::fs::create_dir_all(&sidecar_dir).expect("sidecar dir");
        let sidecar_path = sidecar_dir.join("pnpm");
        std::fs::write(&sidecar_path, b"test pnpm sidecar").expect("write sidecar");

        let result =
            resolve_sidecar_in_dirs(SidecarName::Pnpm, OsKind::Macos, ArchKind::Arm64, &dirs);

        assert!(matches!(
            result,
            Err(PlatformError::InvalidPath(message))
                if message.contains("sidecar executable 'pnpm' is not executable")
                    && message.contains(sidecar_path.to_string_lossy().as_ref())
        ));
        restore_env(previous);
    }

    fn restore_env(previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => std::env::set_var(DEV_SIDECAR_DIR_ENV, value),
            None => std::env::remove_var(DEV_SIDECAR_DIR_ENV),
        }
    }

    #[cfg(unix)]
    fn make_test_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path)
            .expect("sidecar metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("sidecar permissions");
    }

    #[cfg(not(unix))]
    fn make_test_executable(_path: &Path) {}
}
