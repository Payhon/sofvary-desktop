use crate::core::pack_types::{HarnessPackManifest, RuntimePackManifest};
use crate::core::workspace_types::AppBoxManifest;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use thiserror::Error;
use uuid::Uuid;

const PROMPT_ENVELOPE_SCHEMA_VERSION: &str = "1.0";
const MAX_CONTEXT_FILE_BYTES: usize = 32 * 1024;
const MAX_CONTEXT_TOTAL_BYTES: usize = 128 * 1024;
const STATIC_HTML_RUNTIME_KIND: &str = "static-html";
const STATIC_HTML_GENERATED_ROOT: &str = "generated/static";
const STATIC_HTML_ENTRYPOINT: &str = "index.html";
const STATIC_HTML_BIND: &str = "127.0.0.1";
const STATIC_HTML_NETWORK: &str = "local-only";
pub const STATIC_HTML_ALLOWED_FILES: [&str; 3] = ["index.html", "style.css", "app.js"];
const REACT_VITE_RUNTIME_KIND: &str = "react-vite";
const REACT_VITE_GENERATED_ROOT: &str = "generated/react";
const REACT_VITE_ENTRYPOINT: &str = "src/main.tsx";
const REACT_VITE_BIND: &str = "127.0.0.1";
const REACT_VITE_NETWORK: &str = "local-only";
pub const REACT_VITE_ALLOWED_FILES: [&str; 8] = [
    "package.json",
    "index.html",
    "vite.config.ts",
    "tsconfig.json",
    "src/main.tsx",
    "src/App.tsx",
    "src/components/TaskBoard.tsx",
    "src/styles/app.css",
];
const REACT_SQLITE_RUNTIME_KIND: &str = "react-sqlite";
const REACT_SQLITE_GENERATED_ROOT: &str = "generated";
const REACT_SQLITE_ENTRYPOINT: &str = "react/src/main.tsx";
const REACT_SQLITE_BIND: &str = "127.0.0.1";
const REACT_SQLITE_NETWORK: &str = "local-only";
pub const REACT_SQLITE_ALLOWED_FILES: [&str; 14] = [
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
];
const AI_AGENT_APP_RUNTIME_KIND: &str = "ai-agent-app";
const AI_AGENT_APP_GENERATED_ROOT: &str = "generated";
const AI_AGENT_APP_ENTRYPOINT: &str = "react/src/main.tsx";
const AI_AGENT_APP_BIND: &str = "127.0.0.1";
const AI_AGENT_APP_NETWORK: &str = "local-only";
pub const AI_AGENT_APP_ALLOWED_FILES: [&str; 13] = [
    "ai/agents.json",
    "ai/provider-requirements.json",
    "ai/jobs.seed.json",
    "react/package.json",
    "react/index.html",
    "react/vite.config.ts",
    "react/tsconfig.json",
    "react/src/main.tsx",
    "react/src/App.tsx",
    "react/src/components/AiAgentApp.tsx",
    "react/src/components/ProviderSettings.tsx",
    "react/src/components/ArtifactGallery.tsx",
    "react/src/styles/app.css",
];
const CANVAS2D_RUNTIME_KIND: &str = "canvas2d";
const CANVAS2D_GENERATED_ROOT: &str = "generated/canvas";
const CANVAS2D_ENTRYPOINT: &str = "index.html";
const CANVAS2D_BIND: &str = "127.0.0.1";
const CANVAS2D_NETWORK: &str = "local-only";
pub const CANVAS2D_ALLOWED_FILES: [&str; 12] = [
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
];
const MARKDOWN_KNOWLEDGE_RUNTIME_KIND: &str = "markdown-knowledge";
const MARKDOWN_KNOWLEDGE_GENERATED_ROOT: &str = "generated";
const MARKDOWN_KNOWLEDGE_ENTRYPOINT: &str = "react/src/main.tsx";
const MARKDOWN_KNOWLEDGE_BIND: &str = "127.0.0.1";
const MARKDOWN_KNOWLEDGE_NETWORK: &str = "local-only";
pub const MARKDOWN_KNOWLEDGE_ALLOWED_FILES: [&str; 10] = [
    "markdown/index.json",
    "markdown/content/getting-started.md",
    "react/package.json",
    "react/index.html",
    "react/vite.config.ts",
    "react/tsconfig.json",
    "react/src/main.tsx",
    "react/src/App.tsx",
    "react/src/components/MarkdownKnowledgeApp.tsx",
    "react/src/styles/app.css",
];
const DATA_TABLE_RUNTIME_KIND: &str = "data-table";
const DATA_TABLE_GENERATED_ROOT: &str = "generated";
const DATA_TABLE_ENTRYPOINT: &str = "react/src/main.tsx";
const DATA_TABLE_BIND: &str = "127.0.0.1";
const DATA_TABLE_NETWORK: &str = "local-only";
pub const DATA_TABLE_ALLOWED_FILES: [&str; 10] = [
    "data/schema.json",
    "data/tables/inventory.json",
    "react/package.json",
    "react/index.html",
    "react/vite.config.ts",
    "react/tsconfig.json",
    "react/src/main.tsx",
    "react/src/App.tsx",
    "react/src/components/DataTableApp.tsx",
    "react/src/styles/app.css",
];
const FILE_PROCESSOR_RUNTIME_KIND: &str = "file-processor";
const FILE_PROCESSOR_GENERATED_ROOT: &str = "generated";
const FILE_PROCESSOR_ENTRYPOINT: &str = "react/src/main.tsx";
const FILE_PROCESSOR_BIND: &str = "127.0.0.1";
const FILE_PROCESSOR_NETWORK: &str = "local-only";
pub const FILE_PROCESSOR_ALLOWED_FILES: [&str; 10] = [
    "file-processor/policy.json",
    "file-processor/dry-run-template.json",
    "react/package.json",
    "react/index.html",
    "react/vite.config.ts",
    "react/tsconfig.json",
    "react/src/main.tsx",
    "react/src/App.tsx",
    "react/src/components/FileProcessorApp.tsx",
    "react/src/styles/app.css",
];
const DESKTOP_WIDGET_RUNTIME_KIND: &str = "desktop-widget";
const DESKTOP_WIDGET_GENERATED_ROOT: &str = "generated";
const DESKTOP_WIDGET_ENTRYPOINT: &str = "react/src/main.tsx";
const DESKTOP_WIDGET_BIND: &str = "127.0.0.1";
const DESKTOP_WIDGET_NETWORK: &str = "local-only";
pub const DESKTOP_WIDGET_ALLOWED_FILES: [&str; 9] = [
    "widget/manifest.json",
    "react/package.json",
    "react/index.html",
    "react/vite.config.ts",
    "react/tsconfig.json",
    "react/src/main.tsx",
    "react/src/App.tsx",
    "react/src/components/DesktopWidgetApp.tsx",
    "react/src/styles/app.css",
];

#[derive(Debug, Error)]
pub enum HarnessEngineError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("runtime pack is not compatible with the selected harness compiler: {0}")]
    IncompatibleRuntime(String),
    #[error("harness pack is not compatible with runtime pack {runtime_id}: {harness_id}")]
    IncompatibleHarness {
        runtime_id: String,
        harness_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BoxRuntimeContext {
    pub runtime_pack: PackReference,
    pub harness_packs: Vec<PackReference>,
    pub runtime_kind: String,
    pub generated_root: String,
    pub entrypoint: String,
    pub bind: String,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentAppFileContext {
    pub relative_path: String,
    pub contents: String,
    pub byte_size: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentAppState {
    pub app_id: String,
    pub workspace_name: String,
    pub mode: String,
    pub existing_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_context: Vec<CurrentAppFileContext>,
    pub preview_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePolicy {
    pub runtime_kind: String,
    pub allowed_entrypoints: Vec<String>,
    pub allowed_server_bind: String,
    pub network: String,
    pub package_install: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessPolicy {
    pub system_instructions: Vec<String>,
    pub file_system_rules: Vec<String>,
    pub output_rules: Vec<String>,
    pub blocked_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemPolicy {
    pub root: String,
    pub allowed_files: Vec<String>,
    pub allow_external_files: bool,
    pub allow_path_traversal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandPolicy {
    pub allow_shell: bool,
    pub allow_package_install: bool,
    pub allow_global_install: bool,
    pub allowed_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutputContract {
    pub format: String,
    pub files: Vec<String>,
    pub shell_ui_included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptEnvelope {
    pub schema_version: String,
    pub envelope_id: String,
    pub created_at: String,
    pub box_runtime_context: BoxRuntimeContext,
    pub user_intent: String,
    pub current_app_state: CurrentAppState,
    pub runtime_policy: RuntimePolicy,
    pub harness_policy: HarnessPolicy,
    pub file_system_policy: FileSystemPolicy,
    pub command_policy: CommandPolicy,
    pub output_contract: OutputContract,
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptEnvelopeSummary {
    pub runtime: String,
    pub harnesses: Vec<String>,
    pub allowed_files: Vec<String>,
    pub blocked_capabilities: Vec<String>,
    pub output_contract: Vec<String>,
    pub acceptance_criteria_count: usize,
}

#[derive(Default)]
pub struct HarnessEngine;

impl HarnessEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn create_static_html_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        validate_static_html_runtime(runtime_pack)?;
        validate_static_html_harness(runtime_pack, harness_pack)?;

        let existing_files = existing_static_files(manifest)?;
        let current_app_state = build_current_app_state(
            manifest,
            existing_files,
            &manifest.paths.generated_static,
            &STATIC_HTML_ALLOWED_FILES,
        )?;
        let mut harness_policy = merged_static_html_policy(harness_pack);
        let mut acceptance_criteria = vec![
            "Generated output contains exactly index.html, style.css, and app.js.".to_string(),
            "Generated app does not include Sofvary shell UI, floating menu, build overlay, or host controls.".to_string(),
            "Generated app runs through the local static preview server without network downloads.".to_string(),
            format!(
                "Harness {}@{} constraints are represented in the prompt envelope.",
                harness_pack.id, harness_pack.version
            ),
        ];
        append_software_naming_criteria(&mut acceptance_criteria, &manifest.name);
        apply_update_context_policy(
            &current_app_state,
            &mut harness_policy,
            &mut acceptance_criteria,
        );

        Ok(PromptEnvelope {
            schema_version: PROMPT_ENVELOPE_SCHEMA_VERSION.to_string(),
            envelope_id: format!("penv_{}", Uuid::new_v4().simple()),
            created_at: Utc::now().to_rfc3339(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: runtime_pack.id.clone(),
                    version: runtime_pack.version.clone(),
                },
                harness_packs: vec![PackReference {
                    id: harness_pack.id.clone(),
                    version: harness_pack.version.clone(),
                }],
                runtime_kind: STATIC_HTML_RUNTIME_KIND.to_string(),
                generated_root: STATIC_HTML_GENERATED_ROOT.to_string(),
                entrypoint: STATIC_HTML_ENTRYPOINT.to_string(),
                bind: STATIC_HTML_BIND.to_string(),
                network: STATIC_HTML_NETWORK.to_string(),
            },
            user_intent: user_intent.trim().to_string(),
            current_app_state,
            runtime_policy: RuntimePolicy {
                runtime_kind: STATIC_HTML_RUNTIME_KIND.to_string(),
                allowed_entrypoints: vec![STATIC_HTML_ENTRYPOINT.to_string()],
                allowed_server_bind: STATIC_HTML_BIND.to_string(),
                network: STATIC_HTML_NETWORK.to_string(),
                package_install: false,
            },
            harness_policy,
            file_system_policy: FileSystemPolicy {
                root: STATIC_HTML_GENERATED_ROOT.to_string(),
                allowed_files: allowed_static_files(),
                allow_external_files: false,
                allow_path_traversal: false,
            },
            command_policy: CommandPolicy {
                allow_shell: false,
                allow_package_install: false,
                allow_global_install: false,
                allowed_commands: Vec::new(),
            },
            output_contract: OutputContract {
                format: "static-html-files".to_string(),
                files: allowed_static_files(),
                shell_ui_included: false,
            },
            acceptance_criteria,
        })
    }

    pub fn create_react_vite_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        validate_react_vite_runtime(runtime_pack)?;
        validate_runtime_harness(runtime_pack, harness_pack)?;

        let existing_files = existing_react_files(manifest)?;
        let react_root = manifest.paths.generated.join("react");
        let current_app_state = build_current_app_state(
            manifest,
            existing_files,
            &react_root,
            &REACT_VITE_ALLOWED_FILES,
        )?;
        let mut harness_policy = merged_react_vite_policy(harness_pack);
        let mut acceptance_criteria = vec![
            "Generated output contains exactly the React + Vite project file set.".to_string(),
            "Generated app uses React function components, TypeScript, and Vite.".to_string(),
            "Generated app does not include Sofvary shell UI, floating menu, build overlay, or host controls.".to_string(),
            "Generated app uses no Next.js, Electron, external CDN, remote assets, SQLite, or default UI framework.".to_string(),
            "Generated app can run through the local Vite dev server bound to 127.0.0.1.".to_string(),
            format!(
                "Harness {}@{} constraints are represented in the prompt envelope.",
                harness_pack.id, harness_pack.version
            ),
        ];
        append_software_naming_criteria(&mut acceptance_criteria, &manifest.name);
        apply_update_context_policy(
            &current_app_state,
            &mut harness_policy,
            &mut acceptance_criteria,
        );

        Ok(PromptEnvelope {
            schema_version: PROMPT_ENVELOPE_SCHEMA_VERSION.to_string(),
            envelope_id: format!("penv_{}", Uuid::new_v4().simple()),
            created_at: Utc::now().to_rfc3339(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: runtime_pack.id.clone(),
                    version: runtime_pack.version.clone(),
                },
                harness_packs: vec![PackReference {
                    id: harness_pack.id.clone(),
                    version: harness_pack.version.clone(),
                }],
                runtime_kind: REACT_VITE_RUNTIME_KIND.to_string(),
                generated_root: REACT_VITE_GENERATED_ROOT.to_string(),
                entrypoint: REACT_VITE_ENTRYPOINT.to_string(),
                bind: REACT_VITE_BIND.to_string(),
                network: REACT_VITE_NETWORK.to_string(),
            },
            user_intent: user_intent.trim().to_string(),
            current_app_state,
            runtime_policy: RuntimePolicy {
                runtime_kind: REACT_VITE_RUNTIME_KIND.to_string(),
                allowed_entrypoints: vec![REACT_VITE_ENTRYPOINT.to_string()],
                allowed_server_bind: REACT_VITE_BIND.to_string(),
                network: REACT_VITE_NETWORK.to_string(),
                package_install: false,
            },
            harness_policy,
            file_system_policy: FileSystemPolicy {
                root: REACT_VITE_GENERATED_ROOT.to_string(),
                allowed_files: allowed_react_vite_files(),
                allow_external_files: false,
                allow_path_traversal: false,
            },
            command_policy: CommandPolicy {
                allow_shell: false,
                allow_package_install: false,
                allow_global_install: false,
                allowed_commands: Vec::new(),
            },
            output_contract: OutputContract {
                format: "react-vite-project".to_string(),
                files: allowed_react_vite_files(),
                shell_ui_included: false,
            },
            acceptance_criteria,
        })
    }

    pub fn create_react_sqlite_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        validate_react_sqlite_runtime(runtime_pack)?;
        validate_runtime_harness(runtime_pack, harness_pack)?;

        let existing_files = existing_generated_files(manifest)?;
        let current_app_state = build_current_app_state(
            manifest,
            existing_files,
            &manifest.paths.generated,
            &REACT_SQLITE_ALLOWED_FILES,
        )?;
        let mut harness_policy = merged_react_sqlite_policy(harness_pack);
        let mut acceptance_criteria = vec![
            "Generated output contains exactly the React + SQLite project file set.".to_string(),
            "Frontend code calls /api/* endpoints and does not import or open SQLite directly.".to_string(),
            "Node local API owns SQLite access and binds to 127.0.0.1 only.".to_string(),
            "SQLite database file is stored inside generated/data/app.sqlite.".to_string(),
            "Every user-controlled SQL value is passed through parameterized statements.".to_string(),
            "Generated app uses no remote database, cloud service, sensitive credentials, external CDN, or Sofvary shell UI.".to_string(),
            format!(
                "Harness {}@{} constraints are represented in the prompt envelope.",
                harness_pack.id, harness_pack.version
            ),
        ];
        append_software_naming_criteria(&mut acceptance_criteria, &manifest.name);
        apply_update_context_policy(
            &current_app_state,
            &mut harness_policy,
            &mut acceptance_criteria,
        );

        Ok(PromptEnvelope {
            schema_version: PROMPT_ENVELOPE_SCHEMA_VERSION.to_string(),
            envelope_id: format!("penv_{}", Uuid::new_v4().simple()),
            created_at: Utc::now().to_rfc3339(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: runtime_pack.id.clone(),
                    version: runtime_pack.version.clone(),
                },
                harness_packs: vec![PackReference {
                    id: harness_pack.id.clone(),
                    version: harness_pack.version.clone(),
                }],
                runtime_kind: REACT_SQLITE_RUNTIME_KIND.to_string(),
                generated_root: REACT_SQLITE_GENERATED_ROOT.to_string(),
                entrypoint: REACT_SQLITE_ENTRYPOINT.to_string(),
                bind: REACT_SQLITE_BIND.to_string(),
                network: REACT_SQLITE_NETWORK.to_string(),
            },
            user_intent: user_intent.trim().to_string(),
            current_app_state,
            runtime_policy: RuntimePolicy {
                runtime_kind: REACT_SQLITE_RUNTIME_KIND.to_string(),
                allowed_entrypoints: vec![REACT_SQLITE_ENTRYPOINT.to_string()],
                allowed_server_bind: REACT_SQLITE_BIND.to_string(),
                network: REACT_SQLITE_NETWORK.to_string(),
                package_install: false,
            },
            harness_policy,
            file_system_policy: FileSystemPolicy {
                root: REACT_SQLITE_GENERATED_ROOT.to_string(),
                allowed_files: allowed_react_sqlite_files(),
                allow_external_files: false,
                allow_path_traversal: false,
            },
            command_policy: CommandPolicy {
                allow_shell: false,
                allow_package_install: false,
                allow_global_install: false,
                allowed_commands: Vec::new(),
            },
            output_contract: OutputContract {
                format: "react-sqlite-project".to_string(),
                files: allowed_react_sqlite_files(),
                shell_ui_included: false,
            },
            acceptance_criteria,
        })
    }

    pub fn create_ai_agent_app_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        create_generated_project_envelope(
            user_intent,
            manifest,
            runtime_pack,
            harness_pack,
            ProjectEnvelopeSpec {
                runtime_kind: AI_AGENT_APP_RUNTIME_KIND,
                generated_root: AI_AGENT_APP_GENERATED_ROOT,
                entrypoint: AI_AGENT_APP_ENTRYPOINT,
                bind: AI_AGENT_APP_BIND,
                network: AI_AGENT_APP_NETWORK,
                allowed_files: &AI_AGENT_APP_ALLOWED_FILES,
                output_format: "ai-agent-app-project",
                harness_policy: merged_ai_agent_app_policy(harness_pack),
                acceptance_criteria: vec![
                    "Generated output contains exactly the AI Agent App project file set.".to_string(),
                    "Generated provider settings UI stores only provider requirements and binding status.".to_string(),
                    "Generated app calls only the Sofvary AI Gateway on 127.0.0.1 and never calls provider APIs directly.".to_string(),
                    "Generated source, logs, and capsule metadata do not include API keys, secure key refs, local provider ids, or Coding Agent Gateway access.".to_string(),
                    "Generated app can create text, image, video, or multimodal jobs through local gateway request envelopes.".to_string(),
                    format!(
                        "Harness {}@{} constraints are represented in the prompt envelope.",
                        harness_pack.id, harness_pack.version
                    ),
                ],
            },
        )
    }

    pub fn create_canvas2d_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        validate_canvas2d_runtime(runtime_pack)?;
        validate_runtime_harness(runtime_pack, harness_pack)?;

        let existing_files = existing_canvas2d_files(manifest)?;
        let canvas_root = manifest.paths.generated.join("canvas");
        let current_app_state = build_current_app_state(
            manifest,
            existing_files,
            &canvas_root,
            &CANVAS2D_ALLOWED_FILES,
        )?;
        let mut harness_policy = merged_canvas2d_policy(harness_pack);
        let mut acceptance_criteria = vec![
            "Generated output contains exactly the Canvas 2D project file set.".to_string(),
            "Generated app uses the Canvas 2D API and requestAnimationFrame.".to_string(),
            "Generated app does not use React, external CDN, remote assets, npm packages, or Sofvary shell UI.".to_string(),
            "Update, render, input, and state are split across declared engine and game files.".to_string(),
            "Level data is configurable and pause/restart behavior is included where reasonable.".to_string(),
            format!(
                "Harness {}@{} constraints are represented in the prompt envelope.",
                harness_pack.id, harness_pack.version
            ),
        ];
        append_software_naming_criteria(&mut acceptance_criteria, &manifest.name);
        apply_update_context_policy(
            &current_app_state,
            &mut harness_policy,
            &mut acceptance_criteria,
        );

        Ok(PromptEnvelope {
            schema_version: PROMPT_ENVELOPE_SCHEMA_VERSION.to_string(),
            envelope_id: format!("penv_{}", Uuid::new_v4().simple()),
            created_at: Utc::now().to_rfc3339(),
            box_runtime_context: BoxRuntimeContext {
                runtime_pack: PackReference {
                    id: runtime_pack.id.clone(),
                    version: runtime_pack.version.clone(),
                },
                harness_packs: vec![PackReference {
                    id: harness_pack.id.clone(),
                    version: harness_pack.version.clone(),
                }],
                runtime_kind: CANVAS2D_RUNTIME_KIND.to_string(),
                generated_root: CANVAS2D_GENERATED_ROOT.to_string(),
                entrypoint: CANVAS2D_ENTRYPOINT.to_string(),
                bind: CANVAS2D_BIND.to_string(),
                network: CANVAS2D_NETWORK.to_string(),
            },
            user_intent: user_intent.trim().to_string(),
            current_app_state,
            runtime_policy: RuntimePolicy {
                runtime_kind: CANVAS2D_RUNTIME_KIND.to_string(),
                allowed_entrypoints: vec![CANVAS2D_ENTRYPOINT.to_string()],
                allowed_server_bind: CANVAS2D_BIND.to_string(),
                network: CANVAS2D_NETWORK.to_string(),
                package_install: false,
            },
            harness_policy,
            file_system_policy: FileSystemPolicy {
                root: CANVAS2D_GENERATED_ROOT.to_string(),
                allowed_files: allowed_canvas2d_files(),
                allow_external_files: false,
                allow_path_traversal: false,
            },
            command_policy: CommandPolicy {
                allow_shell: false,
                allow_package_install: false,
                allow_global_install: false,
                allowed_commands: Vec::new(),
            },
            output_contract: OutputContract {
                format: "canvas2d-project".to_string(),
                files: allowed_canvas2d_files(),
                shell_ui_included: false,
            },
            acceptance_criteria,
        })
    }

    pub fn create_markdown_knowledge_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        create_generated_project_envelope(
            user_intent,
            manifest,
            runtime_pack,
            harness_pack,
            ProjectEnvelopeSpec {
                runtime_kind: MARKDOWN_KNOWLEDGE_RUNTIME_KIND,
                generated_root: MARKDOWN_KNOWLEDGE_GENERATED_ROOT,
                entrypoint: MARKDOWN_KNOWLEDGE_ENTRYPOINT,
                bind: MARKDOWN_KNOWLEDGE_BIND,
                network: MARKDOWN_KNOWLEDGE_NETWORK,
                allowed_files: &MARKDOWN_KNOWLEDGE_ALLOWED_FILES,
                output_format: "markdown-knowledge-project",
                harness_policy: merged_markdown_knowledge_policy(harness_pack),
                acceptance_criteria: vec![
                    "Generated output contains exactly the Markdown Knowledge project file set.".to_string(),
                    "Markdown content and index stay inside generated/markdown.".to_string(),
                    "Generated app provides local categories, tags, search, preview, and editing state.".to_string(),
                    "Generated app does not access arbitrary user notes, upload notes, use SQLite, call cloud services, or include Sofvary shell UI.".to_string(),
                    format!(
                        "Harness {}@{} constraints are represented in the prompt envelope.",
                        harness_pack.id, harness_pack.version
                    ),
                ],
            },
        )
    }

    pub fn create_data_table_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        create_generated_project_envelope(
            user_intent,
            manifest,
            runtime_pack,
            harness_pack,
            ProjectEnvelopeSpec {
                runtime_kind: DATA_TABLE_RUNTIME_KIND,
                generated_root: DATA_TABLE_GENERATED_ROOT,
                entrypoint: DATA_TABLE_ENTRYPOINT,
                bind: DATA_TABLE_BIND,
                network: DATA_TABLE_NETWORK,
                allowed_files: &DATA_TABLE_ALLOWED_FILES,
                output_format: "data-table-project",
                harness_policy: merged_data_table_policy(harness_pack),
                acceptance_criteria: vec![
                    "Generated output contains exactly the Data Table project file set.".to_string(),
                    "Table data and schema stay inside generated/data.".to_string(),
                    "Generated app supports add, edit, delete, search, filter, and sort for the personal inventory table.".to_string(),
                    "CSV import remains a safe placeholder requiring a user-selected file and no arbitrary path access.".to_string(),
                    "Generated app does not upload data, use cloud services, or include Sofvary shell UI.".to_string(),
                    format!(
                        "Harness {}@{} constraints are represented in the prompt envelope.",
                        harness_pack.id, harness_pack.version
                    ),
                ],
            },
        )
    }

    pub fn create_file_processor_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        create_generated_project_envelope(
            user_intent,
            manifest,
            runtime_pack,
            harness_pack,
            ProjectEnvelopeSpec {
                runtime_kind: FILE_PROCESSOR_RUNTIME_KIND,
                generated_root: FILE_PROCESSOR_GENERATED_ROOT,
                entrypoint: FILE_PROCESSOR_ENTRYPOINT,
                bind: FILE_PROCESSOR_BIND,
                network: FILE_PROCESSOR_NETWORK,
                allowed_files: &FILE_PROCESSOR_ALLOWED_FILES,
                output_format: "file-processor-project",
                harness_policy: merged_file_processor_policy(harness_pack),
                acceptance_criteria: vec![
                    "Generated output contains exactly the File Processor project file set.".to_string(),
                    "Generated app starts read-only and requires explicit user-selected files or folders.".to_string(),
                    "Generated app shows a dry-run preview before any confirmed operation log plan.".to_string(),
                    "Runtime records operation log entries without mutating files in Phase 14 MVP.".to_string(),
                    "Generated app does not access unselected paths, mutate files, delete files, or include Sofvary shell UI.".to_string(),
                    format!(
                        "Harness {}@{} constraints are represented in the prompt envelope.",
                        harness_pack.id, harness_pack.version
                    ),
                ],
            },
        )
    }

    pub fn create_desktop_widget_envelope(
        &self,
        user_intent: &str,
        manifest: &AppBoxManifest,
        runtime_pack: &RuntimePackManifest,
        harness_pack: &HarnessPackManifest,
    ) -> Result<PromptEnvelope, HarnessEngineError> {
        create_generated_project_envelope(
            user_intent,
            manifest,
            runtime_pack,
            harness_pack,
            ProjectEnvelopeSpec {
                runtime_kind: DESKTOP_WIDGET_RUNTIME_KIND,
                generated_root: DESKTOP_WIDGET_GENERATED_ROOT,
                entrypoint: DESKTOP_WIDGET_ENTRYPOINT,
                bind: DESKTOP_WIDGET_BIND,
                network: DESKTOP_WIDGET_NETWORK,
                allowed_files: &DESKTOP_WIDGET_ALLOWED_FILES,
                output_format: "desktop-widget-project",
                harness_policy: merged_desktop_widget_policy(harness_pack),
                acceptance_criteria: vec![
                    "Generated output contains exactly the Desktop Widget project file set.".to_string(),
                    "Generated widget runs inside the main Sofvary preview.".to_string(),
                    "Generated widget keeps a compact layout and avoids unauthorized system APIs.".to_string(),
                    "Generated widget does not create transparent windows, always-on-top windows, tray behavior, notifications, system automation, or Sofvary shell UI.".to_string(),
                    format!(
                        "Harness {}@{} constraints are represented in the prompt envelope.",
                        harness_pack.id, harness_pack.version
                    ),
                ],
            },
        )
    }
}

struct ProjectEnvelopeSpec {
    runtime_kind: &'static str,
    generated_root: &'static str,
    entrypoint: &'static str,
    bind: &'static str,
    network: &'static str,
    allowed_files: &'static [&'static str],
    output_format: &'static str,
    harness_policy: HarnessPolicy,
    acceptance_criteria: Vec<String>,
}

fn create_generated_project_envelope(
    user_intent: &str,
    manifest: &AppBoxManifest,
    runtime_pack: &RuntimePackManifest,
    harness_pack: &HarnessPackManifest,
    spec: ProjectEnvelopeSpec,
) -> Result<PromptEnvelope, HarnessEngineError> {
    validate_project_runtime(
        runtime_pack,
        spec.runtime_kind,
        spec.generated_root,
        spec.entrypoint,
        spec.bind,
        spec.network,
    )?;
    validate_runtime_harness(runtime_pack, harness_pack)?;

    let existing_files = existing_generated_files(manifest)?;
    let current_app_state = build_current_app_state(
        manifest,
        existing_files,
        &manifest.paths.generated,
        spec.allowed_files,
    )?;
    let mut harness_policy = spec.harness_policy;
    let mut acceptance_criteria = spec.acceptance_criteria;
    append_software_naming_criteria(&mut acceptance_criteria, &manifest.name);
    apply_update_context_policy(
        &current_app_state,
        &mut harness_policy,
        &mut acceptance_criteria,
    );

    Ok(PromptEnvelope {
        schema_version: PROMPT_ENVELOPE_SCHEMA_VERSION.to_string(),
        envelope_id: format!("penv_{}", Uuid::new_v4().simple()),
        created_at: Utc::now().to_rfc3339(),
        box_runtime_context: BoxRuntimeContext {
            runtime_pack: PackReference {
                id: runtime_pack.id.clone(),
                version: runtime_pack.version.clone(),
            },
            harness_packs: vec![PackReference {
                id: harness_pack.id.clone(),
                version: harness_pack.version.clone(),
            }],
            runtime_kind: spec.runtime_kind.to_string(),
            generated_root: spec.generated_root.to_string(),
            entrypoint: spec.entrypoint.to_string(),
            bind: spec.bind.to_string(),
            network: spec.network.to_string(),
        },
        user_intent: user_intent.trim().to_string(),
        current_app_state,
        runtime_policy: RuntimePolicy {
            runtime_kind: spec.runtime_kind.to_string(),
            allowed_entrypoints: vec![spec.entrypoint.to_string()],
            allowed_server_bind: spec.bind.to_string(),
            network: spec.network.to_string(),
            package_install: false,
        },
        harness_policy,
        file_system_policy: FileSystemPolicy {
            root: spec.generated_root.to_string(),
            allowed_files: allowed_project_files(spec.allowed_files),
            allow_external_files: false,
            allow_path_traversal: false,
        },
        command_policy: CommandPolicy {
            allow_shell: false,
            allow_package_install: false,
            allow_global_install: false,
            allowed_commands: Vec::new(),
        },
        output_contract: OutputContract {
            format: spec.output_format.to_string(),
            files: allowed_project_files(spec.allowed_files),
            shell_ui_included: false,
        },
        acceptance_criteria,
    })
}

fn append_software_naming_criteria(acceptance_criteria: &mut Vec<String>, fallback_name: &str) {
    append_unique(
        acceptance_criteria,
        &[
            format!(
                "Before writing UI text, choose a concise software name. Use '{fallback_name}' unless you can derive a better short domain name from the user intent."
            ),
            "Visible app title and browser title must use the concise software name, not the full user requirement.".to_string(),
            "Do not render PromptEnvelope text, runtime repair fallback text, diagnostic text, or Sofvary shell copy inside the generated app UI.".to_string(),
        ],
    );
}

fn build_current_app_state(
    manifest: &AppBoxManifest,
    existing_files: Vec<String>,
    context_root: &std::path::Path,
    allowed_files: &[&str],
) -> Result<CurrentAppState, std::io::Error> {
    let file_context = if existing_files.is_empty() {
        Vec::new()
    } else {
        read_existing_file_context(context_root, &existing_files, allowed_files)?
    };

    Ok(CurrentAppState {
        app_id: manifest.app_id.clone(),
        workspace_name: manifest.name.clone(),
        mode: if existing_files.is_empty() {
            "create".to_string()
        } else {
            "update".to_string()
        },
        existing_files,
        file_context,
        preview_state: manifest.preview.state.clone(),
    })
}

fn read_existing_file_context(
    root: &std::path::Path,
    existing_files: &[String],
    allowed_files: &[&str],
) -> Result<Vec<CurrentAppFileContext>, std::io::Error> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut used_bytes = 0_usize;
    let mut context = Vec::new();

    for relative_path in existing_files {
        if used_bytes >= MAX_CONTEXT_TOTAL_BYTES {
            break;
        }
        if !allowed_files
            .iter()
            .any(|allowed_file| *allowed_file == relative_path)
        {
            continue;
        }
        if !is_safe_relative_context_path(relative_path) {
            continue;
        }

        let path = root.join(relative_path);
        if !path.is_file() {
            continue;
        }

        let bytes = fs::read(&path)?;
        let remaining_bytes = MAX_CONTEXT_TOTAL_BYTES.saturating_sub(used_bytes);
        let take_bytes = bytes.len().min(MAX_CONTEXT_FILE_BYTES).min(remaining_bytes);
        if take_bytes == 0 {
            break;
        }

        let truncated = bytes.len() > take_bytes;
        let contents = String::from_utf8_lossy(&bytes[..take_bytes]).to_string();
        used_bytes += take_bytes;
        context.push(CurrentAppFileContext {
            relative_path: relative_path.clone(),
            contents,
            byte_size: bytes.len(),
            truncated,
        });
    }

    Ok(context)
}

fn is_safe_relative_context_path(relative_path: &str) -> bool {
    let path = std::path::Path::new(relative_path);
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

fn apply_update_context_policy(
    current_app_state: &CurrentAppState,
    harness_policy: &mut HarnessPolicy,
    acceptance_criteria: &mut Vec<String>,
) {
    if current_app_state.mode != "update" {
        return;
    }

    append_unique(
        &mut harness_policy.system_instructions,
        &[
            "This is an update to an existing Sofvary app. Use currentAppState.fileContext as the existing implementation context before changing files.".to_string(),
            "Do not rebuild from zero, switch Runtime, change the output contract, or discard existing behavior unless userIntent explicitly asks for it.".to_string(),
        ],
    );
    append_unique(
        &mut harness_policy.output_rules,
        &["For update mode, return the complete allowed file set after applying the requested change so the workspace remains runnable.".to_string()],
    );
    append_unique(
        acceptance_criteria,
        &["Update mode preserves existing app behavior and applies the new request on top of currentAppState.fileContext.".to_string()],
    );
}

pub fn summarize_prompt_envelope(envelope: &PromptEnvelope) -> PromptEnvelopeSummary {
    PromptEnvelopeSummary {
        runtime: format!(
            "{}@{}",
            envelope.box_runtime_context.runtime_pack.id,
            envelope.box_runtime_context.runtime_pack.version
        ),
        harnesses: envelope
            .box_runtime_context
            .harness_packs
            .iter()
            .map(|pack| format!("{}@{}", pack.id, pack.version))
            .collect(),
        allowed_files: envelope.file_system_policy.allowed_files.clone(),
        blocked_capabilities: envelope.harness_policy.blocked_capabilities.clone(),
        output_contract: envelope.output_contract.files.clone(),
        acceptance_criteria_count: envelope.acceptance_criteria.len(),
    }
}

fn validate_static_html_runtime(
    runtime_pack: &RuntimePackManifest,
) -> Result<(), HarnessEngineError> {
    let runtime = &runtime_pack.runtime;
    if runtime.kind != STATIC_HTML_RUNTIME_KIND {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.kind must be {STATIC_HTML_RUNTIME_KIND}, found {}",
            runtime.kind
        )));
    }
    if runtime.generated_root != STATIC_HTML_GENERATED_ROOT {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.generatedRoot must be {STATIC_HTML_GENERATED_ROOT}, found {}",
            runtime.generated_root
        )));
    }
    if runtime.entrypoint != STATIC_HTML_ENTRYPOINT {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.entrypoint must be {STATIC_HTML_ENTRYPOINT}, found {}",
            runtime.entrypoint
        )));
    }
    if runtime.bind != STATIC_HTML_BIND {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.bind must be {STATIC_HTML_BIND}, found {}",
            runtime.bind
        )));
    }
    if runtime.network != STATIC_HTML_NETWORK {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.network must be {STATIC_HTML_NETWORK}, found {}",
            runtime.network
        )));
    }

    Ok(())
}

fn validate_static_html_harness(
    runtime_pack: &RuntimePackManifest,
    harness_pack: &HarnessPackManifest,
) -> Result<(), HarnessEngineError> {
    validate_runtime_harness(runtime_pack, harness_pack)
}

fn validate_react_vite_runtime(
    runtime_pack: &RuntimePackManifest,
) -> Result<(), HarnessEngineError> {
    let runtime = &runtime_pack.runtime;
    if runtime.kind != REACT_VITE_RUNTIME_KIND {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.kind must be {REACT_VITE_RUNTIME_KIND}, found {}",
            runtime.kind
        )));
    }
    if runtime.generated_root != REACT_VITE_GENERATED_ROOT {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.generatedRoot must be {REACT_VITE_GENERATED_ROOT}, found {}",
            runtime.generated_root
        )));
    }
    if runtime.entrypoint != REACT_VITE_ENTRYPOINT {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.entrypoint must be {REACT_VITE_ENTRYPOINT}, found {}",
            runtime.entrypoint
        )));
    }
    if runtime.bind != REACT_VITE_BIND {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.bind must be {REACT_VITE_BIND}, found {}",
            runtime.bind
        )));
    }
    if runtime.network != REACT_VITE_NETWORK {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.network must be {REACT_VITE_NETWORK}, found {}",
            runtime.network
        )));
    }

    Ok(())
}

fn validate_react_sqlite_runtime(
    runtime_pack: &RuntimePackManifest,
) -> Result<(), HarnessEngineError> {
    let runtime = &runtime_pack.runtime;
    if runtime.kind != REACT_SQLITE_RUNTIME_KIND {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.kind must be {REACT_SQLITE_RUNTIME_KIND}, found {}",
            runtime.kind
        )));
    }
    if runtime.generated_root != REACT_SQLITE_GENERATED_ROOT {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.generatedRoot must be {REACT_SQLITE_GENERATED_ROOT}, found {}",
            runtime.generated_root
        )));
    }
    if runtime.entrypoint != REACT_SQLITE_ENTRYPOINT {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.entrypoint must be {REACT_SQLITE_ENTRYPOINT}, found {}",
            runtime.entrypoint
        )));
    }
    if runtime.bind != REACT_SQLITE_BIND {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.bind must be {REACT_SQLITE_BIND}, found {}",
            runtime.bind
        )));
    }
    if runtime.network != REACT_SQLITE_NETWORK {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.network must be {REACT_SQLITE_NETWORK}, found {}",
            runtime.network
        )));
    }

    Ok(())
}

fn validate_canvas2d_runtime(runtime_pack: &RuntimePackManifest) -> Result<(), HarnessEngineError> {
    let runtime = &runtime_pack.runtime;
    if runtime.kind != CANVAS2D_RUNTIME_KIND {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.kind must be {CANVAS2D_RUNTIME_KIND}, found {}",
            runtime.kind
        )));
    }
    if runtime.generated_root != CANVAS2D_GENERATED_ROOT {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.generatedRoot must be {CANVAS2D_GENERATED_ROOT}, found {}",
            runtime.generated_root
        )));
    }
    if runtime.entrypoint != CANVAS2D_ENTRYPOINT {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.entrypoint must be {CANVAS2D_ENTRYPOINT}, found {}",
            runtime.entrypoint
        )));
    }
    if runtime.bind != CANVAS2D_BIND {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.bind must be {CANVAS2D_BIND}, found {}",
            runtime.bind
        )));
    }
    if runtime.network != CANVAS2D_NETWORK {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.network must be {CANVAS2D_NETWORK}, found {}",
            runtime.network
        )));
    }

    Ok(())
}

fn validate_project_runtime(
    runtime_pack: &RuntimePackManifest,
    expected_kind: &str,
    expected_generated_root: &str,
    expected_entrypoint: &str,
    expected_bind: &str,
    expected_network: &str,
) -> Result<(), HarnessEngineError> {
    let runtime = &runtime_pack.runtime;
    if runtime.kind != expected_kind {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.kind must be {expected_kind}, found {}",
            runtime.kind
        )));
    }
    if runtime.generated_root != expected_generated_root {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.generatedRoot must be {expected_generated_root}, found {}",
            runtime.generated_root
        )));
    }
    if runtime.entrypoint != expected_entrypoint {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.entrypoint must be {expected_entrypoint}, found {}",
            runtime.entrypoint
        )));
    }
    if runtime.bind != expected_bind {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.bind must be {expected_bind}, found {}",
            runtime.bind
        )));
    }
    if runtime.network != expected_network {
        return Err(HarnessEngineError::IncompatibleRuntime(format!(
            "runtime.network must be {expected_network}, found {}",
            runtime.network
        )));
    }

    Ok(())
}

fn validate_runtime_harness(
    runtime_pack: &RuntimePackManifest,
    harness_pack: &HarnessPackManifest,
) -> Result<(), HarnessEngineError> {
    if harness_pack.runtime.as_deref() != Some(runtime_pack.id.as_str()) {
        return Err(HarnessEngineError::IncompatibleHarness {
            runtime_id: runtime_pack.id.clone(),
            harness_id: harness_pack.id.clone(),
        });
    }

    Ok(())
}

fn merged_static_html_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a self-contained static app using index.html, style.css, and app.js.".to_string(),
        "Do not require npm, CDNs, remote scripts, external assets, React, SQLite, or plugin execution.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write only inside generated/static for this runtime.".to_string(),
        "Do not read or write paths outside the active Sofvary workspace.".to_string(),
        "Do not create files other than index.html, style.css, and app.js.".to_string(),
        "Use localStorage only for small local preferences.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "Keep Sofvary shell UI out of generated app source.".to_string(),
        "The app must run through the local static preview server.".to_string(),
        "The generated app must be exportable without Sofvary floating menu or build overlay UI."
            .to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "cdn-assets".to_string(),
            "npm-package-install".to_string(),
            "plugin-execution".to_string(),
            "react-runtime".to_string(),
            "remote-network".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
            "sqlite-runtime".to_string(),
        ],
    }
}

fn merged_react_vite_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a React + Vite app using React function components and TypeScript.".to_string(),
        "Do not use Next.js, Electron, external CDNs, remote assets, SQLite, plugin execution, or a default UI framework.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write only inside generated/react for this runtime.".to_string(),
        "Do not read or write paths outside the active Sofvary workspace.".to_string(),
        "Components go under src/components.".to_string(),
        "Styles go under src/styles.".to_string(),
        "Do not create files outside the React + Vite output contract.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "App.tsx is the main app entry component.".to_string(),
        "The generated app must pass npm run build.".to_string(),
        "Keep Sofvary shell UI out of generated app source.".to_string(),
        "The generated app must be exportable without Sofvary floating menu or build overlay UI."
            .to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "cdn-assets".to_string(),
            "electron-runtime".to_string(),
            "external-ui-framework".to_string(),
            "nextjs-runtime".to_string(),
            "plugin-execution".to_string(),
            "remote-network".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
            "sqlite-runtime".to_string(),
        ],
    }
}

fn merged_react_sqlite_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a React + Vite frontend, Node local API, and workspace-local SQLite persistence.".to_string(),
        "Frontend code must call /api/* endpoints only and must never import SQLite, sql.js, database files, or filesystem modules.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write React and Node API files only inside generated/react.".to_string(),
        "Write schema, migrations, seed data, and app.sqlite only inside generated/data."
            .to_string(),
        "Do not read or write paths outside the active Sofvary workspace.".to_string(),
        "Do not create files outside the React + SQLite output contract.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "The frontend calls /api/health and /api/customers through relative /api/* URLs."
            .to_string(),
        "The Node API binds to 127.0.0.1 only and owns every SQLite operation.".to_string(),
        "Use the managed sql.js dependency or the Sofvary better-sqlite3 compatibility API only; do not add native SQLite packages or package install scripts.".to_string(),
        "SQL statements use prepared statements for user-controlled values.".to_string(),
        "The SQLite database file stays at generated/data/app.sqlite.".to_string(),
        "Keep Sofvary shell UI out of generated app source.".to_string(),
        "The generated app must be exportable without Sofvary floating menu or build overlay UI."
            .to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "cdn-assets".to_string(),
            "cloud-service".to_string(),
            "direct-frontend-sqlite-access".to_string(),
            "electron-runtime".to_string(),
            "external-ui-framework".to_string(),
            "nextjs-runtime".to_string(),
            "plugin-execution".to_string(),
            "remote-database".to_string(),
            "remote-network".to_string(),
            "sensitive-credentials".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
        ],
    }
}

fn merged_canvas2d_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a self-contained Canvas 2D app using browser Canvas 2D APIs and requestAnimationFrame.".to_string(),
        "Do not use React, CDNs, remote assets, npm packages, external dependencies, SQLite, or plugin execution.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write only inside generated/canvas for this runtime.".to_string(),
        "Do not read or write paths outside the active Sofvary workspace.".to_string(),
        "Split update, render, input, and state across the declared engine and game files."
            .to_string(),
        "Do not create files outside the Canvas 2D output contract.".to_string(),
        "Use assets/ only for workspace-local assets.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "Use requestAnimationFrame as the main loop driver.".to_string(),
        "Use CanvasRenderingContext2D for rendering.".to_string(),
        "Level data should be configurable.".to_string(),
        "Include pause and restart controls where reasonable.".to_string(),
        "Keep Sofvary shell UI out of generated app source.".to_string(),
        "The generated app must be exportable without Sofvary floating menu or build overlay UI."
            .to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "cdn-assets".to_string(),
            "external-assets".to_string(),
            "npm-package-install".to_string(),
            "plugin-execution".to_string(),
            "react-runtime".to_string(),
            "remote-network".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
            "sqlite-runtime".to_string(),
        ],
    }
}

fn merged_markdown_knowledge_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a React + Vite Markdown knowledge app with local notes, categories, tags, search, preview, and editing.".to_string(),
        "Do not access arbitrary user notes, upload notes, use SQLite, call cloud services, or execute plugins.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write Markdown content and index files only inside generated/markdown.".to_string(),
        "Write React UI files only inside generated/react.".to_string(),
        "Do not read or write paths outside the active Sofvary workspace.".to_string(),
        "Do not create files outside the Markdown Knowledge output contract.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "Start with Markdown preview and editing.".to_string(),
        "Search must use generated local content only.".to_string(),
        "Keep Sofvary shell UI out of generated app source.".to_string(),
        "The generated app must be exportable without Sofvary floating menu or build overlay UI."
            .to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "arbitrary-user-notes".to_string(),
            "cloud-service".to_string(),
            "external-ui-framework".to_string(),
            "note-upload".to_string(),
            "plugin-execution".to_string(),
            "remote-network".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
            "sqlite-runtime".to_string(),
        ],
    }
}

fn merged_data_table_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a React + Vite personal data table app using workspace-local JSON data.".to_string(),
        "Do not upload table data, use remote databases, access arbitrary CSV files, or execute plugins.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write table schema and data only inside generated/data.".to_string(),
        "Write React UI files only inside generated/react.".to_string(),
        "CSV import must require an explicit user-selected file.".to_string(),
        "Do not create files outside the Data Table output contract.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "Support add, edit, delete, search, filter, and sort for table rows.".to_string(),
        "CSV import remains a safe placeholder until a user-selected file is available."
            .to_string(),
        "Keep Sofvary shell UI out of generated app source.".to_string(),
        "The generated app must be exportable without Sofvary floating menu or build overlay UI."
            .to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "arbitrary-csv-access".to_string(),
            "cloud-service".to_string(),
            "data-upload".to_string(),
            "plugin-execution".to_string(),
            "remote-database".to_string(),
            "remote-network".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
            "sqlite-runtime".to_string(),
        ],
    }
}

fn merged_file_processor_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a React file processor app that starts read-only and requires explicit file or folder selection.".to_string(),
        "Phase 14 must not modify files; confirmation records the dry-run plan only.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write React UI files only inside generated/react.".to_string(),
        "Write runtime-local policy metadata only inside generated/file-processor.".to_string(),
        "Never access paths that were not selected by the user.".to_string(),
        "Do not create files outside the File Processor output contract.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "Show a dry-run preview before any write-like operation.".to_string(),
        "Confirmation records the plan in the operation log only.".to_string(),
        "Do not rename, delete, move, or rewrite files in Phase 14.".to_string(),
        "Log planned operations without executing them.".to_string(),
        "Keep Sofvary shell UI out of generated app source.".to_string(),
        "The generated app must be exportable without Sofvary floating menu or build overlay UI."
            .to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "arbitrary-file-access".to_string(),
            "file-mutation".to_string(),
            "permanent-delete".to_string(),
            "global-install".to_string(),
            "path-traversal".to_string(),
            "plugin-execution".to_string(),
            "remote-network".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
            "write-without-dry-run".to_string(),
        ],
    }
}

fn merged_desktop_widget_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a compact React desktop widget app that runs inside the main Sofvary preview.".to_string(),
        "Do not create transparent windows, always-on-top windows, tray behavior, notifications, or system automation.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write React UI files only inside generated/react.".to_string(),
        "Write widget metadata only inside generated/widget.".to_string(),
        "Do not read or write paths outside the active Sofvary workspace.".to_string(),
        "Do not create files outside the Desktop Widget output contract.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "Keep the widget layout compact.".to_string(),
        "Run inside the main Sofvary preview.".to_string(),
        "Avoid unauthorized system APIs.".to_string(),
        "Keep Sofvary shell UI out of generated app source.".to_string(),
        "The generated app must be exportable without Sofvary floating menu or build overlay UI."
            .to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "always-on-top-window".to_string(),
            "notification-plugin".to_string(),
            "plugin-execution".to_string(),
            "remote-network".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
            "system-automation".to_string(),
            "transparent-window".to_string(),
            "tray-integration".to_string(),
        ],
    }
}

fn merged_ai_agent_app_policy(harness_pack: &HarnessPackManifest) -> HarnessPolicy {
    let mut system_instructions = vec![
        "Generate a React + Vite AI Agent App with provider binding UI, job creation, job status, and artifact gallery.".to_string(),
        "The generated app may request text, image, and video capabilities only through the Sofvary AI Gateway on 127.0.0.1.".to_string(),
        "Never call OpenAI, OpenAI-compatible, Ollama, or any provider API directly from generated app source.".to_string(),
        "Never write API keys, secure key refs, author-local provider ids, or Coding Agent Gateway access into generated files.".to_string(),
    ];
    append_unique(&mut system_instructions, &harness_pack.instructions.system);

    let mut file_system_rules = vec![
        "Write React UI files only inside generated/react.".to_string(),
        "Write provider requirement and seed metadata only inside generated/ai.".to_string(),
        "Do not read or write paths outside the active Sofvary workspace.".to_string(),
        "Do not create files outside the AI Agent App output contract.".to_string(),
    ];
    append_unique(
        &mut file_system_rules,
        &harness_pack.instructions.file_system_policy,
    );

    let mut output_rules = vec![
        "Provider settings must bind provider requirements to a shell-owned local provider profile without storing secrets.".to_string(),
        "Generated app requests must target /__sofvary/ai/* or relative gateway proxy paths only.".to_string(),
        "Capsule metadata must describe provider requirements only and must not include secrets, secure key refs, or local provider ids.".to_string(),
        "Imported AI Agent Apps must be able to render a needs-provider-binding state.".to_string(),
        "Keep Sofvary shell UI, floating menu, build overlay, and Coding Agent Gateway out of generated app source.".to_string(),
    ];
    append_unique(&mut output_rules, &harness_pack.instructions.output_rules);

    HarnessPolicy {
        system_instructions,
        file_system_rules,
        output_rules,
        blocked_capabilities: vec![
            "arbitrary-network".to_string(),
            "coding-agent-gateway-access".to_string(),
            "direct-provider-network".to_string(),
            "local-provider-id-export".to_string(),
            "plaintext-api-key".to_string(),
            "plugin-execution".to_string(),
            "remote-network".to_string(),
            "secure-key-ref-export".to_string(),
            "secret-export".to_string(),
            "shell-command".to_string(),
            "sofvary-shell-ui".to_string(),
        ],
    }
}

fn existing_static_files(manifest: &AppBoxManifest) -> Result<Vec<String>, std::io::Error> {
    if !manifest.paths.generated_static.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&manifest.paths.generated_static)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    files.sort();
    Ok(files)
}

fn existing_react_files(manifest: &AppBoxManifest) -> Result<Vec<String>, std::io::Error> {
    let react_root = manifest.paths.generated.join("react");
    if !react_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_relative_files(&react_root, &react_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn existing_generated_files(manifest: &AppBoxManifest) -> Result<Vec<String>, std::io::Error> {
    if !manifest.paths.generated.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_relative_files(
        &manifest.paths.generated,
        &manifest.paths.generated,
        &mut files,
    )?;
    files.sort();
    Ok(files)
}

fn existing_canvas2d_files(manifest: &AppBoxManifest) -> Result<Vec<String>, std::io::Error> {
    let canvas_root = manifest.paths.generated.join("canvas");
    if !canvas_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_relative_files(&canvas_root, &canvas_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_relative_files(
    root: &std::path::Path,
    current: &std::path::Path,
    files: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_relative_files(root, &path, files)?;
        } else if entry.file_type()?.is_file() {
            if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

fn allowed_static_files() -> Vec<String> {
    STATIC_HTML_ALLOWED_FILES
        .iter()
        .map(|value| value.to_string())
        .collect()
}

fn allowed_react_vite_files() -> Vec<String> {
    REACT_VITE_ALLOWED_FILES
        .iter()
        .map(|value| value.to_string())
        .collect()
}

fn allowed_react_sqlite_files() -> Vec<String> {
    REACT_SQLITE_ALLOWED_FILES
        .iter()
        .map(|value| value.to_string())
        .collect()
}

fn allowed_canvas2d_files() -> Vec<String> {
    CANVAS2D_ALLOWED_FILES
        .iter()
        .map(|value| value.to_string())
        .collect()
}

fn allowed_project_files(files: &[&str]) -> Vec<String> {
    files.iter().map(|value| value.to_string()).collect()
}

fn append_unique(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        if !value.trim().is_empty() && !target.contains(value) {
            target.push(value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pack_manager::{parse_harness_pack_manifest, parse_runtime_pack_manifest};
    use crate::core::workspace_types::{
        WorkspaceConstraints, WorkspaceMode, WorkspacePaths, WorkspacePreview,
    };

    const RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.static-html/0.1.0/manifest.json"
    );
    const HARNESS_MANIFEST: &str =
        include_str!("../../builtin-packs/harness/sofvary.harness.static-html/0.1.0/manifest.json");
    const REACT_RUNTIME_MANIFEST: &str =
        include_str!("../../builtin-packs/runtimes/sofvary.runtime.react-vite/0.1.0/manifest.json");
    const REACT_HARNESS_MANIFEST: &str =
        include_str!("../../builtin-packs/harness/sofvary.harness.react-vite/0.1.0/manifest.json");
    const REACT_SQLITE_RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.react-sqlite/0.1.0/manifest.json"
    );
    const REACT_SQLITE_HARNESS_MANIFEST: &str = include_str!(
        "../../builtin-packs/harness/sofvary.harness.react-sqlite/0.1.0/manifest.json"
    );
    const CANVAS2D_RUNTIME_MANIFEST: &str =
        include_str!("../../builtin-packs/runtimes/sofvary.runtime.canvas2d/0.1.0/manifest.json");
    const CANVAS2D_HARNESS_MANIFEST: &str =
        include_str!("../../builtin-packs/harness/sofvary.harness.canvas2d/0.1.0/manifest.json");
    const AI_AGENT_APP_RUNTIME_MANIFEST: &str = include_str!(
        "../../builtin-packs/runtimes/sofvary.runtime.ai-agent-app/0.1.0/manifest.json"
    );
    const AI_AGENT_APP_HARNESS_MANIFEST: &str = include_str!(
        "../../builtin-packs/harness/sofvary.harness.multimodal-studio-agent/0.1.0/manifest.json"
    );

    #[test]
    fn creates_prompt_envelope_from_static_html_packs_and_workspace_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = parse_runtime_pack_manifest(RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(HARNESS_MANIFEST).expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_static_html_envelope("Build a timer", &manifest, &runtime, &harness)
            .expect("envelope");

        assert_eq!(envelope.schema_version, "1.0");
        assert_eq!(envelope.user_intent, "Build a timer");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.static-html"
        );
        assert_eq!(envelope.box_runtime_context.runtime_pack.version, "0.1.0");
        assert_eq!(
            envelope.box_runtime_context.harness_packs,
            vec![PackReference {
                id: "sofvary.harness.static-html".to_string(),
                version: "0.1.0".to_string(),
            }]
        );
        assert_eq!(envelope.current_app_state.mode, "create");
    }

    #[test]
    fn update_envelope_includes_existing_allowed_file_context() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        fs::write(
            manifest.paths.generated_static.join("index.html"),
            "<main>Original timer</main>",
        )
        .expect("write index");
        fs::write(
            manifest.paths.generated_static.join("notes.txt"),
            "not part of the output contract",
        )
        .expect("write notes");
        let runtime = parse_runtime_pack_manifest(RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(HARNESS_MANIFEST).expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_static_html_envelope("Add a reset button", &manifest, &runtime, &harness)
            .expect("envelope");

        assert_eq!(envelope.current_app_state.mode, "update");
        assert!(envelope
            .current_app_state
            .existing_files
            .contains(&"index.html".to_string()));
        assert!(envelope
            .current_app_state
            .existing_files
            .contains(&"notes.txt".to_string()));
        assert_eq!(envelope.current_app_state.file_context.len(), 1);
        assert_eq!(
            envelope.current_app_state.file_context[0].relative_path,
            "index.html"
        );
        assert!(envelope.current_app_state.file_context[0]
            .contents
            .contains("Original timer"));
        assert!(envelope
            .harness_policy
            .system_instructions
            .iter()
            .any(|rule| { rule.contains("update to an existing Sofvary app") }));
        assert!(envelope
            .acceptance_criteria
            .iter()
            .any(|criterion| { criterion.contains("currentAppState.fileContext") }));
    }

    #[test]
    fn includes_static_html_constraints_in_envelope_summary() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = parse_runtime_pack_manifest(RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(HARNESS_MANIFEST).expect("harness");
        let engine = HarnessEngine::new();
        let envelope = engine
            .create_static_html_envelope("Build a dashboard", &manifest, &runtime, &harness)
            .expect("envelope");
        let summary = summarize_prompt_envelope(&envelope);

        assert_eq!(envelope.file_system_policy.root, "generated/static");
        assert_eq!(
            envelope.file_system_policy.allowed_files,
            allowed_static_files()
        );
        assert!(!envelope.command_policy.allow_shell);
        assert!(!envelope.command_policy.allow_package_install);
        assert!(!envelope.output_contract.shell_ui_included);
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"remote-network".to_string()));
        assert!(envelope
            .harness_policy
            .file_system_rules
            .contains(&"Use localStorage only for small local preferences.".to_string()));
        assert_eq!(
            summary.runtime,
            "sofvary.runtime.static-html@0.1.0".to_string()
        );
        assert!(!serde_json::to_string(&summary)
            .expect("summary json")
            .contains("Build a dashboard"));
    }

    #[test]
    fn prompt_envelope_accepts_typescript_golden_fixture_shape() {
        let envelope: PromptEnvelope = serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/static-html-prompt-envelope.golden.json"
        ))
        .expect("golden prompt envelope");

        assert_eq!(envelope.envelope_id, "penv_golden");
        assert_eq!(envelope.runtime_policy.runtime_kind, "static-html");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.static-html"
        );
        assert!(envelope
            .harness_policy
            .file_system_rules
            .contains(&"Use localStorage only for small local preferences.".to_string()));
    }

    #[test]
    fn creates_react_vite_prompt_envelope_from_packs_and_workspace_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = parse_runtime_pack_manifest(REACT_RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(REACT_HARNESS_MANIFEST).expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_react_vite_envelope("Build a task board", &manifest, &runtime, &harness)
            .expect("envelope");

        assert_eq!(envelope.user_intent, "Build a task board");
        assert_eq!(envelope.runtime_policy.runtime_kind, "react-vite");
        assert_eq!(envelope.file_system_policy.root, "generated/react");
        assert_eq!(envelope.output_contract.format, "react-vite-project");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.react-vite"
        );
        assert_eq!(
            envelope.box_runtime_context.harness_packs,
            vec![PackReference {
                id: "sofvary.harness.react-vite".to_string(),
                version: "0.1.0".to_string(),
            }]
        );
        assert!(envelope
            .file_system_policy
            .allowed_files
            .contains(&"src/components/TaskBoard.tsx".to_string()));
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"nextjs-runtime".to_string()));
        assert!(!envelope.command_policy.allow_shell);
        assert!(!envelope.output_contract.shell_ui_included);
    }

    #[test]
    fn react_vite_prompt_envelope_accepts_typescript_golden_fixture_shape() {
        let envelope: PromptEnvelope = serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/react-vite-prompt-envelope.golden.json"
        ))
        .expect("golden prompt envelope");

        assert_eq!(envelope.envelope_id, "penv_react_golden");
        assert_eq!(envelope.runtime_policy.runtime_kind, "react-vite");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.react-vite"
        );
        assert_eq!(envelope.output_contract.format, "react-vite-project");
    }

    #[test]
    fn creates_react_sqlite_prompt_envelope_from_packs_and_workspace_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = parse_runtime_pack_manifest(REACT_SQLITE_RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(REACT_SQLITE_HARNESS_MANIFEST).expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_react_sqlite_envelope("Build a customer manager", &manifest, &runtime, &harness)
            .expect("envelope");

        assert_eq!(envelope.user_intent, "Build a customer manager");
        assert_eq!(envelope.runtime_policy.runtime_kind, "react-sqlite");
        assert_eq!(envelope.file_system_policy.root, "generated");
        assert_eq!(envelope.output_contract.format, "react-sqlite-project");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.react-sqlite"
        );
        assert_eq!(
            envelope.box_runtime_context.harness_packs,
            vec![PackReference {
                id: "sofvary.harness.react-sqlite".to_string(),
                version: "0.1.0".to_string(),
            }]
        );
        assert!(envelope
            .file_system_policy
            .allowed_files
            .contains(&"data/migrations/001_create_customers.sql".to_string()));
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"remote-database".to_string()));
        assert!(envelope.harness_policy.output_rules.contains(
            &"SQL statements use prepared statements for user-controlled values.".to_string()
        ));
        assert!(!envelope.command_policy.allow_shell);
        assert!(!envelope.output_contract.shell_ui_included);
    }

    #[test]
    fn creates_ai_agent_app_prompt_envelope_from_packs_and_workspace_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = parse_runtime_pack_manifest(AI_AGENT_APP_RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(AI_AGENT_APP_HARNESS_MANIFEST).expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_ai_agent_app_envelope(
                "Build an agent app for articles, images, and video briefs",
                &manifest,
                &runtime,
                &harness,
            )
            .expect("envelope");

        assert_eq!(envelope.runtime_policy.runtime_kind, "ai-agent-app");
        assert_eq!(envelope.runtime_policy.allowed_server_bind, "127.0.0.1");
        assert_eq!(envelope.file_system_policy.root, "generated");
        assert_eq!(envelope.output_contract.format, "ai-agent-app-project");
        assert!(envelope
            .file_system_policy
            .allowed_files
            .contains(&"ai/provider-requirements.json".to_string()));
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"coding-agent-gateway-access".to_string()));
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"plaintext-api-key".to_string()));
        let output_rules = envelope.harness_policy.output_rules.join("\n");
        assert!(output_rules.contains("provider requirements"));
        assert!(output_rules.contains("needs-provider-binding"));
        assert!(!envelope.command_policy.allow_shell);
        assert!(!envelope.output_contract.shell_ui_included);
    }

    #[test]
    fn react_sqlite_prompt_envelope_accepts_typescript_golden_fixture_shape() {
        let envelope: PromptEnvelope = serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/react-sqlite-prompt-envelope.golden.json"
        ))
        .expect("golden prompt envelope");

        assert_eq!(envelope.envelope_id, "penv_react_sqlite_golden");
        assert_eq!(envelope.runtime_policy.runtime_kind, "react-sqlite");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.react-sqlite"
        );
        assert_eq!(envelope.output_contract.format, "react-sqlite-project");
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"direct-frontend-sqlite-access".to_string()));
    }

    #[test]
    fn creates_canvas2d_prompt_envelope_from_packs_and_workspace_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = test_manifest(temp.path());
        let runtime = parse_runtime_pack_manifest(CANVAS2D_RUNTIME_MANIFEST).expect("runtime");
        let harness = parse_harness_pack_manifest(CANVAS2D_HARNESS_MANIFEST).expect("harness");
        let engine = HarnessEngine::new();

        let envelope = engine
            .create_canvas2d_envelope("Build a coin collector", &manifest, &runtime, &harness)
            .expect("envelope");

        assert_eq!(envelope.user_intent, "Build a coin collector");
        assert_eq!(envelope.runtime_policy.runtime_kind, "canvas2d");
        assert_eq!(envelope.file_system_policy.root, "generated/canvas");
        assert_eq!(envelope.output_contract.format, "canvas2d-project");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.canvas2d"
        );
        assert_eq!(
            envelope.box_runtime_context.harness_packs,
            vec![PackReference {
                id: "sofvary.harness.canvas2d".to_string(),
                version: "0.1.0".to_string(),
            }]
        );
        assert!(envelope
            .file_system_policy
            .allowed_files
            .contains(&"src/engine/loop.js".to_string()));
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"react-runtime".to_string()));
        assert!(envelope
            .harness_policy
            .output_rules
            .contains(&"Use requestAnimationFrame as the main loop driver.".to_string()));
        assert!(!envelope.command_policy.allow_shell);
        assert!(!envelope.output_contract.shell_ui_included);
    }

    #[test]
    fn canvas2d_prompt_envelope_accepts_typescript_golden_fixture_shape() {
        let envelope: PromptEnvelope = serde_json::from_str(include_str!(
            "../../../../../packages/harness-compiler/fixtures/canvas2d-prompt-envelope.golden.json"
        ))
        .expect("golden prompt envelope");

        assert_eq!(envelope.envelope_id, "penv_canvas2d_golden");
        assert_eq!(envelope.runtime_policy.runtime_kind, "canvas2d");
        assert_eq!(
            envelope.box_runtime_context.runtime_pack.id,
            "sofvary.runtime.canvas2d"
        );
        assert_eq!(envelope.output_contract.format, "canvas2d-project");
        assert!(envelope
            .harness_policy
            .blocked_capabilities
            .contains(&"react-runtime".to_string()));
    }

    fn test_manifest(root: &std::path::Path) -> AppBoxManifest {
        let generated = root.join("generated");
        let generated_static = generated.join("static");
        fs::create_dir_all(&generated_static).expect("static root");

        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: WorkspaceMode::StaticHtml,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec![],
            paths: WorkspacePaths {
                root: root.to_path_buf(),
                generated,
                generated_static,
                runtime: root.join("runtime"),
                snapshots: root.join("snapshots"),
            },
            constraints: WorkspaceConstraints {
                boundary: root.to_path_buf(),
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: WorkspacePreview {
                state: "empty".to_string(),
                url: None,
            },
        }
    }
}
