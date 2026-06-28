import test from "node:test";
import assert from "node:assert/strict";
import type { BuildThreadEntry, BuildThreadSummary, GatewayUniEvent } from "../../types";
import { BuildThreadEventBatcher } from "./buildThreadEventBatcher";

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

const baseEntry: BuildThreadEntry = {
  id: "entry-a",
  threadId: "thread-a",
  timestamp: "2026-06-10T08:00:01Z",
  kind: "assistant",
  content: "Agent message: 正在生成",
  metadata: {},
};

function gatewayEvent(input: Partial<GatewayUniEvent> & Pick<GatewayUniEvent, "type">): GatewayUniEvent {
  return {
    eventId: input.eventId ?? `gateway-${input.sequence ?? 1}`,
    threadId: input.threadId ?? "thread-a",
    timestamp: input.timestamp ?? "2026-06-10T08:00:01Z",
    agentId: input.agentId ?? "sofvary-agent",
    transport: input.transport ?? "pi-native",
    sequence: input.sequence ?? 1,
    type: input.type,
    payload: input.payload ?? {},
  };
}

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

test("BuildThreadEventBatcher merges pending Gateway message deltas", () => {
  const batches: Array<{ summaries: BuildThreadSummary[]; entries: BuildThreadEntry[] }> = [];
  const batcher = new BuildThreadEventBatcher((batch) => batches.push(batch), 1000);

  for (let index = 0; index < 500; index += 1) {
    batcher.pushEntry({
      ...baseEntry,
      id: `entry-${index}`,
      content: "x",
      metadata: {
        gatewayUniEvent: gatewayEvent({
          type: "message.delta",
          sequence: index + 1,
          payload: { text: "x" },
        }),
      },
    });
  }
  batcher.flush();

  assert.equal(batches.length, 1);
  assert.equal(batches[0].entries.length, 1);
  const event = batches[0].entries[0]?.metadata?.gatewayUniEvent as GatewayUniEvent;
  assert.equal(typeof event.payload.text === "string" ? event.payload.text.length : 0, 500);
  assert.equal(batches[0].entries[0]?.metadata?.coalescedCount, 500);
});
