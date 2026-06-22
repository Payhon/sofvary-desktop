use crate::core::agent_cli_bridge::parse_agent_file_output;
use crate::core::agent_config::AgentCommandConfig;
use crate::core::agent_gateway::{
    AgentEvent, AgentEventSink, AgentFileWriteRequest, AgentGatewayError,
};
use crate::core::gateway_uni_event::{GatewayUniEventEmitter, GatewayUniEventType};
use crate::core::harness_engine::PromptEnvelope;
use crate::core::runtime_diagnostic::RuntimeDiagnostic;
use crate::platform::stdio::StdioJsonRpcProcess;
use crate::platform::CommandSpec;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_PI_TIMEOUT_MS: u64 = 180_000;

#[derive(Clone)]
pub struct PiRunRequest<'a> {
    pub command: &'a AgentCommandConfig,
    pub workspace_root: &'a Path,
    pub staging_root: &'a Path,
    pub envelope: &'a PromptEnvelope,
    pub diagnostics: &'a [RuntimeDiagnostic],
    pub thread_id: &'a str,
    pub timeout_ms: u64,
    pub event_sink: Option<AgentEventSink>,
    pub gateway_events: Option<GatewayUniEventEmitter>,
}

#[derive(Debug, Clone, Default)]
pub struct PiRunOutput {
    pub events: Vec<AgentEvent>,
    pub file_writes: Vec<AgentFileWriteRequest>,
}

pub fn run_pi_agent(request: PiRunRequest<'_>) -> Result<PiRunOutput, AgentGatewayError> {
    if !request.staging_root.starts_with(request.workspace_root) {
        return Err(AgentGatewayError::Adapter(format!(
            "Pi staging root escapes workspace: {}",
            request.staging_root.display()
        )));
    }

    fs::create_dir_all(request.staging_root).map_err(|error| {
        AgentGatewayError::Adapter(format!("failed to create Pi staging root: {error}"))
    })?;
    let timeout_ms = if request.timeout_ms == 0 {
        DEFAULT_PI_TIMEOUT_MS
    } else {
        request.timeout_ms
    };
    let mut process = StdioJsonRpcProcess::spawn(&CommandSpec {
        executable: request.command.executable.clone(),
        args: request.command.args.clone(),
        cwd: request.staging_root.to_path_buf(),
        env: request.command.env.clone(),
        allowed_network: false,
        timeout_ms: Some(timeout_ms),
        kill_on_drop: true,
    })
    .map_err(|error| {
        AgentGatewayError::Adapter(format!("failed to start Pi RPC agent: {error}"))
    })?;

    let prompt_id = format!("prompt_{}", Uuid::new_v4());
    let prompt = build_pi_prompt(request.envelope, request.staging_root, request.diagnostics);
    if let Some(events) = &request.gateway_events {
        events.session_started("Sofvary Pi");
        events.turn_started(prompt_id.clone());
        events.status("connecting", "Starting Sofvary Pi RPC harness");
    }
    let line = serde_json::to_string(&json!({
        "id": prompt_id,
        "type": "prompt",
        "message": prompt,
        "threadId": request.thread_id
    }))
    .map_err(|error| AgentGatewayError::Adapter(error.to_string()))?;
    process
        .write_line(&line)
        .map_err(|error| AgentGatewayError::Adapter(format!("Pi RPC write failed: {error}")))?;

    let mut text = String::new();
    let mut final_text = String::new();
    let mut events = Vec::new();
    record_pi_event(
        &mut events,
        request.event_sink.as_ref(),
        AgentEvent::Planning {
            message: "Started Sofvary Pi RPC harness".to_string(),
        },
    );
    let timeout = Duration::from_millis(timeout_ms);
    loop {
        let line = process
            .read_line_timeout(timeout)
            .map_err(|error| AgentGatewayError::Adapter(format!("Pi RPC read failed: {error}")))?
            .ok_or_else(|| {
                AgentGatewayError::Adapter("Pi RPC agent timed out waiting for output".to_string())
            })?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .map_err(|error| AgentGatewayError::Adapter(format!("invalid Pi RPC JSON: {error}")))?;
        if is_failed_pi_response(&value) {
            if let Some(events) = &request.gateway_events {
                events.error(
                    value
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown command error"),
                );
                events.turn_completed("error");
            }
            return Err(AgentGatewayError::Adapter(format!(
                "Pi RPC command failed: {}",
                value
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown command error")
            )));
        }
        if maybe_cancel_pi_ui_request(&mut process, &value, request.gateway_events.as_ref())? {
            record_pi_event(
                &mut events,
                request.event_sink.as_ref(),
                AgentEvent::Planning {
                    message: "Pi RPC requested UI input; Sofvary canceled it for this harness turn"
                        .to_string(),
                },
            );
            continue;
        }
        if is_pi_agent_end(&value) {
            if let Some(message) = pi_final_text(&value) {
                final_text.push_str(&message);
                let gateway_message = message.clone();
                record_pi_event(
                    &mut events,
                    request.event_sink.as_ref(),
                    AgentEvent::TextDelta { text: message },
                );
                if let Some(gateway_events) = &request.gateway_events {
                    gateway_events.message_delta(gateway_message);
                }
            }
            if let Some(gateway_events) = &request.gateway_events {
                gateway_events.turn_completed("ok");
            }
            break;
        }
        if is_success_pi_response(&value) {
            continue;
        }
        if let Some(message) = pi_stream_text(&value) {
            text.push_str(&message);
            let gateway_message = message.clone();
            record_pi_event(
                &mut events,
                request.event_sink.as_ref(),
                AgentEvent::TextDelta { text: message },
            );
            if let Some(gateway_events) = &request.gateway_events {
                gateway_events.message_delta(gateway_message);
            }
        }
    }

    let mut file_writes = collect_staged_files(
        request.staging_root,
        &request.envelope.output_contract.files,
    )?;
    if file_writes.is_empty() {
        let parse_source = if final_text.trim().is_empty() {
            &text
        } else {
            &final_text
        };
        file_writes = parse_agent_file_output(
            parse_source,
            &request.envelope.output_contract.files,
            "Pi RPC agent",
        )?;
    }
    file_writes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    record_pi_event(
        &mut events,
        request.event_sink.as_ref(),
        AgentEvent::Planning {
            message: format!("Pi RPC returned {} output files", file_writes.len()),
        },
    );
    if let Some(gateway_events) = &request.gateway_events {
        for file in &file_writes {
            gateway_events.emit(
                GatewayUniEventType::FileWritten,
                json!({ "path": &file.relative_path, "source": "pi-rpc" }),
            );
        }
    }

    Ok(PiRunOutput {
        events,
        file_writes,
    })
}

fn record_pi_event(
    events: &mut Vec<AgentEvent>,
    event_sink: Option<&AgentEventSink>,
    event: AgentEvent,
) {
    if let Some(event_sink) = event_sink {
        event_sink(event);
    } else {
        events.push(event);
    }
}

pub fn test_pi_agent(command: &AgentCommandConfig) -> Result<String, AgentGatewayError> {
    let mut args = command.args.clone();
    args.push("--help".to_string());
    let adapter = crate::platform::current_adapter();
    let output = adapter
        .run_process(CommandSpec {
            executable: command.executable.clone(),
            args,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            env: command.env.clone(),
            allowed_network: false,
            timeout_ms: Some(10_000),
            kill_on_drop: true,
        })
        .map_err(|error| {
            AgentGatewayError::Adapter(format!("Pi RPC process test failed: {error}"))
        })?;
    if output.status_code == Some(0) {
        Ok("Pi RPC command is reachable".to_string())
    } else {
        Err(AgentGatewayError::Adapter(format!(
            "Pi RPC command failed with {:?}",
            output.status_code
        )))
    }
}

fn is_success_pi_response(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "response")
        && value
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn is_failed_pi_response(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "response")
        && value
            .get("success")
            .and_then(Value::as_bool)
            .is_some_and(|success| !success)
}

fn is_pi_agent_end(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "agent_end" | "done" | "result"))
        || value
            .get("event")
            .and_then(Value::as_str)
            .is_some_and(|kind| matches!(kind, "agent_end" | "done" | "result"))
}

fn maybe_cancel_pi_ui_request(
    process: &mut StdioJsonRpcProcess,
    value: &Value,
    gateway_events: Option<&GatewayUniEventEmitter>,
) -> Result<bool, AgentGatewayError> {
    if !value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "extension_ui_request")
    {
        return Ok(false);
    }
    let method = value.get("method").and_then(Value::as_str).unwrap_or("");
    if !matches!(method, "select" | "confirm" | "input" | "editor") {
        return Ok(true);
    }
    let Some(id) = value.get("id").and_then(Value::as_str) else {
        return Ok(true);
    };
    if let Some(events) = gateway_events {
        events.emit(
            GatewayUniEventType::ApprovalRequested,
            json!({
                "approvalId": id,
                "action": method,
                "subject": "Pi RPC UI input",
                "risks": ["Sofvary cancels interactive UI requests during this generation harness turn"]
            }),
        );
    }
    let line = serde_json::to_string(&json!({
        "type": "extension_ui_response",
        "id": id,
        "cancelled": true
    }))
    .map_err(|error| AgentGatewayError::Adapter(error.to_string()))?;
    process.write_line(&line).map_err(|error| {
        AgentGatewayError::Adapter(format!("Pi RPC UI response failed: {error}"))
    })?;
    if let Some(events) = gateway_events {
        events.emit(
            GatewayUniEventType::ApprovalResolved,
            json!({ "approvalId": id, "decision": "rejected", "source": "sofvary-harness" }),
        );
    }
    Ok(true)
}

fn collect_staged_files(
    staging_root: &Path,
    required_files: &[String],
) -> Result<Vec<AgentFileWriteRequest>, AgentGatewayError> {
    let mut files = Vec::new();
    for relative_path in required_files {
        let target = staging_root.join(relative_path);
        if !target.exists() {
            return Ok(Vec::new());
        }
        let contents = fs::read_to_string(&target).map_err(|error| {
            AgentGatewayError::Adapter(format!("failed to read Pi output {relative_path}: {error}"))
        })?;
        files.push(AgentFileWriteRequest {
            relative_path: relative_path.clone(),
            contents,
        });
    }
    Ok(files)
}

fn pi_stream_text(value: &Value) -> Option<String> {
    value
        .get("assistantMessageEvent")
        .and_then(|event| {
            event
                .get("delta")
                .or_else(|| event.get("content"))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
        .or_else(|| value.get("partialResult").and_then(message_text))
        .or_else(|| value.get("result").and_then(message_text))
        .or_else(|| {
            value
                .get("text")
                .or_else(|| value.get("delta"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn pi_final_text(value: &Value) -> Option<String> {
    value
        .get("messages")
        .and_then(Value::as_array)
        .and_then(|messages| {
            let text = messages
                .iter()
                .filter_map(message_text)
                .collect::<Vec<_>>()
                .join("\n");
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .or_else(|| value.get("message").and_then(message_text))
        .or_else(|| value.get("data").and_then(message_text))
        .or_else(|| {
            value
                .get("text")
                .or_else(|| value.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn message_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    value
        .get("text")
        .and_then(Value::as_str)
        .or_else(|| value.get("content").and_then(Value::as_str))
        .map(str::to_string)
        .or_else(|| value.get("content").and_then(content_text))
        .or_else(|| {
            value
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(content_text)
        })
}

fn content_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(items) = value.as_array() {
        let text = items
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .or_else(|| item.get("content"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect::<Vec<_>>()
            .join("");
        return if text.trim().is_empty() {
            None
        } else {
            Some(text)
        };
    }
    value
        .get("text")
        .or_else(|| value.get("content"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn build_pi_prompt(
    envelope: &PromptEnvelope,
    staging_root: &Path,
    diagnostics: &[RuntimeDiagnostic],
) -> String {
    let envelope_json = serde_json::to_string_pretty(envelope).unwrap_or_else(|_| "{}".to_string());
    let diagnostic_summary = runtime_diagnostic_summary(diagnostics);
    format!(
        "Generate a Sofvary app inside the current staging directory. Required relative files: {}. If you cannot write files, return exactly one JSON object with {{\"files\":[{{\"relativePath\":\"index.html\",\"contents\":\"...\"}}]}}. Do not write outside this staging root: {}. Do not include Sofvary shell UI in generated app source.{}\nPromptEnvelope:\n{}",
        envelope.output_contract.files.join(", "),
        staging_root.display(),
        diagnostic_summary,
        envelope_json
    )
}

fn runtime_diagnostic_summary(diagnostics: &[RuntimeDiagnostic]) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }

    let diagnostics_json =
        serde_json::to_string_pretty(diagnostics).unwrap_or_else(|_| "[]".to_string());
    format!(
        "\nRuntime diagnostics from the failed preview attempt are available here. Use them to repair only generated app files; do not request environment setup or command execution:\n{}",
        diagnostics_json
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_event_text_reads_common_shapes() {
        assert_eq!(
            pi_stream_text(&json!({ "text": "a" })),
            Some("a".to_string())
        );
        assert_eq!(
            pi_stream_text(&json!({
                "assistantMessageEvent": { "type": "text_delta", "delta": "b" }
            })),
            Some("b".to_string())
        );
        assert_eq!(
            pi_final_text(&json!({
                "type": "agent_end",
                "messages": [{ "role": "assistant", "content": [{ "type": "text", "text": "c" }] }]
            })),
            Some("c".to_string())
        );
    }
}
