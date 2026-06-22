import test from "node:test";
import assert from "node:assert/strict";
import type { BuildThreadEntry, GatewayUniEvent } from "../../types";
import {
  mergeBuildThreadPresentationItems,
  presentBuildThreadEntry,
  summarizeBuildThreadEntryForUser,
} from "./buildThreadPresentation";

function entry(input: Partial<BuildThreadEntry> & Pick<BuildThreadEntry, "kind" | "content">) {
  return {
    id: input.id ?? "entry-a",
    threadId: input.threadId ?? "thread-a",
    timestamp: input.timestamp ?? "2026-06-10T08:00:00Z",
    metadata: input.metadata ?? {},
    ...input,
  };
}

function gatewayEvent(input: Partial<GatewayUniEvent> & Pick<GatewayUniEvent, "type">): GatewayUniEvent {
  return {
    eventId: input.eventId ?? "gateway-event-a",
    threadId: input.threadId ?? "thread-a",
    timestamp: input.timestamp ?? "2026-06-10T08:00:00Z",
    agentId: input.agentId ?? "codex",
    transport: input.transport ?? "cli",
    sequence: input.sequence ?? 1,
    payload: input.payload ?? {},
    type: input.type,
  };
}

test("presents generated code payload as a file summary without raw contents", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "assistant",
      content:
        'Agent message: {"files":[{"relativePath":"index.html","contents":"<secret markup>"}]}',
    }),
  );

  assert.equal(item.kind, "file");
  assert.equal(item.title, "已生成 index.html");
  assert.equal(item.hidesTechnicalDetail, true);
  assert.match(item.technicalDetail ?? "", /"relativePath": "index\.html"/);
  assert.match(item.technicalDetail ?? "", /"contents": "<secret markup>"/);
  assert.deepEqual(item.details, [{ label: "文件", value: "index.html" }]);
  assert.doesNotMatch(`${item.title} ${item.description ?? ""}`, /secret markup/);
});

test("maps file write events to friendly file cards", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "file",
      content: "Workspace wrote generated file: src/App.tsx",
    }),
  );

  assert.equal(item.kind, "file");
  assert.equal(item.tone, "success");
  assert.equal(item.title, "已写入 App.tsx");
  assert.equal(item.details[0]?.value, "src/App.tsx");
});

test("folds technical gateway message deltas without leaving an active card", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "assistant",
      content: 'Agent message: {"files":[{"relativePath":"index.html","contents":"<main></main>"}]}',
      metadata: {
        gatewayUniEvent: gatewayEvent({
          type: "message.delta",
          payload: {
            role: "assistant",
            text: '{"files":[{"relativePath":"index.html","contents":"<main></main>"}]}',
          },
        }),
      },
    }),
  );

  assert.equal(item.kind, "assistant");
  assert.equal(item.title, "Agent 输出已折叠");
  assert.equal(item.hidesTechnicalDetail, true);
  assert.equal(item.isActive, false);
  assert.match(item.technicalDetail ?? "", /relativePath/);
});

test("maps command requests without exposing raw command copy as the main text", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "tool",
      content: "Agent requested command: /usr/local/bin/node",
    }),
  );

  assert.equal(item.kind, "tool");
  assert.equal(item.title, "Agent 请求运行本地工具");
  assert.equal(item.description, "Sofvary 正在按安全策略检查这个操作。");
  assert.equal(item.details[0]?.value, "node");
});

test("maps generated file payload notices to a user-facing file card", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "agent-event",
      content: "Codex returned generated file payload",
    }),
  );

  assert.equal(item.kind, "file");
  assert.equal(item.title, "已接收生成结果");
  assert.equal(item.hidesTechnicalDetail, true);
});

test("maps Gateway terminal output to a collapsible terminal card", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "tool",
      content: "stdout: npm install verbose output",
      metadata: {
        gatewayUniEvent: gatewayEvent({
          type: "terminal.output",
          payload: { stream: "stdout", text: "npm install verbose output" },
        }),
      },
    }),
  );

  assert.equal(item.kind, "terminal");
  assert.equal(item.tone, "neutral");
  assert.equal(item.title, "Agent 终端输出");
  assert.equal(item.hidesTechnicalDetail, true);
  assert.equal(item.technicalDetail, "npm install verbose output");
});

test("maps Gateway approval requests to warning approval cards", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "tool",
      content: "Approval requested: install dependency",
      metadata: {
        gatewayUniEvent: gatewayEvent({
          type: "approval.requested",
          payload: { action: "install dependency", subject: "npm install" },
        }),
      },
    }),
  );

  assert.equal(item.kind, "approval");
  assert.equal(item.tone, "warning");
  assert.equal(item.title, "需要确认：install dependency");
  assert.equal(item.description, "npm install");
  assert.equal(item.hidesTechnicalDetail, true);
});

test("maps runtime repair notices to user-facing runtime cards", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "agent-event",
      content: "Runtime repair attempt 1/2: API failed; Agent can attempt a repair",
    }),
  );

  assert.equal(item.kind, "runtime");
  assert.equal(item.tone, "working");
  assert.equal(item.title, "正在自动修复运行问题");
  assert.equal(item.hidesTechnicalDetail, true);
  assert.equal(item.isActive, true);
  assert.match(item.technicalDetail ?? "", /Runtime repair attempt 1\/2/);
});

test("maps runtime repair exhausted summaries to runtime warning cards", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "error",
      content:
        "Sofvary 已自动尝试修复 2 次，但运行问题仍未解决。build command 'build' failed with status 1; Agent can attempt a repair",
    }),
  );

  assert.equal(item.kind, "runtime");
  assert.equal(item.tone, "warning");
  assert.equal(item.hidesTechnicalDetail, true);
});

test("maps non-agent runtime diagnostic summaries to runtime warning cards", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "error",
      content:
        "Sofvary 已完成运行诊断：dependency install command 'install' failed with status 1; Sofvary environment setup is required",
    }),
  );

  assert.equal(item.kind, "runtime");
  assert.equal(item.tone, "warning");
  assert.equal(item.hidesTechnicalDetail, true);
});

test("maps preview-blocked asset summaries to runtime warning cards", () => {
  const item = presentBuildThreadEntry(
    entry({
      kind: "system",
      content:
        "Sofvary 已生成软件资产，但预览环境未就绪：runtime start failed; Sofvary environment setup is required",
      metadata: {
        kind: "runtime-preview-blocked",
      },
    }),
  );

  assert.equal(item.kind, "runtime");
  assert.equal(item.tone, "warning");
  assert.equal(item.title, "预览环境未就绪");
  assert.equal(item.hidesTechnicalDetail, true);
});

test("merges repeated process progress into one updating card", () => {
  const started = presentBuildThreadEntry(
    entry({
      id: "progress-a",
      kind: "agent-event",
      content: "Agent session started with codex adapter",
    }),
  );
  const fileResult = presentBuildThreadEntry(
    entry({
      id: "file-result",
      kind: "agent-event",
      content: "Codex returned generated file payload",
    }),
  );
  const completed = presentBuildThreadEntry(
    entry({
      id: "progress-b",
      kind: "agent-event",
      content: "turn completed",
      timestamp: "2026-06-10T08:01:00Z",
    }),
  );

  const items = mergeBuildThreadPresentationItems([started, fileResult, completed]);

  assert.equal(items.length, 2);
  assert.equal(items[0].kind, "file");
  assert.equal(items[1].id, "progress-a");
  assert.equal(items[1].timestamp, "2026-06-10T08:01:00Z");
  assert.equal(items[1].title, completed.title);
  assert.equal(items[1].isActive, false);
});

test("merges streamed Agent token cards into one feedback card", () => {
  const firstToken = presentBuildThreadEntry(
    entry({
      id: "message-a",
      kind: "assistant",
      content: '\\"\\":\\"',
      metadata: {
        gatewayUniEvent: gatewayEvent({
          eventId: "gateway-message-a",
          type: "message.delta",
          payload: { role: "assistant", text: '\\"\\":\\"' },
        }),
      },
    }),
  );
  const status = presentBuildThreadEntry(
    entry({
      id: "status-a",
      kind: "agent-event",
      content: "generating",
      metadata: {
        gatewayUniEvent: gatewayEvent({
          eventId: "gateway-status-a",
          type: "status.changed",
          sequence: 2,
          payload: { phase: "generating", detail: "Agent is streaming output." },
        }),
      },
    }),
  );
  const secondToken = presentBuildThreadEntry(
    entry({
      id: "message-b",
      kind: "assistant",
      timestamp: "2026-06-10T08:00:01Z",
      content: "vite",
      metadata: {
        gatewayUniEvent: gatewayEvent({
          eventId: "gateway-message-b",
          type: "message.delta",
          sequence: 3,
          payload: { role: "assistant", text: "vite" },
        }),
      },
    }),
  );
  const thirdToken = presentBuildThreadEntry(
    entry({
      id: "message-c",
      kind: "assistant",
      timestamp: "2026-06-10T08:00:02Z",
      content: " build",
      metadata: {
        gatewayUniEvent: gatewayEvent({
          eventId: "gateway-message-c",
          type: "message.delta",
          sequence: 4,
          payload: { role: "assistant", text: " build" },
        }),
      },
    }),
  );

  const items = mergeBuildThreadPresentationItems([firstToken, status, secondToken, thirdToken]);
  const assistantItems = items.filter((item) => item.kind === "assistant");

  assert.equal(assistantItems.length, 1);
  assert.equal(assistantItems[0]?.id, "message-a");
  assert.equal(assistantItems[0]?.timestamp, "2026-06-10T08:00:02Z");
  assert.equal(assistantItems[0]?.description, '\\"\\":\\"vite build');
  assert.equal(items.some((item) => item.id === "status-a"), true);
});

test("summarizes entries using friendly copy instead of raw code", () => {
  const summary = summarizeBuildThreadEntryForUser(
    entry({
      kind: "assistant",
      content:
        'Agent message: {"files":[{"relativePath":"index.html","contents":"<secret markup>"}]}',
    }),
    120,
  );

  assert.match(summary, /已生成 index\.html/);
  assert.doesNotMatch(summary, /secret markup/);
});
