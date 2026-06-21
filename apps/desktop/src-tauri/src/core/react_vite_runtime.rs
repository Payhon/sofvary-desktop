use crate::core::harness_engine::{PromptEnvelope, REACT_VITE_ALLOWED_FILES};
use crate::core::pack_types::{PackCommandSpec, RuntimePackManifest};
use crate::core::policy_engine::{PolicyEngine, PolicyError};
use crate::core::policy_types::{PolicyApprovalSet, PolicyCommandRequest};
use crate::core::runtime_dependency_install::{
    dependency_install_recovery_spec, is_offline_dependency_cache_failure,
};
use crate::core::workspace_types::{AppBoxManifest, RuntimeMode};
use crate::platform::{current_adapter, CommandSpec, PlatformAdapter, PlatformError};
use mime_guess::from_path;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactViteRuntimePreview {
    pub preview_url: String,
    pub logs: Vec<String>,
}

pub struct ReactViteRuntimeServer {
    preview: ReactViteRuntimePreview,
    kind: ReactViteRuntimeServerKind,
}

enum ReactViteRuntimeServerKind {
    Dev {
        pid: u32,
    },
    Dist {
        stop_requested: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    },
}

impl ReactViteRuntimeServer {
    pub fn preview(&self) -> ReactViteRuntimePreview {
        self.preview.clone()
    }

    pub fn stop(&mut self) {
        match &mut self.kind {
            ReactViteRuntimeServerKind::Dev { pid } => {
                let _ = current_adapter().kill_process_tree(*pid);
            }
            ReactViteRuntimeServerKind::Dist {
                stop_requested,
                thread,
            } => {
                stop_requested.store(true, Ordering::SeqCst);
                if let Some(thread) = thread.take() {
                    let _ = thread.join();
                }
            }
        }
    }
}

impl Drop for ReactViteRuntimeServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Error)]
pub enum ReactViteRuntimeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),
    #[error("request attempted to escape react runtime root")]
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
    #[error("Vite dev server did not become ready at {0}")]
    DevServerNotReady(String),
}

#[derive(Default)]
pub struct ReactViteRuntime;

impl ReactViteRuntime {
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
    ) -> Result<ReactViteRuntimeServer, ReactViteRuntimeError> {
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
    ) -> Result<ReactViteRuntimeServer, ReactViteRuntimeError> {
        self.validate_prompt_envelope(envelope)?;
        ensure_exact_workspace_react_files(manifest, &envelope.output_contract.files)?;

        match mode {
            RuntimeMode::Dev => self.start_dev_workspace(manifest, runtime_pack, approvals),
            RuntimeMode::Prod => self.start_prod_workspace(manifest, runtime_pack, approvals),
        }
    }

    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), ReactViteRuntimeError> {
        validate_prompt_envelope(envelope)
    }

    #[allow(dead_code)]
    pub fn command_spec_from_pack(
        &self,
        manifest: &AppBoxManifest,
        command: &PackCommandSpec,
        port: Option<u16>,
        adapter: &dyn PlatformAdapter,
    ) -> Result<CommandSpec, ReactViteRuntimeError> {
        command_spec_from_pack(manifest, command, port, adapter)
    }

    #[allow(dead_code)]
    pub fn start_verified_react_project(
        &self,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
    ) -> Result<ReactViteRuntimeServer, ReactViteRuntimeError> {
        self.start_verified_react_project_with_policy(
            manifest,
            runtime_pack,
            mode,
            &PolicyApprovalSet::default(),
        )
    }

    pub fn start_verified_react_project_with_policy(
        &self,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        mode: RuntimeMode,
        approvals: &PolicyApprovalSet,
    ) -> Result<ReactViteRuntimeServer, ReactViteRuntimeError> {
        match mode {
            RuntimeMode::Dev => self.start_dev_workspace(manifest, runtime_pack, approvals),
            RuntimeMode::Prod => self.start_prod_workspace(manifest, runtime_pack, approvals),
        }
    }

    fn start_dev_workspace(
        &self,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<ReactViteRuntimeServer, ReactViteRuntimeError> {
        let adapter = current_adapter();
        let port = adapter.allocate_local_port()?;
        let install = required_command(runtime_pack, "install")?;
        let dev = required_command(runtime_pack, "dev")?;
        let install_spec = command_spec_from_pack(manifest, install, Some(port), adapter.as_ref())?;
        let dev_spec = command_spec_from_pack(manifest, dev, Some(port), adapter.as_ref())?;
        let log_path = runtime_log_path(manifest, "react-vite-dev.log")?;

        write_runtime_log(
            &log_path,
            &[
                "ReactViteRuntime dev mode".to_string(),
                format!("Install command: {}", display_command(&install_spec)),
                format!("Dev command: {}", display_command(&dev_spec)),
            ],
        )?;

        run_dependency_install(adapter.as_ref(), install_spec, approvals, &log_path)?;

        enforce_command("dev", &dev_spec, approvals)?;
        let handle = adapter.spawn_process(dev_spec.clone())?;
        append_runtime_log(&log_path, &[format!("Vite dev server pid: {}", handle.pid)])?;

        let preview_url = format!("http://127.0.0.1:{port}/");
        if let Err(error) = wait_for_dev_server(&preview_url) {
            let _ = adapter.kill_process_tree(handle.pid);
            append_runtime_log(
                &log_path,
                &[format!(
                    "Vite dev server readiness failed; killed pid {}",
                    handle.pid
                )],
            )?;
            return Err(error);
        }

        Ok(ReactViteRuntimeServer {
            preview: ReactViteRuntimePreview {
                preview_url,
                logs: vec![
                    format!(
                        "ReactViteRuntime dev server started for {}",
                        manifest.app_id
                    ),
                    format!(
                        "Generated root {}",
                        manifest.paths.generated.join("react").display()
                    ),
                    format!("Dev command: {}", display_command(&dev_spec)),
                    format!("Vite process pid {}", handle.pid),
                    format!("Runtime log {}", log_path.display()),
                ],
            },
            kind: ReactViteRuntimeServerKind::Dev { pid: handle.pid },
        })
    }

    fn start_prod_workspace(
        &self,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        approvals: &PolicyApprovalSet,
    ) -> Result<ReactViteRuntimeServer, ReactViteRuntimeError> {
        let adapter = current_adapter();
        let install = required_command(runtime_pack, "install")?;
        let build = required_command(runtime_pack, "build")?;
        let install_spec = command_spec_from_pack(manifest, install, None, adapter.as_ref())?;
        let build_spec = command_spec_from_pack(manifest, build, None, adapter.as_ref())?;
        let log_path = runtime_log_path(manifest, "react-vite-prod.log")?;

        write_runtime_log(
            &log_path,
            &[
                "ReactViteRuntime prod mode".to_string(),
                format!("Install command: {}", display_command(&install_spec)),
                format!("Build command: {}", display_command(&build_spec)),
            ],
        )?;

        enforce_command("build", &build_spec, approvals)?;

        run_dependency_install(adapter.as_ref(), install_spec, approvals, &log_path)?;
        let output = adapter.run_process(build_spec)?;
        if output.status_code != Some(0) {
            append_runtime_log(
                &log_path,
                &[
                    "build command failed".to_string(),
                    output.stdout.clone(),
                    output.stderr.clone(),
                ],
            )?;
            return Err(ReactViteRuntimeError::CommandFailed {
                name: "build".to_string(),
                status: output.status_code,
                stdout: output.stdout,
                stderr: output.stderr,
                log_path: log_path.clone(),
            });
        }

        self.start_dist_server(manifest, log_path)
    }

    fn start_dist_server(
        &self,
        manifest: &AppBoxManifest,
        log_path: PathBuf,
    ) -> Result<ReactViteRuntimeServer, ReactViteRuntimeError> {
        let adapter = current_adapter();
        let port = adapter.allocate_local_port()?;
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        listener.set_nonblocking(true)?;
        let dist_root = prepare_dist_root(manifest)?;
        let served_root = dist_root.clone();
        let stop_requested = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop_requested);

        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = handle_dist_connection(stream, &served_root);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(ReactViteRuntimeServer {
            preview: ReactViteRuntimePreview {
                preview_url: format!("http://127.0.0.1:{port}/"),
                logs: vec![
                    format!(
                        "ReactViteRuntime prod server started for {}",
                        manifest.app_id
                    ),
                    format!("Serving {}", dist_root.display()),
                    format!("Bound to 127.0.0.1:{port}"),
                    format!("Runtime log {}", log_path.display()),
                ],
            },
            kind: ReactViteRuntimeServerKind::Dist {
                stop_requested,
                thread: Some(thread),
            },
        })
    }
}

fn required_command<'a>(
    runtime_pack: &'a RuntimePackManifest,
    name: &str,
) -> Result<&'a PackCommandSpec, ReactViteRuntimeError> {
    runtime_pack
        .commands
        .get(name)
        .ok_or_else(|| ReactViteRuntimeError::MissingCommand {
            name: name.to_string(),
            pack_id: runtime_pack.id.clone(),
            pack_version: runtime_pack.version.clone(),
        })
}

fn enforce_dependency_install(
    name: &str,
    spec: &CommandSpec,
    approvals: &PolicyApprovalSet,
) -> Result<(), ReactViteRuntimeError> {
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
) -> Result<(), ReactViteRuntimeError> {
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
            return Err(ReactViteRuntimeError::CommandFailed {
                name: "install".to_string(),
                status: recovery_output.status_code,
                stdout: recovery_output.stdout,
                stderr: recovery_output.stderr,
                log_path: log_path.to_path_buf(),
            });
        }
    }

    Err(ReactViteRuntimeError::CommandFailed {
        name: "install".to_string(),
        status: install_output.status_code,
        stdout: install_output.stdout,
        stderr: install_output.stderr,
        log_path: log_path.to_path_buf(),
    })
}

fn enforce_command(
    name: &str,
    spec: &CommandSpec,
    approvals: &PolicyApprovalSet,
) -> Result<(), ReactViteRuntimeError> {
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
    adapter: &dyn PlatformAdapter,
) -> Result<CommandSpec, ReactViteRuntimeError> {
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

    Ok(CommandSpec {
        executable,
        args,
        cwd,
        env: command.env.clone().unwrap_or_default(),
        allowed_network: command.allowed_network.unwrap_or(false),
        timeout_ms: command.timeout_ms,
        kill_on_drop: command.kill_on_drop.unwrap_or(true),
    })
}

fn resolve_executable(
    executable: &str,
    adapter: &dyn PlatformAdapter,
) -> Result<PathBuf, ReactViteRuntimeError> {
    if let Some(name) = executable
        .strip_prefix("${sidecar:")
        .and_then(|value| value.strip_suffix('}'))
    {
        return Ok(adapter.resolve_sidecar_executable(name)?);
    }

    Err(ReactViteRuntimeError::InvalidCommandSpec(format!(
        "runtime command executable must use a sidecar placeholder, found '{executable}'"
    )))
}

fn render_command_arg(arg: &str, port: Option<u16>) -> String {
    match port {
        Some(port) => arg.replace("${port}", &port.to_string()),
        None => arg.to_string(),
    }
}

fn validate_prompt_envelope(envelope: &PromptEnvelope) -> Result<(), ReactViteRuntimeError> {
    if envelope.box_runtime_context.runtime_kind != "react-vite" {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(format!(
            "runtimeKind must be react-vite, found {}",
            envelope.box_runtime_context.runtime_kind
        )));
    }
    if envelope.box_runtime_context.generated_root != "generated/react" {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(format!(
            "generatedRoot must be generated/react, found {}",
            envelope.box_runtime_context.generated_root
        )));
    }
    if envelope.box_runtime_context.entrypoint != "src/main.tsx" {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(format!(
            "entrypoint must be src/main.tsx, found {}",
            envelope.box_runtime_context.entrypoint
        )));
    }
    if envelope.file_system_policy.root != "generated/react" {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(format!(
            "fileSystemPolicy.root must be generated/react, found {}",
            envelope.file_system_policy.root
        )));
    }
    if envelope.runtime_policy.runtime_kind != "react-vite" {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(format!(
            "runtimePolicy.runtimeKind must be react-vite, found {}",
            envelope.runtime_policy.runtime_kind
        )));
    }
    if envelope.runtime_policy.allowed_entrypoints.len() != 1
        || envelope.runtime_policy.allowed_entrypoints[0] != "src/main.tsx"
    {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(
            "runtimePolicy.allowedEntrypoints must contain exactly src/main.tsx".to_string(),
        ));
    }
    if envelope.runtime_policy.allowed_server_bind != "127.0.0.1"
        || envelope.box_runtime_context.bind != "127.0.0.1"
    {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(
            "react-vite envelope must bind to 127.0.0.1".to_string(),
        ));
    }
    if envelope.runtime_policy.network != "local-only"
        || envelope.box_runtime_context.network != "local-only"
    {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(
            "react-vite envelope must stay local-only".to_string(),
        ));
    }
    if envelope.command_policy.allow_shell
        || envelope.command_policy.allow_package_install
        || envelope.command_policy.allow_global_install
    {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(
            "react-vite envelope must not allow agent shell commands or package installs"
                .to_string(),
        ));
    }
    if envelope.file_system_policy.allow_external_files
        || envelope.file_system_policy.allow_path_traversal
    {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(
            "react-vite envelope must not allow external files or path traversal".to_string(),
        ));
    }
    if envelope.output_contract.shell_ui_included {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(
            "react-vite output contract must exclude Sofvary shell UI".to_string(),
        ));
    }
    if envelope.output_contract.format != "react-vite-project" {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(format!(
            "outputContract.format must be react-vite-project, found {}",
            envelope.output_contract.format
        )));
    }
    ensure_exact_react_files(
        "fileSystemPolicy.allowedFiles",
        &envelope.file_system_policy.allowed_files,
    )?;
    ensure_exact_react_files("outputContract.files", &envelope.output_contract.files)?;

    Ok(())
}

fn ensure_exact_react_files(field: &str, files: &[String]) -> Result<(), ReactViteRuntimeError> {
    let expected: HashSet<&str> = REACT_VITE_ALLOWED_FILES.iter().copied().collect();
    let actual: HashSet<&str> = files.iter().map(String::as_str).collect();
    if expected == actual && files.len() == REACT_VITE_ALLOWED_FILES.len() {
        Ok(())
    } else {
        Err(ReactViteRuntimeError::InvalidPromptEnvelope(format!(
            "{field} must contain exactly the React + Vite project file set"
        )))
    }
}

fn ensure_exact_workspace_react_files(
    manifest: &AppBoxManifest,
    allowed_files: &[String],
) -> Result<(), ReactViteRuntimeError> {
    ensure_exact_react_files("workspace.generatedReact", allowed_files)?;
    let react_root = prepare_react_root(manifest)?;
    let expected: HashSet<String> = allowed_files.iter().cloned().collect();
    let mut actual = HashSet::new();
    collect_relative_files(&react_root, &react_root, &mut actual)?;

    if actual != expected {
        return Err(ReactViteRuntimeError::InvalidPromptEnvelope(
            "generated/react must contain exactly the React + Vite output contract files"
                .to_string(),
        ));
    }

    Ok(())
}

fn collect_relative_files<'a>(
    root: &'a Path,
    current: &'a Path,
    files: &mut HashSet<String>,
) -> Result<(), ReactViteRuntimeError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_relative_files(root, &path, files)?;
        } else if entry.file_type()?.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| ReactViteRuntimeError::PathEscape)?;
            files.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn prepare_react_root(manifest: &AppBoxManifest) -> Result<PathBuf, ReactViteRuntimeError> {
    ensure_same_path(
        &manifest.paths.generated,
        &manifest.paths.root.join("generated"),
    )?;
    ensure_same_path(&manifest.constraints.boundary, &manifest.paths.root)?;

    let react_root = manifest.paths.generated.join("react");
    fs::create_dir_all(&react_root)?;
    let boundary = manifest.paths.root.canonicalize()?;
    let react_root = react_root.canonicalize()?;
    if react_root.starts_with(&boundary) {
        Ok(react_root)
    } else {
        Err(ReactViteRuntimeError::PathEscape)
    }
}

fn prepare_dist_root(manifest: &AppBoxManifest) -> Result<PathBuf, ReactViteRuntimeError> {
    let dist_root = prepare_react_root(manifest)?.join("dist");
    if dist_root.exists() {
        let canonical_react = prepare_react_root(manifest)?;
        let canonical_dist = dist_root.canonicalize()?;
        if canonical_dist.starts_with(canonical_react) {
            return Ok(canonical_dist);
        }
    }

    Err(ReactViteRuntimeError::InvalidPromptEnvelope(
        "generated/react/dist does not exist after build".to_string(),
    ))
}

fn ensure_same_path(actual: &Path, expected: &Path) -> Result<(), ReactViteRuntimeError> {
    let normalized_actual = normalize_path_lexically(actual);
    let normalized_expected = normalize_path_lexically(expected);
    if normalized_actual == normalized_expected {
        Ok(())
    } else {
        Err(ReactViteRuntimeError::PathEscape)
    }
}

fn ensure_child(root: &Path, child: &Path) -> Result<PathBuf, ReactViteRuntimeError> {
    if child.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ReactViteRuntimeError::PathEscape);
    }

    let candidate = root.join(child);
    let normalized_root = normalize_path_lexically(root);
    let normalized_candidate = normalize_path_lexically(&candidate);
    if normalized_candidate.starts_with(normalized_root) {
        Ok(normalized_candidate)
    } else {
        Err(ReactViteRuntimeError::PathEscape)
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
) -> Result<PathBuf, ReactViteRuntimeError> {
    let logs = manifest.paths.runtime.join("logs");
    fs::create_dir_all(&logs)?;
    Ok(logs.join(filename))
}

fn write_runtime_log(path: &Path, lines: &[String]) -> Result<(), ReactViteRuntimeError> {
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

fn append_runtime_log(path: &Path, lines: &[String]) -> Result<(), ReactViteRuntimeError> {
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

fn wait_for_dev_server(preview_url: &str) -> Result<(), ReactViteRuntimeError> {
    let address = preview_url
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if TcpStream::connect(address).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(120));
    }

    Err(ReactViteRuntimeError::DevServerNotReady(
        preview_url.to_string(),
    ))
}

fn handle_dist_connection(
    mut stream: TcpStream,
    dist_root: &Path,
) -> Result<(), ReactViteRuntimeError> {
    let mut buffer = [0_u8; 2048];
    let bytes = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..bytes]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let file_path = resolve_dist_path(dist_root, path)?;
    if file_path.is_file() {
        let body = fs::read(&file_path)?;
        let content_type = from_path(&file_path).first_or_octet_stream();
        write_response(
            &mut stream,
            "200 OK",
            content_type.as_ref(),
            body.as_slice(),
        )?;
    } else {
        write_response(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"Not Found",
        )?;
    }
    Ok(())
}

fn resolve_dist_path(
    dist_root: &Path,
    request_path: &str,
) -> Result<PathBuf, ReactViteRuntimeError> {
    let path_without_query = request_path.split('?').next().unwrap_or("/");
    let relative = path_without_query.trim_start_matches('/');
    let relative = if relative.is_empty() {
        PathBuf::from("index.html")
    } else {
        PathBuf::from(relative)
    };

    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ReactViteRuntimeError::PathEscape);
    }

    let candidate = dist_root.join(relative);
    if !candidate.exists() {
        return Ok(candidate);
    }

    let canonical_root = dist_root.canonicalize()?;
    let canonical_candidate = candidate.canonicalize()?;
    if canonical_candidate.starts_with(canonical_root) {
        Ok(canonical_candidate)
    } else {
        Err(ReactViteRuntimeError::PathEscape)
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), ReactViteRuntimeError> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness_engine::{
        BoxRuntimeContext, CommandPolicy, CurrentAppState, FileSystemPolicy, HarnessPolicy,
        OutputContract, PackReference, RuntimePolicy,
    };
    use crate::core::pack_manager::parse_runtime_pack_manifest;
    use crate::core::workspace_types::{
        RuntimeKind, WorkspaceConstraints, WorkspacePaths, WorkspacePreview,
    };
    use crate::platform::{
        ArchKind, OsKind, PlatformDirs, PlatformResult, ProcessHandle, ProcessOutput,
        WebviewProfile, WorkArea,
    };

    const REACT_RUNTIME_MANIFEST: &str =
        include_str!("../../builtin-packs/runtimes/sofvary.runtime.react-vite/0.1.0/manifest.json");

    struct TestAdapter;

    impl PlatformAdapter for TestAdapter {
        fn os(&self) -> OsKind {
            OsKind::Macos
        }

        fn arch(&self) -> ArchKind {
            ArchKind::Arm64
        }

        fn dirs(&self) -> PlatformResult<PlatformDirs> {
            Ok(PlatformDirs {
                data_dir: PathBuf::from("/tmp/sofvary-data"),
                cache_dir: PathBuf::from("/tmp/sofvary-cache"),
                config_dir: PathBuf::from("/tmp/sofvary-config"),
            })
        }

        fn normalize_path(&self, input: &str) -> PlatformResult<PathBuf> {
            Ok(PathBuf::from(input))
        }

        fn ensure_executable(&self, _path: &Path) -> PlatformResult<()> {
            Ok(())
        }

        fn resolve_sidecar_executable(&self, name: &str) -> PlatformResult<PathBuf> {
            Ok(PathBuf::from(name))
        }

        fn run_process(&self, _spec: CommandSpec) -> PlatformResult<ProcessOutput> {
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn spawn_process(&self, _spec: CommandSpec) -> PlatformResult<ProcessHandle> {
            Ok(ProcessHandle { pid: 42 })
        }

        fn kill_process_tree(&self, _pid: u32) -> PlatformResult<()> {
            Ok(())
        }

        fn allocate_local_port(&self) -> PlatformResult<u16> {
            Ok(5173)
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
                height: 800,
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

    #[test]
    fn validates_react_vite_prompt_envelope() {
        let runtime = ReactViteRuntime::new();
        runtime
            .validate_prompt_envelope(&test_envelope())
            .expect("valid envelope");
    }

    #[test]
    fn rejects_react_vite_prompt_envelope_with_static_runtime_kind() {
        let runtime = ReactViteRuntime::new();
        let mut envelope = test_envelope();
        envelope.runtime_policy.runtime_kind = "static-html".to_string();

        let result = runtime.validate_prompt_envelope(&envelope);

        assert!(matches!(
            result,
            Err(ReactViteRuntimeError::InvalidPromptEnvelope(_))
        ));
    }

    #[test]
    fn generates_structured_dev_command_spec_from_runtime_pack() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime_pack = parse_runtime_pack_manifest(REACT_RUNTIME_MANIFEST).expect("runtime");
        let runtime = ReactViteRuntime::new();
        let command = runtime_pack.commands.get("dev").expect("dev command");

        let spec = runtime
            .command_spec_from_pack(&manifest, command, Some(4242), &TestAdapter)
            .expect("command spec");

        assert_eq!(spec.executable, PathBuf::from("pnpm"));
        assert_eq!(
            spec.args,
            ["exec", "vite", "--host", "127.0.0.1", "--port", "4242"]
        );
        assert_eq!(spec.cwd, manifest.paths.generated.join("react"));
        assert!(!spec.allowed_network);
        assert!(spec.kill_on_drop);
    }

    #[test]
    fn rejects_runtime_command_without_sidecar_placeholder() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime_pack = parse_runtime_pack_manifest(REACT_RUNTIME_MANIFEST).expect("runtime");
        let mut command = runtime_pack
            .commands
            .get("dev")
            .expect("dev command")
            .clone();
        command.executable = "pnpm".to_string();
        let runtime = ReactViteRuntime::new();

        let result = runtime.command_spec_from_pack(&manifest, &command, Some(4242), &TestAdapter);

        assert!(matches!(
            result,
            Err(ReactViteRuntimeError::InvalidCommandSpec(message))
                if message.contains("sidecar placeholder")
        ));
    }

    #[test]
    fn rejects_dist_path_escape() {
        let root = PathBuf::from("/tmp/sofvary/dist");
        let result = resolve_dist_path(&root, "/../secret.txt");
        assert!(matches!(result, Err(ReactViteRuntimeError::PathEscape)));
    }

    fn test_manifest(root: &Path) -> AppBoxManifest {
        let generated = root.join("generated");
        fs::create_dir_all(generated.join("react")).expect("react root");
        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: RuntimeKind::ReactVite,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: WorkspacePaths {
                root: root.to_path_buf(),
                generated: generated.clone(),
                generated_static: generated.join("static"),
                runtime: root.join("runtime"),
                snapshots: root.join("snapshots"),
            },
            constraints: WorkspaceConstraints {
                boundary: root.to_path_buf(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        }
    }

    fn test_envelope() -> PromptEnvelope {
        PromptEnvelope {
            schema_version: "1.0".to_string(),
            envelope_id: "penv_react_test".to_string(),
            created_at: "now".to_string(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: "sofvary.runtime.react-vite".to_string(),
                    version: "0.1.0".to_string(),
                },
                harness_packs: vec![PackReference {
                    id: "sofvary.harness.react-vite".to_string(),
                    version: "0.1.0".to_string(),
                }],
                runtime_kind: "react-vite".to_string(),
                generated_root: "generated/react".to_string(),
                entrypoint: "src/main.tsx".to_string(),
                bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
            },
            user_intent: "Build a task board".to_string(),
            current_app_state: CurrentAppState {
                app_id: "app_test".to_string(),
                workspace_name: "Test".to_string(),
                mode: "create".to_string(),
                existing_files: Vec::new(),
                file_context: Vec::new(),
                preview_state: "empty".to_string(),
            },
            runtime_policy: RuntimePolicy {
                runtime_kind: "react-vite".to_string(),
                allowed_entrypoints: vec!["src/main.tsx".to_string()],
                allowed_server_bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
                package_install: false,
            },
            harness_policy: HarnessPolicy {
                system_instructions: Vec::new(),
                file_system_rules: Vec::new(),
                output_rules: Vec::new(),
                blocked_capabilities: Vec::new(),
            },
            file_system_policy: FileSystemPolicy {
                root: "generated/react".to_string(),
                allowed_files: REACT_VITE_ALLOWED_FILES
                    .iter()
                    .map(|value| value.to_string())
                    .collect(),
                allow_external_files: false,
                allow_path_traversal: false,
            },
            command_policy: CommandPolicy {
                allow_shell: false,
                allow_package_install: false,
                allow_global_install: false,
                allowed_commands: Vec::new(),
            },
            output_contract: OutputContract {
                format: "react-vite-project".to_string(),
                files: REACT_VITE_ALLOWED_FILES
                    .iter()
                    .map(|value| value.to_string())
                    .collect(),
                shell_ui_included: false,
            },
            acceptance_criteria: Vec::new(),
        }
    }
}
