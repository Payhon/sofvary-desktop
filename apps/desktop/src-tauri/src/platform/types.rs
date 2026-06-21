use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

pub type PlatformResult<T> = Result<T, PlatformError>;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported platform operation: {0}")]
    Unsupported(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OsKind {
    Windows,
    Macos,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchKind {
    X64,
    Arm64,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDirs {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub config_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub allowed_network: bool,
    pub timeout_ms: Option<u64>,
    pub kill_on_drop: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessHandle {
    pub pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOutput {
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewProfile {
    pub engine: String,
    pub supports_transparency: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_spec_is_structured() {
        let spec = CommandSpec {
            executable: PathBuf::from("node"),
            args: vec!["server.js".to_string()],
            cwd: PathBuf::from("/tmp/workspace"),
            env: HashMap::from([("NODE_ENV".to_string(), "development".to_string())]),
            allowed_network: false,
            timeout_ms: Some(1000),
            kill_on_drop: true,
        };

        assert_eq!(spec.args, ["server.js"]);
        assert!(!spec.allowed_network);
        assert!(spec.timeout_ms.is_some());
    }
}
