import test from "node:test";
import assert from "node:assert/strict";
import type { BuildThreadEntry, BuildThreadSummary } from "../../types";
import { BuildThreadEventBatcher } from "./buildThreadEventBatcher";

const baseThread: BuildThreadSummary = {
  id: "thread-a",
  title: "倒计时工具",
  status: "queued",
  runtimeKind: "static-html",
  runtimeMode: "dev",
  agentId: "sofvary-pi",
  createdAt: "2026-06-10T08:00:00Z",
  updatedAt: "2026-06-10T08:00:00Z",
  workspaceId: null,
  appId: null,
  preview: null,
  error: null,
};

const baseEntry: BuildThreadEntry = {
  id: "entry-a",
  threadId: "thread-a",
  timestamp: "2026-06-10T08:00:01Z",
  kind: "assistant",
  content: "Agent message: 正在生成",
  metadata: {},
};

test("BuildThreadEventBatcher coalesces summaries and preserves entry order", () => {
  const batches: Array<{ summaries: BuildThreadSummary[]; entries: BuildThreadEntry[] }> = [];
  const batcher = new BuildThreadEventBatcher((batch) => batches.push(batch), 1000);

  batcher.pushSummary(baseThread);
  batcher.pushSummary({ ...baseThread, status: "building", updatedAt: "2026-06-10T08:00:02Z" });
  batcher.pushEntry(baseEntry);
  batcher.pushEntry({ ...baseEntry, id: "entry-b", content: "完成" });
  batcher.flush();

  assert.equal(batches.length, 1);
  assert.deepEqual(
    batches[0].summaries.map((summary) => summary.status),
    ["building"],
  );
  assert.deepEqual(
    batches[0].entries.map((entry) => entry.id),
    ["entry-a", "entry-b"],
  );
});
