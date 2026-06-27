import test from "node:test";
import assert from "node:assert/strict";
import type { RuntimeKind } from "../../types";
import {
  AI_AGENT_APP_SPECIES_ID,
  type SoftwareSpeciesCatalogItem,
  findSoftwareSpecies,
  getSoftwareSpeciesCatalog,
  selectSoftwareSpecies,
  toRuntimeIntentSelection,
} from "./softwareSpeciesLogic";

const catalog: readonly SoftwareSpeciesCatalogItem[] = [
  species("static-html", "Small single-page tool", "runtime", "static-html", ["static html", "single page"]),
  species("canvas2d", "Canvas 2D experience", "runtime", "canvas2d", ["canvas", "game"]),
  species("data-table", "Data table app", "runtime", "data-table", ["table", "csv"]),
  species("markdown-knowledge", "Markdown knowledge app", "runtime", "markdown-knowledge", ["markdown", "wiki"]),
  species("inventory-table", "Inventory table", "productivity", "data-table", ["inventory table", "item catalog", "quantity"]),
  species("knowledge-base", "Knowledge base", "productivity", "markdown-knowledge", ["knowledge base", "personal wiki", "reading notes", "prompt library"]),
  species("task-board", "Task board", "productivity", "react-vite", ["task board", "kanban"]),
  species("focus-planner", "Focus Planner", "productivity", "react-vite", ["focus planner"]),
  species("knowledge-garden", "Knowledge Garden", "productivity", "markdown-knowledge", ["knowledge garden"]),
  species("data-ledger", "Data Ledger", "productivity", "data-table", ["data ledger"]),
  species("file-triage", "File Triage", "productivity", "file-processor", ["file triage"]),
  species("timebox-widget", "Timebox Widget", "productivity", "desktop-widget", ["timebox widget"]),
  species("personal-crm", "Personal CRM", "productivity", "react-sqlite", ["crm", "customers"]),
  species(AI_AGENT_APP_SPECIES_ID, "AI Agent App", "ai-agent-app", "ai-agent-app", ["ai agent", "智能体", "写小说", "生图", "生成视频"], true),
];

test("software species catalog filters runtime species from supplied catalog", () => {
  const runtimeSpecies = getSoftwareSpeciesCatalog({ categories: ["runtime"] }, catalog);

  assert.deepEqual(runtimeSpecies.map((species) => species.id), [
    "static-html",
    "canvas2d",
    "data-table",
    "markdown-knowledge",
  ]);
  assert.deepEqual(runtimeSpecies.map((species) => species.runtimeKind), [
    "static-html",
    "canvas2d",
    "data-table",
    "markdown-knowledge",
  ]);
});

test("software species catalog includes supplied productivity and AI Agent App species", () => {
  const productivityIds = getSoftwareSpeciesCatalog({ categories: ["productivity"] }, catalog).map(
    (species) => species.id,
  );
  const aiAgentSpecies = findSoftwareSpecies(AI_AGENT_APP_SPECIES_ID, catalog);

  for (const id of [
    "focus-planner",
    "knowledge-garden",
    "data-ledger",
    "file-triage",
    "timebox-widget",
    "task-board",
    "knowledge-base",
    "personal-crm",
    "inventory-table",
  ]) {
    assert.ok(productivityIds.includes(id));
  }
  assert.equal(aiAgentSpecies?.runtimeKind, "ai-agent-app");
  assert.equal(aiAgentSpecies?.requiresProviderBinding, true);
});

test("manual species selection wins and maps to its runtime", () => {
  const selection = selectSoftwareSpecies({
    requirement: "make a game",
    selectedSpeciesId: "inventory-table",
    catalog,
  });

  assert.equal(selection.source, "manual");
  assert.equal(selection.speciesId, "inventory-table");
  assert.equal(selection.runtimeKind, "data-table");
  assert.equal(selection.confidence, 1);
});

test("manual runtime selection falls back to the runtime species", () => {
  const selection = selectSoftwareSpecies({
    requirement: "I need a wiki",
    selectedRuntimeKind: "canvas2d",
    catalog,
  });

  assert.equal(selection.source, "manual");
  assert.equal(selection.speciesId, "canvas2d");
  assert.equal(selection.runtimeKind, "canvas2d");
});

test("intent selection prefers productivity species over raw runtime labels", () => {
  const selection = selectSoftwareSpecies({
    requirement: "Build a CSV inventory table with quantity filters and item catalog fields.",
    catalog,
  });

  assert.equal(selection.speciesId, "inventory-table");
  assert.equal(selection.runtimeKind, "data-table");
  assert.ok(selection.matchedSignals.includes("inventory table"));
});

test("intent selection marks AI Agent App as needing provider binding", () => {
  const selection = selectSoftwareSpecies({
    requirement: "Create an AI agent app with OpenAI support that answers support tickets.",
    catalog,
  });

  assert.equal(selection.speciesId, AI_AGENT_APP_SPECIES_ID);
  assert.equal(selection.runtimeKind, "ai-agent-app");
  assert.equal(selection.requiresProviderBinding, true);
  assert.ok(selection.matchedSignals.includes("ai agent"));
});

test("manual AI Agent App runtime selection uses the agent species", () => {
  const selection = selectSoftwareSpecies({
    selectedRuntimeKind: "ai-agent-app",
    catalog,
  });

  assert.equal(selection.source, "manual");
  assert.equal(selection.speciesId, AI_AGENT_APP_SPECIES_ID);
  assert.equal(selection.runtimeKind, "ai-agent-app");
  assert.equal(selection.requiresProviderBinding, true);
});

test("intent selection handles Chinese AI Agent signals", () => {
  const selection = selectSoftwareSpecies({
    requirement: "创建一个智能体应用，可以写小说、生图、生成视频。",
    catalog,
  });

  assert.equal(selection.speciesId, AI_AGENT_APP_SPECIES_ID);
  assert.equal(selection.runtimeKind, "ai-agent-app");
  assert.ok(selection.requiresProviderBinding);
});

test("runtime intent conversion keeps runtime choices separate from species metadata", () => {
  const selection = selectSoftwareSpecies({
    requirement: "Personal wiki for reading notes and a prompt library.",
    catalog,
  });
  const runtimeIntent = toRuntimeIntentSelection(selection);

  assert.equal(runtimeIntent.runtimeKind, "markdown-knowledge");
  assert.equal(runtimeIntent.softwareType, "knowledge-base");
  assert.equal(runtimeIntent.source, "automatic");
  assert.ok(runtimeIntent.confidence > 0.45);
});

test("unknown or empty intent defaults to first supplied runtime species", () => {
  const selection = selectSoftwareSpecies({ requirement: "", catalog });

  assert.equal(selection.speciesId, "static-html");
  assert.equal(selection.runtimeKind satisfies RuntimeKind, "static-html");
  assert.equal(selection.source, "automatic");
});

function species(
  id: string,
  label: string,
  category: SoftwareSpeciesCatalogItem["category"],
  runtimeKind: RuntimeKind,
  intentSignals: readonly string[],
  requiresProviderBinding = false,
): SoftwareSpeciesCatalogItem {
  return {
    id,
    label,
    summary: `${label} summary`,
    category,
    runtimeKind,
    requiresProviderBinding,
    tags: [],
    intentSignals,
  };
}
