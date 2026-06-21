use crate::core::harness_engine::{PromptEnvelope, CANVAS2D_ALLOWED_FILES};
use crate::core::workspace_types::AppBoxManifest;
use crate::platform::current_adapter;
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
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Canvas2dRuntimePreview {
    pub preview_url: String,
    pub logs: Vec<String>,
}

pub struct Canvas2dRuntimeServer {
    preview: Canvas2dRuntimePreview,
    stop_requested: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Canvas2dRuntimeServer {
    pub fn preview(&self) -> Canvas2dRuntimePreview {
        self.preview.clone()
    }

    pub fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Canvas2dRuntimeServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Error)]
pub enum Canvas2dRuntimeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("platform error: {0}")]
    Platform(#[from] crate::platform::PlatformError),
    #[error("request attempted to escape canvas2d root")]
    PathEscape,
    #[error("invalid prompt envelope: {0}")]
    InvalidPromptEnvelope(String),
}

#[derive(Default)]
pub struct Canvas2dRuntime;

impl Canvas2dRuntime {
    pub fn new() -> Self {
        Self
    }

    pub fn start_workspace_with_envelope(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
    ) -> Result<Canvas2dRuntimeServer, Canvas2dRuntimeError> {
        self.validate_prompt_envelope(envelope)?;
        ensure_exact_workspace_canvas2d_files(manifest, &envelope.output_contract.files)?;
        self.start_workspace(manifest)
    }

    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), Canvas2dRuntimeError> {
        validate_prompt_envelope(envelope)
    }

    pub fn start_workspace(
        &self,
        manifest: &AppBoxManifest,
    ) -> Result<Canvas2dRuntimeServer, Canvas2dRuntimeError> {
        let adapter = current_adapter();
        let port = adapter.allocate_local_port()?;
        let bind_addr = ("127.0.0.1", port);
        let listener = TcpListener::bind(bind_addr)?;
        listener.set_nonblocking(true)?;
        let canvas_root = prepare_canvas_root(manifest)?;
        let served_root = canvas_root.clone();
        let app_id = manifest.app_id.clone();
        let stop_requested = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop_requested);

        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = handle_connection(stream, &served_root);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => break,
                }
            }
        });

        let preview = Canvas2dRuntimePreview {
            preview_url: format!("http://127.0.0.1:{port}/"),
            logs: vec![
                format!("Canvas2dRuntime started for {app_id}"),
                format!("Serving {}", canvas_root.display()),
                format!("Bound to 127.0.0.1:{port}"),
            ],
        };

        Ok(Canvas2dRuntimeServer {
            preview,
            stop_requested,
            thread: Some(thread),
        })
    }
}

fn handle_connection(
    mut stream: TcpStream,
    canvas_root: &Path,
) -> Result<(), Canvas2dRuntimeError> {
    let mut buffer = [0_u8; 2048];
    let bytes = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..bytes]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let file_path = match resolve_canvas_path(canvas_root, path) {
        Ok(path) => path,
        Err(Canvas2dRuntimeError::PathEscape) => {
            write_response(
                &mut stream,
                "403 Forbidden",
                "text/plain; charset=utf-8",
                b"Forbidden",
            )?;
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if file_path.is_file() {
        let body = fs::read(&file_path)?;
        let content_type = mime_guess::from_path(&file_path).first_or_octet_stream();
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

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), Canvas2dRuntimeError> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

fn resolve_canvas_path(
    canvas_root: &Path,
    request_path: &str,
) -> Result<PathBuf, Canvas2dRuntimeError> {
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
        return Err(Canvas2dRuntimeError::PathEscape);
    }

    let relative_name = relative.to_string_lossy();
    if !CANVAS2D_ALLOWED_FILES
        .iter()
        .any(|allowed| *allowed == relative_name)
    {
        return Ok(canvas_root.join("__sofvary_not_found__"));
    }

    let candidate = canvas_root.join(relative);
    if !candidate.exists() {
        return Ok(candidate);
    }

    let canonical_root = canvas_root.canonicalize()?;
    let canonical_candidate = candidate.canonicalize()?;
    if canonical_candidate.starts_with(&canonical_root) {
        Ok(canonical_candidate)
    } else {
        Err(Canvas2dRuntimeError::PathEscape)
    }
}

fn prepare_canvas_root(manifest: &AppBoxManifest) -> Result<PathBuf, Canvas2dRuntimeError> {
    ensure_same_path(
        &manifest.paths.generated,
        &manifest.paths.root.join("generated"),
    )?;
    ensure_same_path(&manifest.constraints.boundary, &manifest.paths.root)?;

    let expected = manifest.paths.root.join("generated").join("canvas");
    fs::create_dir_all(&expected)?;
    let boundary = manifest.paths.root.canonicalize()?;
    let canvas_root = expected.canonicalize()?;
    if canvas_root.starts_with(&boundary) {
        Ok(canvas_root)
    } else {
        Err(Canvas2dRuntimeError::PathEscape)
    }
}

fn validate_prompt_envelope(envelope: &PromptEnvelope) -> Result<(), Canvas2dRuntimeError> {
    if envelope.box_runtime_context.runtime_kind != "canvas2d" {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
            "runtimeKind must be canvas2d, found {}",
            envelope.box_runtime_context.runtime_kind
        )));
    }
    if envelope.box_runtime_context.generated_root != "generated/canvas" {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
            "generatedRoot must be generated/canvas, found {}",
            envelope.box_runtime_context.generated_root
        )));
    }
    if envelope.box_runtime_context.entrypoint != "index.html" {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
            "entrypoint must be index.html, found {}",
            envelope.box_runtime_context.entrypoint
        )));
    }
    if envelope.file_system_policy.root != "generated/canvas" {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
            "fileSystemPolicy.root must be generated/canvas, found {}",
            envelope.file_system_policy.root
        )));
    }
    if envelope.runtime_policy.runtime_kind != "canvas2d" {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
            "runtimePolicy.runtimeKind must be canvas2d, found {}",
            envelope.runtime_policy.runtime_kind
        )));
    }
    if envelope.runtime_policy.allowed_entrypoints.len() != 1
        || envelope.runtime_policy.allowed_entrypoints[0] != "index.html"
    {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(
            "runtimePolicy.allowedEntrypoints must contain exactly index.html".to_string(),
        ));
    }
    if envelope.runtime_policy.allowed_server_bind != "127.0.0.1" {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
            "runtimePolicy.allowedServerBind must be 127.0.0.1, found {}",
            envelope.runtime_policy.allowed_server_bind
        )));
    }
    if envelope.runtime_policy.network != "local-only"
        || envelope.box_runtime_context.network != "local-only"
        || envelope.box_runtime_context.bind != "127.0.0.1"
    {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(
            "canvas2d envelope must stay local-only and bind to 127.0.0.1".to_string(),
        ));
    }
    if envelope.command_policy.allow_shell
        || envelope.command_policy.allow_package_install
        || envelope.command_policy.allow_global_install
        || envelope.runtime_policy.package_install
    {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(
            "canvas2d envelope must not allow shell commands or package installs".to_string(),
        ));
    }
    if !envelope.command_policy.allowed_commands.is_empty() {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(
            "canvas2d envelope must not allow runtime commands".to_string(),
        ));
    }
    if envelope.file_system_policy.allow_external_files
        || envelope.file_system_policy.allow_path_traversal
    {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(
            "canvas2d envelope must not allow external files or path traversal".to_string(),
        ));
    }
    if envelope.output_contract.shell_ui_included {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(
            "canvas2d output contract must exclude Sofvary shell UI".to_string(),
        ));
    }
    if envelope.output_contract.format != "canvas2d-project" {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
            "outputContract.format must be canvas2d-project, found {}",
            envelope.output_contract.format
        )));
    }
    ensure_exact_canvas2d_files(
        "fileSystemPolicy.allowedFiles",
        &envelope.file_system_policy.allowed_files,
    )?;
    ensure_exact_canvas2d_files("outputContract.files", &envelope.output_contract.files)?;

    Ok(())
}

fn ensure_exact_canvas2d_files(field: &str, files: &[String]) -> Result<(), Canvas2dRuntimeError> {
    let expected: HashSet<&str> = CANVAS2D_ALLOWED_FILES.iter().copied().collect();
    let actual: HashSet<&str> = files.iter().map(String::as_str).collect();
    if expected == actual && files.len() == CANVAS2D_ALLOWED_FILES.len() {
        Ok(())
    } else {
        Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
            "{field} must contain exactly the Canvas 2D output contract"
        )))
    }
}

fn ensure_exact_workspace_canvas2d_files(
    manifest: &AppBoxManifest,
    allowed_files: &[String],
) -> Result<(), Canvas2dRuntimeError> {
    ensure_exact_canvas2d_files("workspace.generatedCanvas", allowed_files)?;
    let canvas_root = prepare_canvas_root(manifest)?;
    let expected: HashSet<String> = allowed_files.iter().cloned().collect();
    let mut actual = HashSet::new();
    collect_canvas_files(&canvas_root, &canvas_root, &mut actual)?;

    if actual != expected {
        return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(
            "generated/canvas must contain exactly the Canvas 2D output contract".to_string(),
        ));
    }

    Ok(())
}

fn collect_canvas_files(
    root: &Path,
    current: &Path,
    files: &mut HashSet<String>,
) -> Result<(), Canvas2dRuntimeError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_canvas_files(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Canvas2dRuntimeError::PathEscape)?
                .to_string_lossy()
                .replace('\\', "/");
            if !CANVAS2D_ALLOWED_FILES
                .iter()
                .any(|allowed| *allowed == relative)
            {
                return Err(Canvas2dRuntimeError::InvalidPromptEnvelope(format!(
                    "generated/canvas contains undeclared file {relative}"
                )));
            }
            files.insert(relative);
        }
    }

    Ok(())
}

fn ensure_same_path(actual: &Path, expected: &Path) -> Result<(), Canvas2dRuntimeError> {
    let normalized_actual = normalize_path_lexically(actual);
    let normalized_expected = normalize_path_lexically(expected);
    if normalized_actual == normalized_expected {
        Ok(())
    } else {
        Err(Canvas2dRuntimeError::PathEscape)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness_engine::{
        BoxRuntimeContext, CommandPolicy, CurrentAppState, FileSystemPolicy, HarnessPolicy,
        OutputContract, PackReference, RuntimePolicy,
    };
    use crate::core::workspace_types::{
        AppBoxManifest, RuntimeKind, WorkspaceConstraints, WorkspacePaths, WorkspacePreview,
    };
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};

    #[test]
    fn rejects_canvas_path_escape() {
        let root = PathBuf::from("/tmp/sofvary/canvas");
        let result = resolve_canvas_path(&root, "/../secret.txt");
        assert!(matches!(result, Err(Canvas2dRuntimeError::PathEscape)));
    }

    #[test]
    fn canvas_server_returns_index_and_js() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        write_canvas_files(&manifest);
        let runtime = Canvas2dRuntime::new();
        let server = runtime
            .start_workspace_with_envelope(&manifest, &test_envelope())
            .expect("serve");
        let preview = server.preview();
        assert!(preview.preview_url.starts_with("http://127.0.0.1:"));

        let index = read_http(&preview.preview_url, "/");
        assert!(index.contains("200 OK"));
        assert!(index.contains("Coin Field"));

        let script = read_http(&preview.preview_url, "/src/main.js");
        assert!(script.contains("200 OK"));
        assert!(script.contains("requestAnimationFrame"));
    }

    #[test]
    fn canvas_server_returns_forbidden_for_traversal_requests() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        write_canvas_files(&manifest);
        let runtime = Canvas2dRuntime::new();
        let server = runtime
            .start_workspace_with_envelope(&manifest, &test_envelope())
            .expect("serve");
        let preview = server.preview();

        let response = read_http(&preview.preview_url, "/../secret.txt");

        assert!(response.contains("403 Forbidden"));
        assert!(response.contains("Forbidden"));
    }

    #[test]
    fn canvas_runtime_rejects_extra_workspace_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        write_canvas_files(&manifest);
        fs::write(
            manifest
                .paths
                .generated
                .join("canvas")
                .join("remote-loader.js"),
            "fetch('https://example.com')",
        )
        .expect("extra");

        let result =
            Canvas2dRuntime::new().start_workspace_with_envelope(&manifest, &test_envelope());

        assert!(matches!(
            result,
            Err(Canvas2dRuntimeError::InvalidPromptEnvelope(_))
        ));
    }

    #[test]
    fn canvas_runtime_rejects_wrong_runtime_envelope() {
        let mut envelope = test_envelope();
        envelope.runtime_policy.runtime_kind = "static-html".to_string();

        let result = Canvas2dRuntime::new().validate_prompt_envelope(&envelope);

        assert!(matches!(
            result,
            Err(Canvas2dRuntimeError::InvalidPromptEnvelope(_))
        ));
    }

    fn test_manifest(root: &Path) -> AppBoxManifest {
        let generated = root.join("generated");
        fs::create_dir_all(generated.join("canvas")).expect("canvas root");

        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Canvas Test".to_string(),
            mode: RuntimeKind::Canvas2d,
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

    fn write_canvas_files(manifest: &AppBoxManifest) {
        let canvas_root = manifest.paths.generated.join("canvas");
        for file in CANVAS2D_ALLOWED_FILES {
            let path = canvas_root.join(file);
            fs::create_dir_all(path.parent().expect("parent")).expect("parent");
            let contents = match file {
                "index.html" => {
                    "<!doctype html><title>Coin Field</title><canvas id=\"game\"></canvas>"
                }
                "src/main.js" => "requestAnimationFrame(function loop() {});",
                _ => "export const ok = true;",
            };
            fs::write(path, contents).expect("write file");
        }
        fs::create_dir_all(canvas_root.join("assets")).expect("assets");
    }

    fn test_envelope() -> PromptEnvelope {
        let allowed_files = CANVAS2D_ALLOWED_FILES
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();

        PromptEnvelope {
            schema_version: "1.0".to_string(),
            envelope_id: "penv_test".to_string(),
            created_at: "now".to_string(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: "sofvary.runtime.canvas2d".to_string(),
                    version: "0.1.0".to_string(),
                },
                harness_packs: vec![PackReference {
                    id: "sofvary.harness.canvas2d".to_string(),
                    version: "0.1.0".to_string(),
                }],
                runtime_kind: "canvas2d".to_string(),
                generated_root: "generated/canvas".to_string(),
                entrypoint: "index.html".to_string(),
                bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
            },
            user_intent: "Build a canvas game".to_string(),
            current_app_state: CurrentAppState {
                app_id: "app_test".to_string(),
                workspace_name: "Canvas Test".to_string(),
                mode: "create".to_string(),
                existing_files: Vec::new(),
                file_context: Vec::new(),
                preview_state: "empty".to_string(),
            },
            runtime_policy: RuntimePolicy {
                runtime_kind: "canvas2d".to_string(),
                allowed_entrypoints: vec!["index.html".to_string()],
                allowed_server_bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
                package_install: false,
            },
            harness_policy: HarnessPolicy {
                system_instructions: Vec::new(),
                file_system_rules: Vec::new(),
                output_rules: Vec::new(),
                blocked_capabilities: vec!["react-runtime".to_string()],
            },
            file_system_policy: FileSystemPolicy {
                root: "generated/canvas".to_string(),
                allowed_files: allowed_files.clone(),
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
                format: "canvas2d-project".to_string(),
                files: allowed_files,
                shell_ui_included: false,
            },
            acceptance_criteria: Vec::new(),
        }
    }

    fn read_http(base_url: &str, path: &str) -> String {
        let port = base_url
            .trim_end_matches('/')
            .rsplit(':')
            .next()
            .expect("port")
            .parse::<u16>()
            .expect("port number");
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut last_error = None;
        while Instant::now() < deadline {
            match read_http_once(port, request.as_bytes()) {
                Ok(response) => return response,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe
                            | std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    last_error = Some(error);
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("read http failed: {error}"),
            }
        }
        panic!("read http timed out after transient error: {last_error:?}");
    }

    fn read_http_once(port: u16, request: &[u8]) -> std::io::Result<String> {
        let mut stream = TcpStream::connect(("127.0.0.1", port))?;
        stream.write_all(request)?;

        let mut buffer = Vec::new();
        stream.read_to_end(&mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }
}
