use crate::core::policy_engine::{PolicyEngine, PolicyError};
use crate::core::policy_types::{PolicyAppReleaseRequest, PolicyApprovalSet};
use crate::core::published_app_packager::{
    create_published_app_package, NativeBundleRunner, PlatformNativeBundleRunner,
    PublishedAppNativeBundleInput, PublishedAppNativeBundleResult, PublishedAppPackageInput,
    PublishedAppPackageResult, PublishedAppPackagerError, PublishedAppStealthUiSettings,
};
use crate::core::workspace_manager::{WorkspaceError, WorkspaceManager};
use crate::core::workspace_types::{RuntimeKind, SofvaryLockfile};
use crate::platform::{current_adapter, OsKind, PlatformAdapter};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppReleaseError {
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("packager error: {0}")]
    Packager(#[from] PublishedAppPackagerError),
    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),
    #[error("invalid release request: {0}")]
    Invalid(String),
}

pub type AppReleaseResult<T> = Result<T, AppReleaseError>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppReleasePayload {
    pub app_id: String,
    pub app_name: String,
    pub target_platform: String,
    pub output_dir: PathBuf,
    #[serde(default)]
    pub icon_path: Option<PathBuf>,
    #[serde(default)]
    pub include_ai_continuation: bool,
    #[serde(default)]
    pub stealth_ui: PublishedAppStealthUiSettings,
    #[serde(default)]
    pub selected_runtime_packs: Vec<String>,
    #[serde(default)]
    pub selected_plugin_packs: Vec<String>,
    #[serde(default)]
    pub policy_approvals: PolicyApprovalSet,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAppReleaseJobPayload {
    pub job_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAppReleaseOutputFolderPayload {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppReleaseCapability {
    pub current_platform: String,
    pub beta: bool,
    pub target_platforms: Vec<AppReleasePlatformCapability>,
    pub runtimes: Vec<AppReleaseRuntimeCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppReleasePlatformCapability {
    pub platform: String,
    pub label: String,
    pub enabled: bool,
    pub current: bool,
    pub reason: Option<String>,
    pub output_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppReleaseRuntimeCapability {
    pub runtime_kind: RuntimeKind,
    pub label: String,
    pub supported: bool,
    pub release_strategy: String,
    pub ai_continuation_supported: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AppReleaseJobStatus {
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppReleaseJob {
    pub job_id: String,
    pub status: AppReleaseJobStatus,
    pub app_id: String,
    pub app_name: String,
    pub target_platform: String,
    pub output_dir: PathBuf,
    pub staging_dir: Option<PathBuf>,
    pub artifact_path: Option<PathBuf>,
    pub native_app_path: Option<PathBuf>,
    pub native_installer_path: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub detail: String,
}

pub fn get_app_release_capabilities() -> AppReleaseCapability {
    let adapter = current_adapter();
    get_app_release_capabilities_with_adapter(adapter.as_ref())
}

pub fn get_app_release_capabilities_with_adapter(
    adapter: &dyn PlatformAdapter,
) -> AppReleaseCapability {
    let current_platform = target_platform_for_os(adapter.os()).to_string();
    AppReleaseCapability {
        current_platform: current_platform.clone(),
        beta: true,
        target_platforms: ["windows", "macos", "linux"]
            .into_iter()
            .map(|platform| {
                let current = platform == current_platform;
                AppReleasePlatformCapability {
                    platform: platform.to_string(),
                    label: platform_label(platform).to_string(),
                    enabled: current,
                    current,
                    reason: if current {
                        Some("Unsigned beta packaging is available on this machine.".to_string())
                    } else {
                        Some("本机发布仅支持当前 OS".to_string())
                    },
                    output_kind: output_kind_for_platform(platform).to_string(),
                }
            })
            .collect(),
        runtimes: runtime_capabilities(),
    }
}

pub fn start_app_release_job(
    manager: &WorkspaceManager,
    payload: AppReleasePayload,
) -> AppReleaseResult<AppReleaseJob> {
    let adapter = current_adapter();
    start_app_release_job_with_adapter(manager, payload, adapter.as_ref())
}

pub fn start_app_release_job_with_adapter(
    manager: &WorkspaceManager,
    payload: AppReleasePayload,
    adapter: &dyn PlatformAdapter,
) -> AppReleaseResult<AppReleaseJob> {
    let runner = PlatformNativeBundleRunner::new(adapter);
    start_app_release_job_with_adapter_and_native_runner(
        manager,
        payload,
        adapter,
        &runner,
        published_host_template_dir()?,
    )
}

fn start_app_release_job_with_adapter_and_native_runner(
    manager: &WorkspaceManager,
    payload: AppReleasePayload,
    adapter: &dyn PlatformAdapter,
    native_runner: &dyn NativeBundleRunner,
    host_template_dir: PathBuf,
) -> AppReleaseResult<AppReleaseJob> {
    validate_basic_payload(&payload)?;
    let current_platform = target_platform_for_os(adapter.os());
    if payload.target_platform != current_platform {
        return Err(AppReleaseError::Invalid(format!(
            "本机发布仅支持当前 OS: {current_platform}"
        )));
    }

    let manifest = manager.get_workspace_with_adapter(payload.app_id.clone(), adapter)?;
    let lockfile = manager.read_lockfile_for_manifest(&manifest)?;
    validate_selected_packs(&payload, &lockfile)?;

    let selected_plugins = if payload.selected_plugin_packs.is_empty() {
        lockfile.plugin_packs.keys().cloned().collect::<Vec<_>>()
    } else {
        payload.selected_plugin_packs.clone()
    };
    let engine = PolicyEngine::new();
    let decision = engine.evaluate_app_release(PolicyAppReleaseRequest {
        app_id: manifest.app_id.clone(),
        app_name: payload.app_name.clone(),
        target_platform: payload.target_platform.clone(),
        output_dir: payload.output_dir.clone(),
        include_ai_continuation: payload.include_ai_continuation,
        runtime_kind: format!("{:?}", manifest.mode),
        plugin_packs: selected_plugins,
    });
    engine.enforce(decision, &payload.policy_approvals)?;

    let result = create_published_app_package(PublishedAppPackageInput {
        manifest,
        lockfile,
        app_name: payload.app_name.clone(),
        target_platform: payload.target_platform.clone(),
        output_dir: payload.output_dir.clone(),
        icon_path: payload.icon_path.clone(),
        include_ai_continuation: payload.include_ai_continuation,
        stealth_ui: payload.stealth_ui.clone(),
    })?;
    let native_bundle = crate::core::published_app_packager::create_native_published_app_bundle(
        PublishedAppNativeBundleInput {
            app_name: payload.app_name.clone(),
            target_platform: payload.target_platform.clone(),
            output_dir: payload.output_dir.clone(),
            staging_dir: result.staging_dir.clone(),
            host_template_dir,
            icon_path: payload.icon_path.clone(),
        },
        native_runner,
    )?;

    Ok(completed_job(payload, result, native_bundle))
}

pub fn cancel_app_release_job(payload: CancelAppReleaseJobPayload) -> AppReleaseJob {
    AppReleaseJob {
        job_id: payload.job_id,
        status: AppReleaseJobStatus::Canceled,
        app_id: String::new(),
        app_name: String::new(),
        target_platform: String::new(),
        output_dir: PathBuf::new(),
        staging_dir: None,
        artifact_path: None,
        native_app_path: None,
        native_installer_path: None,
        manifest_path: None,
        detail: "Release job cancellation recorded. This beta packager runs synchronously."
            .to_string(),
    }
}

pub fn target_platform_for_os(os: OsKind) -> &'static str {
    match os {
        OsKind::Windows => "windows",
        OsKind::Macos => "macos",
        OsKind::Linux => "linux",
    }
}

fn validate_basic_payload(payload: &AppReleasePayload) -> AppReleaseResult<()> {
    if payload.app_id.trim().is_empty() {
        return Err(AppReleaseError::Invalid("appId is required".to_string()));
    }
    if payload.app_name.trim().is_empty() {
        return Err(AppReleaseError::Invalid("appName is required".to_string()));
    }
    if payload.target_platform.trim().is_empty() {
        return Err(AppReleaseError::Invalid(
            "targetPlatform is required".to_string(),
        ));
    }
    if payload.output_dir.as_os_str().is_empty() {
        return Err(AppReleaseError::Invalid(
            "outputDir is required".to_string(),
        ));
    }
    Ok(())
}

fn validate_selected_packs(
    payload: &AppReleasePayload,
    lockfile: &SofvaryLockfile,
) -> AppReleaseResult<()> {
    for pack_id in &payload.selected_runtime_packs {
        if !lockfile.runtime_packs.contains_key(pack_id) {
            return Err(AppReleaseError::Invalid(format!(
                "runtime pack is not locked by this workspace: {pack_id}"
            )));
        }
    }
    for pack_id in &payload.selected_plugin_packs {
        if !lockfile.plugin_packs.contains_key(pack_id) {
            return Err(AppReleaseError::Invalid(format!(
                "plugin pack is not locked by this workspace: {pack_id}"
            )));
        }
    }
    Ok(())
}

fn completed_job(
    payload: AppReleasePayload,
    result: PublishedAppPackageResult,
    native_bundle: PublishedAppNativeBundleResult,
) -> AppReleaseJob {
    AppReleaseJob {
        job_id: format!("release-{}", Utc::now().timestamp_millis()),
        status: AppReleaseJobStatus::Completed,
        app_id: result.manifest.app_id.clone(),
        app_name: payload.app_name,
        target_platform: payload.target_platform,
        output_dir: result.output_dir,
        staging_dir: Some(result.staging_dir),
        artifact_path: Some(result.artifact_path),
        native_app_path: native_bundle.app_bundle_path,
        native_installer_path: native_bundle.installer_path,
        manifest_path: Some(result.manifest_path),
        detail:
            "Unsigned beta native app package created. Signing and notarization are future phases."
                .to_string(),
    }
}

fn published_host_template_dir() -> AppReleaseResult<PathBuf> {
    if let Ok(path) = std::env::var("SOFVARY_PUBLISHED_HOST_TEMPLATE_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("published-host"))
}

fn platform_label(platform: &str) -> &'static str {
    match platform {
        "windows" => "Windows",
        "macos" => "Mac",
        "linux" => "Linux",
        _ => "Unknown",
    }
}

fn output_kind_for_platform(platform: &str) -> &'static str {
    match platform {
        "windows" => "NSIS .exe unsigned beta",
        "macos" => ".app + .dmg unsigned beta",
        "linux" => ".AppImage unsigned beta",
        _ => "unknown",
    }
}

fn runtime_capabilities() -> Vec<AppReleaseRuntimeCapability> {
    [
        RuntimeKind::StaticHtml,
        RuntimeKind::ReactVite,
        RuntimeKind::ReactSqlite,
        RuntimeKind::AiAgentApp,
        RuntimeKind::Canvas2d,
        RuntimeKind::MarkdownKnowledge,
        RuntimeKind::DataTable,
        RuntimeKind::FileProcessor,
        RuntimeKind::DesktopWidget,
    ]
    .into_iter()
    .map(runtime_capability)
    .collect()
}

fn runtime_capability(runtime_kind: RuntimeKind) -> AppReleaseRuntimeCapability {
    let (label, release_strategy, notes) = match runtime_kind {
        RuntimeKind::StaticHtml => (
            "Static HTML",
            "seed-static-host",
            vec!["Package generated static files into the published host."],
        ),
        RuntimeKind::Canvas2d => (
            "Canvas 2D",
            "seed-static-host",
            vec!["Package Canvas assets and static host resources."],
        ),
        RuntimeKind::ReactVite => (
            "React + Vite",
            "controlled-prod-build",
            vec!["Run the workspace production build before bundling dist output."],
        ),
        RuntimeKind::ReactSqlite => (
            "React + SQLite",
            "controlled-prod-build-node-api-sqlite",
            vec![
                "Bundle frontend dist, Node API sidecar metadata, schema, seeds, and offline dependency metadata.",
                "Installed app data expands into the per-user data directory on first launch.",
            ],
        ),
        RuntimeKind::AiAgentApp => (
            "AI Agent App",
            "controlled-prod-build-ai-bindings",
            vec!["Bundle provider requirement metadata without raw provider secrets."],
        ),
        RuntimeKind::MarkdownKnowledge => (
            "Markdown Knowledge",
            "controlled-prod-build-content-index",
            vec!["Bundle generated content and runtime metadata."],
        ),
        RuntimeKind::DataTable => (
            "Data Table",
            "controlled-prod-build-data-runtime",
            vec!["Bundle generated table runtime metadata and seed assets."],
        ),
        RuntimeKind::FileProcessor => (
            "File Processor",
            "controlled-prod-build-file-runtime",
            vec!["Bundle processor UI and keep user-selected files outside the release package."],
        ),
        RuntimeKind::DesktopWidget => (
            "Desktop Widget",
            "controlled-prod-build-widget-runtime",
            vec!["Bundle widget assets through the published host template."],
        ),
    };

    AppReleaseRuntimeCapability {
        runtime_kind,
        label: label.to_string(),
        supported: true,
        release_strategy: release_strategy.to_string(),
        ai_continuation_supported: true,
        notes: notes.into_iter().map(ToString::to_string).collect(),
    }
}

pub fn policy_request_for_preview(
    app_id: String,
    app_name: String,
    target_platform: String,
    output_dir: PathBuf,
    include_ai_continuation: bool,
    runtime_kind: RuntimeKind,
    plugin_packs: Vec<String>,
) -> PolicyAppReleaseRequest {
    PolicyAppReleaseRequest {
        app_id,
        app_name,
        target_platform,
        output_dir,
        include_ai_continuation,
        runtime_kind: format!("{:?}", runtime_kind),
        plugin_packs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::policy_engine::app_release_subject;
    use crate::core::policy_types::{PolicyActionKind, PolicyApprovalGrant};
    use crate::platform::windows::WindowsPlatformAdapter;
    use crate::platform::{
        ArchKind, CommandSpec, PlatformDirs, PlatformResult, ProcessHandle, ProcessOutput,
        WebviewProfile, WorkArea,
    };
    use std::path::Path;

    struct TempAdapter {
        os: OsKind,
        dirs: PlatformDirs,
    }

    impl PlatformAdapter for TempAdapter {
        fn os(&self) -> OsKind {
            self.os
        }

        fn arch(&self) -> ArchKind {
            ArchKind::Arm64
        }

        fn dirs(&self) -> PlatformResult<PlatformDirs> {
            Ok(self.dirs.clone())
        }

        fn normalize_path(&self, input: &str) -> PlatformResult<PathBuf> {
            WindowsPlatformAdapter.normalize_path(input)
        }

        fn ensure_executable(&self, path: &Path) -> PlatformResult<()> {
            WindowsPlatformAdapter.ensure_executable(path)
        }

        fn resolve_sidecar_executable(&self, name: &str) -> PlatformResult<PathBuf> {
            WindowsPlatformAdapter.resolve_sidecar_executable(name)
        }

        fn run_process(&self, spec: CommandSpec) -> PlatformResult<ProcessOutput> {
            WindowsPlatformAdapter.run_process(spec)
        }

        fn spawn_process(&self, spec: CommandSpec) -> PlatformResult<ProcessHandle> {
            WindowsPlatformAdapter.spawn_process(spec)
        }

        fn kill_process_tree(&self, pid: u32) -> PlatformResult<()> {
            WindowsPlatformAdapter.kill_process_tree(pid)
        }

        fn allocate_local_port(&self) -> PlatformResult<u16> {
            WindowsPlatformAdapter.allocate_local_port()
        }

        fn open_external(&self, url: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.open_external(url)
        }

        fn reveal_path(&self, _path: &Path) -> PlatformResult<()> {
            Ok(())
        }

        fn register_protocol_handler(&self, protocol: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.register_protocol_handler(protocol)
        }

        fn register_global_shortcut(&self, accelerator: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.register_global_shortcut(accelerator)
        }

        fn unregister_global_shortcut(&self, accelerator: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.unregister_global_shortcut(accelerator)
        }

        fn show_tray_or_menu_bar_item(&self) -> PlatformResult<()> {
            Ok(())
        }

        fn get_active_monitor_work_area(&self) -> PlatformResult<WorkArea> {
            WindowsPlatformAdapter.get_active_monitor_work_area()
        }

        fn secure_store_set(&self, key: &str, value: &str) -> PlatformResult<()> {
            WindowsPlatformAdapter.secure_store_set(key, value)
        }

        fn secure_store_get(&self, key: &str) -> PlatformResult<Option<String>> {
            WindowsPlatformAdapter.secure_store_get(key)
        }

        fn current_webview_profile(&self) -> WebviewProfile {
            WindowsPlatformAdapter.current_webview_profile()
        }
    }

    #[test]
    fn capabilities_only_enable_current_platform() {
        let adapter = TempAdapter {
            os: OsKind::Linux,
            dirs: PlatformDirs {
                data_dir: PathBuf::from("/tmp/sofvary-data"),
                cache_dir: PathBuf::from("/tmp/sofvary-cache"),
                config_dir: PathBuf::from("/tmp/sofvary-config"),
            },
        };
        let capabilities = get_app_release_capabilities_with_adapter(&adapter);
        assert_eq!(capabilities.current_platform, "linux");
        assert!(capabilities
            .target_platforms
            .iter()
            .any(|item| item.platform == "linux" && item.enabled));
        assert!(capabilities
            .target_platforms
            .iter()
            .any(|item| item.platform == "windows" && !item.enabled));
        assert_eq!(capabilities.runtimes.len(), 9);
    }

    #[test]
    fn release_rejects_non_current_platform() {
        let temp = tempfile::tempdir().expect("temp");
        let adapter = TempAdapter {
            os: OsKind::Macos,
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_with_adapter("Release Test".to_string(), &adapter)
            .expect("workspace");
        let error = start_app_release_job_with_adapter(
            &manager,
            AppReleasePayload {
                app_id: manifest.app_id,
                app_name: "Release Test".to_string(),
                target_platform: "windows".to_string(),
                output_dir: temp.path().join("out"),
                icon_path: None,
                include_ai_continuation: false,
                stealth_ui: PublishedAppStealthUiSettings::default(),
                selected_runtime_packs: Vec::new(),
                selected_plugin_packs: Vec::new(),
                policy_approvals: PolicyApprovalSet::default(),
            },
            &adapter,
        )
        .expect_err("platform mismatch should fail");
        assert!(error.to_string().contains("当前 OS"));
    }

    #[test]
    fn release_requires_policy_approval_then_writes_artifact() {
        let temp = tempfile::tempdir().expect("temp");
        let adapter = TempAdapter {
            os: OsKind::Macos,
            dirs: PlatformDirs {
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                config_dir: temp.path().join("config"),
            },
        };
        let manager = WorkspaceManager::new();
        let manifest = manager
            .create_workspace_with_adapter("Release Test".to_string(), &adapter)
            .expect("workspace");
        std::fs::write(
            manifest.paths.generated_static.join("index.html"),
            "<html>ok</html>",
        )
        .expect("seed");
        let output_dir = temp.path().join("release-out");
        let denied = start_app_release_job_with_adapter(
            &manager,
            AppReleasePayload {
                app_id: manifest.app_id.clone(),
                app_name: "Release Test".to_string(),
                target_platform: "macos".to_string(),
                output_dir: output_dir.clone(),
                icon_path: None,
                include_ai_continuation: true,
                stealth_ui: PublishedAppStealthUiSettings {
                    ai_menu_label: "Tune this app".to_string(),
                    ai_shortcut: "CmdOrCtrl+Shift+U".to_string(),
                    ai_panel_title: "Tune app".to_string(),
                    provider_setup_title: "Connect model".to_string(),
                    prompt_placeholder: "What should change?".to_string(),
                },
                selected_runtime_packs: Vec::new(),
                selected_plugin_packs: Vec::new(),
                policy_approvals: PolicyApprovalSet::default(),
            },
            &adapter,
        )
        .expect_err("approval required");
        assert!(matches!(denied, AppReleaseError::Policy(_)));

        let subject = app_release_subject(&manifest.app_id, "macos", &output_dir);
        let host_template_dir = temp.path().join("published-host");
        write_minimal_host_template(&host_template_dir);
        let native_runner = TestNativeBundleRunner;
        let job = start_app_release_job_with_adapter_and_native_runner(
            &manager,
            AppReleasePayload {
                app_id: manifest.app_id,
                app_name: "Release Test".to_string(),
                target_platform: "macos".to_string(),
                output_dir,
                icon_path: None,
                include_ai_continuation: true,
                stealth_ui: PublishedAppStealthUiSettings {
                    ai_menu_label: "Tune this app".to_string(),
                    ai_shortcut: "CmdOrCtrl+Shift+U".to_string(),
                    ai_panel_title: "Tune app".to_string(),
                    provider_setup_title: "Connect model".to_string(),
                    prompt_placeholder: "What should change?".to_string(),
                },
                selected_runtime_packs: Vec::new(),
                selected_plugin_packs: Vec::new(),
                policy_approvals: PolicyApprovalSet {
                    approved: vec![PolicyApprovalGrant {
                        action: PolicyActionKind::AppRelease,
                        subject: Some(subject),
                    }],
                },
            },
            &adapter,
            &native_runner,
            host_template_dir,
        )
        .expect("release");
        assert_eq!(job.status, AppReleaseJobStatus::Completed);
        assert!(job.artifact_path.expect("artifact").exists());
        assert!(job.native_app_path.expect("native app").exists());
        assert!(job
            .native_installer_path
            .expect("native installer")
            .exists());
        assert!(job.manifest_path.expect("manifest").exists());
    }

    struct TestNativeBundleRunner;

    impl NativeBundleRunner for TestNativeBundleRunner {
        fn run(&self, spec: CommandSpec) -> Result<ProcessOutput, PublishedAppPackagerError> {
            let bundle_root = spec.cwd.join("src-tauri/target/release/bundle");
            let app_dir = bundle_root.join("macos/Release Test.app");
            std::fs::create_dir_all(&app_dir).expect("app dir");
            std::fs::write(app_dir.join("Contents.txt"), "app").expect("app");
            let dmg_dir = bundle_root.join("dmg");
            std::fs::create_dir_all(&dmg_dir).expect("dmg dir");
            std::fs::write(dmg_dir.join("Release Test_0.1.0_aarch64.dmg"), "dmg").expect("dmg");
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: "ok".to_string(),
                stderr: String::new(),
            })
        }
    }

    fn write_minimal_host_template(root: &Path) {
        std::fs::create_dir_all(root.join("src-tauri/src")).expect("src-tauri");
        std::fs::write(
            root.join("package.json"),
            "{\"scripts\":{\"build\":\"true\"}}\n",
        )
        .expect("package");
        std::fs::write(
            root.join("src-tauri/tauri.conf.json"),
            "{\"productName\":\"Host\",\"version\":\"0.1.0\",\"identifier\":\"com.test.host\",\"bundle\":{\"active\":true}}\n",
        )
        .expect("config");
    }
}
