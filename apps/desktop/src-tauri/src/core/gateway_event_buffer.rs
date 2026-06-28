use crate::core::gateway_uni_event::{GatewayUniEvent, GatewayUniEventType};
use serde_json::{Map, Value};
use std::time::{Duration, Instant};

const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_millis(350);
const DEFAULT_MAX_PENDING_EVENTS: usize = 32;
const DEFAULT_MAX_PENDING_CHARS: usize = 6_000;

#[derive(Debug)]
pub struct GatewayUniEventBuffer {
    pending: Vec<GatewayUniEvent>,
    last_flush: Instant,
    flush_interval: Duration,
    max_pending_events: usize,
    max_pending_chars: usize,
}

impl Default for GatewayUniEventBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayUniEventBuffer {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            last_flush: Instant::now(),
            flush_interval: DEFAULT_FLUSH_INTERVAL,
            max_pending_events: DEFAULT_MAX_PENDING_EVENTS,
            max_pending_chars: DEFAULT_MAX_PENDING_CHARS,
        }
    }

    #[cfg(test)]
    fn with_limits(
        flush_interval: Duration,
        max_pending_events: usize,
        max_pending_chars: usize,
    ) -> Self {
        Self {
            pending: Vec::new(),
            last_flush: Instant::now(),
            flush_interval,
            max_pending_events,
            max_pending_chars,
        }
    }

    pub fn push(&mut self, event: GatewayUniEvent) -> Vec<GatewayUniEvent> {
        if is_immediate_event(event.event_type) {
            let mut flushed = self.flush();
            flushed.push(event);
            self.last_flush = Instant::now();
            return flushed;
        }

        if let Some(key) = coalesce_key(&event) {
            if let Some(existing) = self
                .pending
                .iter_mut()
                .find(|candidate| coalesce_key(candidate).as_deref() == Some(key.as_str()))
            {
                merge_gateway_event(existing, event);
            } else {
                self.pending.push(mark_single_event(event));
            }
        } else {
            self.pending.push(event);
        }

        if self.should_flush() {
            return self.flush();
        }

        Vec::new()
    }

    pub fn flush(&mut self) -> Vec<GatewayUniEvent> {
        self.last_flush = Instant::now();
        std::mem::take(&mut self.pending)
    }

    fn should_flush(&self) -> bool {
        !self.pending.is_empty()
            && (self.last_flush.elapsed() >= self.flush_interval
                || self.pending.len() >= self.max_pending_events
                || pending_char_count(&self.pending) >= self.max_pending_chars)
    }
}

fn is_immediate_event(event_type: GatewayUniEventType) -> bool {
    matches!(
        event_type,
        GatewayUniEventType::SessionStarted
            | GatewayUniEventType::TurnStarted
            | GatewayUniEventType::ToolStarted
            | GatewayUniEventType::ToolCompleted
            | GatewayUniEventType::ApprovalRequested
            | GatewayUniEventType::ApprovalResolved
            | GatewayUniEventType::FileWriteRequested
            | GatewayUniEventType::FileWritten
            | GatewayUniEventType::TurnCompleted
            | GatewayUniEventType::Error
    )
}

fn coalesce_key(event: &GatewayUniEvent) -> Option<String> {
    match event.event_type {
        GatewayUniEventType::MessageDelta => Some("message.delta".to_string()),
        GatewayUniEventType::ReasoningDelta => Some("reasoning.delta".to_string()),
        GatewayUniEventType::TerminalOutput => Some(format!(
            "terminal.output:{}",
            event
                .payload
                .get("stream")
                .and_then(Value::as_str)
                .unwrap_or("stdout")
        )),
        GatewayUniEventType::ToolDelta => Some(format!(
            "tool.delta:{}:{}",
            event
                .payload
                .get("callId")
                .and_then(Value::as_str)
                .unwrap_or(""),
            event
                .payload
                .get("toolName")
                .and_then(Value::as_str)
                .unwrap_or("tool")
        )),
        GatewayUniEventType::StatusChanged => Some("status.changed".to_string()),
        _ => None,
    }
}

fn merge_gateway_event(existing: &mut GatewayUniEvent, next: GatewayUniEvent) {
    let existing_sequence = existing.sequence;
    let next_sequence = next.sequence;
    let existing_payload = ensure_payload_object(&mut existing.payload);
    let coalesced_count = existing_payload
        .get("_sofvaryCoalescedCount")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        + 1;
    let first_sequence = existing_payload
        .get("_sofvaryFirstSequence")
        .and_then(Value::as_u64)
        .unwrap_or(existing_sequence);

    match existing.event_type {
        GatewayUniEventType::MessageDelta | GatewayUniEventType::ReasoningDelta => {
            append_payload_text(&mut existing.payload, &next.payload, "text", "");
        }
        GatewayUniEventType::TerminalOutput => {
            append_payload_text(&mut existing.payload, &next.payload, "text", "\n");
        }
        GatewayUniEventType::ToolDelta => {
            merge_tool_delta_payload(&mut existing.payload, &next.payload);
        }
        GatewayUniEventType::StatusChanged => {
            existing.payload = next.payload;
        }
        _ => {
            existing.payload = next.payload;
        }
    }

    existing.timestamp = next.timestamp;
    existing.sequence = next_sequence;
    mark_coalesced(existing, coalesced_count, first_sequence, next_sequence);
}

fn mark_single_event(mut event: GatewayUniEvent) -> GatewayUniEvent {
    let sequence = event.sequence;
    mark_coalesced(&mut event, 1, sequence, sequence);
    event
}

fn mark_coalesced(
    event: &mut GatewayUniEvent,
    count: u64,
    first_sequence: u64,
    last_sequence: u64,
) {
    let payload = ensure_payload_object(&mut event.payload);
    payload.insert("_sofvaryCoalescedCount".to_string(), Value::from(count));
    payload.insert(
        "_sofvaryFirstSequence".to_string(),
        Value::from(first_sequence),
    );
    payload.insert(
        "_sofvaryLastSequence".to_string(),
        Value::from(last_sequence),
    );
}

pub fn take_coalesced_metadata(
    event: &mut GatewayUniEvent,
) -> (Option<u64>, Option<u64>, Option<u64>) {
    let Some(payload) = event.payload.as_object_mut() else {
        return (None, None, None);
    };
    let count = payload
        .remove("_sofvaryCoalescedCount")
        .and_then(|value| value.as_u64());
    let first_sequence = payload
        .remove("_sofvaryFirstSequence")
        .and_then(|value| value.as_u64());
    let last_sequence = payload
        .remove("_sofvaryLastSequence")
        .and_then(|value| value.as_u64());
    (count, first_sequence, last_sequence)
}

fn append_payload_text(
    existing_payload: &mut Value,
    next_payload: &Value,
    key: &str,
    separator: &str,
) {
    let next_text = next_payload
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default();
    let payload = ensure_payload_object(existing_payload);
    let current = payload.get(key).and_then(Value::as_str).unwrap_or_default();
    let separator = if current.is_empty() || next_text.is_empty() {
        ""
    } else {
        separator
    };
    payload.insert(
        key.to_string(),
        Value::from(format!("{current}{separator}{next_text}")),
    );
}

fn merge_tool_delta_payload(existing_payload: &mut Value, next_payload: &Value) {
    let payload = ensure_payload_object(existing_payload);
    if let Some(partial_result) = next_payload.get("partialResult") {
        match (
            payload.get("partialResult").and_then(Value::as_str),
            partial_result.as_str(),
        ) {
            (Some(current), Some(next)) if !current.is_empty() && !next.is_empty() => {
                payload.insert(
                    "partialResult".to_string(),
                    Value::from(format!("{current}\n{next}")),
                );
            }
            _ => {
                payload.insert("partialResult".to_string(), partial_result.clone());
            }
        }
    }
}

fn ensure_payload_object(payload: &mut Value) -> &mut Map<String, Value> {
    if !payload.is_object() {
        *payload = Value::Object(Map::new());
    }
    payload.as_object_mut().expect("payload object")
}

fn pending_char_count(events: &[GatewayUniEvent]) -> usize {
    events
        .iter()
        .map(|event| match event.event_type {
            GatewayUniEventType::MessageDelta
            | GatewayUniEventType::ReasoningDelta
            | GatewayUniEventType::TerminalOutput => event
                .payload
                .get("text")
                .and_then(Value::as_str)
                .map(str::len)
                .unwrap_or_default(),
            GatewayUniEventType::ToolDelta => event
                .payload
                .get("partialResult")
                .and_then(Value::as_str)
                .map(str::len)
                .unwrap_or_default(),
            GatewayUniEventType::StatusChanged => event
                .payload
                .get("summary")
                .or_else(|| event.payload.get("detail"))
                .and_then(Value::as_str)
                .map(str::len)
                .unwrap_or_default(),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agent_config::AgentTransportKind;
    use chrono::Utc;
    use serde_json::json;

    fn event(sequence: u64, event_type: GatewayUniEventType, payload: Value) -> GatewayUniEvent {
        GatewayUniEvent {
            event_id: format!("event-{sequence}"),
            thread_id: "thread-a".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            agent_id: "sofvary-agent".to_string(),
            transport: AgentTransportKind::PiNative,
            sequence,
            event_type,
            payload,
        }
    }

    #[test]
    fn coalesces_message_deltas_until_flush() {
        let mut buffer = GatewayUniEventBuffer::with_limits(Duration::from_secs(60), 10, 100);

        assert!(buffer
            .push(event(
                1,
                GatewayUniEventType::MessageDelta,
                json!({ "text": "hello " })
            ))
            .is_empty());
        assert!(buffer
            .push(event(
                2,
                GatewayUniEventType::MessageDelta,
                json!({ "text": "world" })
            ))
            .is_empty());

        let mut events = buffer.flush();
        assert_eq!(events.len(), 1);
        let mut merged = events.pop().expect("merged");
        let metadata = take_coalesced_metadata(&mut merged);
        assert_eq!(merged.sequence, 2);
        assert_eq!(
            merged.payload.get("text").and_then(Value::as_str),
            Some("hello world")
        );
        assert_eq!(metadata, (Some(2), Some(1), Some(2)));
    }

    #[test]
    fn immediate_event_flushes_pending_events_first() {
        let mut buffer = GatewayUniEventBuffer::with_limits(Duration::from_secs(60), 10, 100);
        let _ = buffer.push(event(
            1,
            GatewayUniEventType::ReasoningDelta,
            json!({ "text": "thinking" }),
        ));

        let events = buffer.push(event(
            2,
            GatewayUniEventType::ToolCompleted,
            json!({ "toolName": "workspace_write" }),
        ));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, GatewayUniEventType::ReasoningDelta);
        assert_eq!(events[1].event_type, GatewayUniEventType::ToolCompleted);
    }

    #[test]
    fn keeps_latest_status_changed() {
        let mut buffer = GatewayUniEventBuffer::with_limits(Duration::from_secs(60), 10, 100);
        let _ = buffer.push(event(
            1,
            GatewayUniEventType::StatusChanged,
            json!({ "status": "working", "summary": "one" }),
        ));
        let _ = buffer.push(event(
            2,
            GatewayUniEventType::StatusChanged,
            json!({ "status": "working", "summary": "two" }),
        ));

        let events = buffer.flush();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].payload.get("summary").and_then(Value::as_str),
            Some("two")
        );
    }
}
