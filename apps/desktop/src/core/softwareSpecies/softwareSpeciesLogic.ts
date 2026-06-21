import type { RuntimeChoice, RuntimeIntentSelection, RuntimeKind } from "../../types";

export const AI_AGENT_APP_SPECIES_ID = "ai-agent-app" as const;

export type SoftwareSpeciesId =
  | RuntimeKind
  | "task-board"
  | "local-dashboard"
  | "focus-planner"
  | "knowledge-garden"
  | "data-ledger"
  | "file-triage"
  | "timebox-widget"
  | "knowledge-base"
  | "personal-crm"
  | "inventory-table"
  | "budget-tracker"
  | "file-utility"
  | "focus-widget"
  | typeof AI_AGENT_APP_SPECIES_ID;

export type SoftwareSpeciesCategory = "runtime" | "productivity" | "ai-agent-app";

export interface SoftwareSpeciesCatalogItem {
  id: SoftwareSpeciesId;
  label: string;
  summary: string;
  category: SoftwareSpeciesCategory;
  runtimeKind: RuntimeKind;
  requiresProviderBinding: boolean;
  tags: readonly string[];
  intentSignals: readonly string[];
}

export interface SoftwareSpeciesCatalogFilter {
  categories?: readonly SoftwareSpeciesCategory[];
  includeRuntimeSpecies?: boolean;
  includeProductivitySpecies?: boolean;
  includeAiAgentSpecies?: boolean;
}

export interface SelectSoftwareSpeciesInput {
  requirement?: string | null;
  selectedSpeciesId?: string | null;
  selectedRuntimeKind?: RuntimeChoice | null;
  catalog?: readonly SoftwareSpeciesCatalogItem[];
}

export interface SoftwareSpeciesRank {
  species: SoftwareSpeciesCatalogItem;
  score: number;
  matchedSignals: string[];
}

export interface SoftwareSpeciesSelection {
  species: SoftwareSpeciesCatalogItem;
  speciesId: SoftwareSpeciesId;
  runtimeKind: RuntimeKind;
  confidence: number;
  reason: string;
  matchedSignals: string[];
  alternatives: SoftwareSpeciesCatalogItem[];
  source: RuntimeIntentSelection["source"];
  requiresProviderBinding: boolean;
}

export const existingRuntimeSpeciesKinds: readonly RuntimeKind[] = [
  "static-html",
  "react-vite",
  "react-sqlite",
  "canvas2d",
  "markdown-knowledge",
  "data-table",
  "file-processor",
  "desktop-widget",
];

export const softwareSpeciesCatalog: readonly SoftwareSpeciesCatalogItem[] = [
  {
    id: "static-html",
    label: "Small single-page tool",
    summary: "Static HTML, CSS, and vanilla JavaScript for tiny local tools.",
    category: "runtime",
    runtimeKind: "static-html",
    requiresProviderBinding: false,
    tags: ["simple", "single-page", "vanilla-js", "no-build"],
    intentSignals: [
      "static html",
      "single page",
      "landing",
      "calculator",
      "tiny tool",
      "no build",
      "vanilla javascript",
      "html css",
    ],
  },
  {
    id: "react-vite",
    label: "Rich React app",
    summary: "React and Vite for composed, stateful UI applications.",
    category: "runtime",
    runtimeKind: "react-vite",
    requiresProviderBinding: false,
    tags: ["react", "vite", "frontend", "components"],
    intentSignals: [
      "react",
      "vite",
      "component",
      "complex ui",
      "multi page",
      "frontend app",
      "interactive dashboard",
    ],
  },
  {
    id: "react-sqlite",
    label: "Local database app",
    summary: "React UI with a local API and SQLite data inside the workspace.",
    category: "runtime",
    runtimeKind: "react-sqlite",
    requiresProviderBinding: false,
    tags: ["react", "sqlite", "crud", "local-data"],
    intentSignals: [
      "sqlite",
      "database",
      "crud",
      "local data",
      "offline database",
      "records",
      "customer manager",
      "persistent data",
    ],
  },
  {
    id: "canvas2d",
    label: "Canvas 2D experience",
    summary: "Canvas 2D for games, simulations, animations, and visual scenes.",
    category: "runtime",
    runtimeKind: "canvas2d",
    requiresProviderBinding: false,
    tags: ["canvas", "game", "animation", "simulation"],
    intentSignals: [
      "canvas",
      "game",
      "animation",
      "simulation",
      "sprite",
      "interactive scene",
      "drawing",
    ],
  },
  {
    id: "markdown-knowledge",
    label: "Markdown knowledge app",
    summary: "Markdown-backed notes, local wiki, prompt library, or writing workspace.",
    category: "runtime",
    runtimeKind: "markdown-knowledge",
    requiresProviderBinding: false,
    tags: ["markdown", "notes", "wiki", "knowledge"],
    intentSignals: [
      "markdown",
      "wiki",
      "notes",
      "knowledge",
      "prompt library",
      "writing workspace",
      "reading notes",
    ],
  },
  {
    id: "data-table",
    label: "Data table app",
    summary: "Local tables for CSVs, budgets, inventory, filtering, and sorting.",
    category: "runtime",
    runtimeKind: "data-table",
    requiresProviderBinding: false,
    tags: ["table", "csv", "tracker", "spreadsheet"],
    intentSignals: ["table", "spreadsheet", "csv", "budget", "inventory", "tracker", "filter", "sort"],
  },
  {
    id: "file-processor",
    label: "File processor",
    summary: "Explicitly selected local files with dry-run previews before writes.",
    category: "runtime",
    runtimeKind: "file-processor",
    requiresProviderBinding: false,
    tags: ["files", "batch", "dry-run", "local"],
    intentSignals: [
      "batch rename",
      "file processor",
      "file tool",
      "text replace",
      "csv cleaning",
      "image organizer",
      "dry run",
      "folder",
    ],
  },
  {
    id: "desktop-widget",
    label: "Desktop widget",
    summary: "Compact local widgets such as timers, quick notes, and small panels.",
    category: "runtime",
    runtimeKind: "desktop-widget",
    requiresProviderBinding: false,
    tags: ["widget", "timer", "compact", "desktop"],
    intentSignals: ["pomodoro", "countdown", "quick note", "widget", "small panel", "desktop widget", "timer"],
  },
  {
    id: "task-board",
    label: "Task board",
    summary: "A productivity board for tasks, kanban lanes, and project tracking.",
    category: "productivity",
    runtimeKind: "react-vite",
    requiresProviderBinding: false,
    tags: ["tasks", "kanban", "planning", "productivity"],
    intentSignals: [
      "task board",
      "kanban",
      "todo",
      "to do",
      "project tracker",
      "sprint",
      "tasks",
      "workflow board",
    ],
  },
  {
    id: "local-dashboard",
    label: "Local dashboard",
    summary: "A productivity dashboard for summaries, metrics, charts, and overview screens.",
    category: "productivity",
    runtimeKind: "react-vite",
    requiresProviderBinding: false,
    tags: ["dashboard", "metrics", "charts", "overview"],
    intentSignals: ["dashboard", "metrics", "charts", "overview", "status board", "analytics"],
  },
  {
    id: "focus-planner",
    label: "Focus Planner",
    summary: "A planning workspace for priorities, daily intent, and focused execution blocks.",
    category: "productivity",
    runtimeKind: "react-vite",
    requiresProviderBinding: false,
    tags: ["focus", "planning", "priorities", "productivity"],
    intentSignals: [
      "focus planner",
      "daily plan",
      "priority planner",
      "focus blocks",
      "deep work",
      "execution plan",
    ],
  },
  {
    id: "knowledge-garden",
    label: "Knowledge Garden",
    summary: "A connected local knowledge space for notes, references, and idea trails.",
    category: "productivity",
    runtimeKind: "markdown-knowledge",
    requiresProviderBinding: false,
    tags: ["knowledge", "markdown", "notes", "garden"],
    intentSignals: [
      "knowledge garden",
      "zettelkasten",
      "linked notes",
      "idea garden",
      "research garden",
      "knowledge graph",
    ],
  },
  {
    id: "data-ledger",
    label: "Data Ledger",
    summary: "A table-ledger species for local logs, records, balances, and audit trails.",
    category: "productivity",
    runtimeKind: "data-table",
    requiresProviderBinding: false,
    tags: ["ledger", "records", "audit", "table"],
    intentSignals: [
      "data ledger",
      "ledger",
      "audit trail",
      "records ledger",
      "log book",
      "transaction table",
    ],
  },
  {
    id: "file-triage",
    label: "File Triage",
    summary: "A safe review-first workflow for sorting, renaming, and cleaning selected files.",
    category: "productivity",
    runtimeKind: "file-processor",
    requiresProviderBinding: false,
    tags: ["files", "triage", "cleanup", "dry-run"],
    intentSignals: [
      "file triage",
      "triage files",
      "sort files",
      "clean files",
      "review files",
      "file inbox",
    ],
  },
  {
    id: "timebox-widget",
    label: "Timebox Widget",
    summary: "A compact desktop timebox helper for focused sessions and countdowns.",
    category: "productivity",
    runtimeKind: "desktop-widget",
    requiresProviderBinding: false,
    tags: ["timebox", "timer", "widget", "focus"],
    intentSignals: [
      "timebox widget",
      "timebox",
      "time box",
      "focus countdown",
      "work timer",
      "session timer",
    ],
  },
  {
    id: "knowledge-base",
    label: "Knowledge base",
    summary: "A local wiki or writing workspace with Markdown content and search.",
    category: "productivity",
    runtimeKind: "markdown-knowledge",
    requiresProviderBinding: false,
    tags: ["wiki", "notes", "search", "writing"],
    intentSignals: ["knowledge base", "personal wiki", "reading notes", "prompt library", "writing workspace", "local wiki"],
  },
  {
    id: "personal-crm",
    label: "Personal CRM",
    summary: "A local customer, contact, lead, or follow-up manager.",
    category: "productivity",
    runtimeKind: "react-sqlite",
    requiresProviderBinding: false,
    tags: ["crm", "contacts", "customers", "sqlite"],
    intentSignals: ["crm", "contacts", "customers", "sales pipeline", "leads", "follow up", "client manager"],
  },
  {
    id: "inventory-table",
    label: "Inventory table",
    summary: "A structured inventory, collection, or asset tracker.",
    category: "productivity",
    runtimeKind: "data-table",
    requiresProviderBinding: false,
    tags: ["inventory", "assets", "table", "csv"],
    intentSignals: [
      "inventory table",
      "stock tracker",
      "asset register",
      "collection manager",
      "item catalog",
      "quantity",
      "sku",
      "csv inventory",
    ],
  },
  {
    id: "budget-tracker",
    label: "Budget tracker",
    summary: "A local expense, ledger, or personal finance tracker.",
    category: "productivity",
    runtimeKind: "data-table",
    requiresProviderBinding: false,
    tags: ["budget", "finance", "expenses", "table"],
    intentSignals: ["budget tracker", "expense log", "spending", "personal finance", "ledger", "monthly budget"],
  },
  {
    id: "file-utility",
    label: "File utility",
    summary: "A safe local tool for batch renames, file cleanup, and text replacement.",
    category: "productivity",
    runtimeKind: "file-processor",
    requiresProviderBinding: false,
    tags: ["files", "rename", "cleanup", "dry-run"],
    intentSignals: [
      "batch rename",
      "rename files",
      "bulk files",
      "file cleanup",
      "organize images",
      "replace text in files",
      "file workflow",
    ],
  },
  {
    id: "focus-widget",
    label: "Focus widget",
    summary: "A compact productivity widget for timers, habits, and quick notes.",
    category: "productivity",
    runtimeKind: "desktop-widget",
    requiresProviderBinding: false,
    tags: ["focus", "pomodoro", "habit", "widget"],
    intentSignals: ["focus timer", "pomodoro", "countdown widget", "quick note widget", "small desktop timer", "habit widget"],
  },
  {
    id: AI_AGENT_APP_SPECIES_ID,
    label: "AI Agent App",
    summary: "A local AI-enabled app that binds to a configured provider without storing secrets in generated code.",
    category: "ai-agent-app",
    runtimeKind: "ai-agent-app",
    requiresProviderBinding: true,
    tags: ["ai", "agent", "llm", "provider-binding", "text", "image", "video"],
    intentSignals: [
      "ai agent",
      "agent app",
      "ai agent app",
      "llm",
      "chatbot",
      "copilot",
      "assistant",
      "openai",
      "anthropic",
      "claude",
      "gemini",
      "provider",
      "tool calling",
      "reasoning",
      "prompt workflow",
      "support bot",
      "write article",
      "article agent",
      "write novel",
      "novel agent",
      "image generation",
      "generate image",
      "video generation",
      "generate video",
      "multimodal",
      "生图",
      "生成图片",
      "生成视频",
      "写文章",
      "写小说",
      "智能体",
      "智能体应用",
    ],
  },
];

const categoryPriority: Record<SoftwareSpeciesCategory, number> = {
  "ai-agent-app": 0,
  productivity: 1,
  runtime: 2,
};

export function getSoftwareSpeciesCatalog(
  filter: SoftwareSpeciesCatalogFilter = {},
): SoftwareSpeciesCatalogItem[] {
  const categories = filter.categories ? new Set(filter.categories) : null;
  return softwareSpeciesCatalog.filter((species) => {
    if (categories && !categories.has(species.category)) return false;
    if (filter.includeRuntimeSpecies === false && species.category === "runtime") return false;
    if (filter.includeProductivitySpecies === false && species.category === "productivity") return false;
    if (filter.includeAiAgentSpecies === false && species.category === "ai-agent-app") return false;
    return true;
  });
}

export function findSoftwareSpecies(
  speciesId: string | null | undefined,
  catalog: readonly SoftwareSpeciesCatalogItem[] = softwareSpeciesCatalog,
): SoftwareSpeciesCatalogItem | null {
  if (!speciesId) return null;
  return catalog.find((species) => species.id === speciesId) ?? null;
}

export function getRuntimeSoftwareSpecies(
  runtimeKind: RuntimeKind,
  catalog: readonly SoftwareSpeciesCatalogItem[] = softwareSpeciesCatalog,
): SoftwareSpeciesCatalogItem {
  return (
    catalog.find((species) => species.category === "runtime" && species.runtimeKind === runtimeKind) ??
    catalog.find((species) => species.runtimeKind === runtimeKind) ??
    softwareSpeciesCatalog.find((species) => species.id === "static-html")!
  );
}

export function rankSoftwareSpecies(
  requirement: string | null | undefined,
  catalog: readonly SoftwareSpeciesCatalogItem[] = softwareSpeciesCatalog,
): SoftwareSpeciesRank[] {
  const normalizedRequirement = normalizeIntentText(requirement ?? "");
  return catalog
    .map((species) => scoreSoftwareSpecies(species, normalizedRequirement))
    .sort((left, right) => {
      const scoreDiff = right.score - left.score;
      if (scoreDiff !== 0) return scoreDiff;
      const categoryDiff = categoryPriority[left.species.category] - categoryPriority[right.species.category];
      if (categoryDiff !== 0) return categoryDiff;
      return left.species.label.localeCompare(right.species.label);
    });
}

export function selectSoftwareSpecies(
  input: SelectSoftwareSpeciesInput = {},
): SoftwareSpeciesSelection {
  const catalog = input.catalog ?? softwareSpeciesCatalog;
  const selectedSpecies = findSoftwareSpecies(input.selectedSpeciesId, catalog);
  if (selectedSpecies) {
    return createSelection(selectedSpecies, [], "manual", 1, `Selected ${selectedSpecies.label}.`);
  }

  if (input.selectedRuntimeKind && input.selectedRuntimeKind !== "auto") {
    const runtimeSpecies = getRuntimeSoftwareSpecies(input.selectedRuntimeKind, catalog);
    return createSelection(runtimeSpecies, [], "manual", 1, `Selected ${runtimeSpecies.label}.`);
  }

  const ranked = rankSoftwareSpecies(input.requirement, catalog);
  const best = ranked.find((rank) => rank.score > 0);
  if (!best) {
    const fallback = getRuntimeSoftwareSpecies("static-html", catalog);
    return createSelection(
      fallback,
      ranked.slice(0, 3).map((rank) => rank.species).filter((species) => species.id !== fallback.id),
      "automatic",
      0.35,
      `No specific species signals matched; using ${fallback.label}.`,
    );
  }

  return createSelection(
    best.species,
    ranked
      .filter((rank) => rank.species.id !== best.species.id && rank.score > 0)
      .slice(0, 3)
      .map((rank) => rank.species),
    "automatic",
    Math.min(0.95, 0.45 + best.score / 12),
    `Matched ${best.matchedSignals.join(", ")} for ${best.species.label}.`,
    best.matchedSignals,
  );
}

export function toRuntimeIntentSelection(
  selection: SoftwareSpeciesSelection,
): RuntimeIntentSelection {
  return {
    runtimeKind: selection.runtimeKind,
    softwareType: selection.species.id,
    confidence: selection.confidence,
    reason: selection.reason,
    matchedSignals: selection.matchedSignals,
    alternatives: uniqueRuntimeKinds(
      selection.alternatives
        .map((species) => species.runtimeKind)
        .filter((runtimeKind) => runtimeKind !== selection.runtimeKind),
    ),
    source: selection.source,
  };
}

function createSelection(
  species: SoftwareSpeciesCatalogItem,
  alternatives: SoftwareSpeciesCatalogItem[],
  source: RuntimeIntentSelection["source"],
  confidence: number,
  reason: string,
  matchedSignals: string[] = [],
): SoftwareSpeciesSelection {
  return {
    species,
    speciesId: species.id,
    runtimeKind: species.runtimeKind,
    confidence,
    reason,
    matchedSignals,
    alternatives,
    source,
    requiresProviderBinding: species.requiresProviderBinding,
  };
}

function scoreSoftwareSpecies(
  species: SoftwareSpeciesCatalogItem,
  normalizedRequirement: string,
): SoftwareSpeciesRank {
  if (!normalizedRequirement) {
    return { species, score: 0, matchedSignals: [] };
  }

  const matchedSignals: string[] = [];
  const score = species.intentSignals.reduce((total, signal) => {
    const normalizedSignal = normalizeIntentText(signal);
    if (!normalizedSignal || !containsIntentSignal(normalizedRequirement, normalizedSignal)) {
      return total;
    }

    matchedSignals.push(signal);
    return total + (normalizedSignal.includes(" ") ? 3 : 1);
  }, 0);

  const categoryBoost = species.category === "runtime" || score === 0 ? 0 : 0.25;
  return { species, score: score + categoryBoost, matchedSignals };
}

function normalizeIntentText(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^\p{L}\p{N}+#]+/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function containsIntentSignal(text: string, signal: string): boolean {
  if (signal.includes(" ")) {
    return text.includes(signal);
  }
  return new RegExp(`(^|\\s)${escapeRegExp(signal)}(\\s|$)`).test(text);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function uniqueRuntimeKinds(runtimeKinds: RuntimeKind[]): RuntimeKind[] {
  return [...new Set(runtimeKinds)];
}
