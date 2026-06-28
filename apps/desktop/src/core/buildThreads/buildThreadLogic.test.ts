import test from "node:test";
import assert from "node:assert/strict";
import { createTranslator } from "@sofvary/i18n";
import type {
  BuildThreadDetail,
  BuildThreadSummary,
  GatewayUniEvent,
  WorkspaceSummary,
} from "../../types";
import {
  applyBuildThreadEventBatch,
  appendBuildThreadEntry,
  appendEntryToBuildThreadDetail,
  applyBuildThreadSummaryToDetail,
  canContinueBuildThread,
  formatThreadEntryLabel,
  formatBuildThreadStatus,
  getBuildOverlayViewModel,
  getBuildThreadActivity,
  getWorkspaceBuildThread,
  sortBuildThreads,
  summarizeBuildThreadError,
  summarizeThreadEntryContent,
  upsertBuildThreadSummary,
  visibleThreadEntries,
} from "./buildThreadLogic";

const baseThread: BuildThreadSummary = {
  id: "thread-a",
  title: "倒计时工具",
  status: "queued",
  runtimeKind: "static-html",
  runtimeMode: "dev",
  agentId: "sofvary-pi",
  agentMode: "pi-native",
  createdAt: "2026-06-10T08:00:00Z",
  updatedAt: "2026-06-10T08:00:00Z",
  workspaceId: null,
  appId: null,
  preview: null,
  error: null,
};

function gatewayEvent(input: Partial<GatewayUniEvent> & Pick<GatewayUniEvent, "type">): GatewayUniEvent {
  return {
    eventId: input.eventId ?? "gateway-event-a",
    threadId: input.threadId ?? baseThread.id,
    timestamp: input.timestamp ?? "2026-06-10T08:00:00Z",
    agentId: input.agentId ?? "codex",
    transport: input.transport ?? "cli",
    sequence: input.sequence ?? 1,
    payload: input.payload ?? {},
    type: input.type,
  };
}

test("sortBuildThreads puts the newest updated thread first", () => {
  const older = { ...baseThread, id: "older", updatedAt: "2026-06-10T08:00:00Z" };
  const newer = { ...baseThread, id: "newer", updatedAt: "2026-06-10T09:00:00Z" };

  assert.deepEqual(
    sortBuildThreads([older, newer]).map((thread) => thread.id),
    ["newer", "older"],
  );
});

test("upsertBuildThreadSummary replaces existing thread and keeps newest first", () => {
  const older = { ...baseThread, id: "older", updatedAt: "2026-06-10T08:00:00Z" };
  const current = { ...baseThread, id: "current", updatedAt: "2026-06-10T09:00:00Z" };
  const updatedOlder = { ...older, status: "building" as const, updatedAt: "2026-06-10T10:00:00Z" };

  const result = upsertBuildThreadSummary([older, current], updatedOlder);

  assert.deepEqual(result.map((thread) => thread.id), ["older", "current"]);
  assert.equal(result[0].status, "building");
});

test("formatBuildThreadStatus uses creation lifecycle copy", () => {
  assert.equal(formatBuildThreadStatus(null), "No build tasks");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "planning" }), "Analyzing intent");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "building" }), "Creating software");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "repairing" }), "Auto-repairing runtime issue");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "preview-blocked" }), "Preview environment needs repair");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "failed" }), "Build failed");
});

test("formatBuildThreadStatus supports Chinese translator", () => {
  const t = createTranslator("zh-CN", "desktop");
  assert.equal(formatBuildThreadStatus(null, t), "暂无创建任务");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "planning" }, t), "正在分析意图");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "building" }, t), "正在创建软件");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "repairing" }, t), "正在自动修复运行问题");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "preview-blocked" }, t), "资产已就绪");
  assert.equal(formatBuildThreadStatus({ ...baseThread, status: "failed" }, t), "创建失败");
});

test("summarizeBuildThreadError truncates full thread errors for toast-sized display", () => {
  const error = "x".repeat(260);

  assert.equal(summarizeBuildThreadError({ ...baseThread, error })?.length, 223);
});

test("canContinueBuildThread allows completed, failed, or preview-blocked threads with assets", () => {
  const threadWithApp = { ...baseThread, appId: "app_a", workspaceId: "app_a" };
  assert.equal(canContinueBuildThread({ ...threadWithApp, status: "completed" }), true);
  assert.equal(canContinueBuildThread({ ...threadWithApp, status: "failed" }), true);
  assert.equal(canContinueBuildThread({ ...threadWithApp, status: "preview-blocked" }), true);
  assert.equal(canContinueBuildThread({ ...threadWithApp, status: "building" }), false);
  assert.equal(canContinueBuildThread({ ...threadWithApp, status: "repairing" }), false);
  assert.equal(canContinueBuildThread({ ...baseThread, status: "completed" }), false);
});

test("getWorkspaceBuildThread returns the newest thread associated with a workspace", () => {
  const workspace: WorkspaceSummary = {
    appId: "app_a",
    name: "Local app",
    mode: "static-html",
    updatedAt: "2026-06-10T11:00:00Z",
    root: "/tmp/app_a",
  };
  const older = {
    ...baseThread,
    id: "older",
    appId: "app_a",
    workspaceId: null,
    updatedAt: "2026-06-10T09:00:00Z",
  };
  const newer = {
    ...baseThread,
    id: "newer",
    appId: null,
    workspaceId: "app_a",
    updatedAt: "2026-06-10T10:00:00Z",
  };

  assert.equal(getWorkspaceBuildThread(workspace, [older, newer])?.id, "newer");
  assert.equal(getWorkspaceBuildThread({ ...workspace, appId: "other" }, [older, newer]), null);
});

test("visibleThreadEntries returns persisted dialog entries", () => {
  const detail: BuildThreadDetail = {
    summary: baseThread,
    entries: [
      {
        id: "entry-a",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:00:01Z",
        kind: "agent-event",
        content: "Agent started",
        metadata: {},
      },
    ],
  };

  assert.equal(visibleThreadEntries(detail).length, 1);
  assert.deepEqual(visibleThreadEntries(null), []);
});

test("visibleThreadEntries merges consecutive assistant stream chunks", () => {
  const detail: BuildThreadDetail = {
    summary: baseThread,
    entries: [
      {
        id: "chunk-a",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:00:01Z",
        kind: "assistant",
        content: "Agent message: 正在",
        metadata: {},
      },
      {
        id: "chunk-b",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:00:02Z",
        kind: "assistant",
        content: "生成界面",
        metadata: {},
      },
      {
        id: "file-a",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:00:03Z",
        kind: "file",
        content: "Workspace wrote generated file: index.html",
        metadata: {},
      },
    ],
  };

  const entries = visibleThreadEntries(detail);

  assert.equal(entries.length, 2);
  assert.equal(entries[0].kind, "assistant");
  assert.equal(entries[0].content, "正在 生成界面");
});

test("visibleThreadEntries merges consecutive gateway message deltas into one agent entry", () => {
  const detail: BuildThreadDetail = {
    summary: baseThread,
    entries: [
      {
        id: "chunk-a",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:00:01Z",
        kind: "assistant",
        content: "Agent message: {",
        metadata: {
          gatewayUniEvent: gatewayEvent({
            eventId: "gateway-a",
            type: "message.delta",
            payload: { text: "{" },
          }),
        },
      },
      {
        id: "chunk-b",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:00:02Z",
        kind: "assistant",
        content: '"files":[]}',
        metadata: {
          gatewayUniEvent: gatewayEvent({
            eventId: "gateway-b",
            type: "message.delta",
            sequence: 2,
            payload: { text: '"files":[]}' },
          }),
        },
      },
    ],
  };

  const entries = visibleThreadEntries(detail);
  assert.equal(entries.length, 1);
  const gateway = entries[0]!.metadata!.gatewayUniEvent as GatewayUniEvent | undefined;

  assert.equal(entries[0]!.id, "chunk-a");
  assert.equal(entries[0]!.timestamp, "2026-06-10T08:00:02Z");
  assert.equal(entries[0]!.content, '{"files":[]}');
  assert.deepEqual(entries[0]!.metadata!.mergedEntryIds, ["chunk-a", "chunk-b"]);
  assert.equal(gateway?.eventId, "gateway-a");
  assert.equal(gateway?.sequence, 2);
  assert.equal(gateway?.payload.text, '{"files":[]}');
});

test("visibleThreadEntries does not merge assistant chunks across threads", () => {
  const detail: BuildThreadDetail = {
    summary: baseThread,
    entries: [
      {
        id: "chunk-a",
        threadId: "thread-a",
        timestamp: "2026-06-10T08:00:01Z",
        kind: "assistant",
        content: "Agent message: 线程 A",
        metadata: {},
      },
      {
        id: "chunk-b",
        threadId: "thread-b",
        timestamp: "2026-06-10T08:00:02Z",
        kind: "assistant",
        content: "线程 B",
        metadata: {},
      },
    ],
  };

  const entries = visibleThreadEntries(detail);

  assert.equal(entries.length, 2);
});

test("getBuildThreadActivity summarizes long-running Gateway communication", () => {
  const detail: BuildThreadDetail = {
    summary: { ...baseThread, status: "building", updatedAt: "2026-06-10T08:15:00Z" },
    entries: [
      {
        id: "gateway-a",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:00:01Z",
        kind: "agent-event",
        content: "Session started",
        metadata: {
          gatewayUniEvent: gatewayEvent({
            type: "session.started",
            transport: "pi-rpc",
          }),
        },
      },
      {
        id: "message-a",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:15:00Z",
        kind: "assistant",
        content: "Agent message: 正在生成课程表页面",
        metadata: {
          gatewayUniEvent: gatewayEvent({
            eventId: "gateway-b",
            type: "message.delta",
            timestamp: "2026-06-10T08:15:00Z",
            transport: "pi-rpc",
            sequence: 2,
            payload: { text: "正在生成课程表页面" },
          }),
        },
      },
      {
        id: "file-a",
        threadId: baseThread.id,
        timestamp: "2026-06-10T08:15:01Z",
        kind: "file",
        content: "File written: react/src/App.tsx",
        metadata: {},
      },
    ],
  };

  const activity = getBuildThreadActivity(
    { ...baseThread, status: "building", updatedAt: "2026-06-10T08:15:01Z" },
    detail,
    Date.parse("2026-06-10T08:16:30Z"),
  );

  assert.equal(activity?.eventCount, 3);
  assert.equal(activity?.gatewayEventCount, 2);
  assert.equal(activity?.fileEventCount, 1);
  assert.equal(activity?.transport, "pi-rpc");
  assert.equal(activity?.latestOutputPreview, "正在生成课程表页面");
  assert.equal(activity?.isLongRunning, true);
  assert.equal(activity?.isStale, true);
});

test("appendBuildThreadEntry merges live assistant chunks", () => {
  const first = {
    id: "chunk-a",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:01Z",
    kind: "assistant" as const,
    content: "Agent message: 创建",
    metadata: {},
  };
  const second = {
    id: "chunk-b",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:02Z",
    kind: "assistant" as const,
    content: "完成。",
    metadata: {},
  };

  const entries = appendBuildThreadEntry([first], second);

  assert.equal(entries.length, 1);
  assert.equal(entries[0].content, "创建 完成。");
  assert.equal(formatThreadEntryLabel(entries[0]), "Agent");
});

test("appendEntryToBuildThreadDetail ignores entries for another thread", () => {
  const detail: BuildThreadDetail = {
    summary: baseThread,
    entries: [],
  };
  const entry = {
    id: "foreign",
    threadId: "other-thread",
    timestamp: "2026-06-10T08:00:01Z",
    kind: "system" as const,
    content: "other",
    metadata: {},
  };

  assert.equal(appendEntryToBuildThreadDetail(detail, entry), detail);
});

test("applyBuildThreadSummaryToDetail updates matching summary only", () => {
  const detail: BuildThreadDetail = {
    summary: baseThread,
    entries: [],
  };
  const updated = { ...baseThread, status: "completed" as const };
  const other = { ...baseThread, id: "other", status: "failed" as const };

  assert.equal(applyBuildThreadSummaryToDetail(detail, updated)?.summary.status, "completed");
  assert.equal(applyBuildThreadSummaryToDetail(detail, other), detail);
});

test("applyBuildThreadEventBatch upserts summaries and appends active entries", () => {
  const detail: BuildThreadDetail = {
    summary: baseThread,
    entries: [],
  };
  const updated = {
    ...baseThread,
    status: "building" as const,
    updatedAt: "2026-06-10T08:00:05Z",
  };
  const entry = {
    id: "entry-a",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:06Z",
    kind: "assistant" as const,
    content: "Agent message: 正在生成",
    metadata: {},
  };

  const next = applyBuildThreadEventBatch(
    {
      threads: [baseThread],
      activeThreadId: baseThread.id,
      activeThreadDetail: detail,
    },
    {
      summaries: [updated],
      entries: [entry],
    },
  );

  assert.equal(next.threads[0].status, "building");
  assert.equal(next.activeThreadDetail?.summary.status, "building");
  assert.equal(next.activeThreadDetail?.entries[0].content, "正在生成");
});

test("applyBuildThreadEventBatch can select the first streamed thread", () => {
  const next = applyBuildThreadEventBatch(
    {
      threads: [],
      activeThreadId: null,
      activeThreadDetail: null,
    },
    {
      summaries: [baseThread],
      entries: [],
    },
    { selectFirstThread: true },
  );

  assert.equal(next.activeThreadId, baseThread.id);
});

test("applyBuildThreadEventBatch creates active detail from summary before appending streamed entries", () => {
  const entry = {
    id: "gateway-file",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:02Z",
    kind: "file" as const,
    content: "File written: src/App.tsx",
    metadata: {
      gatewayUniEvent: gatewayEvent({
        type: "file.written",
        payload: { path: "src/App.tsx" },
      }),
    },
  };

  const next = applyBuildThreadEventBatch(
    {
      threads: [baseThread],
      activeThreadId: baseThread.id,
      activeThreadDetail: null,
    },
    {
      summaries: [],
      entries: [entry],
    },
  );

  assert.equal(next.activeThreadDetail?.summary.id, baseThread.id);
  assert.equal(next.activeThreadDetail?.entries.length, 1);
  assert.equal(next.activeThreadDetail?.entries[0].metadata?.gatewayUniEvent, entry.metadata.gatewayUniEvent);
});

test("getBuildOverlayViewModel shows active task and latest entry only while building", () => {
  const entry = {
    id: "entry-a",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:01Z",
    kind: "assistant" as const,
    content: "Agent message: 正在生成布局",
    metadata: {},
  };

  const model = getBuildOverlayViewModel("Building", baseThread, entry);

  assert.equal(model?.title, baseThread.title);
  assert.equal(model?.phase, "Creating software");
  assert.equal(model?.detail, "Agent output is streaming in the Stealth UI session.");
  assert.equal(model?.eventLabel, "sofvary-pi");
  assert.deepEqual(model?.steps.map((step) => [step.id, step.state]), [
    ["intent", "done"],
    ["agent", "active"],
    ["files", "pending"],
    ["preview", "pending"],
  ]);
  assert.equal(getBuildOverlayViewModel("Previewing", baseThread, entry), null);
});

test("getBuildOverlayViewModel keeps streaming Gateway message deltas on the building phase", () => {
  const entry = {
    id: "entry-a",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:01Z",
    kind: "assistant" as const,
    content: "strong",
    metadata: {
      gatewayUniEvent: gatewayEvent({
        type: "message.delta",
        payload: { role: "assistant", text: "strong" },
      }),
    },
  };

  const model = getBuildOverlayViewModel("Building", { ...baseThread, status: "building" }, entry);

  assert.equal(model?.phase, "Creating software");
  assert.equal(model?.detail, "Agent output is streaming in the Stealth UI session.");
  assert.equal(model?.eventLabel, "Agent Gateway");
});

test("getBuildOverlayViewModel surfaces automatic repair phase", () => {
  const repairingThread = { ...baseThread, status: "repairing" as const };
  const entry = {
    id: "entry-a",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:01Z",
    kind: "agent-event" as const,
    content: "Runtime repair attempt 1/2: API failed; Agent can attempt a repair",
    metadata: {},
  };

  const model = getBuildOverlayViewModel("Building", repairingThread, entry);

  assert.equal(model?.phase, "Auto-repairing runtime issue");
  assert.equal(model?.detail, "正在自动修复运行问题 · Sofvary 正在把可修复的运行诊断交给 Agent，并会自动重试预览。");
});

test("getBuildOverlayViewModel surfaces preview-blocked phase as warning", () => {
  const blockedThread: BuildThreadSummary = {
    ...baseThread,
    status: "preview-blocked",
    previewIssue: {
      kind: "managed-pnpm-missing",
      runtimeKind: "react-sqlite",
      summary: "runtime start failed; Sofvary environment setup is required",
      repairAction: "install-runtime-environment",
    },
  };

  const model = getBuildOverlayViewModel("Building", blockedThread, null);

  assert.equal(model?.phase, "Preview environment not ready");
  assert.equal(model?.detail, "runtime start failed; Sofvary environment setup is required");
  assert.deepEqual(model?.steps.map((step) => [step.id, step.state]), [
    ["intent", "done"],
    ["agent", "done"],
    ["files", "done"],
    ["preview", "warning"],
  ]);
});

test("getBuildOverlayViewModel maps Gateway file events to the file stage", () => {
  const entry = {
    id: "entry-a",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:01Z",
    kind: "file" as const,
    content: "File written: src/App.tsx",
    metadata: {
      gatewayUniEvent: gatewayEvent({
        type: "file.written",
        payload: { path: "src/App.tsx" },
      }),
    },
  };

  const model = getBuildOverlayViewModel("Building", { ...baseThread, status: "building" }, entry);

  assert.equal(model?.phase, "Writing generated files");
  assert.equal(model?.eventLabel, "src/App.tsx");
  assert.deepEqual(model?.steps.map((step) => [step.id, step.state]), [
    ["intent", "done"],
    ["agent", "done"],
    ["files", "active"],
    ["preview", "pending"],
  ]);
});

test("getBuildOverlayViewModel hides raw terminal output in the main wait dialog", () => {
  const entry = {
    id: "entry-a",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:01Z",
    kind: "tool" as const,
    content: "stdout: secret verbose terminal line",
    metadata: {
      gatewayUniEvent: gatewayEvent({
        type: "terminal.output",
        payload: { stream: "stdout", text: "secret verbose terminal line" },
      }),
    },
  };

  const model = getBuildOverlayViewModel("Building", { ...baseThread, status: "building" }, entry);

  assert.equal(model?.phase, "Reading terminal output");
  assert.equal(model?.detail, "Terminal detail is available in the Stealth UI session.");
  assert.equal(model?.eventLabel, "stdout");
});

test("getBuildOverlayViewModel uses lightweight activity without raw entries", () => {
  const model = getBuildOverlayViewModel(
    "Building",
    { ...baseThread, status: "building" },
    null,
    undefined,
    {
      threadId: baseThread.id,
      timestamp: "2026-06-10T08:00:02Z",
      agentId: "sofvary-agent",
      transport: "pi-native",
      eventType: "tool.delta",
      phase: "tool",
      safeSummary: "Tool output updated",
      counts: { coalesced: 42, sequence: 42 },
      latestWarning: null,
      latestError: null,
    },
  );

  assert.equal(model?.phase, "Running Agent tool");
  assert.equal(model?.detail, "Tool output updated");
  assert.equal(model?.eventLabel, "pi-native");
});

test("summarizeThreadEntryContent compacts and truncates long entries", () => {
  const entry = {
    id: "entry-a",
    threadId: baseThread.id,
    timestamp: "2026-06-10T08:00:01Z",
    kind: "assistant" as const,
    content: `Agent message: ${"生成 ".repeat(80)}`,
    metadata: {},
  };

  const summary = summarizeThreadEntryContent(entry, 24);

  assert.equal(summary.length, 24);
  assert.ok(summary.endsWith("..."));
});
