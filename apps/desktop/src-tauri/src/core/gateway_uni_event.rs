use crate::core::agent_config::AgentTransportKind;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use uuid::Uuid;

pub type GatewayUniEventSink = Arc<dyn Fn(GatewayUniEvent) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GatewayUniEventType {
    #[serde(rename = "session.started")]
    SessionStarted,
    #[serde(rename = "turn.started")]
    TurnStarted,
    #[serde(rename = "message.delta")]
    MessageDelta,
    #[serde(rename = "reasoning.delta")]
    ReasoningDelta,
    #[serde(rename = "tool.started")]
    ToolStarted,
    #[serde(rename = "tool.delta")]
    ToolDelta,
    #[serde(rename = "tool.completed")]
    ToolCompleted,
    #[serde(rename = "approval.requested")]
    ApprovalRequested,
    #[serde(rename = "approval.resolved")]
    ApprovalResolved,
    #[serde(rename = "terminal.output")]
    TerminalOutput,
    #[serde(rename = "file.write.requested")]
    FileWriteRequested,
    #[serde(rename = "file.written")]
    FileWritten,
    #[serde(rename = "status.changed")]
    StatusChanged,
    #[serde(rename = "turn.completed")]
    TurnCompleted,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayUniEvent {
    pub event_id: String,
    pub thread_id: String,
    pub timestamp: String,
    pub agent_id: String,
    pub transport: AgentTransportKind,
    pub sequence: u64,
    #[serde(rename = "type")]
    pub event_type: GatewayUniEventType,
    pub payload: Value,
}

#[derive(Clone)]
pub struct GatewayUniEventEmitter {
    thread_id: String,
    agent_id: String,
    transport: AgentTransportKind,
    sequence: Arc<AtomicU64>,
    sink: GatewayUniEventSink,
}

impl GatewayUniEventEmitter {
    pub fn new(
        thread_id: impl Into<String>,
        agent_id: impl Into<String>,
        transport: AgentTransportKind,
        sink: GatewayUniEventSink,
    ) -> Self {
        Self {
            thread_id: thread_id.into(),
            agent_id: agent_id.into(),
            transport,
            sequence: Arc::new(AtomicU64::new(0)),
            sink,
        }
    }

    pub fn emit(&self, event_type: GatewayUniEventType, payload: Value) -> GatewayUniEvent {
        let event = GatewayUniEvent {
            event_id: format!("gateway_event_{}", Uuid::new_v4()),
            thread_id: self.thread_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
            agent_id: self.agent_id.clone(),
            transport: self.transport,
            sequence: self.sequence.fetch_add(1, Ordering::SeqCst) + 1,
            event_type,
            payload,
        };
        (self.sink)(event.clone());
        event
    }

    pub fn session_started(&self, label: &str) {
        self.emit(
            GatewayUniEventType::SessionStarted,
            json!({ "label": label }),
        );
    }

    pub fn turn_started(&self, prompt_id: impl Into<String>) {
        self.emit(
            GatewayUniEventType::TurnStarted,
            json!({ "promptId": prompt_id.into() }),
        );
    }

    pub fn status(&self, phase: impl Into<String>, detail: impl Into<String>) {
        self.emit(
            GatewayUniEventType::StatusChanged,
            json!({ "phase": phase.into(), "detail": detail.into() }),
        );
    }

    pub fn message_delta(&self, text: impl Into<String>) {
        self.emit(
            GatewayUniEventType::MessageDelta,
            json!({ "role": "assistant", "text": text.into() }),
        );
    }

    pub fn reasoning_delta(&self, text: impl Into<String>) {
        self.emit(
            GatewayUniEventType::ReasoningDelta,
            json!({ "text": text.into() }),
        );
    }

    pub fn terminal_output(&self, stream: &str, text: impl Into<String>) {
        self.emit(
            GatewayUniEventType::TerminalOutput,
            json!({ "stream": stream, "text": text.into() }),
        );
    }

    pub fn error(&self, message: impl Into<String>) {
        self.emit(
            GatewayUniEventType::Error,
            json!({ "message": message.into() }),
        );
    }

    pub fn turn_completed(&self, status: &str) {
        self.emit(
            GatewayUniEventType::TurnCompleted,
            json!({ "status": status }),
        );
    }
}

pub fn gateway_uni_event_summary(event: &GatewayUniEvent) -> String {
    match event.event_type {
        GatewayUniEventType::SessionStarted => format!(
            "{} session started",
            event
                .payload
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or(event.agent_id.as_str())
        ),
        GatewayUniEventType::TurnStarted => "Agent turn started".to_string(),
        GatewayUniEventType::MessageDelta => event
            .payload
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("Agent responded")
            .to_string(),
        GatewayUniEventType::ReasoningDelta => "Agent reasoning updated".to_string(),
        GatewayUniEventType::ToolStarted => format!(
            "Tool started: {}",
            event
                .payload
                .get("toolName")
                .and_then(Value::as_str)
                .unwrap_or("tool")
        ),
        GatewayUniEventType::ToolDelta => "Tool output updated".to_string(),
        GatewayUniEventType::ToolCompleted => format!(
            "Tool completed: {}",
            event
                .payload
                .get("toolName")
                .and_then(Value::as_str)
                .unwrap_or("tool")
        ),
        GatewayUniEventType::ApprovalRequested => format!(
            "Approval requested: {}",
            event
                .payload
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("agent action")
        ),
        GatewayUniEventType::ApprovalResolved => "Approval resolved".to_string(),
        GatewayUniEventType::TerminalOutput => {
            let stream = event
                .payload
                .get("stream")
                .and_then(Value::as_str)
                .unwrap_or("stdout");
            let text = event
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!("{stream}: {}", summarize_text(text, 160))
        }
        GatewayUniEventType::FileWriteRequested => format!(
            "File write requested: {}",
            event
                .payload
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("generated file")
        ),
        GatewayUniEventType::FileWritten => format!(
            "File written: {}",
            event
                .payload
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("generated file")
        ),
        GatewayUniEventType::StatusChanged => event
            .payload
            .get("detail")
            .and_then(Value::as_str)
            .or_else(|| event.payload.get("phase").and_then(Value::as_str))
            .unwrap_or("Agent status changed")
            .to_string(),
        GatewayUniEventType::TurnCompleted => format!(
            "Agent turn completed: {}",
            event
                .payload
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("ok")
        ),
        GatewayUniEventType::Error => event
            .payload
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Agent error")
            .to_string(),
    }
}

fn summarize_text(value: &str, max_chars: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn emitter_assigns_monotonic_sequence() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let emitter = GatewayUniEventEmitter::new(
            "thread-a",
            "codex",
            AgentTransportKind::Cli,
            Arc::new(move |event| captured.lock().expect("events").push(event)),
        );

        emitter.turn_started("prompt-a");
        emitter.message_delta("hello");

        let events = events.lock().expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[1].sequence, 2);
        assert_eq!(events[0].thread_id, "thread-a");
        assert_eq!(events[1].agent_id, "codex");
    }
}
