use crate::core::agent_config::{AgentCommandConfig, AgentConfig, AgentProvider};
use crate::core::agent_gateway::{
    AgentEvent, AgentEventSink, AgentFileWriteRequest, AgentGatewayError,
};
use crate::core::gateway_uni_event::{GatewayUniEventEmitter, GatewayUniEventType};
use crate::core::harness_engine::PromptEnvelope;
use crate::core::runtime_diagnostic::RuntimeDiagnostic;
use crate::platform::stdio::{StdioLine, StdioLineProcess};
use crate::platform::{current_adapter, CommandSpec};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_CLI_TIMEOUT_MS: u64 = 120_000;
const STREAM_POLL_INTERVAL_MS: u64 = 250;
const STREAM_DRAIN_INTERVAL_MS: u64 = 10;
const STREAM_DRAIN_IDLE_READS: usize = 50;

#[derive(Clone)]
pub struct CliRunRequest<'a> {
    pub config: &'a AgentConfig,
    pub command: &'a AgentCommandConfig,
    pub workspace_root: &'a Path,
    pub staging_root: &'a Path,
    pub envelope: &'a PromptEnvelope,
    pub diagnostics: &'a [RuntimeDiagnostic],
    pub timeout_ms: u64,
    pub event_sink: Option<AgentEventSink>,
    pub gateway_events: Option<GatewayUniEventEmitter>,
}

#[derive(Debug, Clone, Default)]
pub struct CliRunOutput {
    pub events: Vec<AgentEvent>,
    pub file_writes: Vec<AgentFileWriteRequest>,
}

pub fn run_cli_agent(request: CliRunRequest<'_>) -> Result<CliRunOutput, AgentGatewayError> {
    test_cli_agent(request.config).map_err(|error| {
        AgentGatewayError::Adapter(format!(
            "CLI fallback is not ready; run the Agent test again or disable CLI fallback. {error}"
        ))
    })?;

    let prompt = build_cli_prompt(request.envelope, request.staging_root, request.diagnostics);
    if let Some(events) = &request.gateway_events {
        events.session_started(&request.config.label);
        events.turn_started(request.envelope.envelope_id.clone());
        events.status(
            "connecting",
            format!("Starting {} CLI fallback", request.config.label),
        );
    }
    let prompt_file =
        write_cli_prompt_file(request.staging_root, &request.envelope.envelope_id, &prompt)?;
    let args = build_cli_args(
        request.config.provider,
        &request.command.args,
        request.staging_root,
        &prompt_file,
    );
    let stdin_prompt = cli_stdin_prompt(request.config.provider, &prompt);
    let timeout_ms = if request.timeout_ms == 0 {
        DEFAULT_CLI_TIMEOUT_MS
    } else {
        request.timeout_ms
    };
    let spec = CommandSpec {
        executable: request.command.executable.clone(),
        args,
        cwd: request.workspace_root.to_path_buf(),
        env: request.command.env.clone(),
        allowed_network: false,
        timeout_ms: Some(timeout_ms),
        kill_on_drop: true,
    };
    let output = run_streaming_cli_process(
        spec,
        stdin_prompt,
        timeout_ms,
        &request.config.label,
        request.event_sink.as_ref(),
        request.gateway_events.as_ref(),
    )?;

    if output.status_code != Some(0) {
        if let Some(events) = &request.gateway_events {
            let message = summarize_process_error(&output.stderr, &output.stdout);
            events.error(format!(
                "CLI agent exited with {:?}: {message}",
                output.status_code
            ));
            events.turn_completed("error");
        }
        return Err(AgentGatewayError::Adapter(format!(
            "CLI agent exited with {:?}: {}",
            output.status_code,
            summarize_process_error(&output.stderr, &output.stdout)
        )));
    }

    let mut file_writes = collect_staged_files(
        request.staging_root,
        &request.envelope.output_contract.files,
    )?;
    let file_source = if file_writes.is_empty() {
        file_writes = parse_agent_file_output(
            &output.stdout,
            &request.envelope.output_contract.files,
            "CLI agent",
        )?;
        "cli-json-fallback"
    } else {
        "cli-stage"
    };
    file_writes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if let Some(events) = &request.gateway_events {
        for file in &file_writes {
            events.emit(
                GatewayUniEventType::FileWritten,
                serde_json::json!({ "path": &file.relative_path, "source": file_source }),
            );
        }
        events.turn_completed("ok");
    }
    let events = vec![
        AgentEvent::Planning {
            message: format!("Ran {} CLI fallback", request.config.label),
        },
        AgentEvent::TextDelta {
            text: format!("CLI fallback returned {} output files", file_writes.len()),
        },
    ];

    Ok(CliRunOutput {
        events,
        file_writes,
    })
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
            AgentGatewayError::Adapter(format!(
                "failed to read CLI output {relative_path}: {error}"
            ))
        })?;
        files.push(AgentFileWriteRequest {
            relative_path: relative_path.clone(),
            contents,
        });
    }
    Ok(files)
}

#[derive(Debug, Clone, Default)]
struct StreamingProcessOutput {
    status_code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_streaming_cli_process(
    spec: CommandSpec,
    stdin_prompt: Option<&str>,
    timeout_ms: u64,
    agent_label: &str,
    event_sink: Option<&AgentEventSink>,
    gateway_events: Option<&GatewayUniEventEmitter>,
) -> Result<StreamingProcessOutput, AgentGatewayError> {
    let mut process = StdioLineProcess::spawn_with_stdin(&spec, stdin_prompt).map_err(|error| {
        AgentGatewayError::Adapter(format!("failed to start CLI agent process: {error}"))
    })?;
    emit_live_agent_event(
        event_sink,
        AgentEvent::Planning {
            message: format!("Started {agent_label} CLI process"),
        },
    );
    if let Some(events) = gateway_events {
        events.status("connecting", format!("Started {agent_label} CLI process"));
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut output = StreamingProcessOutput::default();
    loop {
        if Instant::now() >= deadline {
            process.kill();
            let timeout_message = format!("process timed out after {timeout_ms} ms");
            output.status_code = None;
            output.stderr = if output.stderr.trim().is_empty() {
                timeout_message.clone()
            } else {
                format!("{timeout_message}\n{}", output.stderr)
            };
            emit_live_agent_event(
                event_sink,
                AgentEvent::Error {
                    message: format!("{agent_label} CLI timed out after {timeout_ms} ms"),
                },
            );
            if let Some(events) = gateway_events {
                events.error(format!("{agent_label} CLI timed out after {timeout_ms} ms"));
                events.turn_completed("error");
            }
            return Ok(output);
        }

        if let Some(line) = process
            .read_line_timeout(Duration::from_millis(STREAM_POLL_INTERVAL_MS))
            .map_err(|error| {
                AgentGatewayError::Adapter(format!("CLI output read failed: {error}"))
            })?
        {
            handle_stream_line(line, &mut output, agent_label, event_sink, gateway_events);
        }

        if let Some(status) = process.try_wait().map_err(|error| {
            AgentGatewayError::Adapter(format!("CLI process wait failed: {error}"))
        })? {
            drain_stream_lines(
                &mut process,
                &mut output,
                agent_label,
                event_sink,
                gateway_events,
            )?;
            output.status_code = status.code();
            return Ok(output);
        }

        thread::sleep(Duration::from_millis(1));
    }
}

fn drain_stream_lines(
    process: &mut StdioLineProcess,
    output: &mut StreamingProcessOutput,
    agent_label: &str,
    event_sink: Option<&AgentEventSink>,
    gateway_events: Option<&GatewayUniEventEmitter>,
) -> Result<(), AgentGatewayError> {
    let mut idle_reads = 0;
    loop {
        match process
            .read_line_timeout(Duration::from_millis(STREAM_DRAIN_INTERVAL_MS))
            .map_err(|error| {
                AgentGatewayError::Adapter(format!("CLI output read failed: {error}"))
            })? {
            Some(line) => {
                idle_reads = 0;
                handle_stream_line(line, output, agent_label, event_sink, gateway_events);
            }
            None => {
                idle_reads += 1;
                if idle_reads >= STREAM_DRAIN_IDLE_READS {
                    return Ok(());
                }
            }
        }
    }
}

fn handle_stream_line(
    line: StdioLine,
    output: &mut StreamingProcessOutput,
    agent_label: &str,
    event_sink: Option<&AgentEventSink>,
    gateway_events: Option<&GatewayUniEventEmitter>,
) {
    match line {
        StdioLine::Stdout(line) => {
            output.stdout.push_str(&line);
            output.stdout.push('\n');
            emit_gateway_stdout_line(&line, agent_label, gateway_events);
            if let Some(event) = stdout_line_event(&line, agent_label) {
                emit_live_agent_event(event_sink, event);
            }
        }
        StdioLine::Stderr(line) => {
            output.stderr.push_str(&line);
            output.stderr.push('\n');
            emit_gateway_stderr_line(&line, agent_label, gateway_events);
            if let Some(event) = stderr_line_event(&line, agent_label) {
                emit_live_agent_event(event_sink, event);
            }
        }
    }
}

fn emit_gateway_stdout_line(
    line: &str,
    agent_label: &str,
    gateway_events: Option<&GatewayUniEventEmitter>,
) {
    let Some(events) = gateway_events else {
        return;
    };
    let trimmed = line.trim();
    if trimmed.is_empty() || contains_prompt_payload(trimmed) {
        return;
    }

    events.terminal_output("stdout", truncate_for_display(trimmed, 1_200));
    if let Ok(value) = serde_json::from_str::<Value>(strip_json_fence(trimmed)) {
        emit_gateway_json_event(&value, agent_label, events);
    } else if !trimmed.contains("\"files\"") && !trimmed.contains("'files'") {
        events.message_delta(truncate_for_display(trimmed, 500));
    }
}

fn emit_gateway_stderr_line(
    line: &str,
    agent_label: &str,
    gateway_events: Option<&GatewayUniEventEmitter>,
) {
    let Some(events) = gateway_events else {
        return;
    };
    let trimmed = line.trim();
    if trimmed.is_empty() || contains_prompt_payload(trimmed) {
        return;
    }

    events.terminal_output("stderr", truncate_for_display(trimmed, 1_200));
    events.status("terminal", format!("{agent_label} wrote to stderr"));
}

fn emit_gateway_json_event(value: &Value, agent_label: &str, events: &GatewayUniEventEmitter) {
    if files_json_from_value(value).is_some() {
        events.status(
            "generating",
            format!("{agent_label} returned generated file payload"),
        );
        return;
    }

    match value.get("type").and_then(Value::as_str) {
        Some("thread.started") => {
            events.status("connecting", format!("{agent_label} thread started"));
        }
        Some("turn.started") => {
            events.emit(
                GatewayUniEventType::TurnStarted,
                serde_json::json!({ "source": "cli-json" }),
            );
        }
        Some("turn.completed") | Some("result") => {
            events.turn_completed("ok");
        }
        Some("turn.failed") | Some("error") => {
            events.error(json_event_message(value, agent_label));
        }
        Some("item.started") | Some("item.updated") | Some("item.completed") => {
            emit_gateway_json_item_event(value, events);
        }
        _ => {
            if let Some(text) = value
                .get("message")
                .or_else(|| value.get("text"))
                .and_then(Value::as_str)
                .filter(|text| !contains_prompt_payload(text))
            {
                events.message_delta(truncate_for_display(text, 500));
            }
        }
    }
}

fn emit_gateway_json_item_event(value: &Value, events: &GatewayUniEventEmitter) {
    let Some(item) = value.get("item") else {
        return;
    };
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("item");
    match item_type {
        "agent_message" => {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                if files_json_from_text(text).is_some() {
                    events.status("generating", "Agent returned generated file payload");
                } else if !contains_prompt_payload(text) {
                    events.message_delta(truncate_for_display(text, 500));
                }
            }
        }
        "reasoning" => {
            let text = item
                .get("text")
                .or_else(|| item.get("summary"))
                .and_then(Value::as_str)
                .unwrap_or("Reasoning updated");
            events.reasoning_delta(truncate_for_display(text, 500));
        }
        "tool_call" | "tool_use" => {
            let call_id = item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("cli-tool-call");
            let tool_name = item
                .get("name")
                .or_else(|| item.get("toolName"))
                .and_then(Value::as_str)
                .unwrap_or("tool");
            events.emit(
                GatewayUniEventType::ToolStarted,
                serde_json::json!({ "callId": call_id, "toolName": tool_name, "input": item.get("input").cloned().unwrap_or(Value::Null) }),
            );
        }
        "tool_result" => {
            let call_id = item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("cli-tool-call");
            let tool_name = item
                .get("name")
                .or_else(|| item.get("toolName"))
                .and_then(Value::as_str)
                .unwrap_or("tool");
            events.emit(
                GatewayUniEventType::ToolCompleted,
                serde_json::json!({ "callId": call_id, "toolName": tool_name, "status": "ok", "output": item.get("output").cloned().unwrap_or(Value::Null) }),
            );
        }
        kind => {
            events.status("agent-item", format!("Agent item {kind} updated"));
        }
    }
}

fn emit_live_agent_event(event_sink: Option<&AgentEventSink>, event: AgentEvent) {
    if let Some(event_sink) = event_sink {
        event_sink(event);
    }
}

fn stdout_line_event(line: &str, agent_label: &str) -> Option<AgentEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<Value>(strip_json_fence(trimmed)) {
        return json_stdout_event(&value, agent_label);
    }

    if contains_prompt_payload(trimmed) {
        return None;
    }
    if trimmed.contains("\"files\"") || trimmed.contains("'files'") {
        return Some(AgentEvent::Planning {
            message: format!("{agent_label} returned generated file payload"),
        });
    }

    Some(AgentEvent::TextDelta {
        text: truncate_for_display(trimmed, 500),
    })
}

fn stderr_line_event(line: &str, agent_label: &str) -> Option<AgentEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() || contains_prompt_payload(trimmed) {
        return None;
    }

    Some(AgentEvent::Planning {
        message: format!(
            "{agent_label} stderr: {}",
            truncate_for_display(trimmed, 360)
        ),
    })
}

fn json_stdout_event(value: &Value, agent_label: &str) -> Option<AgentEvent> {
    if files_json_from_value(value).is_some() {
        return Some(AgentEvent::Planning {
            message: format!("{agent_label} returned generated file payload"),
        });
    }

    match value.get("type").and_then(Value::as_str) {
        Some("thread.started") => Some(AgentEvent::Planning {
            message: format!("{agent_label} thread started"),
        }),
        Some("turn.started") => Some(AgentEvent::Planning {
            message: format!("{agent_label} turn started"),
        }),
        Some("turn.completed") => Some(AgentEvent::Planning {
            message: format!("{agent_label} turn completed"),
        }),
        Some("turn.failed") | Some("error") => Some(AgentEvent::Error {
            message: json_event_message(value, agent_label),
        }),
        Some("item.started") | Some("item.updated") | Some("item.completed") => {
            json_item_event(value, agent_label)
        }
        _ => value
            .get("message")
            .or_else(|| value.get("text"))
            .and_then(Value::as_str)
            .filter(|text| !contains_prompt_payload(text))
            .map(|text| AgentEvent::TextDelta {
                text: truncate_for_display(text, 500),
            }),
    }
}

fn json_item_event(value: &Value, agent_label: &str) -> Option<AgentEvent> {
    let item = value.get("item")?;
    match item.get("type").and_then(Value::as_str) {
        Some("agent_message") => {
            let text = item.get("text").and_then(Value::as_str)?;
            if files_json_from_text(text).is_some() {
                return Some(AgentEvent::Planning {
                    message: format!("{agent_label} returned generated file payload"),
                });
            }
            (!contains_prompt_payload(text)).then(|| AgentEvent::TextDelta {
                text: truncate_for_display(text, 500),
            })
        }
        Some("reasoning") => Some(AgentEvent::Planning {
            message: format!("{agent_label} reasoning updated"),
        }),
        Some(kind) => Some(AgentEvent::Planning {
            message: format!("{agent_label} item {kind} updated"),
        }),
        None => None,
    }
}

fn json_event_message(value: &Value, agent_label: &str) -> String {
    value
        .get("message")
        .or_else(|| value.get("error"))
        .and_then(Value::as_str)
        .map(|message| format!("{agent_label}: {}", truncate_for_display(message, 500)))
        .unwrap_or_else(|| format!("{agent_label} reported an error"))
}

pub fn test_cli_agent(config: &AgentConfig) -> Result<String, AgentGatewayError> {
    let command = config.cli.as_ref().ok_or_else(|| {
        AgentGatewayError::Adapter(format!("{} has no CLI fallback command", config.label))
    })?;
    let adapter = current_adapter();
    let mut args = command.args.clone();
    args.push("--help".to_string());
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
        .map_err(|error| AgentGatewayError::Adapter(format!("CLI process test failed: {error}")))?;

    if output.status_code == Some(0) {
        Ok("CLI fallback command is reachable".to_string())
    } else {
        Err(AgentGatewayError::Adapter(format!(
            "CLI fallback command failed with {:?}: {}",
            output.status_code,
            summarize_process_error(&output.stderr, &output.stdout)
        )))
    }
}

fn summarize_process_error(stderr: &str, stdout: &str) -> String {
    let source = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    if looks_like_blocked_codex_cli(source) {
        return "Codex CLI binary is missing or was blocked by macOS security; use Codex ACP or reinstall a trusted Codex CLI".to_string();
    }
    let first_line = source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("process failed without stderr");
    if contains_prompt_payload(first_line) {
        return "process failed after receiving the Sofvary generation prompt".to_string();
    }
    truncate_for_display(first_line, 360)
}

fn contains_prompt_payload(value: &str) -> bool {
    value.contains("PromptEnvelope")
        || value.contains("Generate a Sofvary app")
        || value.contains("Required relative files")
        || value.contains("relativePath")
}

fn truncate_for_display(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            output.push_str("...");
            return output;
        }
        output.push(ch);
    }
    output
}

fn looks_like_blocked_codex_cli(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("@openai/codex")
        && lower.contains("/vendor/")
        && lower.contains("/codex/codex")
        && (lower.contains("enoent")
            || lower.contains("malicious software")
            || value.contains("恶意软件"))
}

fn build_cli_args(
    provider: AgentProvider,
    base_args: &[String],
    staging_root: &Path,
    prompt_file: &Path,
) -> Vec<String> {
    let mut args = base_args.to_vec();
    let handoff_prompt = build_cli_handoff_prompt(prompt_file);
    match provider {
        AgentProvider::Codex => {
            args.push("--output-schema".to_string());
            args.push(cli_output_schema_path(prompt_file).display().to_string());
            args.push("--cd".to_string());
            args.push(staging_root.display().to_string());
            args.push("-".to_string());
        }
        AgentProvider::ClaudeCode => {
            args.push("--json-schema".to_string());
            args.push(files_schema().to_string());
            args.push(handoff_prompt);
        }
        AgentProvider::Opencode => {
            args.push(handoff_prompt);
        }
        AgentProvider::Cursor
        | AgentProvider::KimiCode
        | AgentProvider::Qoder
        | AgentProvider::DeepseekTui
        | AgentProvider::Custom
        | AgentProvider::SofvaryPi => {
            args.push(handoff_prompt);
        }
    }
    args
}

fn cli_stdin_prompt<'a>(provider: AgentProvider, prompt: &'a str) -> Option<&'a str> {
    match provider {
        AgentProvider::Codex => Some(prompt),
        _ => None,
    }
}

fn write_cli_prompt_file(
    staging_root: &Path,
    envelope_id: &str,
    prompt: &str,
) -> Result<PathBuf, AgentGatewayError> {
    let prompt_dir = staging_root.join(".sofvary-agent-prompts");
    fs::create_dir_all(&prompt_dir).map_err(|error| {
        AgentGatewayError::Adapter(format!("failed to create CLI prompt handoff dir: {error}"))
    })?;

    let prompt_path = prompt_dir.join(format!("{}.md", sanitized_prompt_file_stem(envelope_id)));
    fs::write(&prompt_path, prompt).map_err(|error| {
        AgentGatewayError::Adapter(format!("failed to write CLI prompt handoff file: {error}"))
    })?;
    fs::write(cli_output_schema_path(&prompt_path), files_schema()).map_err(|error| {
        AgentGatewayError::Adapter(format!("failed to write CLI output schema file: {error}"))
    })?;
    Ok(prompt_path)
}

fn cli_output_schema_path(prompt_file: &Path) -> PathBuf {
    prompt_file
        .parent()
        .map(|parent| parent.join("files.schema.json"))
        .unwrap_or_else(|| PathBuf::from("files.schema.json"))
}

fn sanitized_prompt_file_stem(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if output.trim_matches('_').is_empty() {
        output = "prompt".to_string();
    }
    output
}

fn build_cli_handoff_prompt(prompt_file: &Path) -> String {
    format!(
        "Read the full Sofvary generation prompt from this local file inside the current workspace and follow it exactly. Return only the JSON object requested there, with no markdown or extra text: {}",
        prompt_file.display()
    )
}

pub(crate) fn parse_agent_file_output(
    output_text: &str,
    required_files: &[String],
    source_label: &str,
) -> Result<Vec<AgentFileWriteRequest>, AgentGatewayError> {
    let value = parse_agent_json(output_text)?;
    let files = value
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AgentGatewayError::Adapter(format!("{source_label} output missing files array"))
        })?;

    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for file in files {
        let relative_path = file
            .get("relativePath")
            .or_else(|| file.get("path"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AgentGatewayError::Adapter(format!(
                    "{source_label} file entry missing relativePath"
                ))
            })?;
        let relative_path = normalize_relative_path(relative_path, source_label)?;
        if !seen.insert(relative_path.clone()) {
            return Err(AgentGatewayError::Adapter(format!(
                "{source_label} output contains duplicate file: {relative_path}"
            )));
        }
        let contents = file
            .get("contents")
            .or_else(|| file.get("content"))
            .or_else(|| file.get("text"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AgentGatewayError::Adapter(format!(
                    "{source_label} file entry missing contents: {relative_path}"
                ))
            })?
            .to_string();
        output.push(AgentFileWriteRequest {
            relative_path,
            contents,
        });
    }

    let required = required_files.iter().cloned().collect::<HashSet<_>>();
    let produced = output
        .iter()
        .map(|file| file.relative_path.clone())
        .collect::<HashSet<_>>();
    let missing = required
        .difference(&produced)
        .cloned()
        .collect::<Vec<String>>();
    if !missing.is_empty() {
        return Err(AgentGatewayError::Adapter(format!(
            "{source_label} output missing required files: {}",
            missing.join(", ")
        )));
    }

    let extras = produced
        .difference(&required)
        .cloned()
        .collect::<Vec<String>>();
    if !extras.is_empty() {
        return Err(AgentGatewayError::Adapter(format!(
            "{source_label} output contains files outside the runtime contract: {}",
            extras.join(", ")
        )));
    }

    Ok(output)
}

fn parse_agent_json(output_text: &str) -> Result<Value, AgentGatewayError> {
    let trimmed = strip_json_fence(output_text.trim());
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(files_value) = files_json_from_value(&value) {
            return Ok(files_value);
        }
        return Ok(value);
    }

    output_text
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(strip_json_fence(line.trim())).ok())
        .find_map(|value| files_json_from_value(&value))
        .ok_or_else(|| {
            AgentGatewayError::Adapter("Agent did not return a JSON object with files".to_string())
        })
}

fn files_json_from_value(value: &Value) -> Option<Value> {
    if value.get("files").is_some() {
        return Some(value.clone());
    }

    let text = value
        .get("item")
        .and_then(|item| item.get("text"))
        .or_else(|| value.get("text"))
        .and_then(Value::as_str)?;
    files_json_from_text(text)
}

fn files_json_from_text(text: &str) -> Option<Value> {
    let nested = serde_json::from_str::<Value>(strip_json_fence(text.trim())).ok()?;
    nested.get("files").is_some().then_some(nested)
}

fn strip_json_fence(value: &str) -> &str {
    let Some(stripped) = value.strip_prefix("```") else {
        return value;
    };
    let stripped = stripped
        .strip_prefix("json")
        .or_else(|| stripped.strip_prefix("JSON"))
        .unwrap_or(stripped)
        .trim_start();
    stripped
        .strip_suffix("```")
        .map(str::trim_end)
        .unwrap_or(value)
}

fn normalize_relative_path(path: &str, source_label: &str) -> Result<String, AgentGatewayError> {
    if path.trim().is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return Err(AgentGatewayError::Adapter(format!(
            "{source_label} file path escapes workspace: {path}"
        )));
    }
    if path.contains('\\') {
        return Err(AgentGatewayError::Adapter(format!(
            "{source_label} file path must use forward slashes: {path}"
        )));
    }

    let mut parts = Vec::new();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            _ => {
                return Err(AgentGatewayError::Adapter(format!(
                    "{source_label} file path must be normalized: {path}"
                )));
            }
        }
    }
    if parts.is_empty() {
        return Err(AgentGatewayError::Adapter(format!(
            "{source_label} file path cannot be empty"
        )));
    }
    Ok(parts.join("/"))
}

fn build_cli_prompt(
    envelope: &PromptEnvelope,
    staging_root: &Path,
    diagnostics: &[RuntimeDiagnostic],
) -> String {
    let envelope_json = serde_json::to_string_pretty(envelope).unwrap_or_else(|_| "{}".to_string());
    let diagnostic_summary = runtime_diagnostic_summary(diagnostics);
    format!(
        "Generate a Sofvary app from this PromptEnvelope. If your CLI environment can edit files, write each required file under this staging root as soon as it is ready instead of batching everything at the end: {}. Also return exactly one JSON object with this shape for Sofvary verification or fallback: {{\"files\":[{{\"relativePath\":\"index.html\",\"contents\":\"...\"}}]}}. Required relative files: {}. Do not write outside the staging root. Do not include Sofvary shell UI in generated app source.{}\nPromptEnvelope:\n{}",
        staging_root.display(),
        envelope.output_contract.files.join(", "),
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

fn files_schema() -> &'static str {
    r#"{"type":"object","additionalProperties":false,"required":["files"],"properties":{"files":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["relativePath","contents"],"properties":{"relativePath":{"type":"string"},"contents":{"type":"string"}}}}}}"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agent_config::AgentTransportKind;
    use crate::core::gateway_uni_event::GatewayUniEvent;
    use std::sync::{Arc, Mutex};

    fn captured_gateway_emitter() -> (GatewayUniEventEmitter, Arc<Mutex<Vec<GatewayUniEvent>>>) {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let captured = captured.clone();
            Arc::new(move |event| {
                captured.lock().expect("events").push(event);
            })
        };
        (
            GatewayUniEventEmitter::new("thread-a", "codex", AgentTransportKind::Cli, sink),
            captured,
        )
    }

    #[test]
    fn parses_valid_cli_files_json() {
        let files = parse_agent_file_output(
            r#"{"files":[{"relativePath":"index.html","contents":"ok"}]}"#,
            &["index.html".to_string()],
            "CLI agent",
        )
        .expect("files");

        assert_eq!(files[0].relative_path, "index.html");
        assert_eq!(files[0].contents, "ok");
    }

    #[test]
    fn parses_fenced_agent_files_json() {
        let files = parse_agent_file_output(
            "```json\n{\"files\":[{\"relativePath\":\"index.html\",\"contents\":\"ok\"}]}\n```",
            &["index.html".to_string()],
            "ACP agent message",
        )
        .expect("files");

        assert_eq!(files[0].relative_path, "index.html");
    }

    #[test]
    fn parses_codex_jsonl_agent_message_text() {
        let files = parse_agent_file_output(
            "{\"type\":\"turn.started\"}\n{\"type\":\"item.completed\",\"item\":{\"id\":\"item_0\",\"type\":\"agent_message\",\"text\":\"{\\\"files\\\":[{\\\"relativePath\\\":\\\"index.html\\\",\\\"contents\\\":\\\"ok\\\"}]}\"}}\n{\"type\":\"turn.completed\"}",
            &["index.html".to_string()],
            "Codex CLI agent",
        )
        .expect("files");

        assert_eq!(files[0].relative_path, "index.html");
        assert_eq!(files[0].contents, "ok");
    }

    #[test]
    fn codex_jsonl_stream_line_becomes_live_agent_event() {
        let event = stdout_line_event(
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Working on the layout\"}}",
            "Codex",
        )
        .expect("event");

        assert!(matches!(event, AgentEvent::TextDelta { text } if text == "Working on the layout"));
    }

    #[test]
    fn gateway_stdout_maps_codex_jsonl_tool_events() {
        let (gateway_events, captured) = captured_gateway_emitter();

        emit_gateway_stdout_line(
            "{\"type\":\"item.started\",\"item\":{\"id\":\"call-a\",\"type\":\"tool_call\",\"name\":\"read_file\",\"input\":{\"path\":\"src/App.tsx\"}}}",
            "Codex",
            Some(&gateway_events),
        );

        let events = captured.lock().expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, GatewayUniEventType::TerminalOutput);
        assert_eq!(events[0].payload["stream"], "stdout");
        assert_eq!(events[1].event_type, GatewayUniEventType::ToolStarted);
        assert_eq!(events[1].payload["callId"], "call-a");
        assert_eq!(events[1].payload["toolName"], "read_file");
    }

    #[test]
    fn gateway_plain_stdout_falls_back_to_message_delta() {
        let (gateway_events, captured) = captured_gateway_emitter();

        emit_gateway_stdout_line("Working on the layout", "Codex", Some(&gateway_events));

        let events = captured.lock().expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, GatewayUniEventType::TerminalOutput);
        assert_eq!(events[1].event_type, GatewayUniEventType::MessageDelta);
        assert_eq!(events[1].payload["text"], "Working on the layout");
    }

    #[test]
    fn gateway_stderr_maps_to_terminal_output_and_status() {
        let (gateway_events, captured) = captured_gateway_emitter();

        emit_gateway_stderr_line("warning: retrying", "Codex", Some(&gateway_events));

        let events = captured.lock().expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, GatewayUniEventType::TerminalOutput);
        assert_eq!(events[0].payload["stream"], "stderr");
        assert_eq!(events[1].event_type, GatewayUniEventType::StatusChanged);
        assert_eq!(events[1].payload["phase"], "terminal");
    }

    #[test]
    fn stream_file_payload_is_summarized_without_file_contents() {
        let event = stdout_line_event(
            "{\"files\":[{\"relativePath\":\"index.html\",\"contents\":\"<secret markup>\"}]}",
            "Codex",
        )
        .expect("event");

        assert!(
            matches!(event, AgentEvent::Planning { message } if message == "Codex returned generated file payload")
        );
    }

    #[test]
    fn rejects_cli_files_outside_contract() {
        let result = parse_agent_file_output(
            r#"{"files":[{"relativePath":"../index.html","contents":"bad"}]}"#,
            &["index.html".to_string()],
            "CLI agent",
        );

        assert!(
            matches!(result, Err(AgentGatewayError::Adapter(message)) if message.contains("normalized"))
        );
    }

    #[test]
    fn rejects_missing_required_cli_file() {
        let result = parse_agent_file_output(
            r#"{"files":[{"relativePath":"style.css","contents":"ok"}]}"#,
            &["index.html".to_string()],
            "CLI agent",
        );

        assert!(
            matches!(result, Err(AgentGatewayError::Adapter(message)) if message.contains("missing"))
        );
    }

    #[test]
    fn process_error_summary_redacts_prompt_payload() {
        let summary = summarize_process_error(
            "Error: spawn codex ENOENT\nargs: Generate a Sofvary app from this PromptEnvelope",
            "",
        );

        assert_eq!(summary, "Error: spawn codex ENOENT");

        let prompt_first_line =
            summarize_process_error("Generate a Sofvary app from this PromptEnvelope", "");
        assert_eq!(
            prompt_first_line,
            "process failed after receiving the Sofvary generation prompt"
        );
    }

    #[test]
    fn process_error_summary_explains_blocked_codex_cli() {
        let summary = summarize_process_error(
            "Error: spawn /Users/me/.nvm/lib/node_modules/@openai/codex/node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/codex/codex ENOENT",
            "",
        );

        assert!(summary.contains("blocked by macOS security"));
    }

    #[test]
    fn cli_args_use_prompt_file_handoff_instead_of_full_prompt() {
        let prompt_file =
            PathBuf::from("C:/workspace/app/generated/.sofvary-agent-prompts/penv_react_sqlite.md");
        let args = build_cli_args(
            AgentProvider::Codex,
            &["exec".to_string(), "--json".to_string()],
            Path::new("C:/workspace/app/generated"),
            &prompt_file,
        );

        assert!(args.iter().any(|arg| arg == "--cd"));
        assert!(args.iter().any(|arg| arg == "--output-schema"));
        assert!(args.iter().any(|arg| arg.ends_with("files.schema.json")));
        assert_eq!(args.last().map(String::as_str), Some("-"));
        assert!(!args.iter().any(|arg| contains_prompt_payload(arg)));
        assert!(args.join(" ").chars().count() < 1_000);
        assert_eq!(
            cli_stdin_prompt(AgentProvider::Codex, "full prompt"),
            Some("full prompt")
        );
    }

    #[test]
    fn non_codex_cli_args_use_workspace_local_prompt_handoff() {
        let prompt_file =
            PathBuf::from("C:/workspace/app/generated/.sofvary-agent-prompts/penv_react_sqlite.md");
        let args = build_cli_args(
            AgentProvider::KimiCode,
            &[
                "-p".to_string(),
                "--output-format".to_string(),
                "text".to_string(),
            ],
            Path::new("C:/workspace/app/generated"),
            &prompt_file,
        );

        assert!(args
            .last()
            .is_some_and(|arg| arg.contains("current workspace")));
        assert!(args
            .last()
            .is_some_and(|arg| arg.contains(".sofvary-agent-prompts")));
        assert_eq!(
            cli_stdin_prompt(AgentProvider::KimiCode, "full prompt"),
            None
        );
    }

    #[test]
    fn prompt_handoff_files_are_written_inside_staging_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let staging_root = temp.path().join("generated");

        let prompt_file =
            write_cli_prompt_file(&staging_root, "penv_test", "full prompt").expect("prompt file");

        assert!(prompt_file.starts_with(&staging_root));
        assert!(prompt_file
            .to_string_lossy()
            .contains(".sofvary-agent-prompts"));
        assert_eq!(
            fs::read_to_string(&prompt_file).expect("prompt"),
            "full prompt"
        );
        assert!(cli_output_schema_path(&prompt_file).exists());
    }

    #[test]
    fn prompt_handoff_file_stem_is_safe_for_workspace_metadata() {
        assert_eq!(
            sanitized_prompt_file_stem("penv_react/sqlite:客户"),
            "penv_react_sqlite___"
        );
        assert_eq!(sanitized_prompt_file_stem("客户"), "prompt");
    }
}
