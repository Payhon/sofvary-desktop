import assert from "node:assert/strict";
import { describe, it } from "node:test";
import canvas2dGoldenPromptEnvelope from "../fixtures/canvas2d-prompt-envelope.golden.json" with { type: "json" };
import reactSqliteGoldenPromptEnvelope from "../fixtures/react-sqlite-prompt-envelope.golden.json" with { type: "json" };
import reactGoldenPromptEnvelope from "../fixtures/react-vite-prompt-envelope.golden.json" with { type: "json" };
import goldenPromptEnvelope from "../fixtures/static-html-prompt-envelope.golden.json" with { type: "json" };
import {
  createPromptEnvelopeForRuntimeKind,
  summarizePromptEnvelope,
  type CurrentAppState,
} from "./index";

const appState: CurrentAppState = {
  appId: "app_test",
  workspaceName: "Timer",
  mode: "create",
  existingFiles: [],
  previewState: "empty",
};

describe("pack configured prompt envelope compiler", () => {
  it("creates static-html envelope from pack resources", () => {
    const envelope = createPromptEnvelopeForRuntimeKind("static-html", {
      userIntent: "Build a tiny timer",
      currentAppState: appState,
      envelopeId: "penv_golden",
      createdAt: "2026-05-28T00:00:00.000Z",
    });

    assert.deepEqual(envelope, goldenPromptEnvelope);
    assert.deepEqual(envelope.fileSystemPolicy.allowedFiles, envelope.outputContract.files);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("remote-network"));
  });

  it("creates react-vite envelope from pack resources", () => {
    const envelope = createPromptEnvelopeForRuntimeKind("react-vite", {
      userIntent: "Build a task board",
      currentAppState: appState,
      envelopeId: "penv_react_golden",
      createdAt: "2026-06-02T00:00:00.000Z",
    });

    assert.deepEqual(envelope, reactGoldenPromptEnvelope);
    assert.deepEqual(envelope.fileSystemPolicy.allowedFiles, envelope.outputContract.files);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("nextjs-runtime"));
  });

  it("creates react-sqlite envelope from pack resources", () => {
    const envelope = createPromptEnvelopeForRuntimeKind("react-sqlite", {
      userIntent: "Build a local customer manager",
      currentAppState: appState,
      envelopeId: "penv_react_sqlite_golden",
      createdAt: "2026-06-02T00:00:00.000Z",
    });

    assert.deepEqual(envelope, reactSqliteGoldenPromptEnvelope);
    assert.deepEqual(envelope.fileSystemPolicy.allowedFiles, envelope.outputContract.files);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("remote-database"));
  });

  it("creates canvas2d envelope from pack resources", () => {
    const envelope = createPromptEnvelopeForRuntimeKind("canvas2d", {
      userIntent: "Build a coin collector",
      currentAppState: appState,
      envelopeId: "penv_canvas2d_golden",
      createdAt: "2026-06-03T00:00:00.000Z",
    });

    assert.deepEqual(envelope, canvas2dGoldenPromptEnvelope);
    assert.deepEqual(envelope.fileSystemPolicy.allowedFiles, envelope.outputContract.files);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("react-runtime"));
  });

  it("supports generated-project runtime families through the same compiler", () => {
    const envelopes = [
      createPromptEnvelopeForRuntimeKind("markdown-knowledge", {
        userIntent: "Build a markdown knowledge base",
        currentAppState: appState,
      }),
      createPromptEnvelopeForRuntimeKind("data-table", {
        userIntent: "Build an inventory table",
        currentAppState: appState,
      }),
      createPromptEnvelopeForRuntimeKind("file-processor", {
        userIntent: "Build a safe file renamer",
        currentAppState: appState,
      }),
      createPromptEnvelopeForRuntimeKind("desktop-widget", {
        userIntent: "Build a compact countdown widget",
        currentAppState: appState,
      }),
      createPromptEnvelopeForRuntimeKind("ai-agent-app", {
        userIntent: "Build an AI agent app for article and video jobs",
        currentAppState: appState,
      }),
    ];

    assert.equal(envelopes[0].outputContract.format, "markdown-knowledge-project");
    assert.deepEqual(envelopes[1].fileSystemPolicy.allowedFiles, envelopes[1].outputContract.files);
    assert.equal(envelopes[2].boxRuntimeContext.runtimeKind, "file-processor");
    assert.equal(envelopes[3].boxRuntimeContext.runtimeKind, "desktop-widget");
    assert.equal(envelopes[4].boxRuntimeContext.runtimeKind, "ai-agent-app");
  });

  it("summarizes without including full user intent", () => {
    const envelope = createPromptEnvelopeForRuntimeKind("react-vite", {
      userIntent: "Build a private prompt that should not appear in summary",
      currentAppState: appState,
    });
    const summary = summarizePromptEnvelope(envelope);

    assert.ok(!JSON.stringify(summary).includes("private prompt"));
    assert.equal(summary.acceptanceCriteriaCount, envelope.acceptanceCriteria.length);
  });

  it("rejects missing or incompatible harness packs", () => {
    assert.throws(
      () =>
        createPromptEnvelopeForRuntimeKind("static-html", {
          userIntent: "Build a tiny timer",
          currentAppState: appState,
          harnessPacks: [],
        }),
      /requires at least one harness pack/,
    );

    assert.throws(
      () =>
        createPromptEnvelopeForRuntimeKind("static-html", {
          userIntent: "Build a tiny timer",
          currentAppState: appState,
          harnessPacks: [
            {
              id: "sofvary.harness.static-html",
              version: "0.1.0",
              runtime: "sofvary.runtime.react-vite",
              promptPolicy: "prompt/policy.json",
            },
          ],
        }),
      /not compatible with runtime/,
    );
  });
});
