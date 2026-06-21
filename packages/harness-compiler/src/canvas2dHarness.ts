import type {
  CommandPolicy,
  FileSystemPolicy,
  HarnessPolicy,
  OutputContract,
  RuntimePolicy,
} from "./promptEnvelope";

export const CANVAS2D_RUNTIME_PACK_ID = "sofvary.runtime.canvas2d";
export const CANVAS2D_HARNESS_PACK_ID = "sofvary.harness.canvas2d";
export const CANVAS2D_PACK_VERSION = "0.1.0";
export const CANVAS2D_GENERATED_ROOT = "generated/canvas";
export const CANVAS2D_ENTRYPOINT = "index.html";
export const CANVAS2D_ALLOWED_FILES = [
  "index.html",
  "style.css",
  "src/main.js",
  "src/engine/loop.js",
  "src/engine/input.js",
  "src/engine/scene.js",
  "src/engine/collision.js",
  "src/engine/assets.js",
  "src/game/config.js",
  "src/game/player.js",
  "src/game/enemies.js",
  "src/game/levels.js",
] as const;

export const CANVAS2D_BLOCKED_CAPABILITIES = [
  "cdn-assets",
  "external-assets",
  "npm-package-install",
  "plugin-execution",
  "react-runtime",
  "remote-network",
  "shell-command",
  "sofvary-shell-ui",
  "sqlite-runtime",
] as const;

export const CANVAS2D_RUNTIME_POLICY: RuntimePolicy = {
  runtimeKind: "canvas2d",
  allowedEntrypoints: [CANVAS2D_ENTRYPOINT],
  allowedServerBind: "127.0.0.1",
  network: "local-only",
  packageInstall: false,
};

export const CANVAS2D_FILE_SYSTEM_POLICY: FileSystemPolicy = {
  root: CANVAS2D_GENERATED_ROOT,
  allowedFiles: [...CANVAS2D_ALLOWED_FILES],
  allowExternalFiles: false,
  allowPathTraversal: false,
};

export const CANVAS2D_COMMAND_POLICY: CommandPolicy = {
  allowShell: false,
  allowPackageInstall: false,
  allowGlobalInstall: false,
  allowedCommands: [],
};

export const CANVAS2D_OUTPUT_CONTRACT: OutputContract = {
  format: "canvas2d-project",
  files: [...CANVAS2D_ALLOWED_FILES],
  shellUiIncluded: false,
};

export const CANVAS2D_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a self-contained Canvas 2D app using browser Canvas 2D APIs and requestAnimationFrame.",
    "Do not use React, CDNs, remote assets, npm packages, external dependencies, SQLite, or plugin execution.",
  ],
  fileSystemRules: [
    "Write only inside generated/canvas for this runtime.",
    "Do not read or write paths outside the active Sofvary workspace.",
    "Split update, render, input, and state across the declared engine and game files.",
    "Do not create files outside the Canvas 2D output contract.",
    "Use assets/ only for workspace-local assets.",
  ],
  outputRules: [
    "Use requestAnimationFrame as the main loop driver.",
    "Use CanvasRenderingContext2D for rendering.",
    "Level data should be configurable.",
    "Include pause and restart controls where reasonable.",
    "Keep Sofvary shell UI out of generated app source.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [...CANVAS2D_BLOCKED_CAPABILITIES],
};
