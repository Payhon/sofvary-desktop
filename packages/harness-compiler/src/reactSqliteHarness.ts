import type {
  CommandPolicy,
  FileSystemPolicy,
  HarnessPolicy,
  OutputContract,
  RuntimePolicy,
} from "./promptEnvelope";

export const REACT_SQLITE_RUNTIME_PACK_ID = "sofvary.runtime.react-sqlite";
export const REACT_SQLITE_HARNESS_PACK_ID = "sofvary.harness.react-sqlite";
export const REACT_SQLITE_PACK_VERSION = "0.1.0";
export const REACT_SQLITE_GENERATED_ROOT = "generated";
export const REACT_SQLITE_ENTRYPOINT = "react/src/main.tsx";
export const REACT_SQLITE_ALLOWED_FILES = [
  "react/package.json",
  "react/index.html",
  "react/vite.config.ts",
  "react/tsconfig.json",
  "react/src/main.tsx",
  "react/src/App.tsx",
  "react/src/components/CustomerManager.tsx",
  "react/src/styles/app.css",
  "react/server/index.ts",
  "react/server/db.ts",
  "react/server/routes/customers.ts",
  "data/schema.json",
  "data/migrations/001_create_customers.sql",
  "data/seed.sql",
] as const;

export const REACT_SQLITE_BLOCKED_CAPABILITIES = [
  "cdn-assets",
  "cloud-service",
  "direct-frontend-sqlite-access",
  "electron-runtime",
  "external-ui-framework",
  "nextjs-runtime",
  "plugin-execution",
  "remote-database",
  "remote-network",
  "sensitive-credentials",
  "shell-command",
  "sofvary-shell-ui",
] as const;

export const REACT_SQLITE_RUNTIME_POLICY: RuntimePolicy = {
  runtimeKind: "react-sqlite",
  allowedEntrypoints: [REACT_SQLITE_ENTRYPOINT],
  allowedServerBind: "127.0.0.1",
  network: "local-only",
  packageInstall: false,
};

export const REACT_SQLITE_FILE_SYSTEM_POLICY: FileSystemPolicy = {
  root: REACT_SQLITE_GENERATED_ROOT,
  allowedFiles: [...REACT_SQLITE_ALLOWED_FILES],
  allowExternalFiles: false,
  allowPathTraversal: false,
};

export const REACT_SQLITE_COMMAND_POLICY: CommandPolicy = {
  allowShell: false,
  allowPackageInstall: false,
  allowGlobalInstall: false,
  allowedCommands: [],
};

export const REACT_SQLITE_OUTPUT_CONTRACT: OutputContract = {
  format: "react-sqlite-project",
  files: [...REACT_SQLITE_ALLOWED_FILES],
  shellUiIncluded: false,
};

export const REACT_SQLITE_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a React + Vite frontend, Node local API, and workspace-local SQLite persistence.",
    "Frontend code must call /api/* endpoints only and must never import SQLite, sql.js, database files, or filesystem modules.",
  ],
  fileSystemRules: [
    "Write React and Node API files only inside generated/react.",
    "Write schema, migrations, seed data, and app.sqlite only inside generated/data.",
    "Do not read or write paths outside the active Sofvary workspace.",
    "Do not create files outside the React + SQLite output contract.",
  ],
  outputRules: [
    "The frontend calls /api/health and /api/customers through relative /api/* URLs.",
    "The Node API binds to 127.0.0.1 only and owns every SQLite operation.",
    "SQL statements use prepared statements for user-controlled values.",
    "The SQLite database file stays at generated/data/app.sqlite.",
    "Keep Sofvary shell UI out of generated app source.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [...REACT_SQLITE_BLOCKED_CAPABILITIES],
};
