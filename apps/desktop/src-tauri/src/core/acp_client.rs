use crate::core::agent_cli_bridge::parse_agent_file_output;
use crate::core::agent_config::AgentCommandConfig;
use crate::core::agent_context_mcp::{acp_mcp_servers_for_context, SofvaryAgentContext};
use crate::core::agent_gateway::{
    AgentEvent, AgentEventSink, AgentFileWriteRequest, AgentGatewayError,
};
use crate::core::harness_engine::PromptEnvelope;
use crate::core::runtime_diagnostic::RuntimeDiagnostic;
use crate::platform::stdio::StdioJsonRpcProcess;
use crate::platform::CommandSpec;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone)]
pub struct AcpRunRequest<'a> {
    pub agent_id: &'a str,
    pub command: &'a AgentCommandConfig,
    pub workspace_root: &'a Path,
    pub staging_root: &'a Path,
    pub envelope: &'a PromptEnvelope,
    pub diagnostics: &'a [RuntimeDiagnostic],
    pub timeout_ms: u64,
    pub event_sink: Option<AgentEventSink>,
}

#[derive(Debug, Clone, Default)]
pub struct AcpRunOutput {
    pub events: Vec<AgentEvent>,
    pub file_writes: Vec<AgentFileWriteRequest>,
}

pub fn run_acp_agent(request: AcpRunRequest<'_>) -> Result<AcpRunOutput, AgentGatewayError> {
    let mut process = StdioJsonRpcProcess::spawn(&CommandSpec {
        executable: request.command.executable.clone(),
        args: request.command.args.clone(),
        cwd: request.workspace_root.to_path_buf(),
        env: request.command.env.clone(),
        allowed_network: false,
        timeout_ms: Some(request.timeout_ms),
        kill_on_drop: true,
    })
    .map_err(|error| AgentGatewayError::Adapter(format!("failed to start ACP agent: {error}")))?;

    let timeout = Duration::from_millis(request.timeout_ms);
    let mut session = AcpSessionState {
        agent_id: request.agent_id.to_string(),
        workspace_root: request.workspace_root.to_path_buf(),
        staging_root: request.staging_root.to_path_buf(),
        context: Some(
            SofvaryAgentContext::for_acp_session(
                request.envelope.current_app_state.app_id.clone(),
                request.workspace_root,
                request.staging_root,
                request.envelope,
            )
            .with_diagnostics(request.diagnostics.to_vec()),
        ),
        staged_files: HashMap::new(),
        agent_text: String::new(),
        events: Vec::new(),
        event_sink: request.event_sink.clone(),
    };

    send_request(
        &mut process,
        0,
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": {
                    "readTextFile": true,
                    "writeTextFile": true
                },
                "terminal": false
            },
            "clientInfo": {
                "name": "sofvary",
                "title": "Sofvary",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )?;
    let initialize = read_until_response(&mut process, 0, &mut session, timeout)?;
    let protocol_version = initialize
        .get("protocolVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if protocol_version != 1 {
        return Err(AgentGatewayError::Adapter(format!(
            "ACP protocol version mismatch: expected 1, got {protocol_version}"
        )));
    }
    session.record_event(AgentEvent::Planning {
        message: format!("Initialized ACP agent {}", request.agent_id),
    });

    send_request(
        &mut process,
        1,
        "session/new",
        json!({
            "cwd": request.workspace_root.display().to_string(),
            "mcpServers": session
                .context
                .as_ref()
                .map(acp_mcp_servers_for_context)
                .unwrap_or_else(|| json!([]))
        }),
    )?;
    let new_session = read_until_response(&mut process, 1, &mut session, timeout)?;
    let session_id = new_session
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| AgentGatewayError::Adapter("ACP session/new missing sessionId".to_string()))?
        .to_string();

    send_request(
        &mut process,
        2,
        "session/prompt",
        json!({
            "sessionId": session_id,
            "prompt": [
                {
                    "type": "text",
                    "text": build_acp_prompt(request.envelope, request.staging_root)
                }
            ]
        }),
    )?;
    let _ = read_until_response(&mut process, 2, &mut session, timeout)?;

    let file_writes = if session.staged_files.is_empty() {
        parse_agent_file_output(
            &session.agent_text,
            &request.envelope.output_contract.files,
            "ACP agent message",
        )?
    } else {
        session
            .staged_files
            .into_iter()
            .map(|(relative_path, contents)| AgentFileWriteRequest {
                relative_path,
                contents,
            })
            .collect()
    };

    let mut output = AcpRunOutput {
        events: session.events,
        file_writes,
    };
    output
        .file_writes
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(output)
}

pub fn test_acp_connection(command: &AgentCommandConfig) -> Result<String, AgentGatewayError> {
    const TEST_MARKER: &str = "SOFVARY_ACP_OK";
    const TEST_TIMEOUT: Duration = Duration::from_secs(120);

    let temp_dir = tempfile::Builder::new()
        .prefix("sofvary-acp-test-")
        .tempdir()
        .map_err(|error| AgentGatewayError::Adapter(format!("ACP test dir failed: {error}")))?;
    let cwd = temp_dir.path().to_path_buf();
    let mut process = StdioJsonRpcProcess::spawn(&CommandSpec {
        executable: command.executable.clone(),
        args: command.args.clone(),
        cwd: cwd.clone(),
        env: command.env.clone(),
        allowed_network: false,
        timeout_ms: Some(120_000),
        kill_on_drop: true,
    })
    .map_err(|error| AgentGatewayError::Adapter(format!("failed to start ACP agent: {error}")))?;

    let mut session = AcpSessionState {
        agent_id: "test".to_string(),
        workspace_root: cwd,
        staging_root: temp_dir.path().join("staging"),
        context: None,
        staged_files: HashMap::new(),
        agent_text: String::new(),
        events: Vec::new(),
        event_sink: None,
    };
    send_request(
        &mut process,
        0,
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": {
                    "readTextFile": true,
                    "writeTextFile": true
                },
                "terminal": false
            },
            "clientInfo": {
                "name": "sofvary",
                "title": "Sofvary",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )?;
    let initialize = read_until_response(&mut process, 0, &mut session, TEST_TIMEOUT)?;
    let protocol_version = initialize
        .get("protocolVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if protocol_version != 1 {
        return Err(AgentGatewayError::Adapter(format!(
            "ACP protocol version mismatch: expected 1, got {protocol_version}"
        )));
    }

    let session_id = test_acp_session_new(&mut process, &mut session)?;
    send_request(
        &mut process,
        2,
        "session/prompt",
        json!({
            "sessionId": session_id,
            "prompt": [
                {
                    "type": "text",
                    "text": format!(
                        "Reply with exactly this text and no markdown: {TEST_MARKER}. Do not read files, write files, run commands, or include any other text."
                    )
                }
            ]
        }),
    )?;
    let _ = read_until_response(&mut process, 2, &mut session, TEST_TIMEOUT)?;
    if !session.agent_text.contains(TEST_MARKER) {
        return Err(AgentGatewayError::Adapter(format!(
            "ACP prompt round-trip did not return expected marker {TEST_MARKER}"
        )));
    }

    Ok(format!(
        "ACP initialize/session/prompt round-trip succeeded: {session_id}"
    ))
}

fn test_acp_session_new(
    process: &mut StdioJsonRpcProcess,
    session: &mut AcpSessionState,
) -> Result<String, AgentGatewayError> {
    send_request(
        process,
        1,
        "session/new",
        json!({
            "cwd": session.workspace_root.display().to_string(),
            "mcpServers": session
                .context
                .as_ref()
                .map(acp_mcp_servers_for_context)
                .unwrap_or_else(|| json!([]))
        }),
    )?;
    let new_session = read_until_response(process, 1, session, Duration::from_secs(30))?;
    new_session
        .get("sessionId")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| AgentGatewayError::Adapter("ACP session/new missing sessionId".to_string()))
}

fn send_request(
    process: &mut StdioJsonRpcProcess,
    id: u64,
    method: &str,
    params: Value,
) -> Result<(), AgentGatewayError> {
    let line = serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    }))
    .map_err(|error| AgentGatewayError::Adapter(error.to_string()))?;
    process
        .write_line(&line)
        .map_err(|error| AgentGatewayError::Adapter(format!("ACP write failed: {error}")))
}

fn send_response(
    process: &mut StdioJsonRpcProcess,
    id: Value,
    result: Value,
) -> Result<(), AgentGatewayError> {
    let line = serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
    .map_err(|error| AgentGatewayError::Adapter(error.to_string()))?;
    process
        .write_line(&line)
        .map_err(|error| AgentGatewayError::Adapter(format!("ACP response failed: {error}")))
}

fn send_error(
    process: &mut StdioJsonRpcProcess,
    id: Value,
    code: i64,
    message: &str,
) -> Result<(), AgentGatewayError> {
    let line = serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    }))
    .map_err(|error| AgentGatewayError::Adapter(error.to_string()))?;
    process
        .write_line(&line)
        .map_err(|error| AgentGatewayError::Adapter(format!("ACP error response failed: {error}")))
}

fn read_until_response(
    process: &mut StdioJsonRpcProcess,
    id: u64,
    session: &mut AcpSessionState,
    timeout: Duration,
) -> Result<Value, AgentGatewayError> {
    loop {
        let line = process
            .read_line_timeout(timeout)
            .map_err(|error| AgentGatewayError::Adapter(format!("ACP read failed: {error}")))?
            .ok_or_else(|| {
                AgentGatewayError::Adapter(format!("ACP agent timed out waiting for response {id}"))
            })?;
        if line.trim().is_empty() {
            continue;
        }
        let message: Value = serde_json::from_str(&line)
            .map_err(|error| AgentGatewayError::Adapter(format!("invalid ACP JSON: {error}")))?;

        if message.get("method").is_some() {
            handle_agent_message(process, session, message)?;
            continue;
        }

        if message.get("id").and_then(Value::as_u64) == Some(id) {
            if let Some(error) = message.get("error") {
                return Err(AgentGatewayError::Adapter(format!(
                    "ACP response error: {}",
                    error
                )));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }
}

fn handle_agent_message(
    process: &mut StdioJsonRpcProcess,
    session: &mut AcpSessionState,
    message: Value,
) -> Result<(), AgentGatewayError> {
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if message.get("id").is_some() {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        match method {
            "fs/write_text_file" => {
                match stage_write(session, message.get("params").unwrap_or(&Value::Null)) {
                    Ok(relative_path) => {
                        session.record_event(AgentEvent::FileWriteRequested { relative_path });
                        send_response(process, id, json!({}))
                    }
                    Err(error) => send_error(process, id, -32001, &error),
                }
            }
            "fs/read_text_file" => {
                match read_text_file(session, message.get("params").unwrap_or(&Value::Null)) {
                    Ok(content) => send_response(process, id, json!({ "content": content })),
                    Err(error) => send_error(process, id, -32002, &error),
                }
            }
            "session/request_permission" => send_response(
                process,
                id,
                json!({
                    "outcome": {
                        "outcome": "selected",
                        "optionId": "allow-once"
                    }
                }),
            ),
            "mcp/call_tool" => {
                match call_context_tool(session, message.get("params").unwrap_or(&Value::Null)) {
                    Ok(result) => send_response(process, id, result),
                    Err(error) => send_error(process, id, -32003, &error),
                }
            }
            _ => send_error(
                process,
                id,
                -32601,
                "Sofvary does not expose this ACP client method",
            ),
        }
    } else {
        if method == "session/update" {
            handle_session_update(session, message.get("params").unwrap_or(&Value::Null));
        }
        Ok(())
    }
}

fn stage_write(session: &mut AcpSessionState, params: &Value) -> Result<String, String> {
    let path = params
        .get("path")
        .or_else(|| params.get("uri"))
        .and_then(Value::as_str)
        .ok_or_else(|| "fs/write_text_file missing path".to_string())?;
    let content = params
        .get("content")
        .or_else(|| params.get("text"))
        .and_then(Value::as_str)
        .ok_or_else(|| "fs/write_text_file missing content".to_string())?;
    let path = path.strip_prefix("file://").unwrap_or(path);
    let target = PathBuf::from(path);
    let relative = target
        .strip_prefix(&session.staging_root)
        .map_err(|_| "ACP file write must target the Sofvary staging output root".to_string())?;
    let relative = normalize_relative(relative)?;
    session
        .staged_files
        .insert(relative.clone(), content.to_string());
    Ok(relative)
}

fn read_text_file(session: &AcpSessionState, params: &Value) -> Result<String, String> {
    let path = params
        .get("path")
        .or_else(|| params.get("uri"))
        .and_then(Value::as_str)
        .ok_or_else(|| "fs/read_text_file missing path".to_string())?;
    let path = path.strip_prefix("file://").unwrap_or(path);
    let target = PathBuf::from(path);
    if !target.starts_with(&session.workspace_root) {
        return Err("ACP file read must stay inside the active workspace".to_string());
    }
    fs::read_to_string(target).map_err(|error| error.to_string())
}

fn call_context_tool(session: &AcpSessionState, params: &Value) -> Result<Value, String> {
    let context = session
        .context
        .as_ref()
        .ok_or_else(|| "Sofvary context MCP is not available for this session".to_string())?;
    let tool_name = params
        .get("name")
        .or_else(|| params.get("tool"))
        .and_then(Value::as_str)
        .ok_or_else(|| "mcp/call_tool missing tool name".to_string())?;
    let result = match tool_name {
        "get_task_state" => context.get_task_state(),
        "get_runtime_diagnostics" => context.get_runtime_diagnostics(),
        "list_generated_files" => json!(context.list_generated_files()?),
        "get_workspace_manifest" => context.get_workspace_manifest()?,
        _ => {
            return Err(format!(
                "Sofvary context MCP does not expose tool '{tool_name}'"
            ));
        }
    };

    Ok(json!({
        "content": [
            {
                "type": "json",
                "json": result
            }
        ]
    }))
}

fn handle_session_update(session: &mut AcpSessionState, params: &Value) {
    let update = params.get("update").unwrap_or(params);
    match update.get("sessionUpdate").and_then(Value::as_str) {
        Some("plan") => session.record_event(AgentEvent::Planning {
            message: format!("{} reported a plan", session.agent_id),
        }),
        Some("agent_message_chunk") => {
            let text = update
                .get("content")
                .and_then(|content| content.get("text"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if !text.is_empty() {
                session.agent_text.push_str(&text);
                session.record_event(AgentEvent::TextDelta { text });
            }
        }
        Some("tool_call") => session.record_event(AgentEvent::Planning {
            message: "ACP agent requested a tool call".to_string(),
        }),
        _ => {}
    }
}

fn normalize_relative(path: &Path) -> Result<String, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            _ => return Err("ACP generated file path must be relative and normalized".to_string()),
        }
    }
    if parts.is_empty() {
        return Err("ACP generated file path cannot be empty".to_string());
    }
    Ok(parts.join("/"))
}

fn build_acp_prompt(envelope: &PromptEnvelope, staging_root: &Path) -> String {
    let envelope_json = serde_json::to_string_pretty(envelope).unwrap_or_else(|_| "{}".to_string());
    let absolute_targets = envelope
        .output_contract
        .files
        .iter()
        .map(|file| format!("- {file}: {}", staging_root.join(file).display()))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You are generating a Sofvary app. First try to write every required output file through ACP fs/write_text_file using these exact absolute paths:\n{}\nIf ACP fs/write_text_file is not available to you, return exactly one JSON object and no markdown with this shape: {{\"files\":[{{\"relativePath\":\"index.html\",\"contents\":\"...\"}}]}}.\nRequired relative files: {}.\nReturn only after all files are written or after the JSON object is complete. Do not write outside this staging root: {}. Do not include Sofvary shell UI in generated app source.\nPromptEnvelope:\n{}",
        absolute_targets,
        envelope.output_contract.files.join(", "),
        staging_root.display(),
        envelope_json
    )
}

struct AcpSessionState {
    agent_id: String,
    workspace_root: PathBuf,
    staging_root: PathBuf,
    context: Option<SofvaryAgentContext>,
    staged_files: HashMap<String, String>,
    agent_text: String,
    events: Vec<AgentEvent>,
    event_sink: Option<AgentEventSink>,
}

impl AcpSessionState {
    fn record_event(&mut self, event: AgentEvent) {
        if let Some(event_sink) = &self.event_sink {
            event_sink(event);
        } else {
            self.events.push(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_write_rejects_path_outside_staging_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut session = AcpSessionState {
            agent_id: "fake".to_string(),
            workspace_root: temp.path().to_path_buf(),
            staging_root: temp.path().join("staging"),
            context: None,
            staged_files: HashMap::new(),
            agent_text: String::new(),
            events: Vec::new(),
            event_sink: None,
        };
        let result = stage_write(
            &mut session,
            &json!({
                "path": temp.path().join("outside.txt"),
                "content": "bad"
            }),
        );

        assert!(matches!(result, Err(message) if message.contains("staging")));
    }

    #[test]
    fn stage_write_records_normalized_relative_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let staging = temp.path().join("staging");
        let mut session = AcpSessionState {
            agent_id: "fake".to_string(),
            workspace_root: temp.path().to_path_buf(),
            staging_root: staging.clone(),
            context: None,
            staged_files: HashMap::new(),
            agent_text: String::new(),
            events: Vec::new(),
            event_sink: None,
        };
        let relative = stage_write(
            &mut session,
            &json!({
                "path": staging.join("src/App.tsx"),
                "content": "export default function App() {}"
            }),
        )
        .expect("stage write");

        assert_eq!(relative, "src/App.tsx");
        assert!(session.staged_files.contains_key("src/App.tsx"));
    }

    #[test]
    fn session_update_accumulates_agent_text_for_json_fallback() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut session = AcpSessionState {
            agent_id: "fake".to_string(),
            workspace_root: temp.path().to_path_buf(),
            staging_root: temp.path().join("staging"),
            context: None,
            staged_files: HashMap::new(),
            agent_text: String::new(),
            events: Vec::new(),
            event_sink: None,
        };

        handle_session_update(
            &mut session,
            &json!({
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": {
                        "type": "text",
                        "text": "{\"files\":[]}"
                    }
                }
            }),
        );

        assert_eq!(session.agent_text, "{\"files\":[]}");
    }
}
