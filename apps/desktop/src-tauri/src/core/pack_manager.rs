use crate::core::pack_types::{HarnessPackManifest, PluginPackManifest, RuntimePackManifest};
use crate::platform::{current_adapter, PlatformAdapter, PlatformError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use zip::ZipArchive;

pub const STATIC_HTML_RUNTIME_PACK_ID: &str = "sofvary.runtime.static-html";
pub const STATIC_HTML_HARNESS_PACK_ID: &str = "sofvary.harness.static-html";
pub const STATIC_HTML_PACK_VERSION: &str = "0.1.0";
pub const REACT_VITE_RUNTIME_PACK_ID: &str = "sofvary.runtime.react-vite";
pub const REACT_VITE_HARNESS_PACK_ID: &str = "sofvary.harness.react-vite";
pub const REACT_VITE_PACK_VERSION: &str = "0.1.0";
pub const REACT_SQLITE_RUNTIME_PACK_ID: &str = "sofvary.runtime.react-sqlite";
pub const REACT_SQLITE_HARNESS_PACK_ID: &str = "sofvary.harness.react-sqlite";
pub const REACT_SQLITE_PACK_VERSION: &str = "0.1.0";
pub const CANVAS2D_RUNTIME_PACK_ID: &str = "sofvary.runtime.canvas2d";
pub const CANVAS2D_HARNESS_PACK_ID: &str = "sofvary.harness.canvas2d";
pub const CANVAS2D_PACK_VERSION: &str = "0.1.0";
pub const MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID: &str = "sofvary.runtime.markdown-knowledge";
pub const MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID: &str = "sofvary.harness.markdown-knowledge";
pub const MARKDOWN_KNOWLEDGE_PACK_VERSION: &str = "0.1.0";
pub const DATA_TABLE_RUNTIME_PACK_ID: &str = "sofvary.runtime.data-table";
pub const DATA_TABLE_HARNESS_PACK_ID: &str = "sofvary.harness.data-table";
pub const DATA_TABLE_PACK_VERSION: &str = "0.1.0";
pub const FILE_PROCESSOR_RUNTIME_PACK_ID: &str = "sofvary.runtime.file-processor";
pub const FILE_PROCESSOR_HARNESS_PACK_ID: &str = "sofvary.harness.file-processor";
pub const FILE_PROCESSOR_PACK_VERSION: &str = "0.1.0";
pub const DESKTOP_WIDGET_RUNTIME_PACK_ID: &str = "sofvary.runtime.desktop-widget";
pub const DESKTOP_WIDGET_HARNESS_PACK_ID: &str = "sofvary.harness.desktop-widget";
pub const DESKTOP_WIDGET_PACK_VERSION: &str = "0.1.0";
pub const AI_AGENT_APP_RUNTIME_PACK_ID: &str = "sofvary.runtime.ai-agent-app";
#[allow(dead_code)]
pub const ARTICLE_AGENT_HARNESS_PACK_ID: &str = "sofvary.harness.article-agent";
#[allow(dead_code)]
pub const NOVEL_AGENT_HARNESS_PACK_ID: &str = "sofvary.harness.novel-agent";
#[allow(dead_code)]
pub const IMAGE_AGENT_HARNESS_PACK_ID: &str = "sofvary.harness.image-agent";
#[allow(dead_code)]
pub const VIDEO_AGENT_HARNESS_PACK_ID: &str = "sofvary.harness.video-agent";
pub const MULTIMODAL_STUDIO_AGENT_HARNESS_PACK_ID: &str = "sofvary.harness.multimodal-studio-agent";
pub const AI_AGENT_APP_HARNESS_PACK_ID: &str = MULTIMODAL_STUDIO_AGENT_HARNESS_PACK_ID;
pub const AI_AGENT_APP_PACK_VERSION: &str = "0.1.0";

const RUNTIME_PACK_TYPE: &str = "sofvary.runtime-pack";
const HARNESS_PACK_TYPE: &str = "sofvary.harness-pack";
const PLUGIN_PACK_TYPE: &str = "sofvary.plugin-pack";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const PACK_SCHEMA_VERSION: &str = "1.0";
const MAX_PACK_ARCHIVE_ENTRIES: usize = 256;
const MAX_PACK_MANIFEST_BYTES: u64 = 1024 * 1024;
const STATIC_HTML_RUNTIME_KIND: &str = "static-html";
const REACT_VITE_RUNTIME_KIND: &str = "react-vite";
const REACT_SQLITE_RUNTIME_KIND: &str = "react-sqlite";
const CANVAS2D_RUNTIME_KIND: &str = "canvas2d";
const MARKDOWN_KNOWLEDGE_RUNTIME_KIND: &str = "markdown-knowledge";
const DATA_TABLE_RUNTIME_KIND: &str = "data-table";
const FILE_PROCESSOR_RUNTIME_KIND: &str = "file-processor";
const DESKTOP_WIDGET_RUNTIME_KIND: &str = "desktop-widget";
const AI_AGENT_APP_RUNTIME_KIND: &str = "ai-agent-app";
const LOCAL_BIND_ADDR: &str = "127.0.0.1";
const LOCAL_ONLY_NETWORK: &str = "local-only";
const BUILTIN_STATIC_HTML_RUNTIME_MANIFEST: &str =
    include_str!("../../builtin-packs/runtimes/sofvary.runtime.static-html/0.1.0/manifest.json");
const BUILTIN_STATIC_HTML_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.static-html/0.1.0/manifest.json");
const BUILTIN_REACT_VITE_RUNTIME_MANIFEST: &str =
    include_str!("../../builtin-packs/runtimes/sofvary.runtime.react-vite/0.1.0/manifest.json");
const BUILTIN_REACT_VITE_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.react-vite/0.1.0/manifest.json");
const BUILTIN_REACT_SQLITE_RUNTIME_MANIFEST: &str =
    include_str!("../../builtin-packs/runtimes/sofvary.runtime.react-sqlite/0.1.0/manifest.json");
const BUILTIN_REACT_SQLITE_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.react-sqlite/0.1.0/manifest.json");
const BUILTIN_CANVAS2D_RUNTIME_MANIFEST: &str =
    include_str!("../../builtin-packs/runtimes/sofvary.runtime.canvas2d/0.1.0/manifest.json");
const BUILTIN_CANVAS2D_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.canvas2d/0.1.0/manifest.json");
const BUILTIN_MARKDOWN_KNOWLEDGE_RUNTIME_MANIFEST: &str = include_str!(
    "../../builtin-packs/runtimes/sofvary.runtime.markdown-knowledge/0.1.0/manifest.json"
);
const BUILTIN_MARKDOWN_KNOWLEDGE_HARNESS_MANIFEST: &str = include_str!(
    "../../builtin-packs/harness/sofvary.harness.markdown-knowledge/0.1.0/manifest.json"
);
const BUILTIN_DATA_TABLE_RUNTIME_MANIFEST: &str =
    include_str!("../../builtin-packs/runtimes/sofvary.runtime.data-table/0.1.0/manifest.json");
const BUILTIN_DATA_TABLE_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.data-table/0.1.0/manifest.json");
const BUILTIN_FILE_PROCESSOR_RUNTIME_MANIFEST: &str =
    include_str!("../../builtin-packs/runtimes/sofvary.runtime.file-processor/0.1.0/manifest.json");
const BUILTIN_FILE_PROCESSOR_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.file-processor/0.1.0/manifest.json");
const BUILTIN_DESKTOP_WIDGET_RUNTIME_MANIFEST: &str =
    include_str!("../../builtin-packs/runtimes/sofvary.runtime.desktop-widget/0.1.0/manifest.json");
const BUILTIN_DESKTOP_WIDGET_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.desktop-widget/0.1.0/manifest.json");
const BUILTIN_AI_AGENT_APP_RUNTIME_MANIFEST: &str =
    include_str!("../../builtin-packs/runtimes/sofvary.runtime.ai-agent-app/0.1.0/manifest.json");
const BUILTIN_ARTICLE_AGENT_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.article-agent/0.1.0/manifest.json");
const BUILTIN_NOVEL_AGENT_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.novel-agent/0.1.0/manifest.json");
const BUILTIN_IMAGE_AGENT_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.image-agent/0.1.0/manifest.json");
const BUILTIN_VIDEO_AGENT_HARNESS_MANIFEST: &str =
    include_str!("../../builtin-packs/harness/sofvary.harness.video-agent/0.1.0/manifest.json");
const BUILTIN_MULTIMODAL_STUDIO_AGENT_HARNESS_MANIFEST: &str = include_str!(
    "../../builtin-packs/harness/sofvary.harness.multimodal-studio-agent/0.1.0/manifest.json"
);

#[derive(Debug, Error)]
pub enum PackError {
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("pack path escapes local pack cache: {0}")]
    PathEscape(PathBuf),
    #[error("invalid pack id '{0}'")]
    InvalidPackId(String),
    #[error("invalid semver version '{0}'")]
    InvalidVersion(String),
    #[error("invalid pack manifest: {0}")]
    InvalidManifest(String),
    #[error("missing {kind} pack {id}@{version} in local cache")]
    MissingPack {
        kind: PackKind,
        id: String,
        version: String,
    },
    #[error("cached pack version is immutable and cannot be replaced: {0}")]
    ImmutableVersion(PathBuf),
    #[error("pack archive is invalid: {0}")]
    InvalidArchive(String),
}

pub type PackResult<T> = Result<T, PackError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackKind {
    Runtime,
    Harness,
    Plugin,
}

impl PackKind {
    pub fn cache_dir_name(self) -> &'static str {
        match self {
            Self::Runtime => "runtimes",
            Self::Harness => "harness",
            Self::Plugin => "plugins",
        }
    }

    fn manifest_type(self) -> &'static str {
        match self {
            Self::Runtime => RUNTIME_PACK_TYPE,
            Self::Harness => HARNESS_PACK_TYPE,
            Self::Plugin => PLUGIN_PACK_TYPE,
        }
    }

    pub fn from_manifest_type(pack_type: &str) -> PackResult<Self> {
        match pack_type {
            RUNTIME_PACK_TYPE => Ok(Self::Runtime),
            HARNESS_PACK_TYPE => Ok(Self::Harness),
            PLUGIN_PACK_TYPE => Ok(Self::Plugin),
            _ => Err(PackError::InvalidManifest(format!(
                "unsupported pack type '{pack_type}'"
            ))),
        }
    }
}

impl fmt::Display for PackKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime => write!(formatter, "runtime"),
            Self::Harness => write!(formatter, "harness"),
            Self::Plugin => write!(formatter, "plugin"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedPack<T> {
    pub manifest: T,
    #[allow(dead_code)]
    pub pack_dir: PathBuf,
    #[allow(dead_code)]
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LocalPackCache {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPackSummary {
    pub id: String,
    pub version: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub sha256: Option<String>,
    pub signature: Option<String>,
}

impl LocalPackCache {
    pub fn new(adapter: &dyn PlatformAdapter) -> PackResult<Self> {
        let root = adapter.dirs()?.data_dir.join("packs");
        Self::from_root(root)
    }

    #[cfg(test)]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn pack_dir(&self, kind: PackKind, id: &str, version: &str) -> PackResult<PathBuf> {
        validate_pack_id(id)?;
        validate_semver(version)?;

        let child = PathBuf::from(kind.cache_dir_name()).join(id).join(version);
        self.ensure_cache_child(&child)
    }

    fn from_root(root: PathBuf) -> PackResult<Self> {
        let cache = Self { root };
        cache.ensure_cache_dirs()?;
        Ok(cache)
    }

    fn ensure_cache_dirs(&self) -> PackResult<()> {
        for kind in [PackKind::Runtime, PackKind::Harness, PackKind::Plugin] {
            fs::create_dir_all(self.root.join(kind.cache_dir_name()))?;
        }
        Ok(())
    }

    fn install_runtime_manifest(
        &self,
        manifest: &RuntimePackManifest,
    ) -> PackResult<CachedPack<RuntimePackManifest>> {
        validate_runtime_manifest_fields(manifest)?;
        self.install_manifest(
            PackKind::Runtime,
            &manifest.pack_type,
            &manifest.id,
            &manifest.version,
            manifest,
        )?;
        self.read_runtime_manifest(&manifest.id, &manifest.version)
    }

    fn install_harness_manifest(
        &self,
        manifest: &HarnessPackManifest,
    ) -> PackResult<CachedPack<HarnessPackManifest>> {
        validate_harness_manifest_fields(manifest)?;
        self.install_manifest(
            PackKind::Harness,
            &manifest.pack_type,
            &manifest.id,
            &manifest.version,
            manifest,
        )?;
        self.read_harness_manifest(&manifest.id, &manifest.version)
    }

    fn install_manifest<T: Serialize>(
        &self,
        kind: PackKind,
        pack_type: &str,
        id: &str,
        version: &str,
        manifest: &T,
    ) -> PackResult<()> {
        validate_manifest_identity(kind, pack_type, id, version)?;
        let pack_dir = self.pack_dir(kind, id, version)?;
        let manifest_path = pack_dir.join(MANIFEST_FILE_NAME);
        let next_value = serde_json::to_value(manifest)?;

        if pack_dir.exists() {
            self.ensure_existing_path_inside_cache(&pack_dir)?;

            if !manifest_path.exists() {
                return Err(PackError::ImmutableVersion(pack_dir));
            }
            self.ensure_existing_path_inside_cache(&manifest_path)?;

            let existing_value: serde_json::Value =
                serde_json::from_slice(&fs::read(&manifest_path)?)?;
            if existing_value != next_value {
                return Err(PackError::ImmutableVersion(pack_dir));
            }

            return Ok(());
        }

        self.ensure_nearest_existing_ancestor_inside_cache(&pack_dir)?;
        fs::create_dir_all(&pack_dir)?;
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(manifest)? + "\n",
        )?;
        Ok(())
    }

    fn read_runtime_manifest(
        &self,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<RuntimePackManifest>> {
        self.read_manifest(PackKind::Runtime, id, version)
    }

    fn read_harness_manifest(
        &self,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<HarnessPackManifest>> {
        self.read_manifest(PackKind::Harness, id, version)
    }

    fn read_plugin_manifest(
        &self,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<PluginPackManifest>> {
        self.read_manifest(PackKind::Plugin, id, version)
    }

    pub fn install_pack_archive(
        &self,
        kind: PackKind,
        requested_id: &str,
        requested_version: &str,
        bytes: &[u8],
    ) -> PackResult<InstalledPackSummary> {
        let manifest_bytes = manifest_from_archive(bytes)?;
        match kind {
            PackKind::Runtime => {
                let manifest = parse_runtime_pack_manifest(
                    std::str::from_utf8(&manifest_bytes).map_err(|error| {
                        PackError::InvalidArchive(format!("manifest is not utf-8: {error}"))
                    })?,
                )?;
                validate_requested_pack(
                    &manifest.id,
                    &manifest.version,
                    requested_id,
                    requested_version,
                )?;
                let cached = self.install_runtime_manifest(&manifest)?;
                Ok(summary_from_runtime_manifest(&cached.manifest, "registry"))
            }
            PackKind::Harness => {
                let manifest = parse_harness_pack_manifest(
                    std::str::from_utf8(&manifest_bytes).map_err(|error| {
                        PackError::InvalidArchive(format!("manifest is not utf-8: {error}"))
                    })?,
                )?;
                validate_requested_pack(
                    &manifest.id,
                    &manifest.version,
                    requested_id,
                    requested_version,
                )?;
                let cached = self.install_harness_manifest(&manifest)?;
                Ok(summary_from_harness_manifest(&cached.manifest, "registry"))
            }
            PackKind::Plugin => {
                let manifest = parse_plugin_pack_manifest(
                    std::str::from_utf8(&manifest_bytes).map_err(|error| {
                        PackError::InvalidArchive(format!("manifest is not utf-8: {error}"))
                    })?,
                )?;
                validate_requested_pack(
                    &manifest.id,
                    &manifest.version,
                    requested_id,
                    requested_version,
                )?;
                self.install_manifest(
                    PackKind::Plugin,
                    &manifest.pack_type,
                    &manifest.id,
                    &manifest.version,
                    &manifest,
                )?;
                let cached = self.read_plugin_manifest(&manifest.id, &manifest.version)?;
                Ok(summary_from_plugin_manifest(&cached.manifest, "registry"))
            }
        }
    }

    pub fn list_installed_packs(&self) -> PackResult<Vec<InstalledPackSummary>> {
        let mut packs = Vec::new();
        for kind in [PackKind::Runtime, PackKind::Harness, PackKind::Plugin] {
            let kind_root = self.root.join(kind.cache_dir_name());
            if !kind_root.exists() {
                continue;
            }
            self.ensure_existing_path_inside_cache(&kind_root)?;
            for id_entry in fs::read_dir(&kind_root)? {
                let id_entry = id_entry?;
                if !id_entry.file_type()?.is_dir() {
                    continue;
                }
                let id = id_entry.file_name().to_string_lossy().to_string();
                if validate_pack_id(&id).is_err() {
                    continue;
                }
                for version_entry in fs::read_dir(id_entry.path())? {
                    let version_entry = version_entry?;
                    if !version_entry.file_type()?.is_dir() {
                        continue;
                    }
                    let version = version_entry.file_name().to_string_lossy().to_string();
                    if validate_semver(&version).is_err() {
                        continue;
                    }
                    match kind {
                        PackKind::Runtime => {
                            if let Ok(cached) = self.read_runtime_manifest(&id, &version) {
                                packs
                                    .push(summary_from_runtime_manifest(&cached.manifest, "cache"));
                            }
                        }
                        PackKind::Harness => {
                            if let Ok(cached) = self.read_harness_manifest(&id, &version) {
                                packs
                                    .push(summary_from_harness_manifest(&cached.manifest, "cache"));
                            }
                        }
                        PackKind::Plugin => {
                            if let Ok(cached) = self.read_plugin_manifest(&id, &version) {
                                packs.push(summary_from_plugin_manifest(&cached.manifest, "cache"));
                            }
                        }
                    }
                }
            }
        }
        packs.sort_by(|left, right| {
            kind_sort_key(&left.kind)
                .cmp(&kind_sort_key(&right.kind))
                .then(left.id.cmp(&right.id))
                .then(left.version.cmp(&right.version))
        });
        Ok(packs)
    }

    fn read_manifest<T: DeserializeOwned>(
        &self,
        kind: PackKind,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<T>> {
        let pack_dir = self.pack_dir(kind, id, version)?;
        let manifest_path = pack_dir.join(MANIFEST_FILE_NAME);
        if pack_dir.exists() {
            self.ensure_existing_path_inside_cache(&pack_dir)?;
        }
        if !manifest_path.exists() {
            return Err(PackError::MissingPack {
                kind,
                id: id.to_string(),
                version: version.to_string(),
            });
        }
        self.ensure_existing_path_inside_cache(&manifest_path)?;

        Ok(CachedPack {
            manifest: serde_json::from_slice(&fs::read(&manifest_path)?)?,
            pack_dir,
            manifest_path,
        })
    }

    fn ensure_cache_child(&self, child: &Path) -> PackResult<PathBuf> {
        let candidate = self.root.join(child);
        let normalized_root = normalize_for_boundary(&self.root);
        let normalized_candidate = normalize_for_boundary(&candidate);
        if normalized_candidate.starts_with(&normalized_root) {
            Ok(normalized_candidate)
        } else {
            Err(PackError::PathEscape(normalized_candidate))
        }
    }

    fn ensure_existing_path_inside_cache(&self, path: &Path) -> PackResult<()> {
        let canonical_root = self.root.canonicalize()?;
        let canonical_path = path.canonicalize()?;
        if canonical_path.starts_with(canonical_root) {
            Ok(())
        } else {
            Err(PackError::PathEscape(canonical_path))
        }
    }

    fn ensure_nearest_existing_ancestor_inside_cache(&self, path: &Path) -> PackResult<()> {
        let mut ancestor = path;
        while !ancestor.exists() {
            ancestor = ancestor.parent().ok_or_else(|| {
                PackError::InvalidManifest("pack path has no existing cache ancestor".to_string())
            })?;
        }
        self.ensure_existing_path_inside_cache(ancestor)
    }
}

#[derive(Debug, Clone)]
pub struct PackResolver {
    cache: LocalPackCache,
}

impl PackResolver {
    pub fn new(cache: LocalPackCache) -> Self {
        Self { cache }
    }

    pub fn resolve_runtime(
        &self,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<RuntimePackManifest>> {
        let cached = self.cache.read_runtime_manifest(id, version)?;
        validate_manifest_identity(
            PackKind::Runtime,
            &cached.manifest.pack_type,
            &cached.manifest.id,
            &cached.manifest.version,
        )?;
        validate_runtime_manifest_fields(&cached.manifest)?;
        validate_requested_pack(&cached.manifest.id, &cached.manifest.version, id, version)?;
        Ok(cached)
    }

    pub fn resolve_harness(
        &self,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<HarnessPackManifest>> {
        let cached = self.cache.read_harness_manifest(id, version)?;
        validate_manifest_identity(
            PackKind::Harness,
            &cached.manifest.pack_type,
            &cached.manifest.id,
            &cached.manifest.version,
        )?;
        validate_harness_manifest_fields(&cached.manifest)?;
        validate_requested_pack(&cached.manifest.id, &cached.manifest.version, id, version)?;
        Ok(cached)
    }

    pub fn resolve_plugin(
        &self,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<PluginPackManifest>> {
        let cached = self.cache.read_plugin_manifest(id, version)?;
        validate_manifest_identity(
            PackKind::Plugin,
            &cached.manifest.pack_type,
            &cached.manifest.id,
            &cached.manifest.version,
        )?;
        validate_plugin_manifest_fields(&cached.manifest)?;
        validate_requested_pack(&cached.manifest.id, &cached.manifest.version, id, version)?;
        Ok(cached)
    }
}

pub struct StaticHtmlPackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct ReactVitePackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct ReactSqlitePackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct Canvas2dPackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct MarkdownKnowledgePackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct DataTablePackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct FileProcessorPackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct DesktopWidgetPackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct AiAgentAppPackResolution {
    pub runtime: CachedPack<RuntimePackManifest>,
    pub harness: CachedPack<HarnessPackManifest>,
}

pub struct PackManager {
    cache: LocalPackCache,
    resolver: PackResolver,
}

impl PackManager {
    pub fn new() -> PackResult<Self> {
        let adapter = current_adapter();
        Self::new_with_adapter(adapter.as_ref())
    }

    pub fn new_with_adapter(adapter: &dyn PlatformAdapter) -> PackResult<Self> {
        let cache = LocalPackCache::new(adapter)?;
        Self::new_with_cache(cache)
    }

    pub fn resolve_static_html_packs(&self) -> PackResult<StaticHtmlPackResolution> {
        Ok(StaticHtmlPackResolution {
            runtime: self
                .resolver
                .resolve_runtime(STATIC_HTML_RUNTIME_PACK_ID, STATIC_HTML_PACK_VERSION)?,
            harness: self
                .resolver
                .resolve_harness(STATIC_HTML_HARNESS_PACK_ID, STATIC_HTML_PACK_VERSION)?,
        })
    }

    pub fn resolve_react_vite_packs(&self) -> PackResult<ReactVitePackResolution> {
        Ok(ReactVitePackResolution {
            runtime: self
                .resolver
                .resolve_runtime(REACT_VITE_RUNTIME_PACK_ID, REACT_VITE_PACK_VERSION)?,
            harness: self
                .resolver
                .resolve_harness(REACT_VITE_HARNESS_PACK_ID, REACT_VITE_PACK_VERSION)?,
        })
    }

    pub fn resolve_react_sqlite_packs(&self) -> PackResult<ReactSqlitePackResolution> {
        Ok(ReactSqlitePackResolution {
            runtime: self
                .resolver
                .resolve_runtime(REACT_SQLITE_RUNTIME_PACK_ID, REACT_SQLITE_PACK_VERSION)?,
            harness: self
                .resolver
                .resolve_harness(REACT_SQLITE_HARNESS_PACK_ID, REACT_SQLITE_PACK_VERSION)?,
        })
    }

    pub fn resolve_canvas2d_packs(&self) -> PackResult<Canvas2dPackResolution> {
        Ok(Canvas2dPackResolution {
            runtime: self
                .resolver
                .resolve_runtime(CANVAS2D_RUNTIME_PACK_ID, CANVAS2D_PACK_VERSION)?,
            harness: self
                .resolver
                .resolve_harness(CANVAS2D_HARNESS_PACK_ID, CANVAS2D_PACK_VERSION)?,
        })
    }

    pub fn resolve_markdown_knowledge_packs(&self) -> PackResult<MarkdownKnowledgePackResolution> {
        Ok(MarkdownKnowledgePackResolution {
            runtime: self.resolver.resolve_runtime(
                MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID,
                MARKDOWN_KNOWLEDGE_PACK_VERSION,
            )?,
            harness: self.resolver.resolve_harness(
                MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID,
                MARKDOWN_KNOWLEDGE_PACK_VERSION,
            )?,
        })
    }

    pub fn resolve_data_table_packs(&self) -> PackResult<DataTablePackResolution> {
        Ok(DataTablePackResolution {
            runtime: self
                .resolver
                .resolve_runtime(DATA_TABLE_RUNTIME_PACK_ID, DATA_TABLE_PACK_VERSION)?,
            harness: self
                .resolver
                .resolve_harness(DATA_TABLE_HARNESS_PACK_ID, DATA_TABLE_PACK_VERSION)?,
        })
    }

    pub fn resolve_file_processor_packs(&self) -> PackResult<FileProcessorPackResolution> {
        Ok(FileProcessorPackResolution {
            runtime: self
                .resolver
                .resolve_runtime(FILE_PROCESSOR_RUNTIME_PACK_ID, FILE_PROCESSOR_PACK_VERSION)?,
            harness: self
                .resolver
                .resolve_harness(FILE_PROCESSOR_HARNESS_PACK_ID, FILE_PROCESSOR_PACK_VERSION)?,
        })
    }

    pub fn resolve_desktop_widget_packs(&self) -> PackResult<DesktopWidgetPackResolution> {
        Ok(DesktopWidgetPackResolution {
            runtime: self
                .resolver
                .resolve_runtime(DESKTOP_WIDGET_RUNTIME_PACK_ID, DESKTOP_WIDGET_PACK_VERSION)?,
            harness: self
                .resolver
                .resolve_harness(DESKTOP_WIDGET_HARNESS_PACK_ID, DESKTOP_WIDGET_PACK_VERSION)?,
        })
    }

    pub fn resolve_ai_agent_app_packs(&self) -> PackResult<AiAgentAppPackResolution> {
        Ok(AiAgentAppPackResolution {
            runtime: self
                .resolver
                .resolve_runtime(AI_AGENT_APP_RUNTIME_PACK_ID, AI_AGENT_APP_PACK_VERSION)?,
            harness: self
                .resolver
                .resolve_harness(AI_AGENT_APP_HARNESS_PACK_ID, AI_AGENT_APP_PACK_VERSION)?,
        })
    }

    #[cfg(test)]
    pub fn cache_root(&self) -> &Path {
        self.cache.root()
    }

    pub fn resolver(&self) -> &PackResolver {
        &self.resolver
    }

    pub fn install_pack_archive(
        &self,
        kind: PackKind,
        id: &str,
        version: &str,
        bytes: &[u8],
    ) -> PackResult<InstalledPackSummary> {
        self.cache.install_pack_archive(kind, id, version, bytes)
    }

    pub fn list_installed_packs(&self) -> PackResult<Vec<InstalledPackSummary>> {
        self.cache.list_installed_packs()
    }

    fn new_with_cache(cache: LocalPackCache) -> PackResult<Self> {
        let manager = Self {
            resolver: PackResolver::new(cache.clone()),
            cache,
        };
        manager.install_builtins()?;
        Ok(manager)
    }

    fn install_builtins(&self) -> PackResult<()> {
        let runtime = parse_runtime_pack_manifest(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST)?;
        let harness = parse_harness_pack_manifest(BUILTIN_STATIC_HTML_HARNESS_MANIFEST)?;
        let react_runtime = parse_runtime_pack_manifest(BUILTIN_REACT_VITE_RUNTIME_MANIFEST)?;
        let react_harness = parse_harness_pack_manifest(BUILTIN_REACT_VITE_HARNESS_MANIFEST)?;
        let react_sqlite_runtime =
            parse_runtime_pack_manifest(BUILTIN_REACT_SQLITE_RUNTIME_MANIFEST)?;
        let react_sqlite_harness =
            parse_harness_pack_manifest(BUILTIN_REACT_SQLITE_HARNESS_MANIFEST)?;
        let canvas2d_runtime = parse_runtime_pack_manifest(BUILTIN_CANVAS2D_RUNTIME_MANIFEST)?;
        let canvas2d_harness = parse_harness_pack_manifest(BUILTIN_CANVAS2D_HARNESS_MANIFEST)?;
        let markdown_knowledge_runtime =
            parse_runtime_pack_manifest(BUILTIN_MARKDOWN_KNOWLEDGE_RUNTIME_MANIFEST)?;
        let markdown_knowledge_harness =
            parse_harness_pack_manifest(BUILTIN_MARKDOWN_KNOWLEDGE_HARNESS_MANIFEST)?;
        let data_table_runtime = parse_runtime_pack_manifest(BUILTIN_DATA_TABLE_RUNTIME_MANIFEST)?;
        let data_table_harness = parse_harness_pack_manifest(BUILTIN_DATA_TABLE_HARNESS_MANIFEST)?;
        let file_processor_runtime =
            parse_runtime_pack_manifest(BUILTIN_FILE_PROCESSOR_RUNTIME_MANIFEST)?;
        let file_processor_harness =
            parse_harness_pack_manifest(BUILTIN_FILE_PROCESSOR_HARNESS_MANIFEST)?;
        let desktop_widget_runtime =
            parse_runtime_pack_manifest(BUILTIN_DESKTOP_WIDGET_RUNTIME_MANIFEST)?;
        let desktop_widget_harness =
            parse_harness_pack_manifest(BUILTIN_DESKTOP_WIDGET_HARNESS_MANIFEST)?;
        let ai_agent_app_runtime =
            parse_runtime_pack_manifest(BUILTIN_AI_AGENT_APP_RUNTIME_MANIFEST)?;
        let article_agent_harness =
            parse_harness_pack_manifest(BUILTIN_ARTICLE_AGENT_HARNESS_MANIFEST)?;
        let novel_agent_harness =
            parse_harness_pack_manifest(BUILTIN_NOVEL_AGENT_HARNESS_MANIFEST)?;
        let image_agent_harness =
            parse_harness_pack_manifest(BUILTIN_IMAGE_AGENT_HARNESS_MANIFEST)?;
        let video_agent_harness =
            parse_harness_pack_manifest(BUILTIN_VIDEO_AGENT_HARNESS_MANIFEST)?;
        let multimodal_studio_agent_harness =
            parse_harness_pack_manifest(BUILTIN_MULTIMODAL_STUDIO_AGENT_HARNESS_MANIFEST)?;

        self.cache.install_runtime_manifest(&runtime)?;
        self.cache.install_harness_manifest(&harness)?;
        self.cache.install_runtime_manifest(&react_runtime)?;
        self.cache.install_harness_manifest(&react_harness)?;
        self.cache.install_runtime_manifest(&react_sqlite_runtime)?;
        self.cache.install_harness_manifest(&react_sqlite_harness)?;
        self.cache.install_runtime_manifest(&canvas2d_runtime)?;
        self.cache.install_harness_manifest(&canvas2d_harness)?;
        self.cache
            .install_runtime_manifest(&markdown_knowledge_runtime)?;
        self.cache
            .install_harness_manifest(&markdown_knowledge_harness)?;
        self.cache.install_runtime_manifest(&data_table_runtime)?;
        self.cache.install_harness_manifest(&data_table_harness)?;
        self.cache
            .install_runtime_manifest(&file_processor_runtime)?;
        self.cache
            .install_harness_manifest(&file_processor_harness)?;
        self.cache
            .install_runtime_manifest(&desktop_widget_runtime)?;
        self.cache
            .install_harness_manifest(&desktop_widget_harness)?;
        self.cache.install_runtime_manifest(&ai_agent_app_runtime)?;
        self.cache
            .install_harness_manifest(&article_agent_harness)?;
        self.cache.install_harness_manifest(&novel_agent_harness)?;
        self.cache.install_harness_manifest(&image_agent_harness)?;
        self.cache.install_harness_manifest(&video_agent_harness)?;
        self.cache
            .install_harness_manifest(&multimodal_studio_agent_harness)?;
        Ok(())
    }
}

pub fn parse_runtime_pack_manifest(raw: &str) -> PackResult<RuntimePackManifest> {
    let manifest: RuntimePackManifest = serde_json::from_str(raw)?;
    validate_manifest_identity(
        PackKind::Runtime,
        &manifest.pack_type,
        &manifest.id,
        &manifest.version,
    )?;
    validate_runtime_manifest_fields(&manifest)?;
    Ok(manifest)
}

pub fn parse_harness_pack_manifest(raw: &str) -> PackResult<HarnessPackManifest> {
    let manifest: HarnessPackManifest = serde_json::from_str(raw)?;
    validate_manifest_identity(
        PackKind::Harness,
        &manifest.pack_type,
        &manifest.id,
        &manifest.version,
    )?;
    validate_harness_manifest_fields(&manifest)?;
    Ok(manifest)
}

pub fn parse_plugin_pack_manifest(raw: &str) -> PackResult<PluginPackManifest> {
    let manifest: PluginPackManifest = serde_json::from_str(raw)?;
    validate_manifest_identity(
        PackKind::Plugin,
        &manifest.pack_type,
        &manifest.id,
        &manifest.version,
    )?;
    validate_plugin_manifest_fields(&manifest)?;
    Ok(manifest)
}

fn validate_manifest_identity(
    kind: PackKind,
    pack_type: &str,
    id: &str,
    version: &str,
) -> PackResult<()> {
    if pack_type != kind.manifest_type() {
        return Err(PackError::InvalidManifest(format!(
            "expected {} but found {}",
            kind.manifest_type(),
            pack_type
        )));
    }
    validate_pack_id(id)?;
    validate_semver(version)?;
    Ok(())
}

fn validate_runtime_manifest_fields(manifest: &RuntimePackManifest) -> PackResult<()> {
    validate_schema_version(&manifest.schema_version)?;
    validate_compatibility(&manifest.compatibility.arch)?;
    validate_relative_path("runtime.generatedRoot", &manifest.runtime.generated_root)?;
    validate_relative_path("runtime.entrypoint", &manifest.runtime.entrypoint)?;

    if !matches!(
        manifest.runtime.kind.as_str(),
        STATIC_HTML_RUNTIME_KIND
            | REACT_VITE_RUNTIME_KIND
            | REACT_SQLITE_RUNTIME_KIND
            | CANVAS2D_RUNTIME_KIND
            | MARKDOWN_KNOWLEDGE_RUNTIME_KIND
            | DATA_TABLE_RUNTIME_KIND
            | FILE_PROCESSOR_RUNTIME_KIND
            | DESKTOP_WIDGET_RUNTIME_KIND
            | AI_AGENT_APP_RUNTIME_KIND
    ) {
        return Err(PackError::InvalidManifest(format!(
            "unsupported runtime kind '{}'",
            manifest.runtime.kind
        )));
    }

    if manifest.runtime.bind != LOCAL_BIND_ADDR {
        return Err(PackError::InvalidManifest(format!(
            "runtime bind must be {LOCAL_BIND_ADDR}, found '{}'",
            manifest.runtime.bind
        )));
    }

    if manifest.runtime.network != LOCAL_ONLY_NETWORK {
        return Err(PackError::InvalidManifest(format!(
            "runtime network must be {LOCAL_ONLY_NETWORK}, found '{}'",
            manifest.runtime.network
        )));
    }

    for harness_id in &manifest.default_harness {
        validate_pack_id(harness_id)?;
    }

    for (name, command) in &manifest.commands {
        validate_manifest_name("command name", name)?;
        validate_command_spec(command)?;
    }

    validate_integrity(manifest.integrity.as_ref())
}

fn validate_harness_manifest_fields(manifest: &HarnessPackManifest) -> PackResult<()> {
    validate_schema_version(&manifest.schema_version)?;
    validate_compatibility(&manifest.compatibility.arch)?;

    if let Some(runtime_id) = &manifest.runtime {
        validate_pack_id(runtime_id)?;
    }

    validate_string_list("instructions.system", &manifest.instructions.system)?;
    validate_string_list(
        "instructions.fileSystemPolicy",
        &manifest.instructions.file_system_policy,
    )?;
    validate_string_list(
        "instructions.outputRules",
        &manifest.instructions.output_rules,
    )?;
    if let Some(species) = &manifest.software_species {
        validate_manifest_name("softwareSpecies.id", &species.id)?;
        validate_manifest_name("softwareSpecies.name", &species.name)?;
        for runtime_kind in &species.runtime_kinds {
            if !matches!(
                runtime_kind.as_str(),
                STATIC_HTML_RUNTIME_KIND
                    | REACT_VITE_RUNTIME_KIND
                    | REACT_SQLITE_RUNTIME_KIND
                    | CANVAS2D_RUNTIME_KIND
                    | MARKDOWN_KNOWLEDGE_RUNTIME_KIND
                    | DATA_TABLE_RUNTIME_KIND
                    | FILE_PROCESSOR_RUNTIME_KIND
                    | DESKTOP_WIDGET_RUNTIME_KIND
                    | AI_AGENT_APP_RUNTIME_KIND
            ) {
                return Err(PackError::InvalidManifest(format!(
                    "unsupported softwareSpecies runtime kind '{runtime_kind}'"
                )));
            }
        }
        validate_string_list("softwareSpecies.tags", &species.tags)?;
    }
    validate_integrity(manifest.integrity.as_ref())
}

fn validate_plugin_manifest_fields(manifest: &PluginPackManifest) -> PackResult<()> {
    validate_schema_version(&manifest.schema_version)?;
    validate_compatibility(&manifest.compatibility.arch)?;
    validate_string_list("capabilities", &manifest.capabilities)?;

    if let Some(entry) = &manifest.entry {
        validate_command_spec(entry)?;
    }

    validate_integrity(manifest.integrity.as_ref())
}

fn validate_schema_version(schema_version: &str) -> PackResult<()> {
    if schema_version == PACK_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(PackError::InvalidManifest(format!(
            "schemaVersion must be {PACK_SCHEMA_VERSION}, found '{schema_version}'"
        )))
    }
}

fn validate_compatibility(arch: &[String]) -> PackResult<()> {
    if arch.is_empty() {
        return Err(PackError::InvalidManifest(
            "compatibility.arch must not be empty".to_string(),
        ));
    }

    for value in arch {
        match value.as_str() {
            "x64" | "arm64" | "unknown" => {}
            _ => {
                return Err(PackError::InvalidManifest(format!(
                    "unsupported compatibility arch '{value}'"
                )));
            }
        }
    }

    Ok(())
}

fn validate_command_spec(command: &crate::core::pack_types::PackCommandSpec) -> PackResult<()> {
    validate_sidecar_executable("command executable", &command.executable)?;

    if let Some(cwd) = &command.cwd {
        validate_relative_path("command.cwd", cwd)?;
    }

    if command.allowed_network.unwrap_or(false) {
        return Err(PackError::InvalidManifest(
            "command.allowedNetwork must be false for local-only packs".to_string(),
        ));
    }

    if command.timeout_ms == Some(0) {
        return Err(PackError::InvalidManifest(
            "command.timeoutMs must be greater than zero when provided".to_string(),
        ));
    }

    Ok(())
}

fn validate_sidecar_executable(field: &str, value: &str) -> PackResult<()> {
    validate_manifest_name(field, value)?;

    let Some(name) = value
        .strip_prefix("${sidecar:")
        .and_then(|name| name.strip_suffix('}'))
    else {
        return Err(PackError::InvalidManifest(format!(
            "{field} must use a sidecar placeholder such as ${{sidecar:pnpm}}, found '{value}'"
        )));
    };

    match name {
        "node" | "pnpm" => Ok(()),
        _ => Err(PackError::InvalidManifest(format!(
            "{field} references unsupported sidecar '{name}'"
        ))),
    }
}

fn validate_integrity(
    integrity: Option<&crate::core::pack_types::PackIntegrity>,
) -> PackResult<()> {
    let Some(integrity) = integrity else {
        return Ok(());
    };

    if let Some(sha256) = &integrity.sha256 {
        if sha256.len() != 64 || !sha256.chars().all(|value| value.is_ascii_hexdigit()) {
            return Err(PackError::InvalidManifest(
                "integrity.sha256 must be 64 hex characters".to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_string_list(field: &str, values: &[String]) -> PackResult<()> {
    for value in values {
        validate_manifest_name(field, value)?;
    }
    Ok(())
}

fn validate_manifest_name(field: &str, value: &str) -> PackResult<()> {
    if value.trim().is_empty() {
        Err(PackError::InvalidManifest(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn validate_relative_path(field: &str, value: &str) -> PackResult<()> {
    if value.trim().is_empty() {
        return Err(PackError::InvalidManifest(format!(
            "{field} must not be empty"
        )));
    }

    if value.contains('\\') {
        return Err(PackError::InvalidManifest(format!(
            "{field} must use forward slash relative paths, found '{value}'"
        )));
    }

    if looks_like_windows_absolute_path(value) {
        return Err(PackError::InvalidManifest(format!(
            "{field} must be relative, found '{value}'"
        )));
    }

    let path = Path::new(value);
    if path.is_absolute() {
        return Err(PackError::InvalidManifest(format!(
            "{field} must be relative, found '{value}'"
        )));
    }

    if value.split('/').any(|segment| segment == "..")
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(PackError::InvalidManifest(format!(
            "{field} must stay inside the pack/workspace boundary, found '{value}'"
        )));
    }

    Ok(())
}

fn looks_like_windows_absolute_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
}

fn validate_requested_pack(
    actual_id: &str,
    actual_version: &str,
    requested_id: &str,
    requested_version: &str,
) -> PackResult<()> {
    if actual_id == requested_id && actual_version == requested_version {
        Ok(())
    } else {
        Err(PackError::InvalidManifest(format!(
            "cached manifest identity {}@{} does not match requested {}@{}",
            actual_id, actual_version, requested_id, requested_version
        )))
    }
}

fn validate_pack_id(id: &str) -> PackResult<()> {
    if id.trim() != id || !id.contains('.') {
        return Err(PackError::InvalidPackId(id.to_string()));
    }

    for segment in id.split('.') {
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            return Err(PackError::InvalidPackId(id.to_string()));
        };

        if !first.is_ascii_alphanumeric() {
            return Err(PackError::InvalidPackId(id.to_string()));
        }

        if chars.any(|value| !(value.is_ascii_alphanumeric() || value == '-' || value == '_')) {
            return Err(PackError::InvalidPackId(id.to_string()));
        }
    }

    Ok(())
}

fn validate_semver(version: &str) -> PackResult<()> {
    if is_semver(version) {
        Ok(())
    } else {
        Err(PackError::InvalidVersion(version.to_string()))
    }
}

fn is_semver(version: &str) -> bool {
    let (without_build, build) = split_once_optional(version, '+');
    if let Some(build) = build {
        if !valid_identifiers(build, false) {
            return false;
        }
    }

    let (core, prerelease) = split_once_optional(without_build, '-');
    if let Some(prerelease) = prerelease {
        if !valid_identifiers(prerelease, true) {
            return false;
        }
    }

    let parts = core.split('.').collect::<Vec<_>>();
    parts.len() == 3 && parts.iter().all(|part| valid_numeric_identifier(part))
}

fn split_once_optional(input: &str, delimiter: char) -> (&str, Option<&str>) {
    match input.split_once(delimiter) {
        Some((left, right)) => (left, Some(right)),
        None => (input, None),
    }
}

fn valid_numeric_identifier(value: &str) -> bool {
    if value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    value == "0" || !value.starts_with('0')
}

fn valid_identifiers(value: &str, reject_numeric_leading_zero: bool) -> bool {
    if value.is_empty() {
        return false;
    }

    value.split('.').all(|identifier| {
        !identifier.is_empty()
            && identifier
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
            && (!reject_numeric_leading_zero
                || !identifier.chars().all(|ch| ch.is_ascii_digit())
                || valid_numeric_identifier(identifier))
    })
}

fn normalize_for_boundary(path: &Path) -> PathBuf {
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

fn manifest_from_archive(bytes: &[u8]) -> PackResult<Vec<u8>> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;
    if archive.len() > MAX_PACK_ARCHIVE_ENTRIES {
        return Err(PackError::InvalidArchive(format!(
            "pack archive contains too many entries: {} > {}",
            archive.len(),
            MAX_PACK_ARCHIVE_ENTRIES
        )));
    }

    let mut manifest = None;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        validate_archive_entry_path(&name)?;
        if entry.is_dir() {
            continue;
        }
        if name == MANIFEST_FILE_NAME {
            if manifest.is_some() {
                return Err(PackError::InvalidArchive(
                    "pack archive must include exactly one root manifest.json".to_string(),
                ));
            }
            if entry.size() > MAX_PACK_MANIFEST_BYTES {
                return Err(PackError::InvalidArchive(format!(
                    "pack manifest exceeds the limit of {MAX_PACK_MANIFEST_BYTES} bytes"
                )));
            }
            manifest = Some(read_pack_manifest_entry_limited(&mut entry)?);
        }
    }

    manifest.ok_or_else(|| {
        PackError::InvalidArchive("pack archive must include manifest.json at the root".to_string())
    })
}

fn read_pack_manifest_entry_limited<R: Read>(reader: &mut R) -> PackResult<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_PACK_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PACK_MANIFEST_BYTES {
        return Err(PackError::InvalidArchive(format!(
            "pack manifest exceeds the limit of {MAX_PACK_MANIFEST_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

fn validate_archive_entry_path(name: &str) -> PackResult<()> {
    if name.trim().is_empty() || name.contains('\\') || looks_like_windows_absolute_path(name) {
        return Err(PackError::InvalidArchive(format!(
            "unsafe archive entry path '{name}'"
        )));
    }

    let path = Path::new(name);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || name
            .split('/')
            .any(|segment| segment == ".." || segment.is_empty())
    {
        return Err(PackError::InvalidArchive(format!(
            "archive entry escapes pack cache: {name}"
        )));
    }

    Ok(())
}

fn summary_from_runtime_manifest(
    manifest: &RuntimePackManifest,
    source: &str,
) -> InstalledPackSummary {
    InstalledPackSummary {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        kind: "runtime".to_string(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        source: source.to_string(),
        sha256: manifest
            .integrity
            .as_ref()
            .and_then(|integrity| integrity.sha256.clone()),
        signature: manifest
            .integrity
            .as_ref()
            .and_then(|integrity| integrity.signature.clone()),
    }
}

fn summary_from_harness_manifest(
    manifest: &HarnessPackManifest,
    source: &str,
) -> InstalledPackSummary {
    InstalledPackSummary {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        kind: "harness".to_string(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        source: source.to_string(),
        sha256: manifest
            .integrity
            .as_ref()
            .and_then(|integrity| integrity.sha256.clone()),
        signature: manifest
            .integrity
            .as_ref()
            .and_then(|integrity| integrity.signature.clone()),
    }
}

fn summary_from_plugin_manifest(
    manifest: &PluginPackManifest,
    source: &str,
) -> InstalledPackSummary {
    InstalledPackSummary {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        kind: "plugin".to_string(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        source: source.to_string(),
        sha256: manifest
            .integrity
            .as_ref()
            .and_then(|integrity| integrity.sha256.clone()),
        signature: manifest
            .integrity
            .as_ref()
            .and_then(|integrity| integrity.signature.clone()),
    }
}

fn kind_sort_key(kind: &str) -> u8 {
    match kind {
        "runtime" => 0,
        "harness" => 1,
        "plugin" => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_pack_manager() -> (tempfile::TempDir, PackManager) {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("data").join("packs");
        let manager = PackManager::new_with_cache(LocalPackCache::from_root(root).expect("cache"))
            .expect("pack manager");
        (temp, manager)
    }

    fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            zip.start_file(*name, options).expect("start file");
            std::io::Write::write_all(&mut zip, bytes).expect("write file");
        }
        zip.finish().expect("finish zip").into_inner()
    }

    #[test]
    fn parses_runtime_pack_manifest() {
        let manifest =
            parse_runtime_pack_manifest(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, STATIC_HTML_RUNTIME_PACK_ID);
        assert_eq!(manifest.version, STATIC_HTML_PACK_VERSION);
        assert_eq!(manifest.runtime.kind, "static-html");
        assert_eq!(manifest.default_harness, [STATIC_HTML_HARNESS_PACK_ID]);
    }

    #[test]
    fn parses_react_vite_runtime_pack_manifest() {
        let manifest =
            parse_runtime_pack_manifest(BUILTIN_REACT_VITE_RUNTIME_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, REACT_VITE_RUNTIME_PACK_ID);
        assert_eq!(manifest.version, REACT_VITE_PACK_VERSION);
        assert_eq!(manifest.runtime.kind, "react-vite");
        assert_eq!(manifest.runtime.generated_root, "generated/react");
        assert_eq!(manifest.default_harness, [REACT_VITE_HARNESS_PACK_ID]);
        assert_eq!(
            manifest
                .commands
                .get("dev")
                .map(|command| command.executable.as_str()),
            Some("${sidecar:pnpm}")
        );
    }

    #[test]
    fn parses_react_sqlite_runtime_pack_manifest() {
        let manifest =
            parse_runtime_pack_manifest(BUILTIN_REACT_SQLITE_RUNTIME_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, REACT_SQLITE_RUNTIME_PACK_ID);
        assert_eq!(manifest.version, REACT_SQLITE_PACK_VERSION);
        assert_eq!(manifest.runtime.kind, "react-sqlite");
        assert_eq!(manifest.runtime.generated_root, "generated");
        assert_eq!(manifest.runtime.entrypoint, "react/src/main.tsx");
        assert_eq!(manifest.default_harness, [REACT_SQLITE_HARNESS_PACK_ID]);
        assert_eq!(
            manifest
                .commands
                .get("api")
                .map(|command| command.executable.as_str()),
            Some("${sidecar:pnpm}")
        );
    }

    #[test]
    fn parses_canvas2d_runtime_pack_manifest() {
        let manifest =
            parse_runtime_pack_manifest(BUILTIN_CANVAS2D_RUNTIME_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, CANVAS2D_RUNTIME_PACK_ID);
        assert_eq!(manifest.version, CANVAS2D_PACK_VERSION);
        assert_eq!(manifest.runtime.kind, "canvas2d");
        assert_eq!(manifest.runtime.generated_root, "generated/canvas");
        assert_eq!(manifest.runtime.entrypoint, "index.html");
        assert!(manifest.commands.is_empty());
        assert_eq!(manifest.default_harness, [CANVAS2D_HARNESS_PACK_ID]);
    }

    #[test]
    fn parses_phase12_to15_runtime_pack_manifests() {
        for (
            raw,
            expected_id,
            expected_kind,
            expected_root,
            expected_entrypoint,
            expected_harness,
        ) in [
            (
                BUILTIN_MARKDOWN_KNOWLEDGE_RUNTIME_MANIFEST,
                MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID,
                "markdown-knowledge",
                "generated",
                "react/src/main.tsx",
                MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID,
            ),
            (
                BUILTIN_DATA_TABLE_RUNTIME_MANIFEST,
                DATA_TABLE_RUNTIME_PACK_ID,
                "data-table",
                "generated",
                "react/src/main.tsx",
                DATA_TABLE_HARNESS_PACK_ID,
            ),
            (
                BUILTIN_FILE_PROCESSOR_RUNTIME_MANIFEST,
                FILE_PROCESSOR_RUNTIME_PACK_ID,
                "file-processor",
                "generated",
                "react/src/main.tsx",
                FILE_PROCESSOR_HARNESS_PACK_ID,
            ),
            (
                BUILTIN_DESKTOP_WIDGET_RUNTIME_MANIFEST,
                DESKTOP_WIDGET_RUNTIME_PACK_ID,
                "desktop-widget",
                "generated",
                "react/src/main.tsx",
                DESKTOP_WIDGET_HARNESS_PACK_ID,
            ),
        ] {
            let manifest = parse_runtime_pack_manifest(raw).expect("manifest");
            assert_eq!(manifest.id, expected_id);
            assert_eq!(manifest.version, "0.1.0");
            assert_eq!(manifest.runtime.kind, expected_kind);
            assert_eq!(manifest.runtime.generated_root, expected_root);
            assert_eq!(manifest.runtime.entrypoint, expected_entrypoint);
            assert_eq!(manifest.default_harness, [expected_harness]);
            assert_eq!(manifest.runtime.network, "local-only");
            assert_eq!(manifest.runtime.bind, "127.0.0.1");
            assert!(manifest
                .commands
                .values()
                .all(|command| command.executable == "${sidecar:pnpm}"));
        }
    }

    #[test]
    fn parses_ai_agent_app_runtime_pack_manifest() {
        let manifest =
            parse_runtime_pack_manifest(BUILTIN_AI_AGENT_APP_RUNTIME_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, AI_AGENT_APP_RUNTIME_PACK_ID);
        assert_eq!(manifest.version, AI_AGENT_APP_PACK_VERSION);
        assert_eq!(manifest.runtime.kind, "ai-agent-app");
        assert_eq!(manifest.runtime.generated_root, "generated");
        assert_eq!(manifest.runtime.entrypoint, "react/src/main.tsx");
        assert_eq!(manifest.runtime.network, "local-only");
        assert_eq!(manifest.runtime.bind, "127.0.0.1");
        assert_eq!(
            manifest.default_harness,
            [
                ARTICLE_AGENT_HARNESS_PACK_ID,
                NOVEL_AGENT_HARNESS_PACK_ID,
                IMAGE_AGENT_HARNESS_PACK_ID,
                VIDEO_AGENT_HARNESS_PACK_ID,
                MULTIMODAL_STUDIO_AGENT_HARNESS_PACK_ID
            ]
        );
        assert!(manifest
            .commands
            .values()
            .all(|command| command.executable == "${sidecar:pnpm}"));
    }

    #[test]
    fn parses_harness_pack_manifest() {
        let manifest =
            parse_harness_pack_manifest(BUILTIN_STATIC_HTML_HARNESS_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, STATIC_HTML_HARNESS_PACK_ID);
        assert_eq!(manifest.version, STATIC_HTML_PACK_VERSION);
        assert_eq!(
            manifest.runtime.as_deref(),
            Some(STATIC_HTML_RUNTIME_PACK_ID)
        );
    }

    #[test]
    fn installs_runtime_pack_archive_and_resolves_it() {
        let (_temp, manager) = temp_pack_manager();
        let archive = zip_with_entries(&[(
            MANIFEST_FILE_NAME,
            BUILTIN_STATIC_HTML_RUNTIME_MANIFEST.as_bytes(),
        )]);

        let summary = manager
            .install_pack_archive(
                PackKind::Runtime,
                STATIC_HTML_RUNTIME_PACK_ID,
                STATIC_HTML_PACK_VERSION,
                &archive,
            )
            .expect("install archive");

        assert_eq!(summary.id, STATIC_HTML_RUNTIME_PACK_ID);
        assert_eq!(summary.source, "registry");
        let resolved = manager
            .resolver()
            .resolve_runtime(STATIC_HTML_RUNTIME_PACK_ID, STATIC_HTML_PACK_VERSION)
            .expect("resolve installed");
        assert_eq!(resolved.manifest.id, STATIC_HTML_RUNTIME_PACK_ID);
    }

    #[test]
    fn pack_archive_blocks_path_traversal_before_install() {
        let (_temp, manager) = temp_pack_manager();
        let archive = zip_with_entries(&[
            (
                MANIFEST_FILE_NAME,
                BUILTIN_STATIC_HTML_RUNTIME_MANIFEST.as_bytes(),
            ),
            ("../evil.txt", b"evil"),
        ]);

        let result = manager.install_pack_archive(
            PackKind::Runtime,
            STATIC_HTML_RUNTIME_PACK_ID,
            STATIC_HTML_PACK_VERSION,
            &archive,
        );

        assert!(matches!(result, Err(PackError::InvalidArchive(_))));
    }

    #[test]
    fn pack_archive_rejects_too_many_entries() {
        let (_temp, manager) = temp_pack_manager();
        let cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(MANIFEST_FILE_NAME, options)
            .expect("manifest");
        std::io::Write::write_all(&mut zip, BUILTIN_STATIC_HTML_RUNTIME_MANIFEST.as_bytes())
            .expect("write manifest");
        for index in 0..MAX_PACK_ARCHIVE_ENTRIES {
            zip.start_file(format!("extra/{index}.txt"), options)
                .expect("extra");
            std::io::Write::write_all(&mut zip, b"x").expect("write extra");
        }
        let archive = zip.finish().expect("finish zip").into_inner();

        let result = manager.install_pack_archive(
            PackKind::Runtime,
            STATIC_HTML_RUNTIME_PACK_ID,
            STATIC_HTML_PACK_VERSION,
            &archive,
        );

        assert!(matches!(
            result,
            Err(PackError::InvalidArchive(message)) if message.contains("too many entries")
        ));
    }

    #[test]
    fn pack_archive_rejects_oversized_manifest() {
        let (_temp, manager) = temp_pack_manager();
        let manifest = vec![b'{'; (MAX_PACK_MANIFEST_BYTES + 1) as usize];
        let archive = zip_with_entries(&[(MANIFEST_FILE_NAME, &manifest)]);

        let result = manager.install_pack_archive(
            PackKind::Runtime,
            STATIC_HTML_RUNTIME_PACK_ID,
            STATIC_HTML_PACK_VERSION,
            &archive,
        );

        assert!(matches!(
            result,
            Err(PackError::InvalidArchive(message)) if message.contains("manifest exceeds")
        ));
    }

    #[test]
    fn pack_archive_rejects_manifest_identity_mismatch() {
        let (_temp, manager) = temp_pack_manager();
        let archive = zip_with_entries(&[(
            MANIFEST_FILE_NAME,
            BUILTIN_STATIC_HTML_RUNTIME_MANIFEST.as_bytes(),
        )]);

        let result = manager.install_pack_archive(
            PackKind::Runtime,
            "sofvary.runtime.other",
            STATIC_HTML_PACK_VERSION,
            &archive,
        );

        assert!(matches!(result, Err(PackError::InvalidManifest(_))));
    }

    #[test]
    fn lists_installed_builtin_packs() {
        let (_temp, manager) = temp_pack_manager();

        let packs = manager.list_installed_packs().expect("packs");

        assert!(packs
            .iter()
            .any(|pack| pack.id == STATIC_HTML_RUNTIME_PACK_ID));
        assert!(packs
            .iter()
            .any(|pack| pack.id == STATIC_HTML_HARNESS_PACK_ID));
    }

    #[test]
    fn parses_react_vite_harness_pack_manifest() {
        let manifest =
            parse_harness_pack_manifest(BUILTIN_REACT_VITE_HARNESS_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, REACT_VITE_HARNESS_PACK_ID);
        assert_eq!(manifest.version, REACT_VITE_PACK_VERSION);
        assert_eq!(
            manifest.runtime.as_deref(),
            Some(REACT_VITE_RUNTIME_PACK_ID)
        );
    }

    #[test]
    fn parses_react_sqlite_harness_pack_manifest() {
        let manifest =
            parse_harness_pack_manifest(BUILTIN_REACT_SQLITE_HARNESS_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, REACT_SQLITE_HARNESS_PACK_ID);
        assert_eq!(manifest.version, REACT_SQLITE_PACK_VERSION);
        assert_eq!(
            manifest.runtime.as_deref(),
            Some(REACT_SQLITE_RUNTIME_PACK_ID)
        );
        assert!(manifest
            .instructions
            .output_rules
            .iter()
            .any(|rule| rule.contains("parameterized SQL")));
    }

    #[test]
    fn parses_canvas2d_harness_pack_manifest() {
        let manifest =
            parse_harness_pack_manifest(BUILTIN_CANVAS2D_HARNESS_MANIFEST).expect("manifest");

        assert_eq!(manifest.id, CANVAS2D_HARNESS_PACK_ID);
        assert_eq!(manifest.version, CANVAS2D_PACK_VERSION);
        assert_eq!(manifest.runtime.as_deref(), Some(CANVAS2D_RUNTIME_PACK_ID));
        assert!(manifest
            .instructions
            .system
            .iter()
            .any(|rule| rule.contains("requestAnimationFrame")));
    }

    #[test]
    fn parses_phase12_to15_harness_pack_manifests() {
        for (raw, expected_id, expected_runtime) in [
            (
                BUILTIN_MARKDOWN_KNOWLEDGE_HARNESS_MANIFEST,
                MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID,
                MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID,
            ),
            (
                BUILTIN_DATA_TABLE_HARNESS_MANIFEST,
                DATA_TABLE_HARNESS_PACK_ID,
                DATA_TABLE_RUNTIME_PACK_ID,
            ),
            (
                BUILTIN_FILE_PROCESSOR_HARNESS_MANIFEST,
                FILE_PROCESSOR_HARNESS_PACK_ID,
                FILE_PROCESSOR_RUNTIME_PACK_ID,
            ),
            (
                BUILTIN_DESKTOP_WIDGET_HARNESS_MANIFEST,
                DESKTOP_WIDGET_HARNESS_PACK_ID,
                DESKTOP_WIDGET_RUNTIME_PACK_ID,
            ),
            (
                BUILTIN_ARTICLE_AGENT_HARNESS_MANIFEST,
                ARTICLE_AGENT_HARNESS_PACK_ID,
                AI_AGENT_APP_RUNTIME_PACK_ID,
            ),
            (
                BUILTIN_NOVEL_AGENT_HARNESS_MANIFEST,
                NOVEL_AGENT_HARNESS_PACK_ID,
                AI_AGENT_APP_RUNTIME_PACK_ID,
            ),
            (
                BUILTIN_IMAGE_AGENT_HARNESS_MANIFEST,
                IMAGE_AGENT_HARNESS_PACK_ID,
                AI_AGENT_APP_RUNTIME_PACK_ID,
            ),
            (
                BUILTIN_VIDEO_AGENT_HARNESS_MANIFEST,
                VIDEO_AGENT_HARNESS_PACK_ID,
                AI_AGENT_APP_RUNTIME_PACK_ID,
            ),
            (
                BUILTIN_MULTIMODAL_STUDIO_AGENT_HARNESS_MANIFEST,
                MULTIMODAL_STUDIO_AGENT_HARNESS_PACK_ID,
                AI_AGENT_APP_RUNTIME_PACK_ID,
            ),
        ] {
            let manifest = parse_harness_pack_manifest(raw).expect("manifest");
            assert_eq!(manifest.id, expected_id);
            assert_eq!(manifest.version, "0.1.0");
            assert_eq!(manifest.runtime.as_deref(), Some(expected_runtime));
            assert!(!manifest.instructions.system.is_empty());
            assert!(!manifest.instructions.output_rules.is_empty());
            if expected_runtime == AI_AGENT_APP_RUNTIME_PACK_ID {
                let species = manifest.software_species.expect("software species");
                assert_eq!(species.runtime_kinds, ["ai-agent-app"]);
                assert!(!species.tags.is_empty());
            }
        }
    }

    #[test]
    fn rejects_runtime_manifest_with_wrong_schema_version() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("json");
        manifest["schemaVersion"] = serde_json::Value::String("2.0".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("schemaVersion"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_with_non_local_bind() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("json");
        manifest["runtime"]["bind"] = serde_json::Value::String("0.0.0.0".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("127.0.0.1"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_with_remote_network() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("json");
        manifest["runtime"]["network"] = serde_json::Value::String("remote".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("local-only"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_generated_root_traversal() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("json");
        manifest["runtime"]["generatedRoot"] = serde_json::Value::String("../outside".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("generatedRoot"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_windows_absolute_generated_root() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("json");
        manifest["runtime"]["generatedRoot"] =
            serde_json::Value::String("C:/Users/example".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("relative"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_backslash_generated_root() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("json");
        manifest["runtime"]["generatedRoot"] =
            serde_json::Value::String("generated\\static".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("forward slash"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_with_invalid_default_harness_id() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("json");
        manifest["defaultHarness"] =
            serde_json::Value::Array(vec![serde_json::Value::String("../bad".to_string())]);

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(matches!(result, Err(PackError::InvalidPackId(id)) if id == "../bad"));
    }

    #[test]
    fn rejects_harness_manifest_with_invalid_runtime_id() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_HARNESS_MANIFEST).expect("json");
        manifest["runtime"] = serde_json::Value::String("../bad".to_string());

        let result = parse_harness_pack_manifest(&manifest.to_string());

        assert!(matches!(result, Err(PackError::InvalidPackId(id)) if id == "../bad"));
    }

    #[test]
    fn rejects_plugin_manifest_with_cwd_traversal() {
        let raw = r#"{
          "schemaVersion": "1.0",
          "type": "sofvary.plugin-pack",
          "id": "sofvary.plugin.example",
          "version": "0.1.0",
          "name": "Example Plugin",
          "description": "Example only.",
          "compatibility": {
            "client": ">=0.1.0",
            "os": {
              "windows": { "supported": true },
              "macos": { "supported": true },
              "linux": { "supported": true }
            },
          "arch": ["x64"]
        },
        "capabilities": ["example"],
        "entry": {
            "executable": "${sidecar:node}",
            "args": ["index.js"],
            "cwd": "../outside"
        }
      }"#;

        let result = parse_plugin_pack_manifest(raw);

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("command.cwd"))
        );
    }

    #[test]
    fn rejects_command_spec_without_sidecar_placeholder() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_REACT_VITE_RUNTIME_MANIFEST).expect("json");
        manifest["commands"]["dev"]["executable"] = serde_json::Value::String("pnpm".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("${sidecar:pnpm}"))
        );
    }

    #[test]
    fn rejects_command_spec_with_network_enabled() {
        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_REACT_VITE_RUNTIME_MANIFEST).expect("json");
        manifest["commands"]["install"]["allowedNetwork"] = serde_json::Value::Bool(true);

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("allowedNetwork"))
        );
    }

    #[test]
    fn resolves_builtin_static_html_runtime_pack() {
        let (_temp, manager) = temp_pack_manager();
        let resolved = manager.resolve_static_html_packs().expect("resolved");

        assert_eq!(resolved.runtime.manifest.id, STATIC_HTML_RUNTIME_PACK_ID);
        assert!(resolved
            .runtime
            .manifest_path
            .starts_with(manager.cache_root()));
        assert!(resolved.runtime.pack_dir.starts_with(manager.cache_root()));
    }

    #[test]
    fn resolves_builtin_static_html_harness_pack() {
        let (_temp, manager) = temp_pack_manager();
        let resolved = manager.resolve_static_html_packs().expect("resolved");

        assert_eq!(resolved.harness.manifest.id, STATIC_HTML_HARNESS_PACK_ID);
        assert!(resolved
            .harness
            .manifest_path
            .starts_with(manager.cache_root()));
        assert!(resolved.harness.pack_dir.starts_with(manager.cache_root()));
    }

    #[test]
    fn resolves_builtin_react_vite_packs() {
        let (_temp, manager) = temp_pack_manager();
        let resolved = manager.resolve_react_vite_packs().expect("resolved");

        assert_eq!(resolved.runtime.manifest.id, REACT_VITE_RUNTIME_PACK_ID);
        assert_eq!(resolved.harness.manifest.id, REACT_VITE_HARNESS_PACK_ID);
        assert!(resolved
            .runtime
            .manifest_path
            .starts_with(manager.cache_root()));
        assert!(resolved.harness.pack_dir.starts_with(manager.cache_root()));
    }

    #[test]
    fn resolves_builtin_react_sqlite_packs() {
        let (_temp, manager) = temp_pack_manager();
        let resolved = manager.resolve_react_sqlite_packs().expect("resolved");

        assert_eq!(resolved.runtime.manifest.id, REACT_SQLITE_RUNTIME_PACK_ID);
        assert_eq!(resolved.harness.manifest.id, REACT_SQLITE_HARNESS_PACK_ID);
        assert!(resolved
            .runtime
            .manifest_path
            .starts_with(manager.cache_root()));
        assert!(resolved.harness.pack_dir.starts_with(manager.cache_root()));
    }

    #[test]
    fn resolves_builtin_canvas2d_packs() {
        let (_temp, manager) = temp_pack_manager();
        let resolved = manager.resolve_canvas2d_packs().expect("resolved");

        assert_eq!(resolved.runtime.manifest.id, CANVAS2D_RUNTIME_PACK_ID);
        assert_eq!(resolved.harness.manifest.id, CANVAS2D_HARNESS_PACK_ID);
        assert!(resolved
            .runtime
            .manifest_path
            .starts_with(manager.cache_root()));
        assert!(resolved.harness.pack_dir.starts_with(manager.cache_root()));
    }

    #[test]
    fn resolves_builtin_phase12_to15_packs() {
        let (_temp, manager) = temp_pack_manager();

        let markdown = manager
            .resolve_markdown_knowledge_packs()
            .expect("markdown");
        assert_eq!(
            markdown.runtime.manifest.id,
            MARKDOWN_KNOWLEDGE_RUNTIME_PACK_ID
        );
        assert_eq!(
            markdown.harness.manifest.id,
            MARKDOWN_KNOWLEDGE_HARNESS_PACK_ID
        );

        let data_table = manager.resolve_data_table_packs().expect("data table");
        assert_eq!(data_table.runtime.manifest.id, DATA_TABLE_RUNTIME_PACK_ID);
        assert_eq!(data_table.harness.manifest.id, DATA_TABLE_HARNESS_PACK_ID);

        let file_processor = manager
            .resolve_file_processor_packs()
            .expect("file processor");
        assert_eq!(
            file_processor.runtime.manifest.id,
            FILE_PROCESSOR_RUNTIME_PACK_ID
        );
        assert_eq!(
            file_processor.harness.manifest.id,
            FILE_PROCESSOR_HARNESS_PACK_ID
        );

        let widget = manager.resolve_desktop_widget_packs().expect("widget");
        assert_eq!(widget.runtime.manifest.id, DESKTOP_WIDGET_RUNTIME_PACK_ID);
        assert_eq!(widget.harness.manifest.id, DESKTOP_WIDGET_HARNESS_PACK_ID);
        assert!(widget
            .runtime
            .manifest_path
            .starts_with(manager.cache_root()));
        assert!(widget.harness.pack_dir.starts_with(manager.cache_root()));

        let ai_agent = manager.resolve_ai_agent_app_packs().expect("ai agent app");
        assert_eq!(ai_agent.runtime.manifest.id, AI_AGENT_APP_RUNTIME_PACK_ID);
        assert_eq!(
            ai_agent.harness.manifest.id,
            MULTIMODAL_STUDIO_AGENT_HARNESS_PACK_ID
        );
        assert!(ai_agent
            .runtime
            .manifest_path
            .starts_with(manager.cache_root()));
        assert!(ai_agent.harness.pack_dir.starts_with(manager.cache_root()));
    }

    #[test]
    fn missing_pack_returns_clear_error() {
        let (_temp, manager) = temp_pack_manager();
        let result = manager
            .resolver
            .resolve_runtime("sofvary.runtime.missing", "0.1.0");

        assert!(matches!(
            result,
            Err(PackError::MissingPack {
                kind: PackKind::Runtime,
                id,
                version
            }) if id == "sofvary.runtime.missing" && version == "0.1.0"
        ));
    }

    #[test]
    fn pack_cache_path_traversal_is_blocked() {
        let (_temp, manager) = temp_pack_manager();
        let result =
            manager
                .cache
                .pack_dir(PackKind::Runtime, "sofvary.runtime.static-html", "../0.1.0");

        assert!(matches!(result, Err(PackError::InvalidVersion(_))));

        let result =
            manager
                .cache
                .pack_dir(PackKind::Runtime, "../sofvary.runtime.static-html", "0.1.0");

        assert!(matches!(result, Err(PackError::InvalidPackId(_))));
    }

    #[test]
    fn cached_pack_versions_are_immutable() {
        let (_temp, manager) = temp_pack_manager();
        let mut manifest =
            parse_runtime_pack_manifest(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("manifest");
        manifest.description = "changed".to_string();

        let result = manager.cache.install_runtime_manifest(&manifest);

        assert!(matches!(result, Err(PackError::ImmutableVersion(_))));
    }

    #[test]
    fn resolver_rejects_cached_runtime_manifest_with_unsafe_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache =
            LocalPackCache::from_root(temp.path().join("data").join("packs")).expect("cache");
        let pack_dir = cache
            .pack_dir(PackKind::Runtime, "sofvary.runtime.unsafe", "0.1.0")
            .expect("pack dir");
        fs::create_dir_all(&pack_dir).expect("pack dir");

        let mut manifest: serde_json::Value =
            serde_json::from_str(BUILTIN_STATIC_HTML_RUNTIME_MANIFEST).expect("json");
        manifest["id"] = serde_json::Value::String("sofvary.runtime.unsafe".to_string());
        manifest["runtime"]["bind"] = serde_json::Value::String("0.0.0.0".to_string());
        fs::write(pack_dir.join(MANIFEST_FILE_NAME), manifest.to_string()).expect("write");

        let resolver = PackResolver::new(cache);
        let result = resolver.resolve_runtime("sofvary.runtime.unsafe", "0.1.0");

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("127.0.0.1"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn pack_cache_symlink_escape_is_blocked() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache =
            LocalPackCache::from_root(temp.path().join("data").join("packs")).expect("cache");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).expect("outside");
        fs::write(
            outside.join(MANIFEST_FILE_NAME),
            BUILTIN_STATIC_HTML_RUNTIME_MANIFEST,
        )
        .expect("manifest");

        let runtime_parent = cache
            .root()
            .join(PackKind::Runtime.cache_dir_name())
            .join("sofvary.runtime.linked");
        fs::create_dir_all(&runtime_parent).expect("runtime parent");
        std::os::unix::fs::symlink(&outside, runtime_parent.join("0.1.0")).expect("symlink");
        let resolver = PackResolver::new(cache);
        let result = resolver.resolve_runtime("sofvary.runtime.linked", "0.1.0");

        assert!(matches!(result, Err(PackError::PathEscape(_))));
    }
}
