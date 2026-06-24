import type {
  BuildThreadDetail,
  BuildThreadEntry,
  BuildThreadSummary,
  BuildThreadStatus,
  GatewayUniEvent,
  ShellState,
  WorkspaceSummary,
} from "../../types";
import {
  gatewayEventFromEntry,
  summarizeBuildThreadEntryForUser,
} from "./buildThreadPresentation";

type Translator = (key: string, params?: Record<string, string | number | boolean | null | undefined>, fallback?: string) => string;

const LONG_RUNNING_BUILD_MS = 10 * 60 * 1000;
const STALE_BUILD_EVENT_MS = 60 * 1000;

export function sortBuildThreads(threads: BuildThreadSummary[]): BuildThreadSummary[] {
  return [...threads].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
}

export function upsertBuildThreadSummary(
  threads: BuildThreadSummary[],
  thread: BuildThreadSummary,
): BuildThreadSummary[] {
  return sortBuildThreads([thread, ...threads.filter((item) => item.id !== thread.id)]);
}

export function formatBuildThreadStatus(thread: BuildThreadSummary | null, t: Translator = fallbackBuildThreadT): string {
  if (!thread) return t("build.status.empty");
  switch (thread.status) {
    case "queued":
      return t("build.status.queued");
    case "planning":
      return t("build.status.planning");
    case "building":
      return t("build.status.building");
    case "repairing":
      return t("build.status.repairing");
    case "previewing":
      return t("build.status.previewing");
    case "preview-blocked":
      return t("build.status.previewBlocked");
    case "completed":
      return t("build.status.completed");
    case "failed":
      return t("build.status.failed");
    case "canceled":
      return t("build.status.canceled");
  }
}

export function summarizeBuildThreadError(thread: BuildThreadSummary | null): string | null {
  if (!thread?.error) return null;
  return thread.error.length > 220 ? `${thread.error.slice(0, 220)}...` : thread.error;
}

export function canContinueBuildThread(thread: BuildThreadSummary | null): boolean {
  const continuableStatus =
    thread?.status === "completed" ||
    thread?.status === "failed" ||
    thread?.status === "preview-blocked";
  return Boolean(
    thread &&
      continuableStatus &&
      (thread.appId || thread.workspaceId),
  );
}

export function getWorkspaceBuildThread(
  workspace: WorkspaceSummary,
  threads: BuildThreadSummary[],
): BuildThreadSummary | null {
  return (
    sortBuildThreads(threads).find((thread) =>
      buildThreadMatchesWorkspace(thread, workspace.appId),
    ) ?? null
  );
}

export function visibleThreadEntries(detail: BuildThreadDetail | null): BuildThreadEntry[] {
  return mergeBuildThreadEntries(detail?.entries ?? []);
}

export interface BuildThreadActivitySummary {
  eventCount: number;
  gatewayEventCount: number;
  assistantEntryCount: number;
  assistantChars: number;
  fileEventCount: number;
  toolEventCount: number;
  startedAt: string;
  lastEventAt: string | null;
  elapsedMs: number;
  lastEventAgeMs: number | null;
  latestOutputPreview: string | null;
  transport: string | null;
  agentId: string;
  isLongRunning: boolean;
  isStale: boolean;
  hasGatewayEvents: boolean;
}

export function getBuildThreadActivity(
  thread: BuildThreadSummary | null,
  detail: BuildThreadDetail | null,
  nowMs = Date.now(),
): BuildThreadActivitySummary | null {
  if (!thread) return null;

  const entries = detail?.summary.id === thread.id ? detail.entries : [];
  let gatewayEventCount = 0;
  let assistantEntryCount = 0;
  let assistantChars = 0;
  let fileEventCount = 0;
  let toolEventCount = 0;
  let latestAssistant: BuildThreadEntry | null = null;
  let transport: string | null = null;
  let lastEventAt: string | null = null;
  let lastEventMs = timestampMs(thread.updatedAt);

  for (const entry of entries) {
    const entryMs = timestampMs(entry.timestamp);
    if (entryMs !== null && (lastEventMs === null || entryMs >= lastEventMs)) {
      lastEventMs = entryMs;
      lastEventAt = entry.timestamp;
    }

    const gatewayEvent = gatewayEventFromEntry(entry);
    if (gatewayEvent) {
      gatewayEventCount += 1;
      transport = gatewayEvent.transport;
      if (gatewayEvent.type === "message.delta" && entry.kind === "assistant") {
        latestAssistant = entry;
      }
    }

    if (entry.kind === "assistant") {
      assistantEntryCount += 1;
      assistantChars += entry.content.length;
      latestAssistant = entry;
    } else if (entry.kind === "file") {
      fileEventCount += 1;
    } else if (entry.kind === "tool") {
      toolEventCount += 1;
    }
  }

  if (!lastEventAt && lastEventMs !== null) {
    lastEventAt = thread.updatedAt;
  }

  const startedMs = timestampMs(thread.createdAt) ?? nowMs;
  const elapsedMs = Math.max(0, nowMs - startedMs);
  const lastEventAgeMs = lastEventMs === null ? null : Math.max(0, nowMs - lastEventMs);
  const isLive =
    thread.status === "queued" ||
    thread.status === "planning" ||
    thread.status === "building" ||
    thread.status === "repairing" ||
    thread.status === "previewing";

  return {
    eventCount: entries.length,
    gatewayEventCount,
    assistantEntryCount,
    assistantChars,
    fileEventCount,
    toolEventCount,
    startedAt: thread.createdAt,
    lastEventAt,
    elapsedMs,
    lastEventAgeMs,
    latestOutputPreview: latestAssistant ? summarizeThreadEntryContent(latestAssistant, 220) : null,
    transport,
    agentId: thread.agentId,
    isLongRunning: isLive && elapsedMs >= LONG_RUNNING_BUILD_MS,
    isStale: isLive && lastEventAgeMs !== null && lastEventAgeMs >= STALE_BUILD_EVENT_MS,
    hasGatewayEvents: gatewayEventCount > 0,
  };
}

export function applyBuildThreadSummaryToDetail(
  detail: BuildThreadDetail | null,
  thread: BuildThreadSummary,
): BuildThreadDetail | null {
  if (!detail || detail.summary.id !== thread.id) {
    return detail;
  }
  return { ...detail, summary: thread };
}

export function appendEntryToBuildThreadDetail(
  detail: BuildThreadDetail | null,
  entry: BuildThreadEntry,
): BuildThreadDetail | null {
  if (!detail || detail.summary.id !== entry.threadId) {
    return detail;
  }
  return { ...detail, entries: appendBuildThreadEntry(detail.entries, entry) };
}

export interface BuildThreadEventBatch {
  summaries?: BuildThreadSummary[];
  entries?: BuildThreadEntry[];
}

export interface BuildThreadEventState {
  threads: BuildThreadSummary[];
  activeThreadId: string | null;
  activeThreadDetail: BuildThreadDetail | null;
}

export function applyBuildThreadEventBatch(
  state: BuildThreadEventState,
  batch: BuildThreadEventBatch,
  options: { selectFirstThread?: boolean } = {},
): BuildThreadEventState {
  let threads = state.threads;
  let activeThreadId = state.activeThreadId;
  let activeThreadDetail = state.activeThreadDetail;

  for (const summary of batch.summaries ?? []) {
    threads = upsertBuildThreadSummary(threads, summary);
    if (options.selectFirstThread && !activeThreadId) {
      activeThreadId = summary.id;
    }
    activeThreadDetail = applyBuildThreadSummaryToDetail(activeThreadDetail, summary);
  }

  activeThreadDetail = ensureActiveThreadDetail(activeThreadDetail, threads, activeThreadId);

  for (const entry of batch.entries ?? []) {
    if (options.selectFirstThread && !activeThreadId) {
      activeThreadId = entry.threadId;
      activeThreadDetail = ensureActiveThreadDetail(activeThreadDetail, threads, activeThreadId);
    }
    activeThreadDetail = appendEntryToBuildThreadDetail(activeThreadDetail, entry);
  }

  return {
    threads,
    activeThreadId,
    activeThreadDetail,
  };
}

function ensureActiveThreadDetail(
  detail: BuildThreadDetail | null,
  threads: BuildThreadSummary[],
  activeThreadId: string | null,
): BuildThreadDetail | null {
  if (detail || !activeThreadId) {
    return detail;
  }
  const summary = threads.find((thread) => thread.id === activeThreadId);
  return summary ? { summary, entries: [] } : null;
}

export function appendBuildThreadEntry(
  entries: BuildThreadEntry[],
  entry: BuildThreadEntry,
): BuildThreadEntry[] {
  if (entries.some((existing) => existing.id === entry.id)) {
    return entries;
  }
  return mergeBuildThreadEntries([...entries, entry]);
}

export function mergeBuildThreadEntries(entries: BuildThreadEntry[]): BuildThreadEntry[] {
  return entries.reduce<BuildThreadEntry[]>((merged, entry) => {
    const normalized = normalizeThreadEntry(entry);
    const previous = merged[merged.length - 1];
    if (previous && canMergeEntries(previous, normalized)) {
      merged[merged.length - 1] = mergeThreadEntries(previous, normalized);
      return merged;
    }
    merged.push(normalized);
    return merged;
  }, []);
}

export function formatThreadEntryLabel(entry: BuildThreadEntry, t: Translator = fallbackBuildThreadT): string {
  switch (entry.kind) {
    case "user":
      return t("build.entry.user");
    case "assistant":
      return "Agent";
    case "agent-event":
      return t("build.entry.agentEvent");
    case "tool":
      return t("build.entry.tool");
    case "file":
      return t("build.entry.file");
    case "system":
      return t("build.entry.system");
    case "error":
      return t("build.entry.error");
  }
}

export interface BuildOverlayViewModel {
  title: string;
  phase: string;
  detail: string | null;
  eventLabel: string | null;
  actionLabel: string;
  steps: BuildOverlayStep[];
}

export type BuildOverlayStepId = "intent" | "agent" | "files" | "preview";
export type BuildOverlayStepState = "done" | "active" | "pending" | "warning";

export interface BuildOverlayStep {
  id: BuildOverlayStepId;
  label: string;
  state: BuildOverlayStepState;
}

export function getBuildOverlayViewModel(
  state: ShellState,
  thread: BuildThreadSummary | null,
  latestEntry: BuildThreadEntry | null,
  t: Translator = fallbackBuildThreadT,
): BuildOverlayViewModel | null {
  if (state !== "Planning" && state !== "Building") {
    return null;
  }

  const gatewayEvent = latestEntry ? gatewayEventFromEntry(latestEntry) : null;
  const activeStep = getBuildOverlayActiveStep(state, thread, latestEntry, gatewayEvent);
  const activeStepState: BuildOverlayStepState =
    thread?.status === "preview-blocked" ? "warning" : "active";
  const phase = getBuildOverlayPhase(state, thread, gatewayEvent, t);
  return {
    title: thread?.title ?? t("build.overlay.title"),
    phase,
    detail: getBuildOverlayDetail(thread, latestEntry, gatewayEvent, t),
    eventLabel: getBuildOverlayEventLabel(gatewayEvent, t) ?? thread?.agentId ?? null,
    actionLabel:
      thread?.status === "preview-blocked"
        ? t("build.overlay.repairPreview")
        : t("build.overlay.openSession"),
    steps: getBuildOverlaySteps(activeStep, activeStepState, t),
  };
}

function fallbackBuildThreadT(key: string): string {
  const fallback: Record<string, string> = {
    "build.status.empty": "No build tasks",
    "build.status.queued": "Task created",
    "build.status.planning": "Analyzing intent",
    "build.status.building": "Creating software",
    "build.status.repairing": "Auto-repairing runtime issue",
    "build.status.previewing": "Opening preview",
    "build.status.previewBlocked": "Preview environment needs repair",
    "build.status.completed": "Completed",
    "build.status.failed": "Build failed",
    "build.status.canceled": "Canceled",
    "build.entry.user": "You",
    "build.entry.agentEvent": "Process",
    "build.entry.tool": "Tool",
    "build.entry.file": "File",
    "build.entry.system": "System",
    "build.entry.error": "Error",
    "build.overlay.title": "Sofvary is preparing new software",
    "build.overlay.planning": "Analyzing intent",
    "build.overlay.building": "Creating software",
    "build.overlay.intent": "Understanding intent",
    "build.overlay.agent": "Working with Agent",
    "build.overlay.files": "Writing generated files",
    "build.overlay.preview": "Starting preview",
    "build.overlay.previewBlocked": "Preview environment not ready",
    "build.overlay.repairPreview": "Repair preview environment",
    "build.overlay.tool": "Running Agent tool",
    "build.overlay.terminal": "Reading terminal output",
    "build.overlay.approval": "Reviewing approval",
    "build.overlay.reasoning": "Planning implementation",
    "build.overlay.gateway": "Agent Gateway",
    "build.overlay.terminalDetail": "Terminal detail is available in the Stealth UI session.",
    "build.overlay.approvalDetail": "Approval details are available in the Stealth UI session.",
    "build.overlay.reasoningDetail": "Reasoning is folded in the Stealth UI session.",
    "build.overlay.messageDetail": "Agent output is streaming in the Stealth UI session.",
    "build.overlay.previewBlockedDetail": "Software assets are ready. Open the session to repair the Sofvary-managed runtime environment.",
    "build.overlay.openSession": "Open session",
  };
  return fallback[key] ?? key;
}

export function isTerminalBuildThreadStatus(status: BuildThreadStatus): boolean {
  return status === "completed" || status === "failed" || status === "canceled";
}

export function summarizeThreadEntryContent(
  entry: BuildThreadEntry,
  maxLength = 180,
): string {
  return summarizeBuildThreadEntryForUser(entry, maxLength);
}

function getBuildOverlayActiveStep(
  state: ShellState,
  thread: BuildThreadSummary | null,
  latestEntry: BuildThreadEntry | null,
  gatewayEvent: GatewayUniEvent | null,
): BuildOverlayStepId {
  if (thread?.status === "previewing") {
    return "preview";
  }
  if (thread?.status === "preview-blocked") {
    return "preview";
  }
  if (state === "Planning" || (!latestEntry && (thread?.status === "queued" || thread?.status === "planning"))) {
    return "intent";
  }
  if (gatewayEvent?.type === "file.write.requested" || gatewayEvent?.type === "file.written") {
    return "files";
  }
  if (latestEntry?.kind === "file") {
    return "files";
  }
  return "agent";
}

function getBuildOverlayPhase(
  state: ShellState,
  thread: BuildThreadSummary | null,
  gatewayEvent: GatewayUniEvent | null,
  t: Translator,
): string {
  if (thread?.status === "repairing") {
    return t("build.status.repairing");
  }
  if (thread?.status === "preview-blocked") {
    return t("build.overlay.previewBlocked");
  }
  if (!gatewayEvent) {
    return state === "Planning" ? t("build.overlay.planning") : t("build.overlay.building");
  }

  switch (gatewayEvent.type) {
    case "tool.started":
    case "tool.delta":
    case "tool.completed":
      return t("build.overlay.tool");
    case "terminal.output":
      return t("build.overlay.terminal");
    case "approval.requested":
    case "approval.resolved":
      return t("build.overlay.approval");
    case "file.write.requested":
    case "file.written":
      return t("build.overlay.files");
    case "reasoning.delta":
      return t("build.overlay.reasoning");
    case "status.changed":
      return formatGatewayStatusPhase(gatewayEvent, t);
    case "turn.completed":
      return t("build.overlay.preview");
    case "error":
      return t("build.status.failed");
    case "session.started":
    case "turn.started":
      return t("build.overlay.agent");
    case "message.delta":
      return t("build.overlay.building");
  }
}

function getBuildOverlayDetail(
  thread: BuildThreadSummary | null,
  latestEntry: BuildThreadEntry | null,
  gatewayEvent: GatewayUniEvent | null,
  t: Translator,
): string | null {
  if (thread?.status === "preview-blocked") {
    return thread.previewIssue?.summary ?? t("build.overlay.previewBlockedDetail");
  }
  if (!latestEntry) {
    return null;
  }
  if (!gatewayEvent) {
    if (latestEntry.kind === "assistant") {
      return t("build.overlay.messageDetail");
    }
    return summarizeThreadEntryContent(latestEntry, 180);
  }
  switch (gatewayEvent.type) {
    case "terminal.output":
      return t("build.overlay.terminalDetail");
    case "approval.requested":
    case "approval.resolved":
      return t("build.overlay.approvalDetail");
    case "reasoning.delta":
      return t("build.overlay.reasoningDetail");
    case "message.delta":
      return t("build.overlay.messageDetail");
    case "status.changed":
      return payloadString(gatewayEvent, "detail") ?? summarizeThreadEntryContent(latestEntry, 180);
    default:
      return summarizeThreadEntryContent(latestEntry, 180);
  }
}

function getBuildOverlayEventLabel(gatewayEvent: GatewayUniEvent | null, t: Translator): string | null {
  if (!gatewayEvent) {
    return null;
  }
  switch (gatewayEvent.type) {
    case "tool.started":
    case "tool.delta":
    case "tool.completed":
      return payloadString(gatewayEvent, "toolName") ?? t("build.overlay.tool");
    case "terminal.output":
      return payloadString(gatewayEvent, "stream") ?? t("build.overlay.terminal");
    case "approval.requested":
    case "approval.resolved":
      return t("build.overlay.approval");
    case "file.write.requested":
    case "file.written":
      return payloadString(gatewayEvent, "path") ?? t("build.overlay.files");
    default:
      return t("build.overlay.gateway");
  }
}

function getBuildOverlaySteps(
  activeStep: BuildOverlayStepId,
  activeStepState: BuildOverlayStepState,
  t: Translator,
): BuildOverlayStep[] {
  const steps: Array<{ id: BuildOverlayStepId; label: string }> = [
    { id: "intent", label: t("build.overlay.intent") },
    { id: "agent", label: t("build.overlay.agent") },
    { id: "files", label: t("build.overlay.files") },
    { id: "preview", label: t("build.overlay.preview") },
  ];
  const activeIndex = steps.findIndex((step) => step.id === activeStep);
  return steps.map((step, index) => ({
    ...step,
    state: index < activeIndex ? "done" : index === activeIndex ? activeStepState : "pending",
  }));
}

function formatGatewayStatusPhase(event: GatewayUniEvent, t: Translator): string {
  const phase = payloadString(event, "phase");
  switch (phase) {
    case "connecting":
    case "session":
      return t("build.overlay.agent");
    case "planning":
      return t("build.overlay.reasoning");
    case "generating":
    case "terminal":
    case "agent-item":
      return t("build.overlay.building");
    default:
      return phase ?? t("build.overlay.building");
  }
}

function payloadString(event: GatewayUniEvent, key: string): string | null {
  const value = event.payload[key];
  return typeof value === "string" && value.trim() ? value : null;
}

function timestampMs(value: string | null | undefined): number | null {
  if (!value) return null;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? timestamp : null;
}

function normalizeThreadEntry(entry: BuildThreadEntry): BuildThreadEntry {
  if (entry.kind !== "assistant") {
    return entry;
  }
  return {
    ...entry,
    content: stripAgentMessagePrefix(entry.content),
  };
}

function buildThreadMatchesWorkspace(thread: BuildThreadSummary, appId: string): boolean {
  const previewAppId = thread.preview?.appId ?? thread.preview?.manifest.appId ?? null;
  return thread.appId === appId || thread.workspaceId === appId || previewAppId === appId;
}

function stripAgentMessagePrefix(content: string): string {
  return content.replace(/^Agent message:\s*/i, "");
}

function canMergeEntries(left: BuildThreadEntry, right: BuildThreadEntry): boolean {
  const leftGatewayEvent = gatewayEventFromEntry(left);
  const rightGatewayEvent = gatewayEventFromEntry(right);
  return (
    left.threadId === right.threadId &&
    left.kind === "assistant" &&
    right.kind === "assistant" &&
    ((!leftGatewayEvent && !rightGatewayEvent) ||
      Boolean(
        leftGatewayEvent &&
          rightGatewayEvent &&
          canMergeGatewayMessageDeltas(leftGatewayEvent, rightGatewayEvent),
      ))
  );
}

function mergeThreadEntries(left: BuildThreadEntry, right: BuildThreadEntry): BuildThreadEntry {
  const leftGatewayEvent = gatewayEventFromEntry(left);
  const rightGatewayEvent = gatewayEventFromEntry(right);
  if (
    leftGatewayEvent &&
    rightGatewayEvent &&
    canMergeGatewayMessageDeltas(leftGatewayEvent, rightGatewayEvent)
  ) {
    return mergeGatewayMessageDeltaEntries(left, right, leftGatewayEvent, rightGatewayEvent);
  }

  return {
    ...left,
    timestamp: right.timestamp,
    content: appendEntryContent(left.content, right.content),
    metadata: mergedEntryMetadata(left, right),
  };
}

function canMergeGatewayMessageDeltas(
  left: GatewayUniEvent,
  right: GatewayUniEvent,
): boolean {
  return (
    left.type === "message.delta" &&
    right.type === "message.delta" &&
    left.threadId === right.threadId &&
    left.agentId === right.agentId &&
    left.transport === right.transport
  );
}

function mergeGatewayMessageDeltaEntries(
  left: BuildThreadEntry,
  right: BuildThreadEntry,
  leftEvent: GatewayUniEvent,
  rightEvent: GatewayUniEvent,
): BuildThreadEntry {
  const mergedText = appendStreamingContent(
    gatewayDeltaText(leftEvent) ?? left.content,
    gatewayDeltaText(rightEvent) ?? right.content,
  );
  const payload: Record<string, unknown> = {
    ...leftEvent.payload,
    ...rightEvent.payload,
    text: mergedText,
  };

  if (hasPayloadString(leftEvent, "content") || hasPayloadString(rightEvent, "content")) {
    payload.content = mergedText;
  }

  return {
    ...left,
    timestamp: right.timestamp,
    content: appendStreamingContent(left.content, right.content),
    metadata: {
      ...mergedEntryMetadata(left, right),
      gatewayUniEvent: {
        ...leftEvent,
        timestamp: rightEvent.timestamp,
        sequence: rightEvent.sequence,
        payload,
      },
    },
  };
}

function appendEntryContent(left: string, right: string): string {
  const next = right.trim();
  if (!next) return left;
  if (!left) return next;
  if (/\s$/.test(left) || /^[.,;:!?)}\]。，；：！？]/.test(next)) {
    return `${left}${next}`;
  }
  return `${left} ${next}`;
}

function appendStreamingContent(left: string, right: string): string {
  if (!left) return right;
  if (!right) return left;
  return `${left}${right}`;
}

function gatewayDeltaText(event: GatewayUniEvent): string | null {
  const text = event.payload.text;
  if (typeof text === "string") return text;
  const content = event.payload.content;
  if (typeof content === "string") return content;
  return null;
}

function hasPayloadString(event: GatewayUniEvent, key: string): boolean {
  return typeof event.payload[key] === "string";
}

function mergedEntryMetadata(
  left: BuildThreadEntry,
  right: BuildThreadEntry,
): Record<string, unknown> {
  return {
    ...(left.metadata ?? {}),
    ...(right.metadata ?? {}),
    mergedEntryIds: uniqueStrings([
      ...asStringArray(left.metadata?.mergedEntryIds),
      left.id,
      ...asStringArray(right.metadata?.mergedEntryIds),
      right.id,
    ]),
  };
}

function uniqueStrings(values: string[]): string[] {
  return values.filter((value, index) => values.indexOf(value) === index);
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}
