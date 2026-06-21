import test from "node:test";
import assert from "node:assert/strict";
import type { BuildThreadEntry } from "../../types";
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
