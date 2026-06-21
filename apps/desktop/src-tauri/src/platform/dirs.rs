use crate::platform::types::{PlatformDirs, PlatformError, PlatformResult};
use std::env;
use std::path::PathBuf;

fn home_dir() -> PlatformResult<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| PlatformError::InvalidPath("home directory is unavailable".to_string()))
}

pub fn platform_dirs_for_app(app_name: &str) -> PlatformResult<PlatformDirs> {
    #[cfg(target_os = "windows")]
    {
        let app_data = env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home_dir()?.join("AppData").join("Roaming"));
        let local_app_data = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or(home_dir()?.join("AppData").join("Local"));

        return Ok(PlatformDirs {
            data_dir: app_data.join(app_name),
            cache_dir: local_app_data.join(app_name).join("Cache"),
            config_dir: app_data.join(app_name).join("Config"),
        });
    }

    #[cfg(target_os = "macos")]
    {
        let home = home_dir()?;
        return Ok(PlatformDirs {
            data_dir: home
                .join("Library")
                .join("Application Support")
                .join(app_name),
            cache_dir: home.join("Library").join("Caches").join(app_name),
            config_dir: home.join("Library").join("Preferences").join(app_name),
        });
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let home = home_dir()?;
        let data_root = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"));
        let cache_root = env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"));
        let config_root = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));

        return Ok(PlatformDirs {
            data_dir: data_root.join(app_name),
            cache_dir: cache_root.join(app_name),
            config_dir: config_root.join(app_name),
        });
    }

    #[allow(unreachable_code)]
    Err(PlatformError::Unsupported(
        "platform directories are not available".to_string(),
    ))
}
