use crate::core::harness_engine::PromptEnvelope;
#[cfg(test)]
use crate::core::software_naming::suggest_software_name;
use crate::core::workspace_types::AppBoxManifest;
use crate::platform::current_adapter;
#[cfg(test)]
use html_escape::encode_text;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticRuntimePreview {
    pub preview_url: String,
    pub logs: Vec<String>,
}

pub struct StaticRuntimeServer {
    preview: StaticRuntimePreview,
    stop_requested: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StaticRuntimeServer {
    pub fn preview(&self) -> StaticRuntimePreview {
        self.preview.clone()
    }

    pub fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for StaticRuntimeServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Error)]
pub enum StaticRuntimeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("platform error: {0}")]
    Platform(#[from] crate::platform::PlatformError),
    #[error("request attempted to escape static root")]
    PathEscape,
    #[error("invalid prompt envelope: {0}")]
    InvalidPromptEnvelope(String),
}

#[derive(Default)]
pub struct StaticHtmlRuntime;

impl StaticHtmlRuntime {
    pub fn new() -> Self {
        Self
    }

    #[cfg(test)]
    pub fn generate_fake_static_app(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
    ) -> Result<(), StaticRuntimeError> {
        validate_prompt_envelope(envelope)?;
        let static_root = prepare_static_root(manifest)?;

        let title = if envelope.user_intent.trim().is_empty() {
            "Sofvary Static App".to_string()
        } else {
            suggest_software_name(&envelope.user_intent)
        };
        let escaped_title = encode_text(&title);

        fs::write(
            static_root.join("index.html"),
            format!(
                r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{escaped_title}</title>
    <link rel="stylesheet" href="./style.css" />
  </head>
  <body>
    <main class="app">
      <section class="panel">
        <p class="label">Local Static App</p>
        <h1>{escaped_title}</h1>
        <p class="copy">This app runs locally from isolated static files.</p>
        <button id="action" type="button">Run Local Interaction</button>
        <p id="status" class="status">Waiting for interaction.</p>
      </section>
    </main>
    <script src="./app.js"></script>
  </body>
</html>
"#
            ),
        )?;
        fs::write(
            static_root.join("style.css"),
            r#":root {
  color-scheme: dark;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background: #0c111d;
  color: #f7fafc;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  min-height: 100vh;
}

.app {
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 32px;
  background:
    radial-gradient(circle at 25% 20%, rgba(61, 155, 255, 0.22), transparent 34%),
    linear-gradient(135deg, #0c111d 0%, #111827 56%, #151515 100%);
}

.panel {
  width: min(680px, 100%);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  padding: 32px;
  background: rgba(12, 17, 29, 0.82);
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.34);
}

.label {
  margin: 0 0 12px;
  color: #7dd3fc;
  font-size: 13px;
  text-transform: uppercase;
  letter-spacing: 0;
}

h1 {
  margin: 0;
  font-size: clamp(32px, 6vw, 56px);
  line-height: 1.02;
}

.copy {
  max-width: 54ch;
  color: #cbd5e1;
  font-size: 16px;
  line-height: 1.7;
}

button {
  min-height: 44px;
  border: 0;
  border-radius: 8px;
  padding: 0 18px;
  background: #f8fafc;
  color: #0c111d;
  font: inherit;
  font-weight: 700;
  cursor: pointer;
}

.status {
  color: #86efac;
  min-height: 24px;
}
"#,
        )?;
        fs::write(
            static_root.join("app.js"),
            r##"const action = document.querySelector("#action");
const status = document.querySelector("#status");

action?.addEventListener("click", () => {
  const time = new Date().toLocaleTimeString();
  status.textContent = `Local interaction completed at ${time}.`;
});
"##,
        )?;

        Ok(())
    }

    pub fn start_workspace(
        &self,
        manifest: &AppBoxManifest,
    ) -> Result<StaticRuntimeServer, StaticRuntimeError> {
        let static_root = prepare_static_root(manifest)?;
        let allowed_files = list_relative_files(&static_root)?;
        self.start_workspace_with_allowed_files(manifest, allowed_files)
    }

    fn start_workspace_with_allowed_files(
        &self,
        manifest: &AppBoxManifest,
        allowed_files: Vec<String>,
    ) -> Result<StaticRuntimeServer, StaticRuntimeError> {
        let adapter = current_adapter();
        let port = adapter.allocate_local_port()?;
        let bind_addr = ("127.0.0.1", port);
        let listener = TcpListener::bind(bind_addr)?;
        listener.set_nonblocking(true)?;
        let static_root = prepare_static_root(manifest)?;
        let served_root = static_root.clone();
        let served_allowed_files = Arc::new(allowed_files);
        let app_id = manifest.app_id.clone();
        let stop_requested = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop_requested);

        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = handle_connection(stream, &served_root, &served_allowed_files);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => break,
                }
            }
        });

        let preview = StaticRuntimePreview {
            preview_url: format!("http://127.0.0.1:{port}/"),
            logs: vec![
                format!("StaticHtmlRuntime started for {app_id}"),
                format!("Serving {}", static_root.display()),
                format!("Bound to 127.0.0.1:{port}"),
            ],
        };

        Ok(StaticRuntimeServer {
            preview,
            stop_requested,
            thread: Some(thread),
        })
    }

    pub fn start_workspace_with_envelope(
        &self,
        manifest: &AppBoxManifest,
        envelope: &PromptEnvelope,
    ) -> Result<StaticRuntimeServer, StaticRuntimeError> {
        self.validate_prompt_envelope(envelope)?;
        ensure_exact_workspace_static_files(manifest, &envelope.output_contract.files)?;
        self.start_workspace_with_allowed_files(manifest, envelope.output_contract.files.clone())
    }

    pub fn validate_prompt_envelope(
        &self,
        envelope: &PromptEnvelope,
    ) -> Result<(), StaticRuntimeError> {
        validate_prompt_envelope(envelope)
    }
}

fn handle_connection(
    mut stream: TcpStream,
    static_root: &Path,
    allowed_files: &[String],
) -> Result<(), StaticRuntimeError> {
    let mut buffer = [0_u8; 2048];
    let bytes = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..bytes]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let file_path = resolve_static_path(static_root, path, allowed_files)?;
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
) -> Result<(), StaticRuntimeError> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

fn resolve_static_path(
    static_root: &Path,
    request_path: &str,
    allowed_files: &[String],
) -> Result<PathBuf, StaticRuntimeError> {
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
        return Err(StaticRuntimeError::PathEscape);
    }

    let relative_name = relative.to_string_lossy();
    if !allowed_files
        .iter()
        .any(|allowed| allowed == relative_name.as_ref())
    {
        return Ok(static_root.join("__sofvary_not_found__"));
    }

    let candidate = static_root.join(relative);
    if !candidate.exists() {
        return Ok(candidate);
    }

    let canonical_root = static_root.canonicalize()?;
    let canonical_candidate = candidate.canonicalize()?;
    if canonical_candidate.starts_with(&canonical_root) {
        Ok(canonical_candidate)
    } else {
        Err(StaticRuntimeError::PathEscape)
    }
}

fn prepare_static_root(manifest: &AppBoxManifest) -> Result<PathBuf, StaticRuntimeError> {
    let expected = manifest.paths.root.join("generated").join("static");
    ensure_same_path(&manifest.paths.generated_static, &expected)?;
    ensure_same_path(&manifest.constraints.boundary, &manifest.paths.root)?;
    ensure_same_path(
        &manifest.paths.generated,
        &manifest.paths.root.join("generated"),
    )?;

    fs::create_dir_all(&manifest.paths.generated_static)?;
    let boundary = manifest.paths.root.canonicalize()?;
    let static_root = manifest.paths.generated_static.canonicalize()?;
    if static_root.starts_with(&boundary) {
        Ok(static_root)
    } else {
        Err(StaticRuntimeError::PathEscape)
    }
}

fn validate_prompt_envelope(envelope: &PromptEnvelope) -> Result<(), StaticRuntimeError> {
    if envelope.box_runtime_context.runtime_kind != "static-html" {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
            "runtimeKind must be static-html, found {}",
            envelope.box_runtime_context.runtime_kind
        )));
    }
    if envelope.box_runtime_context.generated_root != "generated/static" {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
            "generatedRoot must be generated/static, found {}",
            envelope.box_runtime_context.generated_root
        )));
    }
    if envelope.box_runtime_context.entrypoint != "index.html" {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
            "entrypoint must be index.html, found {}",
            envelope.box_runtime_context.entrypoint
        )));
    }
    if envelope.file_system_policy.root != "generated/static" {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
            "fileSystemPolicy.root must be generated/static, found {}",
            envelope.file_system_policy.root
        )));
    }
    if envelope.runtime_policy.runtime_kind != "static-html" {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
            "runtimePolicy.runtimeKind must be static-html, found {}",
            envelope.runtime_policy.runtime_kind
        )));
    }
    if envelope.runtime_policy.allowed_entrypoints.len() != 1
        || envelope.runtime_policy.allowed_entrypoints[0] != "index.html"
    {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(
            "runtimePolicy.allowedEntrypoints must contain exactly index.html".to_string(),
        ));
    }
    if envelope.runtime_policy.allowed_server_bind != "127.0.0.1" {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
            "runtimePolicy.allowedServerBind must be 127.0.0.1, found {}",
            envelope.runtime_policy.allowed_server_bind
        )));
    }
    if envelope.runtime_policy.network != "local-only"
        || envelope.box_runtime_context.network != "local-only"
        || envelope.box_runtime_context.bind != "127.0.0.1"
    {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(
            "static-html envelope must stay local-only and bind to 127.0.0.1".to_string(),
        ));
    }
    if envelope.command_policy.allow_shell
        || envelope.command_policy.allow_package_install
        || envelope.command_policy.allow_global_install
        || envelope.runtime_policy.package_install
    {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(
            "static-html envelope must not allow shell commands or package installs".to_string(),
        ));
    }
    if !envelope.command_policy.allowed_commands.is_empty() {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(
            "static-html envelope must not allow runtime commands".to_string(),
        ));
    }
    if envelope.file_system_policy.allow_external_files
        || envelope.file_system_policy.allow_path_traversal
    {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(
            "static-html envelope must not allow external files or path traversal".to_string(),
        ));
    }
    if envelope.output_contract.shell_ui_included {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(
            "static-html output contract must exclude Sofvary shell UI".to_string(),
        ));
    }
    if envelope.output_contract.format != "static-html-files" {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
            "outputContract.format must be static-html-files, found {}",
            envelope.output_contract.format
        )));
    }
    ensure_allowed_files_match_output_contract(envelope)?;

    Ok(())
}

fn ensure_allowed_files_match_output_contract(
    envelope: &PromptEnvelope,
) -> Result<(), StaticRuntimeError> {
    if envelope.file_system_policy.allowed_files.is_empty() {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(
            "static-html output contract must declare at least one allowed file".to_string(),
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

    Err(StaticRuntimeError::InvalidPromptEnvelope(
        "fileSystemPolicy.allowedFiles and outputContract.files must match".to_string(),
    ))
}

fn ensure_exact_workspace_static_files(
    manifest: &AppBoxManifest,
    allowed_files: &[String],
) -> Result<(), StaticRuntimeError> {
    let static_root = prepare_static_root(manifest)?;
    let expected: HashSet<&str> = allowed_files.iter().map(String::as_str).collect();
    let mut actual = HashSet::new();

    for entry in fs::read_dir(&static_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(StaticRuntimeError::InvalidPromptEnvelope(
                "generated/static must not contain directories".to_string(),
            ));
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if !expected.contains(name.as_str()) {
            return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
                "generated/static contains undeclared file {name}"
            )));
        }
        actual.insert(name);
    }

    if actual.len() != expected.len() {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(
            "generated/static must contain exactly index.html, style.css, and app.js".to_string(),
        ));
    }

    Ok(())
}

fn list_relative_files(root: &Path) -> Result<Vec<String>, StaticRuntimeError> {
    let mut files = HashSet::new();
    if root.exists() {
        collect_relative_files(root, root, &mut files)?;
    }
    let mut files = files.into_iter().collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn collect_relative_files(
    root: &Path,
    current: &Path,
    files: &mut HashSet<String>,
) -> Result<(), StaticRuntimeError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_relative_files(root, &path, files)?;
        } else if entry.file_type()?.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| StaticRuntimeError::PathEscape)?;
            files.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn validate_relative_contract_file(path: &str) -> Result<(), StaticRuntimeError> {
    if path.trim().is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "..")
        || Path::new(path).components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(StaticRuntimeError::InvalidPromptEnvelope(format!(
            "output contract file path must stay relative inside generated/static: {path}"
        )));
    }
    Ok(())
}

fn ensure_same_path(actual: &Path, expected: &Path) -> Result<(), StaticRuntimeError> {
    let normalized_actual = normalize_path_lexically(actual);
    let normalized_expected = normalize_path_lexically(expected);
    if normalized_actual == normalized_expected {
        Ok(())
    } else {
        Err(StaticRuntimeError::PathEscape)
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
    use crate::core::workspace_manager::WorkspaceManager;
    use std::io::{Read, Write};

    #[test]
    fn fake_agent_generates_static_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manager = WorkspaceManager::new();
        let manifest = crate::core::workspace_types::AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "static-html".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: crate::core::workspace_types::WorkspacePaths {
                root: temp.path().to_path_buf(),
                generated: temp.path().join("generated"),
                generated_static: temp.path().join("generated").join("static"),
                runtime: temp.path().join("runtime"),
                snapshots: temp.path().join("snapshots"),
            },
            constraints: crate::core::workspace_types::WorkspaceConstraints {
                boundary: temp.path().to_path_buf(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: crate::core::workspace_types::WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        };
        let runtime = StaticHtmlRuntime::new();
        runtime
            .generate_fake_static_app(&manifest, &test_envelope("A tiny timer"))
            .expect("generate");

        assert!(manifest.paths.generated_static.join("index.html").exists());
        assert!(manifest.paths.generated_static.join("style.css").exists());
        assert!(manifest.paths.generated_static.join("app.js").exists());
        let mut generated_files = fs::read_dir(&manifest.paths.generated_static)
            .expect("read generated")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();
        generated_files.sort();
        assert_eq!(
            generated_files,
            vec![
                "app.js".to_string(),
                "index.html".to_string(),
                "style.css".to_string()
            ]
        );
        assert!(manager
            .ensure_child(temp.path(), Path::new("generated/static/index.html"))
            .is_ok());
    }

    #[test]
    fn fake_agent_rejects_generated_static_outside_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_root = temp.path().join("workspace");
        fs::create_dir_all(&workspace_root).expect("workspace root");
        let outside = temp.path().join("outside");
        let manifest = crate::core::workspace_types::AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "static-html".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: crate::core::workspace_types::WorkspacePaths {
                root: workspace_root.clone(),
                generated: workspace_root.join("generated"),
                generated_static: outside.clone(),
                runtime: workspace_root.join("runtime"),
                snapshots: workspace_root.join("snapshots"),
            },
            constraints: crate::core::workspace_types::WorkspaceConstraints {
                boundary: workspace_root,
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: crate::core::workspace_types::WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        };
        let runtime = StaticHtmlRuntime::new();
        let result = runtime.generate_fake_static_app(&manifest, &test_envelope("escape"));

        assert!(matches!(result, Err(StaticRuntimeError::PathEscape)));
        assert!(!outside.join("index.html").exists());
    }

    #[test]
    fn fake_agent_rejects_prompt_envelope_with_extra_output_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = StaticHtmlRuntime::new();
        let mut envelope = test_envelope("extra file");
        envelope
            .output_contract
            .files
            .push("extra.html".to_string());

        let result = runtime.generate_fake_static_app(&manifest, &envelope);

        assert!(matches!(
            result,
            Err(StaticRuntimeError::InvalidPromptEnvelope(_))
        ));
        assert!(!manifest.paths.generated_static.join("extra.html").exists());
        assert!(!manifest.paths.generated_static.join("index.html").exists());
    }

    #[test]
    fn fake_agent_uses_prompt_envelope_user_intent_and_excludes_sofvary_shell_ui() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = StaticHtmlRuntime::new();
        runtime
            .generate_fake_static_app(&manifest, &test_envelope("Envelope Timer"))
            .expect("generate");

        let index =
            fs::read_to_string(manifest.paths.generated_static.join("index.html")).expect("index");
        let style =
            fs::read_to_string(manifest.paths.generated_static.join("style.css")).expect("style");
        let script =
            fs::read_to_string(manifest.paths.generated_static.join("app.js")).expect("script");
        let generated = [index, style, script].join("\n");

        assert!(generated.contains("Timer"));
        assert!(!generated.contains("Envelope Timer"));
        for forbidden in [
            "FloatingCommandMenu",
            "BuildOverlay",
            "PromptEnvelope",
            "Sofvary Shell",
        ] {
            assert!(
                !generated.contains(forbidden),
                "generated app source leaked {forbidden}"
            );
        }
    }

    #[test]
    fn rejects_static_path_escape() {
        let root = PathBuf::from("/tmp/sofvary/static");
        let allowed_files = vec!["index.html".to_string()];
        let result = resolve_static_path(&root, "/../secret.txt", &allowed_files);
        assert!(matches!(result, Err(StaticRuntimeError::PathEscape)));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_static_symlink_escape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let static_root = temp.path().join("static");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&static_root).expect("static root");
        fs::create_dir_all(&outside).expect("outside root");
        fs::write(outside.join("secret.txt"), "secret").expect("secret");
        std::os::unix::fs::symlink(outside.join("secret.txt"), static_root.join("index.html"))
            .expect("symlink");

        let allowed_files = vec!["index.html".to_string()];
        let result = resolve_static_path(&static_root, "/index.html", &allowed_files);

        assert!(matches!(result, Err(StaticRuntimeError::PathEscape)));
    }

    #[test]
    fn static_server_returns_index() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = crate::core::workspace_types::AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "static-html".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: crate::core::workspace_types::WorkspacePaths {
                root: temp.path().to_path_buf(),
                generated: temp.path().join("generated"),
                generated_static: temp.path().join("generated").join("static"),
                runtime: temp.path().join("runtime"),
                snapshots: temp.path().join("snapshots"),
            },
            constraints: crate::core::workspace_types::WorkspaceConstraints {
                boundary: temp.path().to_path_buf(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: crate::core::workspace_types::WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        };
        let runtime = StaticHtmlRuntime::new();
        runtime
            .generate_fake_static_app(&manifest, &test_envelope("HTTP test"))
            .expect("generate");
        let server = runtime.start_workspace(&manifest).expect("serve");
        let preview = server.preview();
        let address = preview
            .preview_url
            .trim_start_matches("http://")
            .trim_end_matches('/');

        let mut stream = TcpStream::connect(address).expect("connect");
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .expect("write request");
        let response = read_http_response(stream);
        assert!(response.contains("200 OK"));
        assert!(response.contains("Http Test"));
        assert!(!response.contains("HTTP test"));
        assert!(preview.preview_url.starts_with("http://127.0.0.1:"));
    }

    #[test]
    fn start_workspace_with_envelope_rejects_invalid_runtime_policy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = StaticHtmlRuntime::new();
        runtime
            .generate_fake_static_app(&manifest, &test_envelope("Policy test"))
            .expect("generate");
        let mut envelope = test_envelope("Policy test");
        envelope.runtime_policy.allowed_server_bind = "0.0.0.0".to_string();

        let result = runtime.start_workspace_with_envelope(&manifest, &envelope);

        assert!(matches!(
            result,
            Err(StaticRuntimeError::InvalidPromptEnvelope(_))
        ));
    }

    #[test]
    fn static_server_does_not_serve_extra_static_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = StaticHtmlRuntime::new();
        runtime
            .generate_fake_static_app(&manifest, &test_envelope("HTTP test"))
            .expect("generate");
        fs::write(manifest.paths.generated_static.join("extra.html"), "extra").expect("extra");
        let server = runtime
            .start_workspace_with_allowed_files(
                &manifest,
                test_envelope("HTTP test").output_contract.files,
            )
            .expect("serve");
        let preview = server.preview();
        let address = preview
            .preview_url
            .trim_start_matches("http://")
            .trim_end_matches('/');

        let mut stream = TcpStream::connect(address).expect("connect");
        stream
            .write_all(b"GET /extra.html HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .expect("write request");
        let response = read_http_response(stream);

        assert!(response.contains("404 Not Found"));
        assert!(!response.contains("extra"));
    }

    #[test]
    fn static_server_handle_stops_accepting_connections() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = crate::core::workspace_types::AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "static-html".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: crate::core::workspace_types::WorkspacePaths {
                root: temp.path().to_path_buf(),
                generated: temp.path().join("generated"),
                generated_static: temp.path().join("generated").join("static"),
                runtime: temp.path().join("runtime"),
                snapshots: temp.path().join("snapshots"),
            },
            constraints: crate::core::workspace_types::WorkspaceConstraints {
                boundary: temp.path().to_path_buf(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: crate::core::workspace_types::WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        };
        let runtime = StaticHtmlRuntime::new();
        runtime
            .generate_fake_static_app(&manifest, &test_envelope("Stop test"))
            .expect("generate");
        let mut server = runtime.start_workspace(&manifest).expect("serve");
        let address = server
            .preview()
            .preview_url
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();

        TcpStream::connect(&address).expect("server initially accepts connections");
        server.stop();

        assert!(TcpStream::connect(&address).is_err());
    }

    fn read_http_response(mut stream: TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        let mut response = Vec::new();
        let mut buffer = [0_u8; 512];

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes) => response.extend_from_slice(&buffer[..bytes]),
                Err(error)
                    if error.kind() == std::io::ErrorKind::ConnectionReset
                        && !response.is_empty() =>
                {
                    break;
                }
                Err(error) => panic!("read response: {error}"),
            }

            if http_response_is_complete(&response) {
                break;
            }
        }

        String::from_utf8(response).expect("utf8 response")
    }

    fn http_response_is_complete(response: &[u8]) -> bool {
        let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
            return false;
        };
        let headers = String::from_utf8_lossy(&response[..header_end]);
        let Some(content_length) = headers
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .and_then(|value| value.parse::<usize>().ok())
        else {
            return false;
        };

        response.len() >= header_end + 4 + content_length
    }

    fn test_manifest(root: &Path) -> AppBoxManifest {
        let generated = root.join("generated");
        let generated_static = generated.join("static");

        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "static-html".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: crate::core::workspace_types::WorkspacePaths {
                root: root.to_path_buf(),
                generated,
                generated_static,
                runtime: root.join("runtime"),
                snapshots: root.join("snapshots"),
            },
            constraints: crate::core::workspace_types::WorkspaceConstraints {
                boundary: root.to_path_buf(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: crate::core::workspace_types::WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        }
    }

    fn test_envelope(user_intent: &str) -> PromptEnvelope {
        PromptEnvelope {
            schema_version: "1.0".to_string(),
            envelope_id: "penv_test".to_string(),
            created_at: "now".to_string(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: "sofvary.runtime.static-html".to_string(),
                    version: "0.1.0".to_string(),
                },
                harness_packs: vec![PackReference {
                    id: "sofvary.harness.static-html".to_string(),
                    version: "0.1.0".to_string(),
                }],
                runtime_kind: "static-html".to_string(),
                generated_root: "generated/static".to_string(),
                entrypoint: "index.html".to_string(),
                bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
            },
            user_intent: user_intent.to_string(),
            current_app_state: CurrentAppState {
                app_id: "app_test".to_string(),
                workspace_name: "Test".to_string(),
                mode: "create".to_string(),
                existing_files: Vec::new(),
                file_context: Vec::new(),
                preview_state: "empty".to_string(),
            },
            runtime_policy: RuntimePolicy {
                runtime_kind: "static-html".to_string(),
                allowed_entrypoints: vec!["index.html".to_string()],
                allowed_server_bind: "127.0.0.1".to_string(),
                network: "local-only".to_string(),
                package_install: false,
            },
            harness_policy: HarnessPolicy {
                system_instructions: Vec::new(),
                file_system_rules: Vec::new(),
                output_rules: Vec::new(),
                blocked_capabilities: vec!["remote-network".to_string()],
            },
            file_system_policy: FileSystemPolicy {
                root: "generated/static".to_string(),
                allowed_files: vec![
                    "index.html".to_string(),
                    "style.css".to_string(),
                    "app.js".to_string(),
                ],
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
                format: "static-html-files".to_string(),
                files: vec![
                    "index.html".to_string(),
                    "style.css".to_string(),
                    "app.js".to_string(),
                ],
                shell_ui_included: false,
            },
            acceptance_criteria: Vec::new(),
        }
    }
}
