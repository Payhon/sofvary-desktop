use crate::core::agent_cli_bridge::parse_agent_file_output;
use crate::core::agent_config::AgentCommandConfig;
use crate::core::agent_gateway::{
    AgentEvent, AgentEventSink, AgentFileWriteRequest, AgentGatewayError, AgentLiveFileSink,
};
use crate::core::gateway_uni_event::{GatewayUniEventEmitter, GatewayUniEventType};
use crate::core::harness_engine::PromptEnvelope;
use crate::core::runtime_diagnostic::RuntimeDiagnostic;
use crate::platform::stdio::StdioJsonRpcProcess;
use crate::platform::CommandSpec;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{Duration, Instant, UNIX_EPOCH};
use uuid::Uuid;

const DEFAULT_PI_TIMEOUT_MS: u64 = 180_000;
const MAX_PI_MANAGED_STREAM_MS: u64 = 30 * 60 * 1000;
const MAX_PI_STREAM_CHARS: usize = 240_000;
const PI_READ_POLL_MS: u64 = 1_000;
const PI_GATEWAY_DELTA_FLUSH_MS: u64 = 2_000;
const PI_GATEWAY_DELTA_FLUSH_CHARS: usize = 1_200;
const PI_STAGED_FILE_SETTLE_MS: u64 = 1_000;

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
    pub live_file_sink: Option<AgentLiveFileSink>,
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
        if !request.diagnostics.is_empty() {
            events.status(
                "repair-context",
                format!(
                    "Passing {} runtime diagnostics to Sofvary Pi for repair.",
                    request.diagnostics.len()
                ),
            );
        }
        events.emit(
            GatewayUniEventType::ToolStarted,
            json!({
                "toolName": "pi.prompt",
                "status": "running",
                "summary": "Sending constrained PromptEnvelope to Sofvary Pi"
            }),
        );
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
    if let Some(events) = &request.gateway_events {
        events.emit(
            GatewayUniEventType::ToolCompleted,
            json!({
                "toolName": "pi.prompt",
                "status": "ok",
                "summary": "PromptEnvelope delivered"
            }),
        );
        events.emit(
            GatewayUniEventType::ToolStarted,
            json!({
                "toolName": "workspace.collect_staged_files",
                "status": "pending",
                "summary": "Waiting for generated files in the bounded staging directory"
            }),
        );
    }

    let started_at = Instant::now();
    let mut last_progress_at = started_at;
    let mut staged_progress = StagedFileProgress::new(
        request.staging_root,
        &request.envelope.output_contract.files,
    );
    let mut file_writes = Vec::new();
    let mut turn_completed_emitted = false;
    let mut text = String::new();
    let mut final_text = String::new();
    let mut pending_gateway_text = String::new();
    let mut last_gateway_delta_flush = Instant::now();
    let mut events = Vec::new();
    record_pi_event(
        &mut events,
        request.event_sink.as_ref(),
        AgentEvent::Planning {
            message: "Started Sofvary Pi RPC harness".to_string(),
        },
    );
    let timeout = Duration::from_millis(timeout_ms);
    let read_poll = Duration::from_millis(timeout_ms.min(PI_READ_POLL_MS).max(1));
    loop {
        let line = match process
            .read_line_timeout(read_poll)
            .map_err(|error| AgentGatewayError::Adapter(format!("Pi RPC read failed: {error}")))?
        {
            Some(line) => line,
            None => {
                if sync_ready_staged_files(
                    &mut staged_progress,
                    &mut file_writes,
                    request.live_file_sink.as_ref(),
                    &mut events,
                    request.event_sink.as_ref(),
                    request.gateway_events.as_ref(),
                    Instant::now(),
                )? {
                    last_progress_at = Instant::now();
                }
                if staged_progress.all_required_files_synced() {
                    flush_pi_gateway_delta(
                        request.gateway_events.as_ref(),
                        &mut pending_gateway_text,
                    );
                    if let Some(gateway_events) = &request.gateway_events {
                        gateway_events.status(
                            "files-ready",
                            format!(
                                "Detected {} stable generated files from Pi; ending the slow text stream and starting preview.",
                                request.envelope.output_contract.files.len()
                            ),
                        );
                        gateway_events.turn_completed("ok");
                    }
                    turn_completed_emitted = true;
                    process.kill();
                    break;
                }
                if process
                    .try_wait()
                    .map_err(|error| {
                        AgentGatewayError::Adapter(format!("Pi RPC status check failed: {error}"))
                    })?
                    .is_some()
                {
                    break;
                }
                if last_progress_at.elapsed() >= timeout {
                    return Err(pi_no_output_error(&mut process, timeout_ms));
                }
                enforce_pi_stream_limits(started_at, text.chars().count())?;
                continue;
            }
        };
        last_progress_at = Instant::now();
        if line.trim().is_empty() {
            if sync_ready_staged_files(
                &mut staged_progress,
                &mut file_writes,
                request.live_file_sink.as_ref(),
                &mut events,
                request.event_sink.as_ref(),
                request.gateway_events.as_ref(),
                Instant::now(),
            )? {
                last_progress_at = Instant::now();
            }
        } else {
            let value: Value = serde_json::from_str(&line).map_err(|error| {
                AgentGatewayError::Adapter(format!("invalid Pi RPC JSON: {error}"))
            })?;
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
                        message:
                            "Pi RPC requested UI input; Sofvary canceled it for this harness turn"
                                .to_string(),
                    },
                );
            } else if is_pi_agent_end(&value) {
                if let Some(message) = pi_final_text(&value) {
                    final_text.push_str(&message);
                    if text.trim().is_empty() {
                        emit_pi_text_delta(
                            &mut events,
                            request.event_sink.as_ref(),
                            request.gateway_events.as_ref(),
                            &mut pending_gateway_text,
                            &mut last_gateway_delta_flush,
                            &message,
                            true,
                        );
                    }
                }
                flush_pi_gateway_delta(request.gateway_events.as_ref(), &mut pending_gateway_text);
                if let Some(gateway_events) = &request.gateway_events {
                    gateway_events.turn_completed("ok");
                }
                turn_completed_emitted = true;
                break;
            } else if !is_success_pi_response(&value) {
                if let Some(message) = pi_stream_text(&value) {
                    text.push_str(&message);
                    emit_pi_text_delta(
                        &mut events,
                        request.event_sink.as_ref(),
                        request.gateway_events.as_ref(),
                        &mut pending_gateway_text,
                        &mut last_gateway_delta_flush,
                        &message,
                        false,
                    );
                }
            }
        }

        if sync_ready_staged_files(
            &mut staged_progress,
            &mut file_writes,
            request.live_file_sink.as_ref(),
            &mut events,
            request.event_sink.as_ref(),
            request.gateway_events.as_ref(),
            Instant::now(),
        )? {
            last_progress_at = Instant::now();
        }
        if staged_progress.all_required_files_synced() {
            flush_pi_gateway_delta(request.gateway_events.as_ref(), &mut pending_gateway_text);
            if let Some(gateway_events) = &request.gateway_events {
                gateway_events.status(
                    "files-ready",
                    format!(
                        "Detected {} stable generated files from Pi; ending the slow text stream and starting preview.",
                        request.envelope.output_contract.files.len()
                    ),
                );
                gateway_events.turn_completed("ok");
            }
            turn_completed_emitted = true;
            process.kill();
            break;
        }
        enforce_pi_stream_limits(started_at, text.chars().count())?;
    }

    let final_staged_files = collect_staged_files(
        request.staging_root,
        &request.envelope.output_contract.files,
    )?;
    merge_file_writes(&mut file_writes, final_staged_files);
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
    if !turn_completed_emitted {
        flush_pi_gateway_delta(request.gateway_events.as_ref(), &mut pending_gateway_text);
        if let Some(gateway_events) = &request.gateway_events {
            gateway_events.turn_completed(if file_writes.is_empty() {
                "incomplete"
            } else {
                "ok"
            });
        }
    }
    record_pi_event(
        &mut events,
        request.event_sink.as_ref(),
        AgentEvent::Planning {
            message: format!("Pi RPC returned {} output files", file_writes.len()),
        },
    );
    if let Some(gateway_events) = &request.gateway_events {
        gateway_events.emit(
            GatewayUniEventType::ToolCompleted,
            json!({
                "toolName": "workspace.collect_staged_files",
                "status": "ok",
                "fileCount": file_writes.len()
            }),
        );
        for file in &file_writes {
            if staged_progress.synced.contains_key(&file.relative_path) {
                continue;
            }
            gateway_events.emit(
                GatewayUniEventType::FileWriteRequested,
                json!({ "path": &file.relative_path, "source": "pi-rpc" }),
            );
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

fn emit_pi_text_delta(
    events: &mut Vec<AgentEvent>,
    event_sink: Option<&AgentEventSink>,
    gateway_events: Option<&GatewayUniEventEmitter>,
    pending_gateway_text: &mut String,
    last_gateway_delta_flush: &mut Instant,
    message: &str,
    force_flush: bool,
) {
    if message.is_empty() {
        return;
    }
    if gateway_events.is_none() {
        record_pi_event(
            events,
            event_sink,
            AgentEvent::TextDelta {
                text: message.to_string(),
            },
        );
        return;
    }

    pending_gateway_text.push_str(message);
    let should_flush = force_flush
        || pending_gateway_text.chars().count() >= PI_GATEWAY_DELTA_FLUSH_CHARS
        || last_gateway_delta_flush.elapsed() >= Duration::from_millis(PI_GATEWAY_DELTA_FLUSH_MS);
    if should_flush {
        flush_pi_gateway_delta(gateway_events, pending_gateway_text);
        *last_gateway_delta_flush = Instant::now();
    }
}

fn flush_pi_gateway_delta(
    gateway_events: Option<&GatewayUniEventEmitter>,
    pending_gateway_text: &mut String,
) {
    let Some(gateway_events) = gateway_events else {
        pending_gateway_text.clear();
        return;
    };
    if pending_gateway_text.trim().is_empty() {
        pending_gateway_text.clear();
        return;
    }
    gateway_events.message_delta(std::mem::take(pending_gateway_text));
}

fn sync_ready_staged_files(
    staged_progress: &mut StagedFileProgress<'_>,
    file_writes: &mut Vec<AgentFileWriteRequest>,
    live_file_sink: Option<&AgentLiveFileSink>,
    events: &mut Vec<AgentEvent>,
    event_sink: Option<&AgentEventSink>,
    gateway_events: Option<&GatewayUniEventEmitter>,
    now: Instant,
) -> Result<bool, AgentGatewayError> {
    let ready_files = staged_progress.collect_ready_files(now)?;
    if ready_files.is_empty() {
        return Ok(false);
    }

    for file in ready_files {
        record_pi_event(
            events,
            event_sink,
            AgentEvent::FileWriteRequested {
                relative_path: file.relative_path.clone(),
            },
        );
        if let Some(gateway_events) = gateway_events {
            gateway_events.emit(
                GatewayUniEventType::FileWriteRequested,
                json!({
                    "path": &file.relative_path,
                    "source": "pi-rpc",
                    "mode": "live",
                    "bytes": file.contents.len()
                }),
            );
        }
        if let Some(live_file_sink) = live_file_sink {
            live_file_sink(file.clone())?;
        }
        merge_file_write(file_writes, file.clone());
        record_pi_event(
            events,
            event_sink,
            AgentEvent::FileWritten {
                relative_path: file.relative_path.clone(),
            },
        );
        if let Some(gateway_events) = gateway_events {
            gateway_events.emit(
                GatewayUniEventType::FileWritten,
                json!({
                    "path": &file.relative_path,
                    "source": "pi-rpc",
                    "mode": "live"
                }),
            );
        }
    }
    Ok(true)
}

fn merge_file_writes(
    file_writes: &mut Vec<AgentFileWriteRequest>,
    incoming: Vec<AgentFileWriteRequest>,
) {
    for file in incoming {
        merge_file_write(file_writes, file);
    }
}

fn merge_file_write(file_writes: &mut Vec<AgentFileWriteRequest>, incoming: AgentFileWriteRequest) {
    if let Some(existing) = file_writes
        .iter_mut()
        .find(|file| file.relative_path == incoming.relative_path)
    {
        *existing = incoming;
    } else {
        file_writes.push(incoming);
    }
}

fn enforce_pi_stream_limits(
    started_at: Instant,
    streamed_chars: usize,
) -> Result<(), AgentGatewayError> {
    let elapsed_ms = started_at.elapsed().as_millis() as u64;
    if elapsed_ms > MAX_PI_MANAGED_STREAM_MS {
        return Err(AgentGatewayError::Adapter(format!(
            "Pi RPC exceeded managed session limit after {} minutes without completing. Cancel this run or switch to Workspace Handoff for long native agent sessions.",
            elapsed_ms / 60_000
        )));
    }
    if streamed_chars > MAX_PI_STREAM_CHARS {
        return Err(AgentGatewayError::Adapter(format!(
            "Pi RPC streamed more than {} characters without completing. The output was not accepted as a finished app.",
            MAX_PI_STREAM_CHARS
        )));
    }
    Ok(())
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
            "Pi RPC command failed with {:?}: {}",
            output.status_code,
            summarize_command_output(&output.stderr, &output.stdout)
        )))
    }
}

fn pi_no_output_error(process: &mut StdioJsonRpcProcess, timeout_ms: u64) -> AgentGatewayError {
    let stderr = collect_stderr_for_error(process);
    let stderr_summary = summarize_text_for_error(&stderr);
    match process.try_wait() {
        Ok(Some(status)) => AgentGatewayError::Adapter(format!(
            "Pi RPC agent exited before output with {}{}",
            exit_status_summary(status),
            stderr_suffix(&stderr_summary)
        )),
        Ok(None) => AgentGatewayError::Adapter(format!(
            "Pi RPC agent timed out waiting for output after {timeout_ms} ms{}",
            stderr_suffix(&stderr_summary)
        )),
        Err(error) => AgentGatewayError::Adapter(format!(
            "Pi RPC agent timed out waiting for output after {timeout_ms} ms; process status unavailable: {error}{}",
            stderr_suffix(&stderr_summary)
        )),
    }
}

fn collect_stderr_for_error(process: &mut StdioJsonRpcProcess) -> String {
    for _ in 0..5 {
        if process
            .drain_stderr()
            .map(|lines| lines.is_empty())
            .unwrap_or(true)
        {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    process.recent_stderr().unwrap_or_default()
}

fn exit_status_summary(status: ExitStatus) -> String {
    status
        .code()
        .map(|code| format!("status code {code}"))
        .unwrap_or_else(|| "unknown status".to_string())
}

fn stderr_suffix(stderr: &str) -> String {
    if stderr.trim().is_empty() {
        String::new()
    } else {
        format!("; stderr: {stderr}")
    }
}

fn summarize_command_output(stderr: &str, stdout: &str) -> String {
    let stderr = summarize_text_for_error(stderr);
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = summarize_text_for_error(stdout);
    if stdout.is_empty() {
        "no output".to_string()
    } else {
        stdout
    }
}

fn summarize_text_for_error(text: &str) -> String {
    const MAX_ERROR_TEXT: usize = 1200;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut summary = trimmed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(16)
        .collect::<Vec<_>>()
        .join("\n");
    if summary.len() > MAX_ERROR_TEXT {
        summary.truncate(MAX_ERROR_TEXT);
        summary.push_str("...");
    }
    summary
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
        if path_escapes_staging_root(relative_path) {
            return Err(AgentGatewayError::Adapter(format!(
                "Pi output contract contains unsafe path: {relative_path}"
            )));
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct StagedFileSignature {
    relative_path: String,
    len: u64,
    modified_nanos: u128,
}

struct StagedFileProgress<'a> {
    staging_root: &'a Path,
    required_files: &'a [String],
    observed: HashMap<String, ObservedStagedFile>,
    synced: HashMap<String, StagedFileSignature>,
}

struct ObservedStagedFile {
    signature: StagedFileSignature,
    stable_since: Instant,
}

impl<'a> StagedFileProgress<'a> {
    fn new(staging_root: &'a Path, required_files: &'a [String]) -> Self {
        Self {
            staging_root,
            required_files,
            observed: HashMap::new(),
            synced: HashMap::new(),
        }
    }

    fn collect_ready_files(
        &mut self,
        now: Instant,
    ) -> Result<Vec<AgentFileWriteRequest>, AgentGatewayError> {
        let mut ready = Vec::new();
        for relative_path in self.required_files {
            let Some(signature) = staged_file_signature_for_path(self.staging_root, relative_path)?
            else {
                self.observed.remove(relative_path);
                continue;
            };

            let stable_since = self
                .observed
                .get(relative_path)
                .filter(|observed| observed.signature == signature)
                .map(|observed| observed.stable_since)
                .unwrap_or(now);

            self.observed.insert(
                relative_path.clone(),
                ObservedStagedFile {
                    signature: signature.clone(),
                    stable_since,
                },
            );

            if now.duration_since(stable_since) < Duration::from_millis(PI_STAGED_FILE_SETTLE_MS) {
                continue;
            }
            if self.synced.get(relative_path) == Some(&signature) {
                continue;
            }

            let target = self.staging_root.join(relative_path);
            let contents = fs::read_to_string(&target).map_err(|error| {
                AgentGatewayError::Adapter(format!(
                    "failed to read Pi output {relative_path}: {error}"
                ))
            })?;
            self.synced.insert(relative_path.clone(), signature.clone());
            ready.push(AgentFileWriteRequest {
                relative_path: relative_path.clone(),
                contents,
            });
        }
        Ok(ready)
    }

    fn all_required_files_synced(&self) -> bool {
        !self.required_files.is_empty()
            && self
                .required_files
                .iter()
                .all(|relative_path| self.synced.contains_key(relative_path))
    }
}

fn staged_file_signature_for_path(
    staging_root: &Path,
    relative_path: &str,
) -> Result<Option<StagedFileSignature>, AgentGatewayError> {
    if path_escapes_staging_root(relative_path) {
        return Err(AgentGatewayError::Adapter(format!(
            "Pi output contract contains unsafe path: {relative_path}"
        )));
    }
    let target = staging_root.join(relative_path);
    let metadata = match fs::metadata(&target) {
        Ok(metadata) if metadata.is_file() && metadata.len() > 0 => metadata,
        _ => return Ok(None),
    };
    Ok(Some(StagedFileSignature {
        relative_path: relative_path.to_string(),
        len: metadata.len(),
        modified_nanos: metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default(),
    }))
}

fn path_escapes_staging_root(relative_path: &str) -> bool {
    let path = Path::new(relative_path);
    path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
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

    #[test]
    fn pi_no_output_error_includes_stderr_from_exited_process() {
        let spec = CommandSpec {
            executable: test_shell_executable(),
            args: test_shell_stderr_exit_args(),
            cwd: std::env::current_dir().expect("cwd"),
            env: std::collections::HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(5_000),
            kill_on_drop: true,
        };
        let mut process = StdioJsonRpcProcess::spawn(&spec).expect("process");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            process.drain_stderr().expect("stderr");
            if process.try_wait().expect("wait").is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let error = pi_no_output_error(&mut process, 100);

        assert!(error
            .to_string()
            .contains("Pi RPC agent exited before output"));
        assert!(error.to_string().contains("pi-rpc-boom"));
    }

    #[test]
    fn staged_file_progress_collects_files_after_required_files_are_stable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let files = vec!["src/App.tsx".to_string(), "package.json".to_string()];
        fs::create_dir_all(root.join("src")).expect("src");
        fs::write(
            root.join("src/App.tsx"),
            "export default function App() { return null; }",
        )
        .expect("app");
        fs::write(root.join("package.json"), "{}").expect("package");

        let start = Instant::now();
        let mut progress = StagedFileProgress::new(root, &files);

        assert!(progress
            .collect_ready_files(start)
            .expect("first")
            .is_empty());
        assert!(progress
            .collect_ready_files(start + Duration::from_millis(PI_STAGED_FILE_SETTLE_MS - 1))
            .expect("not settled")
            .is_empty());
        let ready = progress
            .collect_ready_files(start + Duration::from_millis(PI_STAGED_FILE_SETTLE_MS))
            .expect("settled");

        assert_eq!(ready.len(), 2);
        assert!(progress.all_required_files_synced());
    }

    #[test]
    fn staged_file_progress_rejects_unsafe_output_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let files = vec!["../escape.ts".to_string()];
        let mut progress = StagedFileProgress::new(temp.path(), &files);

        assert!(progress.collect_ready_files(Instant::now()).is_err());
    }

    #[test]
    fn pi_stream_limits_fail_before_multi_hour_runs() {
        let started = Instant::now() - Duration::from_millis(MAX_PI_MANAGED_STREAM_MS + 1);
        let error = enforce_pi_stream_limits(started, 10).expect_err("limit should fail");

        assert!(error.to_string().contains("exceeded managed session limit"));
    }

    #[cfg(windows)]
    fn test_shell_executable() -> PathBuf {
        PathBuf::from("cmd")
    }

    #[cfg(windows)]
    fn test_shell_stderr_exit_args() -> Vec<String> {
        vec![
            "/C".to_string(),
            "echo pi-rpc-boom 1>&2 && exit /B 7".to_string(),
        ]
    }

    #[cfg(unix)]
    fn test_shell_executable() -> PathBuf {
        PathBuf::from("sh")
    }

    #[cfg(unix)]
    fn test_shell_stderr_exit_args() -> Vec<String> {
        vec![
            "-c".to_string(),
            "printf 'pi-rpc-boom\\n' >&2; exit 7".to_string(),
        ]
    }
}
