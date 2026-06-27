use crate::core::workspace_types::{AppBoxManifest, RuntimeKind, SofvaryLockfile};
use crate::platform::{CommandSpec, PlatformAdapter, PlatformError, ProcessOutput};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Debug, Error)]
pub enum PublishedAppPackagerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("invalid release package: {0}")]
    Invalid(String),
}

pub type PublishedAppPackagerResult<T> = Result<T, PublishedAppPackagerError>;
const PUBLISHED_HOST_CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*; frame-src http://127.0.0.1:*; child-src http://127.0.0.1:*";
const MIN_SOURCE_ICON_SIZE: u32 = 512;
const GENERATED_ICON_FILES: [&str; 5] = [
    "32x32.png",
    "128x128.png",
    "128x128@2x.png",
    "icon.icns",
    "icon.ico",
];
const RELEASE_RESOURCES_TARGET_DIR: &str = "target/sofvary-release-resources";

#[derive(Debug, Clone)]
pub struct PublishedAppPackageInput {
    pub manifest: AppBoxManifest,
    pub lockfile: SofvaryLockfile,
    pub app_name: String,
    pub target_platform: String,
    pub output_dir: PathBuf,
    pub icon_path: Option<PathBuf>,
    pub include_ai_continuation: bool,
    pub stealth_ui: PublishedAppStealthUiSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublishedAppManifest {
    pub schema_version: String,
    pub package_type: String,
    pub app_id: String,
    pub app_name: String,
    pub target_platform: String,
    pub runtime_kind: RuntimeKind,
    pub created_at: String,
    pub beta: bool,
    pub unsigned: bool,
    pub sofvary_shell_included: bool,
    pub ai_continuation: PublishedAppAiContinuation,
    pub stealth_ui: PublishedAppStealthUiSettings,
    pub seed_workspace_path: String,
    pub lockfile_path: String,
    pub plugin_packs: BTreeMap<String, String>,
    pub checksums: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublishedAppAiContinuation {
    pub enabled: bool,
    pub credential_source: String,
    pub includes_raw_secret: bool,
    pub writes_to_user_data_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublishedAppStealthUiSettings {
    pub ai_menu_label: String,
    pub ai_shortcut: String,
    pub ai_panel_title: String,
    pub provider_setup_title: String,
    pub prompt_placeholder: String,
}

impl Default for PublishedAppStealthUiSettings {
    fn default() -> Self {
        Self {
            ai_menu_label: "Optimize with AI".to_string(),
            ai_shortcut: "CmdOrCtrl+Shift+I".to_string(),
            ai_panel_title: "AI Optimize".to_string(),
            provider_setup_title: "Connect your AI provider".to_string(),
            prompt_placeholder: "Describe what you want to improve in this app.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublishedAppPackageResult {
    pub output_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub artifact_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: PublishedAppManifest,
}

#[derive(Debug, Clone)]
pub struct PublishedAppNativeBundleInput {
    pub app_name: String,
    pub target_platform: String,
    pub output_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub host_template_dir: PathBuf,
    pub pnpm_executable: PathBuf,
    pub icon_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublishedAppNativeBundleResult {
    pub app_bundle_path: Option<PathBuf>,
    pub installer_path: Option<PathBuf>,
    pub bundle_root: PathBuf,
    pub config_path: PathBuf,
}

pub trait NativeBundleRunner {
    fn run(&self, spec: CommandSpec) -> PublishedAppPackagerResult<ProcessOutput>;
}

pub struct PlatformNativeBundleRunner<'a> {
    adapter: &'a dyn PlatformAdapter,
}

impl<'a> PlatformNativeBundleRunner<'a> {
    pub fn new(adapter: &'a dyn PlatformAdapter) -> Self {
        Self { adapter }
    }
}

impl NativeBundleRunner for PlatformNativeBundleRunner<'_> {
    fn run(&self, spec: CommandSpec) -> PublishedAppPackagerResult<ProcessOutput> {
        Ok(self.adapter.run_process(spec)?)
    }
}

pub fn create_published_app_package(
    input: PublishedAppPackageInput,
) -> PublishedAppPackagerResult<PublishedAppPackageResult> {
    validate_output_dir(&input.output_dir)?;
    fs::create_dir_all(&input.output_dir)?;
    let safe_name = sanitize_release_name(&input.app_name);
    let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let staging_dir = input
        .output_dir
        .join(format!("{safe_name}-{}-{timestamp}", input.target_platform));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)?;
    }
    fs::create_dir_all(&staging_dir)?;

    let seed_root = staging_dir.join("seed-workspace");
    fs::create_dir_all(&seed_root)?;
    copy_generated_seed(
        &input.manifest.paths.generated,
        &seed_root.join("generated"),
    )?;
    fs::write(
        seed_root.join("sofvary.lock.json"),
        serde_json::to_string_pretty(&input.lockfile)? + "\n",
    )?;
    fs::write(
        seed_root.join("app.box.json"),
        serde_json::to_string_pretty(&input.manifest)? + "\n",
    )?;

    if let Some(icon_path) = &input.icon_path {
        copy_icon(icon_path, &staging_dir.join("branding"))?;
    }

    fs::write(staging_dir.join("README.txt"), release_readme(&input))?;
    let checksums = checksums_for_dir(&staging_dir)?;
    let manifest = PublishedAppManifest {
        schema_version: "1.0".to_string(),
        package_type: "sofvary.published-app".to_string(),
        app_id: input.manifest.app_id.clone(),
        app_name: input.app_name.clone(),
        target_platform: input.target_platform.clone(),
        runtime_kind: input.manifest.mode,
        created_at: Utc::now().to_rfc3339(),
        beta: true,
        unsigned: true,
        sofvary_shell_included: false,
        ai_continuation: PublishedAppAiContinuation {
            enabled: input.include_ai_continuation,
            credential_source: if input.include_ai_continuation {
                "end-user-secure-store".to_string()
            } else {
                "none".to_string()
            },
            includes_raw_secret: false,
            writes_to_user_data_dir: input.include_ai_continuation,
        },
        stealth_ui: normalize_stealth_ui_settings(input.stealth_ui.clone()),
        seed_workspace_path: "seed-workspace".to_string(),
        lockfile_path: "seed-workspace/sofvary.lock.json".to_string(),
        plugin_packs: input
            .lockfile
            .plugin_packs
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        checksums,
    };
    let manifest_path = staging_dir.join("published-app.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;

    let artifact_path = input.output_dir.join(format!(
        "{safe_name}-{}-unsigned-beta.sfrelease.zip",
        input.target_platform
    ));
    if artifact_path.exists() {
        fs::remove_file(&artifact_path)?;
    }
    zip_dir(&staging_dir, &artifact_path)?;

    Ok(PublishedAppPackageResult {
        output_dir: input.output_dir,
        staging_dir,
        artifact_path,
        manifest_path,
        manifest,
    })
}

pub fn create_native_published_app_bundle(
    input: PublishedAppNativeBundleInput,
    runner: &dyn NativeBundleRunner,
) -> PublishedAppPackagerResult<PublishedAppNativeBundleResult> {
    validate_output_dir(&input.output_dir)?;
    fs::create_dir_all(&input.output_dir)?;
    if !input.staging_dir.is_dir() {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "release staging directory does not exist: {}",
            input.staging_dir.display()
        )));
    }
    if !input.host_template_dir.join("src-tauri").is_dir() {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "published host template is missing src-tauri: {}",
            input.host_template_dir.display()
        )));
    }

    let safe_name = sanitize_release_name(&input.app_name);
    let config_path = input
        .staging_dir
        .join(format!("{safe_name}-tauri.release.conf.json"));
    let icon_paths = prepare_native_icon_paths(&input, runner)?;
    let release_resources_source = prepare_native_release_resources(&input, &safe_name)?;
    write_native_tauri_config(&input, &config_path, &icon_paths, &release_resources_source)?;

    let bundle_root = input
        .host_template_dir
        .join("src-tauri")
        .join("target")
        .join("release")
        .join("bundle");
    if bundle_root.exists() {
        fs::remove_dir_all(&bundle_root)?;
    }

    let host_build_output = runner.run(native_host_build_spec(&input))?;
    if host_build_output.status_code != Some(0) {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "published host frontend build failed: {}",
            summarize_process_output(&host_build_output)
        )));
    }

    let output = runner.run(native_tauri_build_spec(&input, &config_path)?)?;
    if output.status_code != Some(0) {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "native Tauri bundle failed: {}",
            summarize_process_output(&output)
        )));
    }

    copy_native_bundle_outputs(&input, &bundle_root, &config_path)
}

pub fn sanitize_release_name(value: &str) -> String {
    let safe = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    safe.trim_matches(['-', '.', '_'])
        .chars()
        .take(64)
        .collect::<String>()
        .if_empty("sofvary-app")
}

fn normalize_stealth_ui_settings(
    settings: PublishedAppStealthUiSettings,
) -> PublishedAppStealthUiSettings {
    let defaults = PublishedAppStealthUiSettings::default();
    PublishedAppStealthUiSettings {
        ai_menu_label: text_or_default(settings.ai_menu_label, &defaults.ai_menu_label),
        ai_shortcut: text_or_default(settings.ai_shortcut, &defaults.ai_shortcut),
        ai_panel_title: text_or_default(settings.ai_panel_title, &defaults.ai_panel_title),
        provider_setup_title: text_or_default(
            settings.provider_setup_title,
            &defaults.provider_setup_title,
        ),
        prompt_placeholder: text_or_default(
            settings.prompt_placeholder,
            &defaults.prompt_placeholder,
        ),
    }
}

fn text_or_default(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(96).collect()
    }
}

fn prepare_native_icon_paths(
    input: &PublishedAppNativeBundleInput,
    runner: &dyn NativeBundleRunner,
) -> PublishedAppPackagerResult<Vec<String>> {
    let Some(icon_path) = &input.icon_path else {
        return Ok(Vec::new());
    };
    if !icon_path.is_file() {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "icon file does not exist: {}",
            icon_path.display()
        )));
    }
    let extension = icon_extension(icon_path)?;
    let icon_root = input
        .host_template_dir
        .join("src-tauri")
        .join("target")
        .join("sofvary-release-icons")
        .join(sanitize_release_name(&input.app_name));
    if icon_root.exists() {
        fs::remove_dir_all(&icon_root)?;
    }
    fs::create_dir_all(&icon_root)?;
    match extension.as_str() {
        "png" => generate_icon_set_from_png(input, runner, icon_path, &icon_root),
        "icns" => {
            if input.target_platform != "macos" {
                return Err(PublishedAppPackagerError::Invalid(
                    "macOS .icns icon files can only be used when publishing for macOS. Use a square PNG at least 512x512 px for cross-platform icon generation.".to_string(),
                ));
            }
            copy_single_icon(icon_path, &icon_root.join("icon.icns"), input)
        }
        "ico" => {
            if input.target_platform != "windows" {
                return Err(PublishedAppPackagerError::Invalid(
                    "Windows .ico icon files can only be used when publishing for Windows. Use a square PNG at least 512x512 px for cross-platform icon generation.".to_string(),
                ));
            }
            copy_single_icon(icon_path, &icon_root.join("icon.ico"), input)
        }
        _ => Err(PublishedAppPackagerError::Invalid(
            "unsupported icon file type. Use a square PNG at least 512x512 px, macOS .icns, or Windows .ico file.".to_string(),
        )),
    }
}

fn generate_icon_set_from_png(
    input: &PublishedAppNativeBundleInput,
    runner: &dyn NativeBundleRunner,
    icon_path: &Path,
    icon_root: &Path,
) -> PublishedAppPackagerResult<Vec<String>> {
    validate_png_icon_source(icon_path)?;
    let output = runner.run(native_tauri_icon_spec(input, icon_path, icon_root)?)?;
    if output.status_code != Some(0) {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "custom icon is invalid. Use a square PNG at least {MIN_SOURCE_ICON_SIZE}x{MIN_SOURCE_ICON_SIZE} px. Tauri icon generation failed: {}",
            summarize_process_output(&output)
        )));
    }

    let mut icon_paths = Vec::new();
    for file_name in GENERATED_ICON_FILES {
        let generated = icon_root.join(file_name);
        if generated.is_file() {
            icon_paths.push(relative_icon_path(&generated, input)?);
        }
    }
    if icon_paths.is_empty() {
        return Err(PublishedAppPackagerError::Invalid(
            "custom icon generation did not produce any Tauri icon files. Use a square PNG at least 512x512 px.".to_string(),
        ));
    }
    Ok(icon_paths)
}

fn copy_single_icon(
    source: &Path,
    target: &Path,
    input: &PublishedAppNativeBundleInput,
) -> PublishedAppPackagerResult<Vec<String>> {
    fs::copy(source, target)?;
    Ok(vec![relative_icon_path(target, input)?])
}

fn relative_icon_path(
    icon_path: &Path,
    input: &PublishedAppNativeBundleInput,
) -> PublishedAppPackagerResult<String> {
    let relative = icon_path
        .strip_prefix(input.host_template_dir.join("src-tauri"))
        .map_err(|_| {
            PublishedAppPackagerError::Invalid("icon path escaped host template".to_string())
        })?;
    Ok(normalize_archive_path(relative))
}

fn icon_extension(path: &Path) -> PublishedAppPackagerResult<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            PublishedAppPackagerError::Invalid(
                "icon file must have an extension: png, icns, or ico".to_string(),
            )
        })
}

fn validate_png_icon_source(path: &Path) -> PublishedAppPackagerResult<()> {
    let mut header = [0_u8; 24];
    fs::File::open(path)?.read_exact(&mut header).map_err(|_| {
        PublishedAppPackagerError::Invalid(format!(
            "custom icon PNG is unreadable or too small: {}",
            path.display()
        ))
    })?;
    if header[0..8] != [137, 80, 78, 71, 13, 10, 26, 10] || &header[12..16] != b"IHDR" {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "custom icon must be a valid PNG file: {}",
            path.display()
        )));
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    if width != height {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "custom icon PNG must be square. Selected image is {width}x{height} px."
        )));
    }
    if width < MIN_SOURCE_ICON_SIZE {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "custom icon PNG is too small. Use a square PNG at least {MIN_SOURCE_ICON_SIZE}x{MIN_SOURCE_ICON_SIZE} px. Selected image is {width}x{height} px."
        )));
    }
    Ok(())
}

fn write_native_tauri_config(
    input: &PublishedAppNativeBundleInput,
    config_path: &Path,
    icon_paths: &[String],
    release_resources_source: &str,
) -> PublishedAppPackagerResult<()> {
    let mut resources = Map::new();
    resources.insert(
        release_resources_source.to_string(),
        Value::String("release-resources".to_string()),
    );
    let mut bundle = json!({
        "resources": Value::Object(resources)
    });
    if !icon_paths.is_empty() {
        bundle["icon"] = json!(icon_paths);
    }

    let config = json!({
        "productName": input.app_name,
        "identifier": release_identifier(&input.app_name),
        "build": {
            "beforeBuildCommand": "",
            "frontendDist": "../dist"
        },
        "app": {
            "security": {
                "csp": PUBLISHED_HOST_CSP
            },
            "windows": [
                {
                    "label": "main",
                    "title": input.app_name,
                    "width": 1100,
                    "height": 720,
                    "minWidth": 760,
                    "minHeight": 520,
                    "visible": true,
                    "decorations": true
                }
            ]
        },
        "bundle": bundle
    });
    fs::write(config_path, serde_json::to_string_pretty(&config)? + "\n")?;
    Ok(())
}

fn prepare_native_release_resources(
    input: &PublishedAppNativeBundleInput,
    safe_name: &str,
) -> PublishedAppPackagerResult<String> {
    let relative = format!("{RELEASE_RESOURCES_TARGET_DIR}/{safe_name}");
    let target = input.host_template_dir.join("src-tauri").join(&relative);
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::create_dir_all(&target)?;
    copy_dir_filtered(&input.staging_dir, &target, &input.staging_dir)?;
    Ok(format!("{relative}/"))
}

fn release_identifier(app_name: &str) -> String {
    let safe = sanitize_release_name(app_name)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .if_empty("sofvary-app");
    format!("com.sofvary.published.{safe}")
}

fn native_tauri_icon_spec(
    input: &PublishedAppNativeBundleInput,
    icon_path: &Path,
    icon_root: &Path,
) -> PublishedAppPackagerResult<CommandSpec> {
    let output_path = icon_root
        .strip_prefix(&input.host_template_dir)
        .map_err(|_| {
            PublishedAppPackagerError::Invalid("icon output path escaped host template".to_string())
        })?;
    Ok(CommandSpec {
        executable: input.pnpm_executable.clone(),
        args: vec![
            "tauri".to_string(),
            "icon".to_string(),
            icon_path.to_string_lossy().to_string(),
            "--output".to_string(),
            normalize_archive_path(output_path),
        ],
        cwd: input.host_template_dir.clone(),
        env: HashMap::new(),
        allowed_network: false,
        timeout_ms: Some(2 * 60 * 1000),
        kill_on_drop: true,
    })
}

fn native_host_build_spec(input: &PublishedAppNativeBundleInput) -> CommandSpec {
    CommandSpec {
        executable: input.pnpm_executable.clone(),
        args: vec!["build".to_string()],
        cwd: input.host_template_dir.clone(),
        env: HashMap::new(),
        allowed_network: false,
        timeout_ms: Some(5 * 60 * 1000),
        kill_on_drop: true,
    }
}

fn native_tauri_build_spec(
    input: &PublishedAppNativeBundleInput,
    config_path: &Path,
) -> PublishedAppPackagerResult<CommandSpec> {
    let bundle_target = native_bundle_target(&input.target_platform)?;
    Ok(CommandSpec {
        executable: input.pnpm_executable.clone(),
        args: vec![
            "tauri".to_string(),
            "build".to_string(),
            "--bundles".to_string(),
            bundle_target.to_string(),
            "--ci".to_string(),
            "--config".to_string(),
            config_path.to_string_lossy().to_string(),
        ],
        cwd: input.host_template_dir.clone(),
        env: HashMap::new(),
        allowed_network: false,
        timeout_ms: Some(20 * 60 * 1000),
        kill_on_drop: true,
    })
}

fn native_bundle_target(target_platform: &str) -> PublishedAppPackagerResult<&'static str> {
    match target_platform {
        "macos" => Ok("app,dmg"),
        "windows" => Ok("nsis"),
        "linux" => Ok("appimage"),
        other => Err(PublishedAppPackagerError::Invalid(format!(
            "unsupported native bundle platform: {other}"
        ))),
    }
}

fn copy_native_bundle_outputs(
    input: &PublishedAppNativeBundleInput,
    bundle_root: &Path,
    config_path: &Path,
) -> PublishedAppPackagerResult<PublishedAppNativeBundleResult> {
    match input.target_platform.as_str() {
        "macos" => copy_macos_bundle_outputs(input, bundle_root, config_path),
        "windows" => copy_installer_outputs(
            input,
            bundle_root,
            "nsis",
            "exe",
            "native Windows installer was not produced",
            config_path,
        ),
        "linux" => copy_installer_outputs(
            input,
            bundle_root,
            "appimage",
            "AppImage",
            "native Linux AppImage was not produced",
            config_path,
        ),
        other => Err(PublishedAppPackagerError::Invalid(format!(
            "unsupported native bundle platform: {other}"
        ))),
    }
}

fn copy_macos_bundle_outputs(
    input: &PublishedAppNativeBundleInput,
    bundle_root: &Path,
    config_path: &Path,
) -> PublishedAppPackagerResult<PublishedAppNativeBundleResult> {
    let app_source = find_first_with_extension(&bundle_root.join("macos"), "app")?;
    let Some(app_source) = app_source else {
        return Err(PublishedAppPackagerError::Invalid(
            "native macOS bundle was not produced".to_string(),
        ));
    };
    let app_target = input.output_dir.join(file_name(&app_source)?);
    copy_path_replace(&app_source, &app_target)?;

    let installer_source = find_first_with_extension(&bundle_root.join("dmg"), "dmg")?;
    let installer_path = if let Some(installer_source) = installer_source {
        let installer_target = input.output_dir.join(file_name(&installer_source)?);
        copy_path_replace(&installer_source, &installer_target)?;
        Some(installer_target)
    } else {
        None
    };

    Ok(PublishedAppNativeBundleResult {
        app_bundle_path: Some(app_target),
        installer_path,
        bundle_root: bundle_root.to_path_buf(),
        config_path: config_path.to_path_buf(),
    })
}

fn copy_installer_outputs(
    input: &PublishedAppNativeBundleInput,
    bundle_root: &Path,
    bundle_subdir: &str,
    extension: &str,
    missing_message: &str,
    config_path: &Path,
) -> PublishedAppPackagerResult<PublishedAppNativeBundleResult> {
    let installer_source = find_first_with_extension(&bundle_root.join(bundle_subdir), extension)?;
    let Some(installer_source) = installer_source else {
        return Err(PublishedAppPackagerError::Invalid(
            missing_message.to_string(),
        ));
    };
    let installer_target = input.output_dir.join(file_name(&installer_source)?);
    copy_path_replace(&installer_source, &installer_target)?;

    Ok(PublishedAppNativeBundleResult {
        app_bundle_path: None,
        installer_path: Some(installer_target),
        bundle_root: bundle_root.to_path_buf(),
        config_path: config_path.to_path_buf(),
    })
}

fn find_first_with_extension(
    root: &Path,
    extension: &str,
) -> PublishedAppPackagerResult<Option<PathBuf>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let expected = extension.to_ascii_lowercase();
    let mut matches = Vec::new();
    collect_paths_with_extension(root, &expected, &mut matches)?;
    matches.sort();
    Ok(matches.into_iter().next())
}

fn collect_paths_with_extension(
    current: &Path,
    extension: &str,
    matches: &mut Vec<PathBuf>,
) -> PublishedAppPackagerResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if has_extension(&path, extension) {
            matches.push(path);
        } else if file_type.is_dir() {
            collect_paths_with_extension(&path, extension, matches)?;
        }
    }
    Ok(())
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

fn copy_path_replace(source: &Path, target: &Path) -> PublishedAppPackagerResult<()> {
    if target.exists() {
        if target.is_dir() {
            fs::remove_dir_all(target)?;
        } else {
            fs::remove_file(target)?;
        }
    }
    if source.is_dir() {
        fs::create_dir_all(target)?;
        copy_dir_filtered(source, target, source)?;
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
    }
    Ok(())
}

fn file_name(path: &Path) -> PublishedAppPackagerResult<&OsStr> {
    path.file_name().ok_or_else(|| {
        PublishedAppPackagerError::Invalid(format!("path has no file name: {}", path.display()))
    })
}

fn summarize_process_output(output: &ProcessOutput) -> String {
    let lines = output
        .stderr
        .lines()
        .chain(output.stdout.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return "process failed without output".to_string();
    }

    let start = lines
        .iter()
        .position(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error")
                || lower.contains("failed")
                || lower.contains("doesn't exist")
                || lower.contains("not found")
        })
        .unwrap_or(0);
    let summary = lines
        .iter()
        .skip(start)
        .take(6)
        .copied()
        .collect::<Vec<_>>()
        .join(" | ");
    if summary.chars().count() > 900 {
        format!("{}...", summary.chars().take(900).collect::<String>())
    } else {
        summary
    }
}

fn validate_output_dir(path: &Path) -> PublishedAppPackagerResult<()> {
    if path.as_os_str().is_empty() {
        return Err(PublishedAppPackagerError::Invalid(
            "output directory is required".to_string(),
        ));
    }
    let normalized = normalize_path(path);
    if normalized.components().count() <= 1 {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "refusing to write release output at {}",
            path.display()
        )));
    }
    Ok(())
}

fn copy_generated_seed(source: &Path, target: &Path) -> PublishedAppPackagerResult<()> {
    if !source.exists() {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "generated source does not exist: {}",
            source.display()
        )));
    }
    fs::create_dir_all(target)?;
    copy_dir_filtered(source, target, source)
}

fn copy_dir_filtered(
    root: &Path,
    target_root: &Path,
    current: &Path,
) -> PublishedAppPackagerResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|_| {
            PublishedAppPackagerError::Invalid("path escaped generated root".to_string())
        })?;
        if should_skip_seed_path(relative) {
            continue;
        }
        let target = target_root.join(relative);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_filtered(root, target_root, &path)?;
        } else if file_type.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn should_skip_seed_path(relative: &Path) -> bool {
    relative.components().any(|component| {
        matches!(
            component,
            Component::Normal(name)
                if name == OsStr::new("node_modules")
                    || name == OsStr::new(".vite")
                    || name == OsStr::new("target")
        )
    }) || relative == Path::new("data/app.sqlite")
}

fn copy_icon(icon_path: &Path, branding_dir: &Path) -> PublishedAppPackagerResult<()> {
    if !icon_path.is_file() {
        return Err(PublishedAppPackagerError::Invalid(format!(
            "icon file does not exist: {}",
            icon_path.display()
        )));
    }
    fs::create_dir_all(branding_dir)?;
    let extension = icon_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("icon");
    fs::copy(icon_path, branding_dir.join(format!("icon.{extension}")))?;
    Ok(())
}

fn release_readme(input: &PublishedAppPackageInput) -> String {
    let ai = if input.include_ai_continuation {
        "AI continuation: enabled. The installed app user must configure their own provider credential."
    } else {
        "AI continuation: disabled."
    };
    format!(
        "{} beta release package\n\nTarget platform: {}\nRuntime: {:?}\nUnsigned beta: yes\nSofvary shell included: no\n{}\n",
        input.app_name, input.target_platform, input.manifest.mode, ai
    )
}

fn checksums_for_dir(root: &Path) -> PublishedAppPackagerResult<BTreeMap<String, String>> {
    let mut checksums = BTreeMap::new();
    collect_checksums(root, root, &mut checksums)?;
    Ok(checksums)
}

fn collect_checksums(
    root: &Path,
    current: &Path,
    checksums: &mut BTreeMap<String, String>,
) -> PublishedAppPackagerResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_checksums(root, &path, checksums)?;
        } else if entry.file_type()?.is_file() {
            let relative = normalize_archive_path(path.strip_prefix(root).map_err(|_| {
                PublishedAppPackagerError::Invalid("checksum path escape".to_string())
            })?);
            checksums.insert(relative, sha256_file(&path)?);
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> PublishedAppPackagerResult<String> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn zip_dir(source_dir: &Path, artifact_path: &Path) -> PublishedAppPackagerResult<()> {
    let file = fs::File::create(artifact_path)?;
    let mut writer = ZipWriter::new(file);
    zip_dir_inner(source_dir, source_dir, &mut writer)?;
    writer.finish()?;
    Ok(())
}

fn zip_dir_inner<W: Write + Seek>(
    root: &Path,
    current: &Path,
    writer: &mut ZipWriter<W>,
) -> PublishedAppPackagerResult<()> {
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            zip_dir_inner(root, &path, writer)?;
        } else if entry.file_type()?.is_file() {
            let archive_path =
                normalize_archive_path(path.strip_prefix(root).map_err(|_| {
                    PublishedAppPackagerError::Invalid("zip path escape".to_string())
                })?);
            writer.start_file(archive_path, options)?;
            let mut bytes = Vec::new();
            fs::File::open(&path)?.read_to_end(&mut bytes)?;
            writer.write_all(&bytes)?;
        }
    }
    Ok(())
}

fn normalize_archive_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::workspace_types::{WorkspaceConstraints, WorkspacePaths, WorkspacePreview};
    use std::collections::HashMap;

    #[test]
    fn sanitizes_release_names() {
        assert_eq!(sanitize_release_name(" Customer CRM! "), "customer-crm");
        assert_eq!(sanitize_release_name("!!!"), "sofvary-app");
    }

    #[test]
    fn package_excludes_node_modules_and_sqlite_data() {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path().join("workspace");
        let generated = root.join("generated");
        fs::create_dir_all(generated.join("react/node_modules/pkg")).expect("dirs");
        fs::create_dir_all(generated.join("data")).expect("data");
        fs::write(generated.join("react/index.html"), "html").expect("html");
        fs::write(generated.join("react/node_modules/pkg/index.js"), "pkg").expect("pkg");
        fs::write(generated.join("data/app.sqlite"), "private-data").expect("sqlite");
        fs::write(generated.join("data/seed.sql"), "seed").expect("seed");

        let output = temp.path().join("out");
        let result = create_published_app_package(PublishedAppPackageInput {
            manifest: test_manifest(root.clone(), generated),
            lockfile: SofvaryLockfile {
                client_version: "0.1.0".to_string(),
                runtime_packs: HashMap::new(),
                harness_packs: HashMap::new(),
                plugin_packs: HashMap::new(),
                pack_sources: HashMap::new(),
                agent_adapter: "mock".to_string(),
            },
            app_name: "CRM".to_string(),
            target_platform: "macos".to_string(),
            output_dir: output,
            icon_path: None,
            include_ai_continuation: true,
            stealth_ui: PublishedAppStealthUiSettings {
                ai_menu_label: "Tune this app".to_string(),
                ai_shortcut: "CmdOrCtrl+Shift+U".to_string(),
                ai_panel_title: "Tune app".to_string(),
                provider_setup_title: "Connect model".to_string(),
                prompt_placeholder: "What should change?".to_string(),
            },
        })
        .expect("package");

        assert!(result.artifact_path.exists());
        assert!(result
            .staging_dir
            .join("seed-workspace/generated/react/index.html")
            .exists());
        assert!(result
            .staging_dir
            .join("seed-workspace/generated/data/seed.sql")
            .exists());
        assert!(!result
            .staging_dir
            .join("seed-workspace/generated/data/app.sqlite")
            .exists());
        assert!(!result
            .staging_dir
            .join("seed-workspace/generated/react/node_modules/pkg/index.js")
            .exists());
        assert!(result.manifest.ai_continuation.enabled);
        assert_eq!(result.manifest.stealth_ui.ai_menu_label, "Tune this app");
        assert_eq!(result.manifest.stealth_ui.ai_shortcut, "CmdOrCtrl+Shift+U");
        assert!(!result.manifest.sofvary_shell_included);
    }

    #[test]
    fn native_macos_bundle_uses_structured_tauri_build_and_copies_outputs() {
        let temp = tempfile::tempdir().expect("temp");
        let staging_dir = temp.path().join("release-staging");
        let output_dir = temp.path().join("release-output");
        let host_template_dir = temp.path().join("host-template");
        fs::create_dir_all(staging_dir.join("seed-workspace/generated")).expect("staging");
        fs::write(staging_dir.join("published-app.json"), "{}\n").expect("manifest");
        write_minimal_host_template(&host_template_dir);

        let runner = RecordingNativeBundleRunner::new();
        let result = create_native_published_app_bundle(
            PublishedAppNativeBundleInput {
                app_name: "Customer CRM".to_string(),
                target_platform: "macos".to_string(),
                output_dir: output_dir.clone(),
                staging_dir,
                host_template_dir: host_template_dir.clone(),
                pnpm_executable: PathBuf::from("pnpm"),
                icon_path: None,
            },
            &runner,
        )
        .expect("native bundle");

        let command = runner.commands.borrow();
        assert_eq!(command.len(), 2);
        assert_eq!(command[0].executable, PathBuf::from("pnpm"));
        assert_eq!(command[0].args, ["build"].map(String::from));
        assert_eq!(command[1].executable, PathBuf::from("pnpm"));
        assert_eq!(
            command[1].args,
            ["tauri", "build", "--bundles", "app,dmg", "--ci", "--config"]
                .map(String::from)
                .into_iter()
                .chain(std::iter::once(command[1].args[6].clone()))
                .collect::<Vec<_>>()
        );
        assert!(!command[0].allowed_network);
        assert!(!command[1].allowed_network);
        let generated_config = fs::read_to_string(&command[1].args[6]).expect("generated config");
        let generated_config: Value = serde_json::from_str(&generated_config).expect("config json");
        assert_eq!(
            generated_config.pointer("/build/beforeBuildCommand"),
            Some(&Value::String(String::new()))
        );
        let csp = generated_config
            .pointer("/app/security/csp")
            .and_then(|value| value.as_str())
            .expect("csp");
        assert!(csp.contains("frame-src http://127.0.0.1:*"));
        assert!(csp.contains("child-src http://127.0.0.1:*"));
        let resources = generated_config
            .pointer("/bundle/resources")
            .and_then(|value| value.as_object())
            .expect("resources");
        assert!(resources.contains_key("target/sofvary-release-resources/customer-crm/"));
        assert!(host_template_dir
            .join("src-tauri/target/sofvary-release-resources/customer-crm/published-app.json")
            .exists());

        let app_path = result.app_bundle_path.expect("app bundle");
        let dmg_path = result.installer_path.expect("dmg");
        assert!(app_path.ends_with("Customer CRM.app"));
        assert!(dmg_path.ends_with("Customer CRM_0.1.0_aarch64.dmg"));
        assert!(app_path.exists());
        assert!(dmg_path.exists());
    }

    #[test]
    fn native_bundle_generates_tauri_icon_set_from_png_source() {
        let temp = tempfile::tempdir().expect("temp");
        let staging_dir = temp.path().join("release-staging");
        let output_dir = temp.path().join("release-output");
        let host_template_dir = temp.path().join("host-template");
        let icon_path = temp.path().join("app-icon.png");
        fs::create_dir_all(staging_dir.join("seed-workspace/generated")).expect("staging");
        fs::write(staging_dir.join("published-app.json"), "{}\n").expect("manifest");
        write_minimal_host_template(&host_template_dir);
        write_png_header(&icon_path, 512, 512);

        let runner = RecordingNativeBundleRunner::new();
        let result = create_native_published_app_bundle(
            PublishedAppNativeBundleInput {
                app_name: "Customer CRM".to_string(),
                target_platform: "macos".to_string(),
                output_dir,
                staging_dir,
                host_template_dir,
                pnpm_executable: PathBuf::from("pnpm"),
                icon_path: Some(icon_path.clone()),
            },
            &runner,
        )
        .expect("native bundle");

        let commands = runner.commands.borrow();
        assert_eq!(commands.len(), 3);
        assert_eq!(
            commands[0].args,
            vec![
                "tauri".to_string(),
                "icon".to_string(),
                icon_path.to_string_lossy().to_string(),
                "--output".to_string(),
                "src-tauri/target/sofvary-release-icons/customer-crm".to_string(),
            ]
        );
        let generated_config = fs::read_to_string(result.config_path).expect("generated config");
        let generated_config: Value = serde_json::from_str(&generated_config).expect("config json");
        let icon_paths = generated_config
            .pointer("/bundle/icon")
            .and_then(|value| value.as_array())
            .expect("bundle icons");
        assert!(icon_paths.iter().any(|value| {
            value
                .as_str()
                .unwrap_or_default()
                .ends_with("target/sofvary-release-icons/customer-crm/icon.icns")
        }));
    }

    #[test]
    fn native_bundle_rejects_small_png_icon_before_build() {
        let temp = tempfile::tempdir().expect("temp");
        let staging_dir = temp.path().join("release-staging");
        let output_dir = temp.path().join("release-output");
        let host_template_dir = temp.path().join("host-template");
        let icon_path = temp.path().join("small-icon.png");
        fs::create_dir_all(staging_dir.join("seed-workspace/generated")).expect("staging");
        fs::write(staging_dir.join("published-app.json"), "{}\n").expect("manifest");
        write_minimal_host_template(&host_template_dir);
        write_png_header(&icon_path, 256, 256);

        let runner = RecordingNativeBundleRunner::new();
        let error = create_native_published_app_bundle(
            PublishedAppNativeBundleInput {
                app_name: "Customer CRM".to_string(),
                target_platform: "macos".to_string(),
                output_dir,
                staging_dir,
                host_template_dir,
                pnpm_executable: PathBuf::from("pnpm"),
                icon_path: Some(icon_path),
            },
            &runner,
        )
        .expect_err("small icon should fail");

        assert!(error
            .to_string()
            .contains("Use a square PNG at least 512x512 px"));
        assert!(runner.commands.borrow().is_empty());
    }

    #[test]
    fn native_macos_bundle_rejects_windows_ico_icon_before_build() {
        let temp = tempfile::tempdir().expect("temp");
        let staging_dir = temp.path().join("release-staging");
        let output_dir = temp.path().join("release-output");
        let host_template_dir = temp.path().join("host-template");
        let icon_path = temp.path().join("app-icon.ico");
        fs::create_dir_all(staging_dir.join("seed-workspace/generated")).expect("staging");
        fs::write(staging_dir.join("published-app.json"), "{}\n").expect("manifest");
        fs::write(&icon_path, "ico").expect("ico");
        write_minimal_host_template(&host_template_dir);

        let runner = RecordingNativeBundleRunner::new();
        let error = create_native_published_app_bundle(
            PublishedAppNativeBundleInput {
                app_name: "Customer CRM".to_string(),
                target_platform: "macos".to_string(),
                output_dir,
                staging_dir,
                host_template_dir,
                pnpm_executable: PathBuf::from("pnpm"),
                icon_path: Some(icon_path),
            },
            &runner,
        )
        .expect_err("ico should fail for macOS");

        assert!(error
            .to_string()
            .contains("Windows .ico icon files can only be used when publishing for Windows"));
        assert!(runner.commands.borrow().is_empty());
    }

    #[test]
    fn native_bundle_fails_when_tauri_build_has_no_macos_app_output() {
        let temp = tempfile::tempdir().expect("temp");
        let staging_dir = temp.path().join("release-staging");
        let output_dir = temp.path().join("release-output");
        let host_template_dir = temp.path().join("host-template");
        fs::create_dir_all(staging_dir.join("seed-workspace/generated")).expect("staging");
        fs::write(staging_dir.join("published-app.json"), "{}\n").expect("manifest");
        write_minimal_host_template(&host_template_dir);

        let runner = RecordingNativeBundleRunner::without_outputs();
        let error = create_native_published_app_bundle(
            PublishedAppNativeBundleInput {
                app_name: "Customer CRM".to_string(),
                target_platform: "macos".to_string(),
                output_dir,
                staging_dir,
                host_template_dir,
                pnpm_executable: PathBuf::from("pnpm"),
                icon_path: None,
            },
            &runner,
        )
        .expect_err("missing native app should fail");

        assert!(error
            .to_string()
            .contains("native macOS bundle was not produced"));
    }

    #[test]
    fn process_summary_prefers_actionable_error_lines() {
        let output = ProcessOutput {
            status_code: Some(1),
            stdout: "Info Looking up installed tauri packages\nresource path `/Users/payhon/tmp` doesn't exist\nfailed to build app\n".to_string(),
            stderr: String::new(),
        };

        let summary = summarize_process_output(&output);

        assert!(summary.contains("resource path"));
        assert!(!summary.starts_with("Info Looking up"));
    }

    fn write_minimal_host_template(root: &Path) {
        fs::create_dir_all(root.join("src-tauri/src")).expect("src-tauri");
        fs::write(root.join("index.html"), "<div id=\"root\"></div>").expect("html");
        fs::write(
            root.join("package.json"),
            "{\"scripts\":{\"build\":\"true\"}}\n",
        )
        .expect("package");
        fs::write(
            root.join("src-tauri/Cargo.toml"),
            "[package]\nname=\"host\"\n",
        )
        .expect("cargo");
        fs::write(root.join("src-tauri/build.rs"), "fn main() {}\n").expect("build");
        fs::write(root.join("src-tauri/src/main.rs"), "fn main() {}\n").expect("main");
        fs::write(root.join("src-tauri/src/lib.rs"), "pub fn run() {}\n").expect("lib");
        fs::write(
            root.join("src-tauri/tauri.conf.json"),
            "{\"productName\":\"Host\",\"version\":\"0.1.0\",\"identifier\":\"com.test.host\",\"build\":{\"frontendDist\":\"../dist\"},\"bundle\":{\"active\":true}}\n",
        )
        .expect("config");
    }

    struct RecordingNativeBundleRunner {
        commands: std::cell::RefCell<Vec<CommandSpec>>,
        create_outputs: bool,
    }

    impl RecordingNativeBundleRunner {
        fn new() -> Self {
            Self {
                commands: std::cell::RefCell::new(Vec::new()),
                create_outputs: true,
            }
        }

        fn without_outputs() -> Self {
            Self {
                commands: std::cell::RefCell::new(Vec::new()),
                create_outputs: false,
            }
        }
    }

    impl NativeBundleRunner for RecordingNativeBundleRunner {
        fn run(&self, spec: CommandSpec) -> PublishedAppPackagerResult<ProcessOutput> {
            if spec.args.get(1).map(String::as_str) == Some("icon") {
                if let Some(output_arg) = spec.args.get(4) {
                    let icon_root = spec.cwd.join(output_arg);
                    fs::create_dir_all(&icon_root).expect("icon output");
                    for file_name in GENERATED_ICON_FILES {
                        fs::write(icon_root.join(file_name), "icon").expect("generated icon");
                    }
                }
            } else if spec.args.get(1).map(String::as_str) == Some("build") && self.create_outputs {
                let bundle_root = spec.cwd.join("src-tauri/target/release/bundle");
                let app_dir = bundle_root.join("macos/Customer CRM.app");
                fs::create_dir_all(&app_dir).expect("app bundle");
                fs::write(app_dir.join("Contents.txt"), "app").expect("app file");
                let dmg_dir = bundle_root.join("dmg");
                fs::create_dir_all(&dmg_dir).expect("dmg dir");
                fs::write(dmg_dir.join("Customer CRM_0.1.0_aarch64.dmg"), "dmg").expect("dmg");
            }
            self.commands.borrow_mut().push(spec);
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: "ok".to_string(),
                stderr: String::new(),
            })
        }
    }

    fn test_manifest(root: PathBuf, generated: PathBuf) -> AppBoxManifest {
        AppBoxManifest {
            app_id: "app_test".to_string(),
            name: "Test".to_string(),
            mode: "react-sqlite".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            stack: vec!["React".to_string()],
            paths: WorkspacePaths {
                root: root.clone(),
                generated,
                generated_static: root.join("generated/static"),
                runtime: root.join("runtime"),
                snapshots: root.join("snapshots"),
            },
            constraints: WorkspaceConstraints {
                boundary: root,
                allow_external_files: false,
                allow_remote_network: false,
            },
            preview: WorkspacePreview {
                state: "ready".to_string(),
                url: None,
            },
        }
    }

    fn write_png_header(path: &Path, width: u32, height: u32) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        fs::write(path, bytes).expect("png header");
    }
}
