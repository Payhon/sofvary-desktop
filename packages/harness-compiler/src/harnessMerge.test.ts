import assert from "node:assert/strict";
import { describe, it } from "node:test";
import canvas2dGoldenPromptEnvelope from "../fixtures/canvas2d-prompt-envelope.golden.json" with { type: "json" };
import reactSqliteGoldenPromptEnvelope from "../fixtures/react-sqlite-prompt-envelope.golden.json" with { type: "json" };
import reactGoldenPromptEnvelope from "../fixtures/react-vite-prompt-envelope.golden.json" with { type: "json" };
import goldenPromptEnvelope from "../fixtures/static-html-prompt-envelope.golden.json" with { type: "json" };
import {
  AI_AGENT_APP_ALLOWED_FILES,
  CANVAS2D_ALLOWED_FILES,
  CANVAS2D_HARNESS_PACK_ID,
  CANVAS2D_PACK_VERSION,
  CANVAS2D_RUNTIME_PACK_ID,
  createAiAgentAppPromptEnvelope,
  createCanvas2dPromptEnvelope,
  createDataTablePromptEnvelope,
  createDesktopWidgetPromptEnvelope,
  createFileProcessorPromptEnvelope,
  createMarkdownKnowledgePromptEnvelope,
  createReactSqlitePromptEnvelope,
  createReactVitePromptEnvelope,
  createStaticHtmlPromptEnvelope,
  DATA_TABLE_ALLOWED_FILES,
  DESKTOP_WIDGET_ALLOWED_FILES,
  FILE_PROCESSOR_ALLOWED_FILES,
  MARKDOWN_KNOWLEDGE_ALLOWED_FILES,
  mergeHarnessFragments,
  REACT_SQLITE_ALLOWED_FILES,
  REACT_SQLITE_HARNESS_PACK_ID,
  REACT_SQLITE_PACK_VERSION,
  REACT_SQLITE_RUNTIME_PACK_ID,
  REACT_VITE_ALLOWED_FILES,
  REACT_VITE_HARNESS_PACK_ID,
  REACT_VITE_PACK_VERSION,
  REACT_VITE_RUNTIME_PACK_ID,
  STATIC_HTML_ALLOWED_FILES,
  STATIC_HTML_HARNESS_PACK_ID,
  STATIC_HTML_PACK_VERSION,
  STATIC_HTML_RUNTIME_PACK_ID,
  summarizePromptEnvelope,
  type CurrentAppState,
  type HarnessFragment,
} from "./index";

const appState: CurrentAppState = {
  appId: "app_test",
  workspaceName: "Timer",
  mode: "create",
  existingFiles: [],
  previewState: "empty",
};

describe("createStaticHtmlPromptEnvelope", () => {
  it("creates an envelope from user intent plus static-html runtime and harness packs", () => {
    const envelope = createStaticHtmlPromptEnvelope({
      userIntent: "Build a tiny timer",
      currentAppState: appState,
      envelopeId: "penv_test",
      createdAt: "2026-05-28T00:00:00.000Z",
    });

    assert.equal(envelope.schemaVersion, "1.0");
    assert.equal(envelope.userIntent, "Build a tiny timer");
    assert.deepEqual(envelope.boxRuntimeContext.runtimePack, {
      id: STATIC_HTML_RUNTIME_PACK_ID,
      version: STATIC_HTML_PACK_VERSION,
    });
    assert.deepEqual(envelope.boxRuntimeContext.harnessPacks, [
      { id: STATIC_HTML_HARNESS_PACK_ID, version: STATIC_HTML_PACK_VERSION },
    ]);
  });

  it("matches the static-html golden prompt envelope fixture", () => {
    const envelope = createStaticHtmlPromptEnvelope({
      userIntent: "Build a tiny timer",
      currentAppState: appState,
      envelopeId: "penv_golden",
      createdAt: "2026-05-28T00:00:00.000Z",
    });

    assert.deepEqual(envelope, goldenPromptEnvelope);
  });

  it("includes static-html constraints and summarizes without full prompt text", () => {
    const envelope = createStaticHtmlPromptEnvelope({
      userIntent: "Build a private prompt that should not appear in the summary",
      currentAppState: appState,
    });
    const summary = summarizePromptEnvelope(envelope);

    assert.deepEqual(envelope.fileSystemPolicy.allowedFiles, [...STATIC_HTML_ALLOWED_FILES]);
    assert.equal(envelope.fileSystemPolicy.root, "generated/static");
    assert.equal(envelope.commandPolicy.allowShell, false);
    assert.equal(envelope.commandPolicy.allowPackageInstall, false);
    assert.equal(envelope.outputContract.shellUiIncluded, false);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("remote-network"));
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("sofvary-shell-ui"));
    assert.ok(envelope.harnessPolicy.fileSystemRules.includes("Use localStorage only for small local preferences."));
    assert.ok(!JSON.stringify(summary).includes("private prompt"));
  });

  it("rejects missing or incompatible harness packs", () => {
    assert.throws(
      () =>
        createStaticHtmlPromptEnvelope({
          userIntent: "Build a tiny timer",
          currentAppState: appState,
          harnessPacks: [],
        }),
      /requires at least one harness pack/,
    );

    assert.throws(
      () =>
        createStaticHtmlPromptEnvelope({
          userIntent: "Build a tiny timer",
          currentAppState: appState,
          harnessPacks: [
            {
              id: STATIC_HTML_HARNESS_PACK_ID,
              version: STATIC_HTML_PACK_VERSION,
              runtime: "sofvary.runtime.react-vite",
              instructions: {
                system: [],
                fileSystemPolicy: [],
                outputRules: [],
              },
            },
          ],
        }),
      /not compatible with runtime/,
    );
  });
});

describe("createReactVitePromptEnvelope", () => {
  it("creates an envelope from user intent plus react-vite runtime and harness packs", () => {
    const envelope = createReactVitePromptEnvelope({
      userIntent: "Build a task board",
      currentAppState: appState,
      envelopeId: "penv_react_test",
      createdAt: "2026-06-02T00:00:00.000Z",
    });

    assert.equal(envelope.schemaVersion, "1.0");
    assert.equal(envelope.userIntent, "Build a task board");
    assert.deepEqual(envelope.boxRuntimeContext.runtimePack, {
      id: REACT_VITE_RUNTIME_PACK_ID,
      version: REACT_VITE_PACK_VERSION,
    });
    assert.deepEqual(envelope.boxRuntimeContext.harnessPacks, [
      { id: REACT_VITE_HARNESS_PACK_ID, version: REACT_VITE_PACK_VERSION },
    ]);
    assert.equal(envelope.runtimePolicy.runtimeKind, "react-vite");
  });

  it("matches the react-vite golden prompt envelope fixture", () => {
    const envelope = createReactVitePromptEnvelope({
      userIntent: "Build a task board",
      currentAppState: appState,
      envelopeId: "penv_react_golden",
      createdAt: "2026-06-02T00:00:00.000Z",
    });

    assert.deepEqual(envelope, reactGoldenPromptEnvelope);
  });

  it("includes react-vite constraints and summarizes without full prompt text", () => {
    const envelope = createReactVitePromptEnvelope({
      userIntent: "Build a private React prompt that should not appear in the summary",
      currentAppState: appState,
    });
    const summary = summarizePromptEnvelope(envelope);

    assert.deepEqual(envelope.fileSystemPolicy.allowedFiles, [...REACT_VITE_ALLOWED_FILES]);
    assert.equal(envelope.fileSystemPolicy.root, "generated/react");
    assert.equal(envelope.commandPolicy.allowShell, false);
    assert.equal(envelope.commandPolicy.allowPackageInstall, false);
    assert.equal(envelope.outputContract.shellUiIncluded, false);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("nextjs-runtime"));
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("sofvary-shell-ui"));
    assert.ok(envelope.harnessPolicy.outputRules.includes("The generated app must pass npm run build."));
    assert.ok(!JSON.stringify(summary).includes("private React prompt"));
  });

  it("rejects missing or incompatible react-vite harness packs", () => {
    assert.throws(
      () =>
        createReactVitePromptEnvelope({
          userIntent: "Build a task board",
          currentAppState: appState,
          harnessPacks: [],
        }),
      /requires at least one harness pack/,
    );

    assert.throws(
      () =>
        createReactVitePromptEnvelope({
          userIntent: "Build a task board",
          currentAppState: appState,
          harnessPacks: [
            {
              id: REACT_VITE_HARNESS_PACK_ID,
              version: REACT_VITE_PACK_VERSION,
              runtime: "sofvary.runtime.static-html",
              instructions: {
                system: [],
                fileSystemPolicy: [],
                outputRules: [],
              },
            },
          ],
        }),
      /not compatible with runtime/,
    );
  });
});

describe("createReactSqlitePromptEnvelope", () => {
  it("creates an envelope from user intent plus react-sqlite runtime and harness packs", () => {
    const envelope = createReactSqlitePromptEnvelope({
      userIntent: "Build a local customer manager",
      currentAppState: appState,
      envelopeId: "penv_react_sqlite_test",
      createdAt: "2026-06-02T00:00:00.000Z",
    });

    assert.equal(envelope.schemaVersion, "1.0");
    assert.equal(envelope.userIntent, "Build a local customer manager");
    assert.deepEqual(envelope.boxRuntimeContext.runtimePack, {
      id: REACT_SQLITE_RUNTIME_PACK_ID,
      version: REACT_SQLITE_PACK_VERSION,
    });
    assert.deepEqual(envelope.boxRuntimeContext.harnessPacks, [
      { id: REACT_SQLITE_HARNESS_PACK_ID, version: REACT_SQLITE_PACK_VERSION },
    ]);
    assert.equal(envelope.runtimePolicy.runtimeKind, "react-sqlite");
  });

  it("matches the react-sqlite golden prompt envelope fixture", () => {
    const envelope = createReactSqlitePromptEnvelope({
      userIntent: "Build a local customer manager",
      currentAppState: appState,
      envelopeId: "penv_react_sqlite_golden",
      createdAt: "2026-06-02T00:00:00.000Z",
    });

    assert.deepEqual(envelope, reactSqliteGoldenPromptEnvelope);
  });

  it("includes react-sqlite constraints and summarizes without full prompt text", () => {
    const envelope = createReactSqlitePromptEnvelope({
      userIntent: "Build a private SQLite prompt that should not appear in the summary",
      currentAppState: appState,
    });
    const summary = summarizePromptEnvelope(envelope);

    assert.deepEqual(envelope.fileSystemPolicy.allowedFiles, [...REACT_SQLITE_ALLOWED_FILES]);
    assert.equal(envelope.fileSystemPolicy.root, "generated");
    assert.equal(envelope.outputContract.format, "react-sqlite-project");
    assert.equal(envelope.commandPolicy.allowShell, false);
    assert.equal(envelope.commandPolicy.allowPackageInstall, false);
    assert.equal(envelope.outputContract.shellUiIncluded, false);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("remote-database"));
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("direct-frontend-sqlite-access"));
    assert.ok(envelope.harnessPolicy.outputRules.includes("SQL statements use prepared statements for user-controlled values."));
    assert.ok(!JSON.stringify(summary).includes("private SQLite prompt"));
  });

  it("rejects missing or incompatible react-sqlite harness packs", () => {
    assert.throws(
      () =>
        createReactSqlitePromptEnvelope({
          userIntent: "Build a customer manager",
          currentAppState: appState,
          harnessPacks: [],
        }),
      /requires at least one harness pack/,
    );

    assert.throws(
      () =>
        createReactSqlitePromptEnvelope({
          userIntent: "Build a customer manager",
          currentAppState: appState,
          harnessPacks: [
            {
              id: REACT_SQLITE_HARNESS_PACK_ID,
              version: REACT_SQLITE_PACK_VERSION,
              runtime: "sofvary.runtime.react-vite",
              instructions: {
                system: [],
                fileSystemPolicy: [],
                outputRules: [],
              },
            },
          ],
        }),
      /not compatible with runtime/,
    );
  });
});

describe("createCanvas2dPromptEnvelope", () => {
  it("creates an envelope from user intent plus canvas2d runtime and harness packs", () => {
    const envelope = createCanvas2dPromptEnvelope({
      userIntent: "Build a canvas coin collector",
      currentAppState: appState,
      envelopeId: "penv_canvas2d_test",
      createdAt: "2026-06-03T00:00:00.000Z",
    });

    assert.equal(envelope.schemaVersion, "1.0");
    assert.equal(envelope.userIntent, "Build a canvas coin collector");
    assert.deepEqual(envelope.boxRuntimeContext.runtimePack, {
      id: CANVAS2D_RUNTIME_PACK_ID,
      version: CANVAS2D_PACK_VERSION,
    });
    assert.deepEqual(envelope.boxRuntimeContext.harnessPacks, [
      { id: CANVAS2D_HARNESS_PACK_ID, version: CANVAS2D_PACK_VERSION },
    ]);
    assert.equal(envelope.runtimePolicy.runtimeKind, "canvas2d");
  });

  it("matches the canvas2d golden prompt envelope fixture", () => {
    const envelope = createCanvas2dPromptEnvelope({
      userIntent: "Build a canvas coin collector",
      currentAppState: appState,
      envelopeId: "penv_canvas2d_golden",
      createdAt: "2026-06-03T00:00:00.000Z",
    });

    assert.deepEqual(envelope, canvas2dGoldenPromptEnvelope);
  });

  it("includes canvas2d constraints and summarizes without full prompt text", () => {
    const envelope = createCanvas2dPromptEnvelope({
      userIntent: "Build a private Canvas prompt that should not appear in the summary",
      currentAppState: appState,
    });
    const summary = summarizePromptEnvelope(envelope);

    assert.deepEqual(envelope.fileSystemPolicy.allowedFiles, [...CANVAS2D_ALLOWED_FILES]);
    assert.equal(envelope.fileSystemPolicy.root, "generated/canvas");
    assert.equal(envelope.outputContract.format, "canvas2d-project");
    assert.equal(envelope.commandPolicy.allowShell, false);
    assert.equal(envelope.commandPolicy.allowPackageInstall, false);
    assert.equal(envelope.outputContract.shellUiIncluded, false);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("react-runtime"));
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("remote-network"));
    assert.ok(envelope.harnessPolicy.outputRules.includes("Use requestAnimationFrame as the main loop driver."));
    assert.ok(!JSON.stringify(summary).includes("private Canvas prompt"));
  });

  it("rejects missing or incompatible canvas2d harness packs", () => {
    assert.throws(
      () =>
        createCanvas2dPromptEnvelope({
          userIntent: "Build a canvas game",
          currentAppState: appState,
          harnessPacks: [],
        }),
      /requires at least one harness pack/,
    );

    assert.throws(
      () =>
        createCanvas2dPromptEnvelope({
          userIntent: "Build a canvas game",
          currentAppState: appState,
          harnessPacks: [
            {
              id: CANVAS2D_HARNESS_PACK_ID,
              version: CANVAS2D_PACK_VERSION,
              runtime: "sofvary.runtime.react-vite",
              instructions: {
                system: [],
                fileSystemPolicy: [],
                outputRules: [],
              },
            },
          ],
        }),
      /not compatible with runtime/,
    );
  });
});

describe("create generated project prompt envelopes", () => {
  it("creates a markdown-knowledge envelope with local notes constraints", () => {
    const envelope = createMarkdownKnowledgePromptEnvelope({
      userIntent: "Build a reading note app",
      currentAppState: appState,
      envelopeId: "penv_markdown_test",
      createdAt: "2026-06-04T00:00:00.000Z",
    });

    assert.equal(envelope.runtimePolicy.runtimeKind, "markdown-knowledge");
    assert.equal(envelope.fileSystemPolicy.root, "generated");
    assert.equal(envelope.outputContract.format, "markdown-knowledge-project");
    assert.deepEqual(envelope.outputContract.files, [...MARKDOWN_KNOWLEDGE_ALLOWED_FILES]);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("note-upload"));
    assert.ok(envelope.harnessPolicy.outputRules.includes("Search must use generated local content only."));
  });

  it("creates a data-table envelope with safe csv placeholder constraints", () => {
    const envelope = createDataTablePromptEnvelope({
      userIntent: "Build an inventory table",
      currentAppState: appState,
    });

    assert.equal(envelope.runtimePolicy.runtimeKind, "data-table");
    assert.deepEqual(envelope.outputContract.files, [...DATA_TABLE_ALLOWED_FILES]);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("arbitrary-csv-access"));
    assert.ok(envelope.harnessPolicy.outputRules.some((rule) => rule.includes("CSV import remains")));
  });

  it("creates a file-processor envelope with dry-run first constraints", () => {
    const envelope = createFileProcessorPromptEnvelope({
      userIntent: "Build a batch rename tool",
      currentAppState: appState,
    });

    assert.equal(envelope.runtimePolicy.runtimeKind, "file-processor");
    assert.deepEqual(envelope.outputContract.files, [...FILE_PROCESSOR_ALLOWED_FILES]);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("file-mutation"));
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("write-without-dry-run"));
    assert.ok(
      envelope.harnessPolicy.outputRules.includes(
        "Confirmation records the plan in the operation log only.",
      ),
    );
  });

  it("creates a desktop-widget envelope without system automation", () => {
    const envelope = createDesktopWidgetPromptEnvelope({
      userIntent: "Build a pomodoro widget",
      currentAppState: appState,
    });

    assert.equal(envelope.runtimePolicy.runtimeKind, "desktop-widget");
    assert.deepEqual(envelope.outputContract.files, [...DESKTOP_WIDGET_ALLOWED_FILES]);
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("transparent-window"));
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("system-automation"));
  });

  it("creates an ai-agent-app envelope with provider binding and gateway constraints", () => {
    const envelope = createAiAgentAppPromptEnvelope({
      userIntent: "Build an AI studio that writes articles, novels, images, and videos",
      currentAppState: appState,
      envelopeId: "penv_ai_agent_app_test",
      createdAt: "2026-06-16T00:00:00.000Z",
    });

    assert.equal(envelope.boxRuntimeContext.runtimeKind, "ai-agent-app");
    assert.equal(envelope.runtimePolicy.runtimeKind, "ai-agent-app");
    assert.equal(envelope.outputContract.format, "ai-agent-app-project");
    assert.deepEqual(envelope.outputContract.files, [...AI_AGENT_APP_ALLOWED_FILES]);
    assert.ok(envelope.fileSystemPolicy.allowedFiles.includes("ai/provider-requirements.json"));
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("plaintext-api-key"));
    assert.ok(envelope.harnessPolicy.blockedCapabilities.includes("coding-agent-gateway-access"));
    assert.ok(
      envelope.harnessPolicy.outputRules.some((rule) =>
        rule.includes("Sofvary AI Gateway loopback endpoints"),
      ),
    );
  });
});

describe("mergeHarnessFragments", () => {
  it("merges fragments deterministically by pack identity", () => {
    const fragments: HarnessFragment[] = [
      {
        pack: { id: "z.pack", version: "1.0.0" },
        policy: {
          systemInstructions: ["second"],
          fileSystemRules: ["same"],
          outputRules: ["z output"],
          blockedCapabilities: ["z-cap"],
        },
        acceptanceCriteria: ["z criteria"],
      },
      {
        pack: { id: "a.pack", version: "1.0.0" },
        policy: {
          systemInstructions: ["first"],
          fileSystemRules: ["same", "a rule"],
          outputRules: ["a output"],
          blockedCapabilities: ["a-cap"],
        },
        acceptanceCriteria: ["a criteria"],
      },
    ];

    const merged = mergeHarnessFragments(fragments);

    assert.deepEqual(merged.policy.systemInstructions, ["first", "second"]);
    assert.deepEqual(merged.policy.fileSystemRules, ["same", "a rule"]);
    assert.deepEqual(merged.policy.outputRules, ["a output", "z output"]);
    assert.ok(merged.policy.blockedCapabilities.includes("a-cap"));
    assert.ok(merged.policy.blockedCapabilities.includes("z-cap"));
    assert.ok(merged.acceptanceCriteria.indexOf("a criteria") < merged.acceptanceCriteria.indexOf("z criteria"));
  });
});
