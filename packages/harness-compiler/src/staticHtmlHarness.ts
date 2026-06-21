import type {
  CommandPolicy,
  FileSystemPolicy,
  HarnessPolicy,
  OutputContract,
  RuntimePolicy,
} from "./promptEnvelope";

export const STATIC_HTML_RUNTIME_PACK_ID = "sofvary.runtime.static-html";
export const STATIC_HTML_HARNESS_PACK_ID = "sofvary.harness.static-html";
export const STATIC_HTML_PACK_VERSION = "0.1.0";
export const STATIC_HTML_GENERATED_ROOT = "generated/static";
export const STATIC_HTML_ENTRYPOINT = "index.html";
export const STATIC_HTML_ALLOWED_FILES = ["index.html", "style.css", "app.js"] as const;

export const STATIC_HTML_BLOCKED_CAPABILITIES = [
  "react-runtime",
  "sqlite-runtime",
  "npm-package-install",
  "cdn-assets",
  "remote-network",
  "shell-command",
  "plugin-execution",
  "sofvary-shell-ui",
] as const;

export const STATIC_HTML_RUNTIME_POLICY: RuntimePolicy = {
  runtimeKind: "static-html",
  allowedEntrypoints: [STATIC_HTML_ENTRYPOINT],
  allowedServerBind: "127.0.0.1",
  network: "local-only",
  packageInstall: false,
};

export const STATIC_HTML_FILE_SYSTEM_POLICY: FileSystemPolicy = {
  root: STATIC_HTML_GENERATED_ROOT,
  allowedFiles: [...STATIC_HTML_ALLOWED_FILES],
  allowExternalFiles: false,
  allowPathTraversal: false,
};

export const STATIC_HTML_COMMAND_POLICY: CommandPolicy = {
  allowShell: false,
  allowPackageInstall: false,
  allowGlobalInstall: false,
  allowedCommands: [],
};

export const STATIC_HTML_OUTPUT_CONTRACT: OutputContract = {
  format: "static-html-files",
  files: [...STATIC_HTML_ALLOWED_FILES],
  shellUiIncluded: false,
};

export const STATIC_HTML_HARNESS_POLICY: HarnessPolicy = {
  systemInstructions: [
    "Generate a self-contained static app using index.html, style.css, and app.js.",
    "Do not require npm, CDNs, remote scripts, external assets, React, SQLite, or plugin execution.",
  ],
  fileSystemRules: [
    "Write only inside generated/static for this runtime.",
    "Do not read or write paths outside the active Sofvary workspace.",
    "Do not create files other than index.html, style.css, and app.js.",
    "Use localStorage only for small local preferences.",
  ],
  outputRules: [
    "Keep Sofvary shell UI out of generated app source.",
    "The app must run through the local static preview server.",
    "The generated app must be exportable without Sofvary floating menu or build overlay UI.",
  ],
  blockedCapabilities: [...STATIC_HTML_BLOCKED_CAPABILITIES],
};
