import type {
  BuildThreadDetail,
  BuildThreadEntry,
  BuildThreadSummary,
  BuildThreadStatus,
  ShellState,
  WorkspaceSummary,
} from "../../types";
import { summarizeBuildThreadEntryForUser } from "./buildThreadPresentation";

type Translator = (key: string, params?: Record<string, string | number | boolean | null | undefined>, fallback?: string) => string;

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
  return Boolean(
    thread &&
      (thread.status === "completed" || thread.status === "failed") &&
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

  for (const entry of batch.entries ?? []) {
    if (options.selectFirstThread && !activeThreadId) {
      activeThreadId = entry.threadId;
    }
    activeThreadDetail = appendEntryToBuildThreadDetail(activeThreadDetail, entry);
  }

  return {
    threads,
    activeThreadId,
    activeThreadDetail,
  };
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
      merged[merged.length - 1] = {
        ...previous,
        timestamp: normalized.timestamp,
        content: appendEntryContent(previous.content, normalized.content),
        metadata: {
          ...(previous.metadata ?? {}),
          mergedEntryIds: [
            ...asStringArray(previous.metadata?.mergedEntryIds),
            previous.id,
            normalized.id,
          ],
        },
      };
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

  const phase =
    thread?.status === "repairing"
      ? t("build.status.repairing")
      : state === "Planning"
        ? t("build.overlay.planning")
        : t("build.overlay.building");
  return {
    title: thread?.title ?? t("build.overlay.title"),
    phase,
    detail: latestEntry ? summarizeThreadEntryContent(latestEntry, 180) : null,
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
  return left.threadId === right.threadId && left.kind === "assistant" && right.kind === "assistant";
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

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}
