import type { BuildThreadEntry, BuildThreadSummary, GatewayUniEvent } from "../../types";

export interface BuildThreadEventBatch {
  summaries: BuildThreadSummary[];
  entries: BuildThreadEntry[];
}

export class BuildThreadEventBatcher {
  private summaries = new Map<string, BuildThreadSummary>();
  private entries: BuildThreadEntry[] = [];
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly flushBatch: (batch: BuildThreadEventBatch) => void,
    private readonly delayMs = 64,
    private readonly maxPendingEntries = 120,
  ) {}

  pushSummary(summary: BuildThreadSummary): void {
    this.summaries.set(summary.id, summary);
    this.flushSoon();
  }

  pushEntry(entry: BuildThreadEntry): void {
    const previous = this.entries[this.entries.length - 1] ?? null;
    const merged = previous ? mergePendingGatewayEntry(previous, entry) : null;
    if (merged) {
      this.entries[this.entries.length - 1] = merged;
    } else {
      this.entries.push(entry);
    }
    if (this.entries.length >= this.maxPendingEntries) {
      this.flush();
      return;
    }
    this.flushSoon();
  }

  flush(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }

    if (this.summaries.size === 0 && this.entries.length === 0) {
      return;
    }

    const batch = {
      summaries: [...this.summaries.values()],
      entries: this.entries,
    };
    this.summaries = new Map();
    this.entries = [];
    this.flushBatch(batch);
  }

  dispose(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.summaries.clear();
    this.entries = [];
  }

  private flushSoon(): void {
    if (this.timer) {
      return;
    }

    this.timer = setTimeout(() => this.flush(), this.delayMs);
  }
}

function mergePendingGatewayEntry(
  previous: BuildThreadEntry,
  next: BuildThreadEntry,
): BuildThreadEntry | null {
  const previousEvent = gatewayEventFromEntry(previous);
  const nextEvent = gatewayEventFromEntry(next);
  if (!previousEvent || !nextEvent || !canCoalesceGatewayEvents(previousEvent, nextEvent)) {
    return null;
  }

  const event = mergeGatewayEvents(previousEvent, nextEvent);
  return {
    ...next,
    id: previous.id,
    timestamp: next.timestamp,
    kind: previous.kind,
    content: contentFromGatewayEvent(event, `${previous.content}${next.content}`),
    metadata: {
      ...next.metadata,
      gatewayUniEvent: event,
      mergedEntryIds: mergeEntryIds(previous, next),
      coalescedCount:
        Number(previous.metadata?.coalescedCount ?? 1) +
        Number(next.metadata?.coalescedCount ?? 1),
      firstSequence: Number(previous.metadata?.firstSequence ?? previousEvent.sequence),
      lastSequence: Number(next.metadata?.lastSequence ?? nextEvent.sequence),
    },
  };
}

function gatewayEventFromEntry(entry: BuildThreadEntry): GatewayUniEvent | null {
  const event = entry.metadata?.gatewayUniEvent;
  return event && typeof event === "object" ? (event as GatewayUniEvent) : null;
}

function canCoalesceGatewayEvents(left: GatewayUniEvent, right: GatewayUniEvent): boolean {
  if (left.threadId !== right.threadId || left.type !== right.type) {
    return false;
  }
  switch (left.type) {
    case "message.delta":
    case "reasoning.delta":
    case "status.changed":
      return true;
    case "terminal.output":
      return stringPayload(left, "stream") === stringPayload(right, "stream");
    case "tool.delta":
      return (
        stringPayload(left, "callId") === stringPayload(right, "callId") &&
        stringPayload(left, "toolName") === stringPayload(right, "toolName")
      );
    default:
      return false;
  }
}

function mergeGatewayEvents(left: GatewayUniEvent, right: GatewayUniEvent): GatewayUniEvent {
  const payload = { ...left.payload };
  switch (left.type) {
    case "message.delta":
    case "reasoning.delta":
      payload.text = `${stringPayload(left, "text")}${stringPayload(right, "text")}`;
      break;
    case "terminal.output": {
      const separator = stringPayload(left, "text") && stringPayload(right, "text") ? "\n" : "";
      payload.text = `${stringPayload(left, "text")}${separator}${stringPayload(right, "text")}`;
      break;
    }
    case "tool.delta": {
      const leftResult = stringPayload(left, "partialResult");
      const rightResult = stringPayload(right, "partialResult");
      payload.partialResult = leftResult && rightResult ? `${leftResult}\n${rightResult}` : rightResult || leftResult;
      break;
    }
    case "status.changed":
      return { ...right, payload: { ...right.payload } };
  }
  return {
    ...left,
    timestamp: right.timestamp,
    sequence: right.sequence,
    payload,
  };
}

function contentFromGatewayEvent(event: GatewayUniEvent, fallback: string): string {
  switch (event.type) {
    case "message.delta":
    case "reasoning.delta":
      return stringPayload(event, "text") || fallback;
    case "terminal.output":
      return `${stringPayload(event, "stream") || "stdout"}: ${stringPayload(event, "text")}`;
    case "tool.delta":
      return stringPayload(event, "partialResult") || fallback;
    case "status.changed":
      return stringPayload(event, "summary") || stringPayload(event, "detail") || fallback;
    default:
      return fallback;
  }
}

function mergeEntryIds(previous: BuildThreadEntry, next: BuildThreadEntry): string[] {
  const previousIds = Array.isArray(previous.metadata?.mergedEntryIds)
    ? previous.metadata?.mergedEntryIds.filter((id): id is string => typeof id === "string")
    : [previous.id];
  const nextIds = Array.isArray(next.metadata?.mergedEntryIds)
    ? next.metadata?.mergedEntryIds.filter((id): id is string => typeof id === "string")
    : [next.id];
  return [...previousIds, ...nextIds];
}

function stringPayload(event: GatewayUniEvent, key: string): string {
  const value = event.payload[key];
  return typeof value === "string" ? value : "";
}
