use crate::core::harness_engine::PromptEnvelope;
use crate::core::pack_types::{PackCommandSpec, RuntimePackManifest};
use crate::core::policy_engine::{PolicyEngine, PolicyError};
use crate::core::policy_types::{PolicyApprovalSet, PolicyCommandRequest};
use crate::core::runtime_dependency_install::{
    dependency_install_recovery_spec, is_offline_dependency_cache_failure,
};
use crate::core::workspace_types::{AppBoxManifest, RuntimeMode};
use crate::platform::{current_adapter, CommandSpec, PlatformAdapter, PlatformError};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactSqliteRuntimePreview {
    pub preview_url: String,
    pub logs: Vec<String>,
}

pub struct ReactSqliteRuntimeServer {
    preview: ReactSqliteRuntimePreview,
    api_pid: u32,
    vite_pid: u32,
}

impl ReactSqliteRuntimeServer {
    pub fn preview(&self) -> ReactSqliteRuntimePreview {
        self.preview.clone()
    }

    pub fn stop(&mut self) {
        let adapter = current_adapter();
        let _ = adapter.kill_process_tree(self.vite_pid);
        let _ = adapter.kill_process_tree(self.api_pid);
    }
}

impl Drop for ReactSqliteRuntimeServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Error)]
pub enum ReactSqliteRuntimeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),
    #[error("request attempted to escape react-sqlite runtime root")]
    PathEscape,
    #[error("invalid prompt envelope: {0}")]
    InvalidPromptEnvelope(String),
    #[error("invalid runtime command spec: {0}")]
    InvalidCommandSpec(String),
    #[error("runtime command '{name}' is missing from pack {pack_id}@{pack_version}")]
    MissingCommand {
        name: String,
        pack_id: String,
        pack_version: String,
    },
    #[error("runtime command '{name}' failed with status {status:?}: {stderr}")]
    CommandFailed {
        name: String,
        status: Option<i32>,
        stdout: String,
        stderr: String,
        log_path: PathBuf,
    },
    #[error("React + SQLite runtime supports dev mode only in Phase 10")]
    UnsupportedMode,
    #[error("local API did not become ready at {0}")]
    ApiNotReady(String),
    #[error("Vite dev server did not become ready at {0}")]
    DevServerNotReady(String),
}

#[derive(Default)]
pub struct ReactSqliteRuntime;

impl ReactSqliteRuntime {
    pub fn new() -> Self {
        Self
    }

    #[allow(dead_code)]
    pub fn start_workspace_with_envelope(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
    ) -> Result<ReactSqliteRuntimeServer, ReactSqliteRuntimeError> {
        self.start_workspace_with_envelope_with_policy(
            manifest,
            envelope,
            runtime_pack,
            mode,
            &PolicyApprovalSet::default(),
        )
    }

    pub fn start_workspace_with_envelope_with_policy(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
        approvals: &PolicyApprovalSet,
    ) -> Result<ReactSqliteRuntimeServer, ReactSqliteRuntimeError> {
        self.validate_prompt_envelope(envelope)?;
        ensure_exact_workspace_react_sqlite_files(manifest, &envelope.output_contract.files)?;
        let sqlite_path = workspace_sqlite_path(manifest)?;

        match mode {
            RuntimeMode::Dev => {
                self.start_dev_workspace(manifest, runtime_pack, sqlite_path, approvals)
            }
            RuntimeMode::Prod => Err(ReactSqliteRuntimeError::UnsupportedMode),
        }
    }

    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), ReactSqliteRuntimeError> {
        validate_prompt_envelope(envelope)
    }

    #[allow(dead_code)]
    pub fn workspace_sqlite_path(
        &self,
        manifest: &AppBoxManifest,
    ) -> Result<PathBuf, ReactSqliteRuntimeError> {
        workspace_sqlite_path(manifest)
    }

    fn start_dev_workspace(
        &self,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        sqlite_path: PathBuf,
        approvals: &PolicyApprovalSet,
    ) -> Result<ReactSqliteRuntimeServer, ReactSqliteRuntimeError> {
        let adapter = current_adapter();
        let api_port = adapter.allocate_local_port()?;
        let vite_port = adapter.allocate_local_port()?;
        let api_token = Uuid::new_v4().simple().to_string();
        let install = required_command(runtime_pack, "install")?;
        let api = required_command(runtime_pack, "api")?;
        let dev = required_command(runtime_pack, "dev")?;
        let install_spec = command_spec_from_pack(manifest, install, None, None, adapter.as_ref())?;
        let api_spec = command_spec_from_pack(
            manifest,
            api,
            None,
            Some(RuntimeEnv {
                api_port,
                sqlite_path: Some(sqlite_path.clone()),
                api_token: Some(api_token.clone()),
            }),
            adapter.as_ref(),
        )?;
        let dev_spec = command_spec_from_pack(
            manifest,
            dev,
            Some(vite_port),
            Some(RuntimeEnv {
                api_port,
                sqlite_path: None,
                api_token: Some(api_token),
            }),
            adapter.as_ref(),
        )?;
        let log_path = runtime_log_path(manifest, "react-sqlite-dev.log")?;

        write_runtime_log(
            &log_path,
            &[
                "ReactSqliteRuntime dev mode".to_string(),
                format!("Install command: {}", display_command(&install_spec)),
                format!("API command: {}", display_command(&api_spec)),
                format!("Dev command: {}", display_command(&dev_spec)),
                format!("SQLite path: {}", sqlite_path.display()),
            ],
        )?;

        run_dependency_install(adapter.as_ref(), install_spec, approvals, &log_path)?;
        ensure_better_sqlite3_compat_shim(manifest)?;

        enforce_command("api", &api_spec, approvals)?;
        let api_handle = adapter.spawn_process(api_spec.clone())?;
        append_runtime_log(&log_path, &[format!("Node API pid: {}", api_handle.pid)])?;
        let api_health_url = format!("http://127.0.0.1:{api_port}/api/health");
        if let Err(error) = wait_for_http(&api_health_url, RuntimeWaitKind::Api) {
            let _ = adapter.kill_process_tree(api_handle.pid);
            append_runtime_log(
                &log_path,
                &[format!(
                    "API readiness failed; killed pid {}",
                    api_handle.pid
                )],
            )?;
            return Err(error);
        }

        enforce_command("dev", &dev_spec, approvals)?;
        let vite_handle = adapter.spawn_process(dev_spec.clone())?;
        append_runtime_log(
            &log_path,
            &[format!("Vite dev server pid: {}", vite_handle.pid)],
        )?;
        let preview_url = format!("http://127.0.0.1:{vite_port}/");
        if let Err(error) = wait_for_http(&preview_url, RuntimeWaitKind::Vite) {
            let _ = adapter.kill_process_tree(vite_handle.pid);
            let _ = adapter.kill_process_tree(api_handle.pid);
            append_runtime_log(
                &log_path,
                &[format!(
                    "Vite readiness failed; killed pid {} and API pid {}",
                    vite_handle.pid, api_handle.pid
                )],
            )?;
            return Err(error);
        }

        Ok(ReactSqliteRuntimeServer {
            preview: ReactSqliteRuntimePreview {
                preview_url,
                logs: vec![
                    format!(
                        "ReactSqliteRuntime dev preview started for {}",
                        manifest.app_id
                    ),
                    format!("API health {}", api_health_url),
                    format!("SQLite database {}", sqlite_path.display()),
                    format!("API process pid {}", api_handle.pid),
                    format!("Vite process pid {}", vite_handle.pid),
                    format!("Runtime log {}", log_path.display()),
                ],
            },
            api_pid: api_handle.pid,
            vite_pid: vite_handle.pid,
        })
    }
}

#[derive(Clone)]
struct RuntimeEnv {
    api_port: u16,
    sqlite_path: Option<PathBuf>,
    api_token: Option<String>,
}

fn required_command<'a>(
    runtime_pack: &'a RuntimePackManifest,
    name: &str,
) -> Result<&'a PackCommandSpec, ReactSqliteRuntimeError> {
    runtime_pack
        .commands
        .get(name)
        .ok_or_else(|| ReactSqliteRuntimeError::MissingCommand {
            name: name.to_string(),
            pack_id: runtime_pack.id.clone(),
            pack_version: runtime_pack.version.clone(),
        })
}

fn enforce_dependency_install(
    name: &str,
    spec: &CommandSpec,
    approvals: &PolicyApprovalSet,
) -> Result<(), ReactSqliteRuntimeError> {
    let engine = PolicyEngine::new();
    let decision = engine.evaluate_dependency_install(PolicyCommandRequest {
        name: name.to_string(),
        command: spec.clone(),
    });
    Ok(engine.enforce(decision, approvals)?)
}

fn run_dependency_install(
    adapter: &dyn PlatformAdapter,
    install_spec: CommandSpec,
    approvals: &PolicyApprovalSet,
    log_path: &Path,
) -> Result<(), ReactSqliteRuntimeError> {
    enforce_dependency_install("install", &install_spec, approvals)?;
    let install_output = adapter.run_process(install_spec.clone())?;
    if install_output.status_code == Some(0) {
        return Ok(());
    }

    append_runtime_log(
        log_path,
        &[
            "Install command failed".to_string(),
            install_output.stdout.clone(),
            install_output.stderr.clone(),
        ],
    )?;

    if is_offline_dependency_cache_failure(&install_output.stdout, &install_output.stderr) {
        if let Some(recovery_spec) = dependency_install_recovery_spec(&install_spec) {
            append_runtime_log(
                log_path,
                &[
                    "Offline dependency cache missing; retrying with policy-approved dependency recovery."
                        .to_string(),
                    format!("Recovery install command: {}", display_command(&recovery_spec)),
                ],
            )?;
            enforce_dependency_install("install", &recovery_spec, approvals)?;
            let recovery_output = adapter.run_process(recovery_spec)?;
            if recovery_output.status_code == Some(0) {
                append_runtime_log(
                    log_path,
                    &["Dependency recovery command succeeded".to_string()],
                )?;
                return Ok(());
            }
            append_runtime_log(
                log_path,
                &[
                    "Dependency recovery command failed".to_string(),
                    recovery_output.stdout.clone(),
                    recovery_output.stderr.clone(),
                ],
            )?;
            return Err(ReactSqliteRuntimeError::CommandFailed {
                name: "install".to_string(),
                status: recovery_output.status_code,
                stdout: recovery_output.stdout,
                stderr: recovery_output.stderr,
                log_path: log_path.to_path_buf(),
            });
        }
    }

    Err(ReactSqliteRuntimeError::CommandFailed {
        name: "install".to_string(),
        status: install_output.status_code,
        stdout: install_output.stdout,
        stderr: install_output.stderr,
        log_path: log_path.to_path_buf(),
    })
}

fn ensure_better_sqlite3_compat_shim(
    manifest: &AppBoxManifest,
) -> Result<(), ReactSqliteRuntimeError> {
    let generated_root = prepare_generated_root(manifest)?;
    let shim_root = ensure_child(
        &generated_root,
        Path::new("react/node_modules/better-sqlite3"),
    )?;
    fs::create_dir_all(&shim_root)?;
    fs::write(
        shim_root.join("package.json"),
        r#"{"name":"better-sqlite3","version":"0.0.0-sofvary-shim","main":"index.cjs","types":"index.d.ts","type":"commonjs"}
"#,
    )?;
    fs::write(shim_root.join("index.cjs"), BETTER_SQLITE3_COMPAT_SHIM)?;
    fs::write(shim_root.join("index.d.ts"), BETTER_SQLITE3_COMPAT_TYPES)?;
    Ok(())
}

const BETTER_SQLITE3_COMPAT_SHIM: &str = r#"const { DatabaseSync } = require("node:sqlite");

class SofvaryStatement {
  constructor(statement) {
    this.statement = statement;
  }

  all(...params) {
    return this.statement.all(...params);
  }

  get(...params) {
    return this.statement.get(...params);
  }

  run(...params) {
    return this.statement.run(...params);
  }
}

class SofvaryBetterSqliteDatabase {
  constructor(filename) {
    this.database = new DatabaseSync(filename);
  }

  exec(sql) {
    this.database.exec(sql);
    return this;
  }

  prepare(sql) {
    return new SofvaryStatement(this.database.prepare(sql));
  }

  transaction(callback) {
    return (...args) => {
      this.database.exec("BEGIN");
      try {
        const result = callback(...args);
        this.database.exec("COMMIT");
        return result;
      } catch (error) {
        try {
          this.database.exec("ROLLBACK");
        } catch (_) {}
        throw error;
      }
    };
  }

  close() {
    this.database.close();
  }
}

module.exports = SofvaryBetterSqliteDatabase;
module.exports.default = SofvaryBetterSqliteDatabase;
"#;

const BETTER_SQLITE3_COMPAT_TYPES: &str = r#"declare class Database {
  constructor(filename: string);
  exec(sql: string): this;
  prepare(sql: string): Database.Statement;
  transaction<T extends (...args: any[]) => any>(callback: T): T;
  close(): void;
}

declare namespace Database {
  interface Statement {
    all(...params: unknown[]): any[];
    get(...params: unknown[]): any;
    run(...params: unknown[]): { changes: number; lastInsertRowid: number | bigint };
  }
}

export = Database;
"#;

fn enforce_command(
    name: &str,
    spec: &CommandSpec,
    approvals: &PolicyApprovalSet,
) -> Result<(), ReactSqliteRuntimeError> {
    let engine = PolicyEngine::new();
    let decision = engine.evaluate_command(PolicyCommandRequest {
        name: name.to_string(),
        command: spec.clone(),
    });
    Ok(engine.enforce(decision, approvals)?)
}

fn command_spec_from_pack(
    manifest: &AppBoxManifest,
    command: &PackCommandSpec,
    port: Option<u16>,
    runtime_env: Option<RuntimeEnv>,
    adapter: &dyn PlatformAdapter,
) -> Result<CommandSpec, ReactSqliteRuntimeError> {
    let executable = resolve_executable(&command.executable, adapter)?;
    let cwd = match &command.cwd {
        Some(cwd) => ensure_child(&manifest.paths.root, Path::new(cwd))?,
        None => manifest.paths.root.clone(),
    };
    let args = command
        .args
        .iter()
        .map(|arg| render_command_arg(arg, port))
        .collect();
    let mut env = command.env.clone().unwrap_or_default();
    if let Some(runtime_env) = runtime_env {
        insert_runtime_env(&mut env, runtime_env);
    }

    Ok(CommandSpec {
        executable,
        args,
        cwd,
        env,
        allowed_network: command.allowed_network.unwrap_or(false),
        timeout_ms: command.timeout_ms,
        kill_on_drop: command.kill_on_drop.unwrap_or(true),
    })
}

fn insert_runtime_env(env: &mut HashMap<String, String>, runtime_env: RuntimeEnv) {
    let is_api_process = runtime_env.sqlite_path.is_some();
    env.insert(
        "SOFVARY_API_PORT".to_string(),
        runtime_env.api_port.to_string(),
    );
    if is_api_process {
        env.insert("PORT".to_string(), runtime_env.api_port.to_string());
    }
    if let Some(sqlite_path) = runtime_env.sqlite_path {
        env.insert(
            "SOFVARY_SQLITE_PATH".to_string(),
            sqlite_path.display().to_string(),
        );
    }
    if let Some(api_token) = runtime_env.api_token {
        env.insert("SOFVARY_API_TOKEN".to_string(), api_token);
    }
}

fn resolve_executable(
    executable: &str,
    adapter: &dyn PlatformAdapter,
) -> Result<PathBuf, ReactSqliteRuntimeError> {
    if let Some(name) = executable
        .strip_prefix("${sidecar:")
        .and_then(|value| value.strip_suffix('}'))
    {
        return Ok(adapter.resolve_sidecar_executable(name)?);
    }

    Err(ReactSqliteRuntimeError::InvalidCommandSpec(format!(
        "runtime command executable must use a sidecar placeholder, found '{executable}'"
    )))
}

fn render_command_arg(arg: &str, port: Option<u16>) -> String {
    match port {
        Some(port) => arg.replace("${port}", &port.to_string()),
        None => arg.to_string(),
    }
}

fn validate_prompt_envelope(envelope: &PromptEnvelope) -> Result<(), ReactSqliteRuntimeError> {
    if envelope.box_runtime_context.runtime_kind != "react-sqlite" {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(format!(
            "runtimeKind must be react-sqlite, found {}",
            envelope.box_runtime_context.runtime_kind
        )));
    }
    if envelope.box_runtime_context.generated_root != "generated" {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(format!(
            "generatedRoot must be generated, found {}",
            envelope.box_runtime_context.generated_root
        )));
    }
    if envelope.box_runtime_context.entrypoint != "react/src/main.tsx" {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(format!(
            "entrypoint must be react/src/main.tsx, found {}",
            envelope.box_runtime_context.entrypoint
        )));
    }
    if envelope.file_system_policy.root != "generated" {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(format!(
            "fileSystemPolicy.root must be generated, found {}",
            envelope.file_system_policy.root
        )));
    }
    if envelope.runtime_policy.runtime_kind != "react-sqlite" {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(format!(
            "runtimePolicy.runtimeKind must be react-sqlite, found {}",
            envelope.runtime_policy.runtime_kind
        )));
    }
    if envelope.runtime_policy.allowed_entrypoints.len() != 1
        || envelope.runtime_policy.allowed_entrypoints[0] != "react/src/main.tsx"
    {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
            "runtimePolicy.allowedEntrypoints must contain exactly react/src/main.tsx".to_string(),
        ));
    }
    if envelope.runtime_policy.allowed_server_bind != "127.0.0.1"
        || envelope.box_runtime_context.bind != "127.0.0.1"
    {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
            "react-sqlite envelope must bind to 127.0.0.1".to_string(),
        ));
    }
    if envelope.runtime_policy.network != "local-only"
        || envelope.box_runtime_context.network != "local-only"
    {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
            "react-sqlite envelope must stay local-only".to_string(),
        ));
    }
    if envelope.command_policy.allow_shell
        || envelope.command_policy.allow_package_install
        || envelope.command_policy.allow_global_install
    {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
            "react-sqlite envelope must not allow agent shell commands or package installs"
                .to_string(),
        ));
    }
    if envelope.file_system_policy.allow_external_files
        || envelope.file_system_policy.allow_path_traversal
    {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
            "react-sqlite envelope must not allow external files or path traversal".to_string(),
        ));
    }
    if envelope.output_contract.shell_ui_included {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
            "react-sqlite output contract must exclude Sofvary shell UI".to_string(),
        ));
    }
    if envelope.output_contract.format != "react-sqlite-project" {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(format!(
            "outputContract.format must be react-sqlite-project, found {}",
            envelope.output_contract.format
        )));
    }
    ensure_allowed_files_match_output_contract(envelope)?;

    Ok(())
}

fn ensure_allowed_files_match_output_contract(
    envelope: &PromptEnvelope,
) -> Result<(), ReactSqliteRuntimeError> {
    if envelope.file_system_policy.allowed_files.is_empty() {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
            "react-sqlite output contract must declare at least one allowed file".to_string(),
        ));
    }

    for file in envelope
        .file_system_policy
        .allowed_files
        .iter()
        .chain(envelope.output_contract.files.iter())
    {
        validate_relative_contract_file(file)?;
    }

    let expected: HashSet<&str> = envelope
        .file_system_policy
        .allowed_files
        .iter()
        .map(String::as_str)
        .collect();
    let actual: HashSet<&str> = envelope
        .output_contract
        .files
        .iter()
        .map(String::as_str)
        .collect();
    if expected == actual && expected.len() == envelope.output_contract.files.len() {
        return Ok(());
    }

    Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
        "fileSystemPolicy.allowedFiles and outputContract.files must match".to_string(),
    ))
}

fn ensure_exact_workspace_react_sqlite_files(
    manifest: &AppBoxManifest,
    allowed_files: &[String],
) -> Result<(), ReactSqliteRuntimeError> {
    let generated_root = prepare_generated_root(manifest)?;
    let expected: HashSet<String> = allowed_files.iter().cloned().collect();
    let mut actual = HashSet::new();
    collect_relative_files(&generated_root, &generated_root, &mut actual)?;

    // Dependency install runs inside generated/react; preview retries must ignore those artifacts.
    actual.retain(|path| {
        (path.starts_with("react/") || path.starts_with("data/"))
            && !is_react_sqlite_runtime_artifact(path)
    });

    if actual != expected {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(
            "generated must contain exactly the React + SQLite output contract files".to_string(),
        ));
    }

    Ok(())
}

fn is_react_sqlite_runtime_artifact(path: &str) -> bool {
    path == "data/app.sqlite"
        || path == "react/pnpm-lock.yaml"
        || path == "react/package-lock.json"
        || path == "react/yarn.lock"
        || path.starts_with("react/node_modules/")
        || path.starts_with("react/.vite/")
        || path.starts_with("react/dist/")
}

fn collect_relative_files<'a>(
    root: &'a Path,
    current: &'a Path,
    files: &mut HashSet<String>,
) -> Result<(), ReactSqliteRuntimeError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_relative_files(root, &path, files)?;
        } else if entry.file_type()?.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| ReactSqliteRuntimeError::PathEscape)?;
            files.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn validate_relative_contract_file(path: &str) -> Result<(), ReactSqliteRuntimeError> {
    if path.trim().is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "..")
        || Path::new(path).components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(ReactSqliteRuntimeError::InvalidPromptEnvelope(format!(
            "output contract file path must stay relative inside generated: {path}"
        )));
    }
    Ok(())
}

fn prepare_generated_root(manifest: &AppBoxManifest) -> Result<PathBuf, ReactSqliteRuntimeError> {
    ensure_same_path(
        &manifest.paths.generated,
        &manifest.paths.root.join("generated"),
    )?;
    ensure_same_path(&manifest.constraints.boundary, &manifest.paths.root)?;

    fs::create_dir_all(&manifest.paths.generated)?;
    let boundary = manifest.paths.root.canonicalize()?;
    let generated_root = manifest.paths.generated.canonicalize()?;
    if generated_root.starts_with(&boundary) {
        Ok(generated_root)
    } else {
        Err(ReactSqliteRuntimeError::PathEscape)
    }
}

fn workspace_sqlite_path(manifest: &AppBoxManifest) -> Result<PathBuf, ReactSqliteRuntimeError> {
    let generated_root = prepare_generated_root(manifest)?;
    let data_root = ensure_child(&generated_root, Path::new("data"))?;
    fs::create_dir_all(&data_root)?;
    let sqlite_path = ensure_child(&data_root, Path::new("app.sqlite"))?;
    let boundary = manifest.paths.root.canonicalize()?;
    let parent = sqlite_path
        .parent()
        .ok_or(ReactSqliteRuntimeError::PathEscape)?
        .canonicalize()?;
    if parent.starts_with(boundary) {
        Ok(sqlite_path)
    } else {
        Err(ReactSqliteRuntimeError::PathEscape)
    }
}

fn ensure_same_path(actual: &Path, expected: &Path) -> Result<(), ReactSqliteRuntimeError> {
    let normalized_actual = normalize_path_lexically(actual);
    let normalized_expected = normalize_path_lexically(expected);
    if normalized_actual == normalized_expected {
        Ok(())
    } else {
        Err(ReactSqliteRuntimeError::PathEscape)
    }
}

fn ensure_child(root: &Path, child: &Path) -> Result<PathBuf, ReactSqliteRuntimeError> {
    if child.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ReactSqliteRuntimeError::PathEscape);
    }

    let candidate = root.join(child);
    let normalized_root = normalize_path_lexically(root);
    let normalized_candidate = normalize_path_lexically(&candidate);
    if normalized_candidate.starts_with(normalized_root) {
        Ok(normalized_candidate)
    } else {
        Err(ReactSqliteRuntimeError::PathEscape)
    }
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
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

fn runtime_log_path(
    manifest: &AppBoxManifest,
    filename: &str,
) -> Result<PathBuf, ReactSqliteRuntimeError> {
    let logs = manifest.paths.runtime.join("logs");
    fs::create_dir_all(&logs)?;
    Ok(logs.join(filename))
}

fn write_runtime_log(path: &Path, lines: &[String]) -> Result<(), ReactSqliteRuntimeError> {
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

fn append_runtime_log(path: &Path, lines: &[String]) -> Result<(), ReactSqliteRuntimeError> {
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)?;
    for line in lines {
        writeln!(file, "{line}")?;
    }
    Ok(())
}

fn display_command(spec: &CommandSpec) -> String {
    let mut parts = vec![spec.executable.display().to_string()];
    parts.extend(spec.args.clone());
    parts.join(" ")
}

enum RuntimeWaitKind {
    Api,
    Vite,
}

fn wait_for_http(preview_url: &str, kind: RuntimeWaitKind) -> Result<(), ReactSqliteRuntimeError> {
    let address = preview_url
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(preview_url);
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if request_http(preview_url, address).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(120));
    }

    match kind {
        RuntimeWaitKind::Api => Err(ReactSqliteRuntimeError::ApiNotReady(
            preview_url.to_string(),
        )),
        RuntimeWaitKind::Vite => Err(ReactSqliteRuntimeError::DevServerNotReady(
            preview_url.to_string(),
        )),
    }
}

fn request_http(url: &str, address: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(address)?;
    let path = url
        .trim_start_matches("http://")
        .split_once('/')
        .map(|(_, path)| format!("/{path}"))
        .unwrap_or_else(|| "/".to_string());
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200") {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "non-200 response",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness_engine::PromptEnvelope;
    use crate::core::workspace_types::{WorkspaceConstraints, WorkspacePaths, WorkspacePreview};

    #[test]
    fn validates_react_sqlite_prompt_envelope_fixture() {
        let envelope: PromptEnvelope = serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/react-sqlite-prompt-envelope.golden.json"
        ))
        .expect("fixture");

        ReactSqliteRuntime::new()
            .validate_prompt_envelope(&envelope)
            .expect("valid envelope");
    }

    #[test]
    fn rejects_non_react_sqlite_prompt_envelope() {
        let mut envelope: PromptEnvelope = serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/react-sqlite-prompt-envelope.golden.json"
        ))
        .expect("fixture");
        envelope.runtime_policy.runtime_kind = "react-vite".to_string();

        let error = ReactSqliteRuntime::new()
            .validate_prompt_envelope(&envelope)
            .expect_err("invalid envelope");

        assert!(matches!(
            error,
            ReactSqliteRuntimeError::InvalidPromptEnvelope(message)
                if message.contains("runtimePolicy.runtimeKind")
        ));
    }

    #[test]
    fn workspace_sqlite_path_stays_inside_generated_data() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());

        let sqlite_path = ReactSqliteRuntime::new()
            .workspace_sqlite_path(&manifest)
            .expect("sqlite path");

        assert!(sqlite_path.ends_with("generated/data/app.sqlite"));
        assert!(sqlite_path
            .parent()
            .expect("sqlite parent")
            .canonicalize()
            .expect("canonical parent")
            .starts_with(manifest.paths.root.canonicalize().expect("canonical root")));
    }

    #[test]
    fn workspace_sqlite_path_rejects_tampered_generated_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut manifest = test_manifest(temp.path());
        manifest.paths.generated = temp.path().join("outside");

        let error = ReactSqliteRuntime::new()
            .workspace_sqlite_path(&manifest)
            .expect_err("tampered generated root");

        assert!(matches!(error, ReactSqliteRuntimeError::PathEscape));
    }

    #[test]
    fn runtime_env_includes_workspace_api_token() {
        let mut env = HashMap::new();
        insert_runtime_env(
            &mut env,
            RuntimeEnv {
                api_port: 43123,
                sqlite_path: Some(PathBuf::from("/tmp/app.sqlite")),
                api_token: Some("workspace-token".to_string()),
            },
        );

        assert_eq!(env.get("SOFVARY_API_PORT"), Some(&"43123".to_string()));
        assert_eq!(env.get("PORT"), Some(&"43123".to_string()));
        assert_eq!(
            env.get("SOFVARY_API_TOKEN"),
            Some(&"workspace-token".to_string())
        );
        assert!(env
            .get("SOFVARY_SQLITE_PATH")
            .is_some_and(|path| path.ends_with("app.sqlite")));
    }

    #[test]
    fn runtime_env_omits_standard_port_for_vite_process() {
        let mut env = HashMap::new();
        insert_runtime_env(
            &mut env,
            RuntimeEnv {
                api_port: 43123,
                sqlite_path: None,
                api_token: Some("workspace-token".to_string()),
            },
        );

        assert_eq!(env.get("SOFVARY_API_PORT"), Some(&"43123".to_string()));
        assert_eq!(env.get("PORT"), None);
    }

    #[test]
    fn exact_workspace_file_set_accepts_declared_react_and_data_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        write_allowed_files(&manifest);
        let allowed = react_sqlite_test_files()
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();

        ensure_exact_workspace_react_sqlite_files(&manifest, &allowed).expect("exact file set");
    }

    #[test]
    fn exact_workspace_file_set_allows_runtime_created_sqlite_database() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        write_allowed_files(&manifest);
        fs::write(manifest.paths.generated.join("data/app.sqlite"), "sqlite").expect("sqlite");
        let allowed = react_sqlite_test_files()
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();

        ensure_exact_workspace_react_sqlite_files(&manifest, &allowed)
            .expect("runtime sqlite artifact is allowed");
    }

    #[test]
    fn exact_workspace_file_set_allows_dependency_install_artifacts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        write_allowed_files(&manifest);
        fs::write(
            manifest.paths.generated.join("react/pnpm-lock.yaml"),
            "lockfile",
        )
        .expect("pnpm lockfile");
        let node_bin = manifest
            .paths
            .generated
            .join("react/node_modules/.bin/vite");
        fs::create_dir_all(node_bin.parent().expect("node bin parent")).expect("node bin");
        fs::write(node_bin, "vite").expect("vite bin");
        let vite_cache = manifest.paths.generated.join("react/.vite/deps/react.js");
        fs::create_dir_all(vite_cache.parent().expect("vite cache parent")).expect("vite cache");
        fs::write(vite_cache, "cache").expect("vite cache file");
        let allowed = react_sqlite_test_files()
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();

        ensure_exact_workspace_react_sqlite_files(&manifest, &allowed)
            .expect("dependency install artifacts are allowed");
    }

    #[test]
    fn better_sqlite3_compat_shim_is_written_as_runtime_artifact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());

        ensure_better_sqlite3_compat_shim(&manifest).expect("shim");

        let shim_root = manifest
            .paths
            .generated
            .join("react/node_modules/better-sqlite3");
        let package_json = fs::read_to_string(shim_root.join("package.json")).expect("package");
        let index = fs::read_to_string(shim_root.join("index.cjs")).expect("index");
        let types = fs::read_to_string(shim_root.join("index.d.ts")).expect("types");
        assert!(package_json.contains("0.0.0-sofvary-shim"));
        assert!(package_json.contains("index.d.ts"));
        assert!(index.contains("node:sqlite"));
        assert!(index.contains("lastInsertRowid") || index.contains("statement.run"));
        assert!(types.contains("declare class Database"));

        let allowed = react_sqlite_test_files()
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        write_allowed_files(&manifest);
        ensure_exact_workspace_react_sqlite_files(&manifest, &allowed)
            .expect("better-sqlite3 shim remains a runtime artifact");
    }

    #[test]
    fn exact_workspace_file_set_rejects_extra_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        write_allowed_files(&manifest);
        fs::write(manifest.paths.generated.join("data/extra.sql"), "extra").expect("extra");
        let allowed = react_sqlite_test_files()
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();

        let error = ensure_exact_workspace_react_sqlite_files(&manifest, &allowed)
            .expect_err("extra file should fail");

        assert!(matches!(
            error,
            ReactSqliteRuntimeError::InvalidPromptEnvelope(message)
                if message.contains("exactly")
        ));
    }

    fn test_manifest(root: &Path) -> AppBoxManifest {
        let root = root.join("app_test");
        let generated = root.join("generated");
        fs::create_dir_all(generated.join("react")).expect("react");
        fs::create_dir_all(generated.join("data/migrations")).expect("data");
        fs::create_dir_all(root.join("runtime/logs")).expect("runtime");

        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "react-sqlite".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: WorkspacePaths {
                root: root.clone(),
                generated: generated.clone(),
                generated_static: generated.join("static"),
                runtime: root.join("runtime"),
                snapshots: root.join("snapshots"),
            },
            constraints: WorkspaceConstraints {
                boundary: root,
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        }
    }

    fn write_allowed_files(manifest: &AppBoxManifest) {
        for relative_path in react_sqlite_test_files() {
            let target = manifest.paths.generated.join(relative_path);
            fs::create_dir_all(target.parent().expect("parent")).expect("parent dir");
            fs::write(target, format!("file: {relative_path}")).expect("file");
        }
    }

    fn react_sqlite_test_files() -> Vec<&'static str> {
        vec![
            "react/package.json",
            "react/index.html",
            "react/vite.config.ts",
            "react/tsconfig.json",
            "react/src/main.tsx",
            "react/src/App.tsx",
            "react/src/components/CustomerManager.tsx",
            "react/src/styles/app.css",
            "react/server/index.ts",
            "react/server/db.ts",
            "react/server/routes/customers.ts",
            "data/schema.json",
            "data/migrations/001_create_customers.sql",
            "data/seed.sql",
        ]
    }
}
