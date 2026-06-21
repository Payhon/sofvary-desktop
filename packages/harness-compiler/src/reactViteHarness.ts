import type {
  CommandPolicy,
  FileSystemPolicy,
  HarnessPolicy,
  OutputContract,
  RuntimePolicy,
} from "./promptEnvelope";

export const REACT_VITE_RUNTIME_PACK_ID = "sofvary.runtime.react-vite";
export const REACT_VITE_HARNESS_PACK_ID = "sofvary.harness.react-vite";
export const REACT_VITE_PACK_VERSION = "0.1.0";
export const REACT_VITE_GENERATED_ROOT = "generated/react";
export const REACT_VITE_ENTRYPOINT = "src/main.tsx";
export const REACT_VITE_ALLOWED_FILES = [
  "package.json",
  "index.html",
  "vite.config.ts",
  "tsconfig.json",
  "src/main.tsx",
  "src/App.tsx",
  "src/components/TaskBoard.tsx",
  "src/styles/app.css",
] as const;

export const REACT_VITE_BLOCKED_CAPABILITIES = [
  "cdn-assets",
  "electron-runtime",
  "external-ui-framework",
  "nextjs-runtime",
  "plugin-execution",
  "remote-network",
  "shell-command",
  "sofvary-shell-ui",
  "sqlite-runtime",
] as const;

export const REACT_VITE_RUNTIME_POLICY: RuntimePolicy = {
  runtimeKind: "react-vite",
  allowedEntrypoints: [REACT_VITE_ENTRYPOINT],
  allowedServerBind: "127.0.0.1",
  network: "local-only",
  packageInstall: false,
};

export const REACT_VITE_FILE_SYSTEM_POLICY: FileSystemPolicy = {
  root: REACT_VITE_GENERATED_ROOT,
  allowedFiles: [...REACT_VITE_ALLOWED_FILES],
  allowExternalFiles: false,
  allowPathTraversal: false,
};

export const REACT_VITE_COMMAND_POLICY: CommandPolicy = {
  allowShell: false,
  allowPackageInstall: false,
  allowGlobalInstall: false,
  allowedCommands: [],
};

export const REACT_VITE_OUTPUT_CONTRACT: OutputContract = {
  format: "react-vite-project",
  files: [...REACT_VITE_ALLOWED_FILES],
  shellUiIncluded: false,
};

export const REACT_VITE_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a React + Vite app using React function components and TypeScript.",
    "Do not use Next.js, Electron, external CDNs, remote assets, SQLite, plugin execution, or a default UI framework.",
  ],
  fileSystemRules: [
    "Write only inside generated/react for this runtime.",
    "Do not read or write paths outside the active Sofvary workspace.",
    "Components go under src/components.",
    "Styles go under src/styles.",
    "Do not create files outside the React + Vite output contract.",
  ],
  outputRules: [
    "App.tsx is the main app entry component.",
    "The generated app must pass npm run build.",
    "Keep Sofvary shell UI out of generated app source.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [...REACT_VITE_BLOCKED_CAPABILITIES],
};
