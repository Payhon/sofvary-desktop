use crate::core::harness_engine::PromptEnvelope;
use crate::core::runtime_diagnostic::RuntimeDiagnostic;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SofvaryAgentContext {
    pub task_id: String,
    pub workspace_root: PathBuf,
    pub staging_root: PathBuf,
    pub runtime_kind: String,
    pub envelope_id: String,
    pub allowed_files: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

impl SofvaryAgentContext {
    pub fn for_acp_session(
        task_id: impl Into<String>,
        workspace_root: &Path,
        staging_root: &Path,
        envelope: &PromptEnvelope,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            workspace_root: workspace_root.to_path_buf(),
            staging_root: staging_root.to_path_buf(),
            runtime_kind: envelope.runtime_policy.runtime_kind.clone(),
            envelope_id: envelope.envelope_id.clone(),
            allowed_files: envelope.output_contract.files.clone(),
            diagnostics: Vec::new(),
        }
    }

    pub fn with_diagnostics(mut self, diagnostics: Vec<RuntimeDiagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    pub fn get_task_state(&self) -> Value {
        json!({
            "taskId": self.task_id,
            "runtimeKind": self.runtime_kind,
            "envelopeId": self.envelope_id,
            "allowedFiles": self.allowed_files,
            "diagnosticCount": self.diagnostics.len()
        })
    }

    pub fn get_runtime_diagnostics(&self) -> Value {
        json!(self.diagnostics)
    }

    pub fn list_generated_files(&self) -> Result<Vec<String>, String> {
        let generated_root = self.resolve_workspace_child(Path::new("generated"))?;
        if !generated_root.exists() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        collect_files(&generated_root, &generated_root, &mut files)?;
        files.sort();
        Ok(files)
    }

    pub fn get_workspace_manifest(&self) -> Result<Value, String> {
        let manifest = self.resolve_workspace_child(Path::new("app.box.json"))?;
        let contents = fs::read_to_string(manifest).map_err(|error| error.to_string())?;
        serde_json::from_str(&contents).map_err(|error| error.to_string())
    }

    fn resolve_workspace_child(&self, relative: &Path) -> Result<PathBuf, String> {
        if relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err("Sofvary MCP context path must stay inside the workspace".to_string());
        }
        let root = normalize_path_lexically(&self.workspace_root);
        let candidate = normalize_path_lexically(&self.workspace_root.join(relative));
        if candidate.starts_with(root) {
            Ok(candidate)
        } else {
            Err("Sofvary MCP context path escaped the workspace".to_string())
        }
    }
}

pub fn acp_mcp_servers_for_context(_context: &SofvaryAgentContext) -> Value {
    // This context is currently process-local. ACP mcpServers must describe a
    // concrete server transport, so do not advertise the internal descriptor.
    json!([])
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            collect_files(root, &path, files)?;
        } else if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "generated file escaped context root".to_string())?;
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
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
    use crate::core::harness_engine::PromptEnvelope;

    #[test]
    fn acp_mcp_servers_omit_internal_context_descriptor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let envelope = test_envelope();
        let context = SofvaryAgentContext::for_acp_session(
            "task-a",
            temp.path(),
            &temp.path().join("generated"),
            &envelope,
        );

        let servers = acp_mcp_servers_for_context(&context);

        assert_eq!(servers, json!([]));
    }

    #[test]
    fn context_exposes_current_runtime_diagnostics() {
        let temp = tempfile::tempdir().expect("tempdir");
        let envelope = test_envelope();
        let diagnostic = crate::core::runtime_diagnostic::diagnostic_from_command_failure(
            crate::core::workspace_types::RuntimeKind::ReactVite,
            "build",
            Some(1),
            "",
            "src/App.tsx: Expected closing tag",
            None,
        );
        let context = SofvaryAgentContext::for_acp_session(
            "task-a",
            temp.path(),
            &temp.path().join("generated"),
            &envelope,
        )
        .with_diagnostics(vec![diagnostic]);

        let diagnostics = context
            .get_runtime_diagnostics()
            .as_array()
            .expect("diagnostics")
            .clone();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(context.get_task_state()["diagnosticCount"], 1);
    }

    #[test]
    fn context_rejects_workspace_escape_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let envelope = test_envelope();
        let context = SofvaryAgentContext::for_acp_session(
            "task-a",
            temp.path(),
            &temp.path().join("generated"),
            &envelope,
        );

        let result = context.resolve_workspace_child(Path::new("../outside.txt"));

        assert!(matches!(result, Err(message) if message.contains("workspace")));
    }

    #[test]
    fn list_generated_files_is_scoped_to_current_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let generated = temp.path().join("generated/react/src");
        fs::create_dir_all(&generated).expect("generated");
        fs::write(
            generated.join("App.tsx"),
            "export default function App() {}",
        )
        .expect("file");
        fs::create_dir_all(temp.path().join("outside")).expect("outside");
        fs::write(temp.path().join("outside/secret.txt"), "secret").expect("secret");
        let envelope = test_envelope();
        let context = SofvaryAgentContext::for_acp_session(
            "task-a",
            temp.path(),
            &temp.path().join("generated"),
            &envelope,
        );

        let files = context.list_generated_files().expect("files");

        assert_eq!(files, vec!["react/src/App.tsx"]);
    }

    fn test_envelope() -> PromptEnvelope {
        serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/react-vite-prompt-envelope.golden.json"
        ))
        .expect("fixture")
    }
}
