import test from "node:test";
import assert from "node:assert/strict";
import type { RuntimeKind } from "../../types";
import {
  AI_AGENT_APP_SPECIES_ID,
  existingRuntimeSpeciesKinds,
  findSoftwareSpecies,
  getSoftwareSpeciesCatalog,
  selectSoftwareSpecies,
  toRuntimeIntentSelection,
} from "./softwareSpeciesLogic";

test("software species catalog includes every existing runtime species", () => {
  const runtimeSpecies = getSoftwareSpeciesCatalog({ categories: ["runtime"] });

  assert.deepEqual(
    runtimeSpecies.map((species) => species.id),
    existingRuntimeSpeciesKinds,
  );
  assert.deepEqual(
    runtimeSpecies.map((species) => species.runtimeKind),
    existingRuntimeSpeciesKinds,
  );
});

test("software species catalog includes productivity and AI Agent App species", () => {
  const productivityIds = getSoftwareSpeciesCatalog({ categories: ["productivity"] }).map(
    (species) => species.id,
  );
  const aiAgentSpecies = findSoftwareSpecies(AI_AGENT_APP_SPECIES_ID);

  for (const id of [
    "focus-planner",
    "knowledge-garden",
    "data-ledger",
    "file-triage",
    "timebox-widget",
    "task-board",
    "local-dashboard",
    "knowledge-base",
    "personal-crm",
    "inventory-table",
  ]) {
    assert.ok(productivityIds.includes(id as typeof productivityIds[number]));
  }
  assert.equal(aiAgentSpecies?.runtimeKind, "ai-agent-app");
  assert.equal(aiAgentSpecies?.requiresProviderBinding, true);
});

test("manual species selection wins and maps to its runtime", () => {
  const selection = selectSoftwareSpecies({
    requirement: "make a game",
    selectedSpeciesId: "inventory-table",
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
  });

  assert.equal(selection.source, "manual");
  assert.equal(selection.speciesId, "canvas2d");
  assert.equal(selection.runtimeKind, "canvas2d");
});

test("intent selection prefers productivity species over raw runtime labels", () => {
  const selection = selectSoftwareSpecies({
    requirement: "Build a CSV inventory table with quantity filters and item catalog fields.",
  });

  assert.equal(selection.speciesId, "inventory-table");
  assert.equal(selection.runtimeKind, "data-table");
  assert.ok(selection.matchedSignals.includes("inventory table"));
});

test("intent selection marks AI Agent App as needing provider binding", () => {
  const selection = selectSoftwareSpecies({
    requirement: "Create an AI agent app with OpenAI support that answers support tickets.",
  });

  assert.equal(selection.speciesId, AI_AGENT_APP_SPECIES_ID);
  assert.equal(selection.runtimeKind, "ai-agent-app");
  assert.equal(selection.requiresProviderBinding, true);
  assert.ok(selection.matchedSignals.includes("ai agent"));
});

test("manual AI Agent App runtime selection uses the agent species", () => {
  const selection = selectSoftwareSpecies({
    selectedRuntimeKind: "ai-agent-app",
  });

  assert.equal(selection.source, "manual");
  assert.equal(selection.speciesId, AI_AGENT_APP_SPECIES_ID);
  assert.equal(selection.runtimeKind, "ai-agent-app");
  assert.equal(selection.requiresProviderBinding, true);
});

test("intent selection handles Chinese AI Agent signals", () => {
  const selection = selectSoftwareSpecies({
    requirement: "创建一个智能体应用，可以写小说、生图、生成视频。",
  });

  assert.equal(selection.speciesId, AI_AGENT_APP_SPECIES_ID);
  assert.equal(selection.runtimeKind, "ai-agent-app");
  assert.ok(selection.requiresProviderBinding);
});

test("runtime intent conversion keeps runtime choices separate from species metadata", () => {
  const selection = selectSoftwareSpecies({
    requirement: "Personal wiki for reading notes and a prompt library.",
  });
  const runtimeIntent = toRuntimeIntentSelection(selection);

  assert.equal(runtimeIntent.runtimeKind, "markdown-knowledge");
  assert.equal(runtimeIntent.softwareType, "knowledge-base");
  assert.equal(runtimeIntent.source, "automatic");
  assert.ok(runtimeIntent.confidence > 0.45);
});

test("unknown or empty intent defaults to static HTML", () => {
  const selection = selectSoftwareSpecies({ requirement: "" });

  assert.equal(selection.speciesId, "static-html");
  assert.equal(selection.runtimeKind satisfies RuntimeKind, "static-html");
  assert.equal(selection.source, "automatic");
});
