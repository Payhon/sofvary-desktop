use crate::core::builtin_resources::{builtin_resource_paths, get_builtin_resource};
use crate::core::pack_types::{HarnessPackManifest, PluginPackManifest, RuntimePackManifest};
use crate::platform::{current_adapter, PlatformAdapter, PlatformError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use zip::ZipArchive;

const RUNTIME_PACK_TYPE: &str = "sofvary.runtime-pack";
const HARNESS_PACK_TYPE: &str = "sofvary.harness-pack";
const PLUGIN_PACK_TYPE: &str = "sofvary.plugin-pack";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const PACK_SCHEMA_VERSION: &str = "1.0";
const MAX_PACK_ARCHIVE_ENTRIES: usize = 256;
const MAX_PACK_MANIFEST_BYTES: u64 = 1024 * 1024;
const LOCAL_BIND_ADDR: &str = "127.0.0.1";
const LOCAL_ONLY_NETWORK: &str = "local-only";
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
    #[cfg(test)]
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
    pub pack_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub source: PackSource,
    pub source_path: Option<PathBuf>,
    pub sha256: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub sha256: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackSource {
    RuntimeOverride,
    Cache,
    Registry,
    CompiledBuiltin,
}

impl PackSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeOverride => "runtime-override",
            Self::Cache => "cache",
            Self::Registry => "registry",
            Self::CompiledBuiltin => "compiled-builtin",
        }
    }
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    fn install_pack_archive_dir(
        &self,
        kind: PackKind,
        pack_type: &str,
        id: &str,
        version: &str,
        bytes: &[u8],
    ) -> PackResult<()> {
        validate_manifest_identity(kind, pack_type, id, version)?;
        let pack_dir = self.pack_dir(kind, id, version)?;
        self.ensure_nearest_existing_ancestor_inside_cache(&pack_dir)?;

        if pack_dir.exists() {
            self.ensure_existing_path_inside_cache(&pack_dir)?;
            fs::remove_dir_all(&pack_dir)?;
        }
        fs::create_dir_all(&pack_dir)?;

        let cursor = Cursor::new(bytes);
        let mut archive = ZipArchive::new(cursor)?;
        if archive.len() > MAX_PACK_ARCHIVE_ENTRIES {
            return Err(PackError::InvalidArchive(format!(
                "pack archive contains too many entries: {} > {}",
                archive.len(),
                MAX_PACK_ARCHIVE_ENTRIES
            )));
        }

        let mut saw_manifest = false;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let name = entry.name().to_string();
            validate_archive_entry_path(&name)?;
            if entry.is_dir() {
                continue;
            }
            if name == MANIFEST_FILE_NAME {
                saw_manifest = true;
            }

            let output_path = pack_dir.join(&name);
            let normalized_output = normalize_for_boundary(&output_path);
            let normalized_pack_dir = normalize_for_boundary(&pack_dir);
            if !normalized_output.starts_with(&normalized_pack_dir) {
                return Err(PackError::PathEscape(normalized_output));
            }
            if let Some(parent) = normalized_output.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = fs::File::create(&normalized_output)?;
            std::io::copy(&mut entry, &mut output)?;
        }

        if !saw_manifest {
            return Err(PackError::InvalidArchive(
                "pack archive must include manifest.json at the root".to_string(),
            ));
        }

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
                self.install_pack_archive_dir(
                    PackKind::Runtime,
                    &manifest.pack_type,
                    &manifest.id,
                    &manifest.version,
                    bytes,
                )?;
                Ok(summary_from_runtime_manifest(&manifest, "registry"))
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
                self.install_pack_archive_dir(
                    PackKind::Harness,
                    &manifest.pack_type,
                    &manifest.id,
                    &manifest.version,
                    bytes,
                )?;
                Ok(summary_from_harness_manifest(&manifest, "registry"))
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
                self.install_pack_archive_dir(
                    PackKind::Plugin,
                    &manifest.pack_type,
                    &manifest.id,
                    &manifest.version,
                    bytes,
                )?;
                Ok(summary_from_plugin_manifest(&manifest, "registry"))
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
            pack_dir: pack_dir.clone(),
            manifest_path: manifest_path.clone(),
            source: PackSource::Cache,
            source_path: Some(pack_dir),
            sha256: sha256_hex(&fs::read(&manifest_path)?),
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
    override_roots: Vec<PathBuf>,
}

impl PackResolver {
    pub fn new(cache: LocalPackCache, override_roots: Vec<PathBuf>) -> Self {
        Self {
            cache,
            override_roots,
        }
    }

    pub fn resolve_runtime(
        &self,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<RuntimePackManifest>> {
        let cached: CachedPack<RuntimePackManifest> =
            self.resolve_manifest(PackKind::Runtime, id, version)?;
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
        let cached: CachedPack<HarnessPackManifest> =
            self.resolve_manifest(PackKind::Harness, id, version)?;
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

    pub fn read_resource_text(
        &self,
        kind: PackKind,
        id: &str,
        version: &str,
        resource_path: &str,
    ) -> PackResult<String> {
        validate_relative_path("pack resource path", resource_path)?;

        match kind {
            PackKind::Runtime => {
                let cached: CachedPack<RuntimePackManifest> =
                    self.resolve_manifest(kind, id, version)?;
                read_resolved_pack_resource(kind, &cached, resource_path)
            }
            PackKind::Harness => {
                let cached: CachedPack<HarnessPackManifest> =
                    self.resolve_manifest(kind, id, version)?;
                read_resolved_pack_resource(kind, &cached, resource_path)
            }
            PackKind::Plugin => {
                let cached = self.resolve_plugin(id, version)?;
                read_resolved_pack_resource(kind, &cached, resource_path)
            }
        }
    }

    pub fn list_installed_packs(&self) -> PackResult<Vec<InstalledPackSummary>> {
        let mut summaries = Vec::new();
        let mut seen = HashSet::new();

        for root in &self.override_roots {
            append_pack_root_summaries(
                root,
                PackSource::RuntimeOverride,
                &mut seen,
                &mut summaries,
            )?;
        }
        append_pack_root_summaries(
            &self.cache.root,
            PackSource::Cache,
            &mut seen,
            &mut summaries,
        )?;
        append_builtin_pack_summaries(&mut seen, &mut summaries)?;

        summaries.sort_by(|left, right| {
            kind_sort_key(&left.kind)
                .cmp(&kind_sort_key(&right.kind))
                .then(left.id.cmp(&right.id))
                .then(left.version.cmp(&right.version))
                .then(left.source.cmp(&right.source))
        });
        Ok(summaries)
    }

    fn resolve_manifest<T: DeserializeOwned>(
        &self,
        kind: PackKind,
        id: &str,
        version: &str,
    ) -> PackResult<CachedPack<T>> {
        validate_pack_id(id)?;
        validate_semver(version)?;

        for root in &self.override_roots {
            if let Some(cached) =
                read_pack_manifest_from_root(root, PackSource::RuntimeOverride, kind, id, version)?
            {
                return Ok(cached);
            }
        }

        let cached = match self.cache.read_manifest(kind, id, version) {
            Ok(cached) => Some(cached),
            Err(PackError::MissingPack { .. }) => None,
            Err(error) => return Err(error),
        };
        if let Some(cached) = cached {
            return Ok(cached);
        }

        read_builtin_manifest(kind, id, version)?.ok_or_else(|| PackError::MissingPack {
            kind,
            id: id.to_string(),
            version: version.to_string(),
        })
    }
}

pub struct RuntimePackResolution {
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
        let override_roots = runtime_override_roots(&cache.root);
        Self::new_with_cache_and_roots(cache, override_roots)
    }

    pub fn resolver(&self) -> &PackResolver {
        &self.resolver
    }

    pub fn runtime_catalog_manifests(&self) -> PackResult<Vec<RuntimePackManifest>> {
        let mut manifests = self
            .list_installed_packs()?
            .into_iter()
            .filter(|summary| summary.kind == "runtime")
            .map(|summary| {
                self.resolver
                    .resolve_runtime(&summary.id, &summary.version)
                    .map(|cached| cached.manifest)
            })
            .collect::<PackResult<Vec<_>>>()?;
        manifests.sort_by(|left, right| {
            left.selection
                .priority
                .cmp(&right.selection.priority)
                .then(left.id.cmp(&right.id))
                .then(left.version.cmp(&right.version))
        });
        Ok(manifests)
    }

    pub fn resolve_default_runtime(&self) -> PackResult<CachedPack<RuntimePackManifest>> {
        let manifest = self
            .runtime_catalog_manifests()?
            .into_iter()
            .next()
            .ok_or_else(|| {
                PackError::InvalidManifest("runtime catalog contains no runtime packs".to_string())
            })?;
        self.resolver
            .resolve_runtime(&manifest.id, &manifest.version)
    }

    pub fn resolve_runtime_by_kind(
        &self,
        runtime_kind: &str,
    ) -> PackResult<CachedPack<RuntimePackManifest>> {
        validate_manifest_name("runtime.kind", runtime_kind)?;
        let manifest = self
            .runtime_catalog_manifests()?
            .into_iter()
            .find(|manifest| manifest.runtime.kind == runtime_kind)
            .ok_or_else(|| PackError::MissingPack {
                kind: PackKind::Runtime,
                id: runtime_kind.to_string(),
                version: "*".to_string(),
            })?;
        self.resolver
            .resolve_runtime(&manifest.id, &manifest.version)
    }

    pub fn resolve_default_harness_for_runtime(
        &self,
        runtime_pack: &RuntimePackManifest,
    ) -> PackResult<CachedPack<HarnessPackManifest>> {
        let harness_id = runtime_pack.default_harness.first().ok_or_else(|| {
            PackError::InvalidManifest(format!(
                "runtime pack {}@{} must declare defaultHarness",
                runtime_pack.id, runtime_pack.version
            ))
        })?;
        match self
            .resolver
            .resolve_harness(harness_id, &runtime_pack.version)
        {
            Ok(harness) => Ok(harness),
            Err(PackError::MissingPack { .. }) => {
                let summary = self
                    .list_installed_packs()?
                    .into_iter()
                    .filter(|summary| summary.kind == "harness" && summary.id == *harness_id)
                    .max_by(|left, right| left.version.cmp(&right.version))
                    .ok_or_else(|| PackError::MissingPack {
                        kind: PackKind::Harness,
                        id: harness_id.clone(),
                        version: runtime_pack.version.clone(),
                    })?;
                self.resolver.resolve_harness(&summary.id, &summary.version)
            }
            Err(error) => Err(error),
        }
    }

    pub fn resolve_runtime_packs_by_kind(
        &self,
        runtime_kind: &str,
    ) -> PackResult<RuntimePackResolution> {
        let runtime = self.resolve_runtime_by_kind(runtime_kind)?;
        let harness = self.resolve_default_harness_for_runtime(&runtime.manifest)?;
        Ok(RuntimePackResolution { runtime, harness })
    }

    pub fn resolve_default_runtime_packs(&self) -> PackResult<RuntimePackResolution> {
        let runtime = self.resolve_default_runtime()?;
        let harness = self.resolve_default_harness_for_runtime(&runtime.manifest)?;
        Ok(RuntimePackResolution { runtime, harness })
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
        self.resolver.list_installed_packs()
    }

    fn new_with_cache(cache: LocalPackCache) -> PackResult<Self> {
        Self::new_with_cache_and_roots(cache, Vec::new())
    }

    fn new_with_cache_and_roots(
        cache: LocalPackCache,
        override_roots: Vec<PathBuf>,
    ) -> PackResult<Self> {
        Ok(Self {
            resolver: PackResolver::new(cache.clone(), override_roots),
            cache,
        })
    }
}

fn runtime_override_roots(cache_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(value) = std::env::var_os("SOFVARY_PACK_ROOTS") {
        roots.extend(std::env::split_paths(&value));
    }
    roots.push(cache_root.join("overrides"));
    roots
}

pub fn read_pack_resource_text(
    kind: PackKind,
    id: &str,
    version: &str,
    resource_path: &str,
) -> PackResult<String> {
    validate_pack_id(id)?;
    validate_semver(version)?;
    validate_relative_path("pack resource path", resource_path)?;

    let adapter = current_adapter();
    let manager = PackManager::new_with_adapter(adapter.as_ref())?;
    manager
        .resolver
        .read_resource_text(kind, id, version, resource_path)
}

pub fn runtime_catalog_manifests() -> PackResult<Vec<RuntimePackManifest>> {
    PackManager::new()?.runtime_catalog_manifests()
}

fn append_builtin_pack_summaries(
    seen: &mut HashSet<String>,
    summaries: &mut Vec<InstalledPackSummary>,
) -> PackResult<()> {
    for path in builtin_resource_paths() {
        if is_builtin_manifest_path(path, PackKind::Runtime) {
            if let Some(contents) = get_builtin_resource(path) {
                let manifest = parse_runtime_pack_manifest(contents)?;
                push_pack_summary(
                    seen,
                    summaries,
                    summary_from_runtime_source(
                        &manifest,
                        PackSource::CompiledBuiltin,
                        None,
                        sha256_hex(contents.as_bytes()),
                    ),
                );
            }
        } else if is_builtin_manifest_path(path, PackKind::Harness) {
            if let Some(contents) = get_builtin_resource(path) {
                let manifest = parse_harness_pack_manifest(contents)?;
                push_pack_summary(
                    seen,
                    summaries,
                    summary_from_harness_source(
                        &manifest,
                        PackSource::CompiledBuiltin,
                        None,
                        sha256_hex(contents.as_bytes()),
                    ),
                );
            }
        } else if is_builtin_manifest_path(path, PackKind::Plugin) {
            if let Some(contents) = get_builtin_resource(path) {
                let manifest = parse_plugin_pack_manifest(contents)?;
                push_pack_summary(
                    seen,
                    summaries,
                    summary_from_plugin_source(
                        &manifest,
                        PackSource::CompiledBuiltin,
                        None,
                        sha256_hex(contents.as_bytes()),
                    ),
                );
            }
        }
    }
    Ok(())
}

fn append_pack_root_summaries(
    root: &Path,
    source: PackSource,
    seen: &mut HashSet<String>,
    summaries: &mut Vec<InstalledPackSummary>,
) -> PackResult<()> {
    if !root.exists() {
        return Ok(());
    }

    for kind in [PackKind::Runtime, PackKind::Harness, PackKind::Plugin] {
        let kind_root = root.join(kind.cache_dir_name());
        if !kind_root.exists() {
            continue;
        }
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
                        if let Some(cached) = read_pack_manifest_from_root::<RuntimePackManifest>(
                            root, source, kind, &id, &version,
                        )? {
                            validate_runtime_manifest_fields(&cached.manifest)?;
                            push_pack_summary(
                                seen,
                                summaries,
                                summary_from_runtime_source(
                                    &cached.manifest,
                                    cached.source,
                                    cached.source_path.as_ref(),
                                    cached.sha256,
                                ),
                            );
                        }
                    }
                    PackKind::Harness => {
                        if let Some(cached) = read_pack_manifest_from_root::<HarnessPackManifest>(
                            root, source, kind, &id, &version,
                        )? {
                            validate_harness_manifest_fields(&cached.manifest)?;
                            push_pack_summary(
                                seen,
                                summaries,
                                summary_from_harness_source(
                                    &cached.manifest,
                                    cached.source,
                                    cached.source_path.as_ref(),
                                    cached.sha256,
                                ),
                            );
                        }
                    }
                    PackKind::Plugin => {
                        if let Some(cached) = read_pack_manifest_from_root::<PluginPackManifest>(
                            root, source, kind, &id, &version,
                        )? {
                            validate_plugin_manifest_fields(&cached.manifest)?;
                            push_pack_summary(
                                seen,
                                summaries,
                                summary_from_plugin_source(
                                    &cached.manifest,
                                    cached.source,
                                    cached.source_path.as_ref(),
                                    cached.sha256,
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn push_pack_summary(
    seen: &mut HashSet<String>,
    summaries: &mut Vec<InstalledPackSummary>,
    summary: InstalledPackSummary,
) {
    let key = format!("{}:{}@{}", summary.kind, summary.id, summary.version);
    if seen.insert(key) {
        summaries.push(summary);
    }
}

fn read_builtin_manifest<T: DeserializeOwned>(
    kind: PackKind,
    id: &str,
    version: &str,
) -> PackResult<Option<CachedPack<T>>> {
    let Some(contents) = read_builtin_pack_resource(kind, id, version, MANIFEST_FILE_NAME) else {
        return Ok(None);
    };
    let manifest_path = builtin_pack_resource_path(kind, id, version, MANIFEST_FILE_NAME);
    let pack_dir = PathBuf::from(format!(
        "builtin-packs/{}/{}/{}",
        kind.cache_dir_name(),
        id,
        version
    ));
    Ok(Some(CachedPack {
        manifest: serde_json::from_str(contents)?,
        pack_dir,
        manifest_path: PathBuf::from(manifest_path),
        source: PackSource::CompiledBuiltin,
        source_path: None,
        sha256: sha256_hex(contents.as_bytes()),
    }))
}

fn read_pack_manifest_from_root<T: DeserializeOwned>(
    root: &Path,
    source: PackSource,
    kind: PackKind,
    id: &str,
    version: &str,
) -> PackResult<Option<CachedPack<T>>> {
    if !root.exists() {
        return Ok(None);
    }

    let pack_dir = normalize_for_boundary(&root.join(kind.cache_dir_name()).join(id).join(version));
    let normalized_root = normalize_for_boundary(root);
    if !pack_dir.starts_with(&normalized_root) {
        return Err(PackError::PathEscape(pack_dir));
    }

    let manifest_path = pack_dir.join(MANIFEST_FILE_NAME);
    if !manifest_path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&manifest_path)?;
    Ok(Some(CachedPack {
        manifest: serde_json::from_slice(&bytes)?,
        pack_dir: pack_dir.clone(),
        manifest_path,
        source,
        source_path: Some(pack_dir),
        sha256: sha256_hex(&bytes),
    }))
}

fn read_resolved_pack_resource<T: PackManifestIdentity>(
    kind: PackKind,
    cached: &CachedPack<T>,
    resource_path: &str,
) -> PackResult<String> {
    if cached.source == PackSource::CompiledBuiltin {
        if let Some(contents) =
            read_builtin_pack_resource(kind, pack_id(cached)?, pack_version(cached)?, resource_path)
        {
            return Ok(contents.to_string());
        }
    }

    let resource_file = normalize_for_boundary(&cached.pack_dir.join(resource_path));
    let normalized_pack_dir = normalize_for_boundary(&cached.pack_dir);
    if !resource_file.starts_with(&normalized_pack_dir) {
        return Err(PackError::PathEscape(resource_file));
    }
    Ok(fs::read_to_string(resource_file)?)
}

fn pack_id<T>(cached: &CachedPack<T>) -> PackResult<&str>
where
    T: PackManifestIdentity,
{
    Ok(cached.manifest.id())
}

fn pack_version<T>(cached: &CachedPack<T>) -> PackResult<&str>
where
    T: PackManifestIdentity,
{
    Ok(cached.manifest.version())
}

trait PackManifestIdentity {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
}

impl PackManifestIdentity for RuntimePackManifest {
    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> &str {
        &self.version
    }
}

impl PackManifestIdentity for HarnessPackManifest {
    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> &str {
        &self.version
    }
}

impl PackManifestIdentity for PluginPackManifest {
    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> &str {
        &self.version
    }
}

fn read_builtin_pack_resource(
    kind: PackKind,
    id: &str,
    version: &str,
    resource_path: &str,
) -> Option<&'static str> {
    let path = builtin_pack_resource_path(kind, id, version, resource_path);
    get_builtin_resource(&path)
}

fn builtin_pack_resource_path(
    kind: PackKind,
    id: &str,
    version: &str,
    resource_path: &str,
) -> String {
    format!(
        "builtin-packs/{}/{}/{}/{}",
        kind.cache_dir_name(),
        id,
        version,
        resource_path
    )
}

fn is_builtin_manifest_path(path: &str, kind: PackKind) -> bool {
    path.starts_with(&format!("builtin-packs/{}/", kind.cache_dir_name()))
        && path.ends_with("/manifest.json")
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
    validate_manifest_name("runtime.kind", &manifest.runtime.kind)?;
    validate_relative_path("runtime.generatedRoot", &manifest.runtime.generated_root)?;
    validate_relative_path("runtime.entrypoint", &manifest.runtime.entrypoint)?;
    validate_executor(&manifest.executor)?;
    validate_relative_path("promptEnvelope", &manifest.prompt_envelope)?;
    validate_selection(&manifest.selection)?;

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

    validate_relative_path("promptPolicy", &manifest.prompt_policy)?;
    if let Some(species) = &manifest.software_species {
        validate_manifest_name("softwareSpecies.id", &species.id)?;
        validate_manifest_name("softwareSpecies.name", &species.name)?;
        for runtime_kind in &species.runtime_kinds {
            validate_manifest_name("softwareSpecies.runtimeKinds", runtime_kind)?;
        }
        validate_string_list("softwareSpecies.tags", &species.tags)?;
    }
    validate_integrity(manifest.integrity.as_ref())
}

fn validate_executor(executor: &crate::core::pack_types::RuntimePackExecutor) -> PackResult<()> {
    validate_manifest_name("executor.kind", &executor.kind)?;
    validate_string_list("executor.requiredToolchains", &executor.required_toolchains)?;
    for value in &executor.allowed_top_level_dirs {
        validate_relative_path("executor.allowedTopLevelDirs", value)?;
    }
    for value in &executor.clear_roots {
        validate_relative_path("executor.clearRoots", value)?;
    }
    for value in &executor.preserve_files {
        validate_relative_path("executor.preserveFiles", value)?;
    }
    if let Some(context_root) = &executor.context_root {
        validate_relative_path("executor.contextRoot", context_root)?;
    }
    Ok(())
}

fn validate_selection(selection: &crate::core::pack_types::RuntimePackSelection) -> PackResult<()> {
    validate_manifest_name("selection.softwareType", &selection.software_type)?;
    validate_manifest_name("selection.reason", &selection.reason)?;
    validate_string_list("selection.signals", &selection.signals)?;
    if selection.signals.is_empty() {
        return Err(PackError::InvalidManifest(
            "selection.signals must not be empty".to_string(),
        ));
    }
    if selection.weight <= 0 {
        return Err(PackError::InvalidManifest(
            "selection.weight must be greater than zero".to_string(),
        ));
    }
    Ok(())
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

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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
        source_path: None,
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

fn summary_from_runtime_source(
    manifest: &RuntimePackManifest,
    source: PackSource,
    source_path: Option<&PathBuf>,
    sha256: String,
) -> InstalledPackSummary {
    InstalledPackSummary {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        kind: "runtime".to_string(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        source: source.as_str().to_string(),
        source_path: source_path.map(|path| path.display().to_string()),
        sha256: Some(sha256),
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
        source_path: None,
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

fn summary_from_harness_source(
    manifest: &HarnessPackManifest,
    source: PackSource,
    source_path: Option<&PathBuf>,
    sha256: String,
) -> InstalledPackSummary {
    InstalledPackSummary {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        kind: "harness".to_string(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        source: source.as_str().to_string(),
        source_path: source_path.map(|path| path.display().to_string()),
        sha256: Some(sha256),
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
        source_path: None,
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

fn summary_from_plugin_source(
    manifest: &PluginPackManifest,
    source: PackSource,
    source_path: Option<&PathBuf>,
    sha256: String,
) -> InstalledPackSummary {
    InstalledPackSummary {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        kind: "plugin".to_string(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        source: source.as_str().to_string(),
        source_path: source_path.map(|path| path.display().to_string()),
        sha256: Some(sha256),
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

    fn temp_pack_manager_with_override() -> (tempfile::TempDir, PackManager, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache =
            LocalPackCache::from_root(temp.path().join("data").join("packs")).expect("cache");
        let override_root = temp.path().join("overrides");
        let manager = PackManager::new_with_cache_and_roots(cache, vec![override_root.clone()])
            .expect("pack manager");
        (temp, manager, override_root)
    }

    fn builtin_runtime_manifests() -> Vec<RuntimePackManifest> {
        builtin_resource_paths()
            .into_iter()
            .filter(|path| is_builtin_manifest_path(path, PackKind::Runtime))
            .map(|path| {
                parse_runtime_pack_manifest(get_builtin_resource(path).expect("runtime manifest"))
                    .expect("runtime manifest parses")
            })
            .collect()
    }

    fn builtin_harness_manifests() -> Vec<HarnessPackManifest> {
        builtin_resource_paths()
            .into_iter()
            .filter(|path| is_builtin_manifest_path(path, PackKind::Harness))
            .map(|path| {
                parse_harness_pack_manifest(get_builtin_resource(path).expect("harness manifest"))
                    .expect("harness manifest parses")
            })
            .collect()
    }

    fn first_runtime_manifest() -> RuntimePackManifest {
        builtin_runtime_manifests()
            .into_iter()
            .next()
            .expect("compiled builtin runtime")
    }

    fn first_harness_manifest() -> HarnessPackManifest {
        builtin_harness_manifests()
            .into_iter()
            .next()
            .expect("compiled builtin harness")
    }

    fn runtime_manifest_with_commands() -> RuntimePackManifest {
        builtin_runtime_manifests()
            .into_iter()
            .find(|manifest| !manifest.commands.is_empty())
            .expect("runtime with commands")
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
    fn parses_all_compiled_builtin_runtime_and_harness_manifests() {
        let runtimes = builtin_runtime_manifests();
        let harnesses = builtin_harness_manifests();
        assert!(!runtimes.is_empty());
        assert!(!harnesses.is_empty());

        for manifest in &runtimes {
            validate_runtime_manifest_fields(manifest).expect("runtime fields");
            assert!(manifest.builtin);
            assert_eq!(manifest.pack_type, RUNTIME_PACK_TYPE);
            assert!(!manifest.runtime.kind.trim().is_empty());
            assert!(!manifest.executor.kind.trim().is_empty());
            assert!(!manifest.default_harness.is_empty());

            let envelope = read_pack_resource_text(
                PackKind::Runtime,
                &manifest.id,
                &manifest.version,
                &manifest.prompt_envelope,
            )
            .expect("runtime envelope resource");
            let envelope: serde_json::Value =
                serde_json::from_str(&envelope).expect("envelope json");
            assert!(envelope["allowedFiles"]
                .as_array()
                .is_some_and(|files| !files.is_empty()));
        }

        for manifest in &harnesses {
            validate_harness_manifest_fields(manifest).expect("harness fields");
            assert!(manifest.builtin);
            assert_eq!(manifest.pack_type, HARNESS_PACK_TYPE);
            let policy = read_pack_resource_text(
                PackKind::Harness,
                &manifest.id,
                &manifest.version,
                &manifest.prompt_policy,
            )
            .expect("harness policy resource");
            serde_json::from_str::<serde_json::Value>(&policy).expect("policy json");
        }
    }

    #[test]
    fn runtime_catalog_resolves_default_runtime_and_default_harness_without_pack_ids() {
        let (_temp, manager) = temp_pack_manager();
        let catalog = manager.runtime_catalog_manifests().expect("catalog");
        assert!(!catalog.is_empty());

        let resolved = manager.resolve_default_runtime_packs().expect("resolved");
        assert_eq!(
            resolved.runtime.manifest.runtime.kind,
            catalog[0].runtime.kind
        );
        assert!(resolved.runtime.source == PackSource::CompiledBuiltin);
        assert!(resolved.harness.source == PackSource::CompiledBuiltin);
        assert!(resolved.runtime.manifest_path.ends_with(MANIFEST_FILE_NAME));
        assert!(resolved.harness.manifest_path.ends_with(MANIFEST_FILE_NAME));
    }

    #[test]
    fn runtime_override_shadows_compiled_builtin_for_same_pack_identity() {
        let (_temp, manager, override_root) = temp_pack_manager_with_override();
        let mut manifest = first_runtime_manifest();
        manifest.name = "Override Runtime".to_string();
        let pack_dir = override_root
            .join(PackKind::Runtime.cache_dir_name())
            .join(&manifest.id)
            .join(&manifest.version);
        fs::create_dir_all(&pack_dir).expect("pack dir");
        fs::write(
            pack_dir.join(MANIFEST_FILE_NAME),
            serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
        )
        .expect("write manifest");

        let resolved = manager
            .resolver()
            .resolve_runtime(&manifest.id, &manifest.version)
            .expect("override runtime");

        assert_eq!(resolved.manifest.name, "Override Runtime");
        assert_eq!(resolved.source, PackSource::RuntimeOverride);
        assert_eq!(resolved.source_path.as_deref(), Some(pack_dir.as_path()));
    }

    #[test]
    fn installs_runtime_pack_archive_and_resolves_it_from_cache() {
        let (_temp, manager) = temp_pack_manager();
        let manifest = first_runtime_manifest();
        let raw = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
        let archive = zip_with_entries(&[(MANIFEST_FILE_NAME, raw.as_slice())]);

        let summary = manager
            .install_pack_archive(PackKind::Runtime, &manifest.id, &manifest.version, &archive)
            .expect("install archive");
        let resolved = manager
            .resolver()
            .resolve_runtime(&manifest.id, &manifest.version)
            .expect("resolve installed");

        assert_eq!(summary.id, manifest.id);
        assert_eq!(summary.source, "registry");
        assert_eq!(resolved.source, PackSource::Cache);
        assert_eq!(resolved.manifest.id, manifest.id);
    }

    #[test]
    fn pack_archive_blocks_path_traversal_before_install() {
        let (_temp, manager) = temp_pack_manager();
        let manifest = first_runtime_manifest();
        let raw = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
        let archive = zip_with_entries(&[
            (MANIFEST_FILE_NAME, raw.as_slice()),
            ("../evil.txt", b"evil"),
        ]);

        let result = manager.install_pack_archive(
            PackKind::Runtime,
            &manifest.id,
            &manifest.version,
            &archive,
        );

        assert!(matches!(result, Err(PackError::InvalidArchive(_))));
    }

    #[test]
    fn pack_archive_rejects_too_many_entries() {
        let (_temp, manager) = temp_pack_manager();
        let manifest = first_runtime_manifest();
        let raw = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
        let cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(MANIFEST_FILE_NAME, options)
            .expect("manifest");
        std::io::Write::write_all(&mut zip, &raw).expect("write manifest");
        for index in 0..MAX_PACK_ARCHIVE_ENTRIES {
            zip.start_file(format!("extra/{index}.txt"), options)
                .expect("extra");
            std::io::Write::write_all(&mut zip, b"x").expect("write extra");
        }
        let archive = zip.finish().expect("finish zip").into_inner();

        let result = manager.install_pack_archive(
            PackKind::Runtime,
            &manifest.id,
            &manifest.version,
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
        let manifest = first_runtime_manifest();
        let oversized = vec![b'{'; (MAX_PACK_MANIFEST_BYTES + 1) as usize];
        let archive = zip_with_entries(&[(MANIFEST_FILE_NAME, &oversized)]);

        let result = manager.install_pack_archive(
            PackKind::Runtime,
            &manifest.id,
            &manifest.version,
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
        let manifest = first_runtime_manifest();
        let raw = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
        let archive = zip_with_entries(&[(MANIFEST_FILE_NAME, raw.as_slice())]);

        let result = manager.install_pack_archive(
            PackKind::Runtime,
            "example.runtime.other",
            &manifest.version,
            &archive,
        );

        assert!(matches!(result, Err(PackError::InvalidManifest(_))));
    }

    #[test]
    fn list_installed_packs_includes_source_metadata() {
        let (_temp, manager) = temp_pack_manager();
        let packs = manager.list_installed_packs().expect("packs");

        assert!(packs
            .iter()
            .any(|pack| pack.kind == "runtime" && pack.source == "compiled-builtin"));
        assert!(packs
            .iter()
            .any(|pack| pack.kind == "harness" && pack.source == "compiled-builtin"));
        assert!(packs
            .iter()
            .all(|pack| pack.sha256.as_deref().is_some_and(|hash| !hash.is_empty())));
    }

    #[test]
    fn rejects_runtime_manifest_with_wrong_schema_version() {
        let mut manifest = serde_json::to_value(first_runtime_manifest()).expect("json");
        manifest["schemaVersion"] = serde_json::Value::String("2.0".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("schemaVersion"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_with_non_local_bind() {
        let mut manifest = serde_json::to_value(first_runtime_manifest()).expect("json");
        manifest["runtime"]["bind"] = serde_json::Value::String("0.0.0.0".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("127.0.0.1"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_with_remote_network() {
        let mut manifest = serde_json::to_value(first_runtime_manifest()).expect("json");
        manifest["runtime"]["network"] = serde_json::Value::String("remote".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("local-only"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_generated_root_traversal() {
        let mut manifest = serde_json::to_value(first_runtime_manifest()).expect("json");
        manifest["runtime"]["generatedRoot"] = serde_json::Value::String("../outside".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("generatedRoot"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_windows_absolute_generated_root() {
        let mut manifest = serde_json::to_value(first_runtime_manifest()).expect("json");
        manifest["runtime"]["generatedRoot"] =
            serde_json::Value::String("C:/Users/example".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("relative"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_backslash_generated_root() {
        let mut manifest = serde_json::to_value(first_runtime_manifest()).expect("json");
        manifest["runtime"]["generatedRoot"] =
            serde_json::Value::String("generated\\static".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("forward slash"))
        );
    }

    #[test]
    fn rejects_runtime_manifest_with_invalid_default_harness_id() {
        let mut manifest = serde_json::to_value(first_runtime_manifest()).expect("json");
        manifest["defaultHarness"] =
            serde_json::Value::Array(vec![serde_json::Value::String("../bad".to_string())]);

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(matches!(result, Err(PackError::InvalidPackId(id)) if id == "../bad"));
    }

    #[test]
    fn rejects_harness_manifest_with_invalid_runtime_id() {
        let mut manifest = serde_json::to_value(first_harness_manifest()).expect("json");
        manifest["runtime"] = serde_json::Value::String("../bad".to_string());

        let result = parse_harness_pack_manifest(&manifest.to_string());

        assert!(matches!(result, Err(PackError::InvalidPackId(id)) if id == "../bad"));
    }

    #[test]
    fn rejects_command_spec_without_sidecar_placeholder() {
        let mut manifest = serde_json::to_value(runtime_manifest_with_commands()).expect("json");
        let command_name = manifest["commands"]
            .as_object()
            .and_then(|commands| commands.keys().next().cloned())
            .expect("command name");
        manifest["commands"][&command_name]["executable"] =
            serde_json::Value::String("pnpm".to_string());

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("${sidecar:"))
        );
    }

    #[test]
    fn rejects_command_spec_with_network_enabled() {
        let mut manifest = serde_json::to_value(runtime_manifest_with_commands()).expect("json");
        let command_name = manifest["commands"]
            .as_object()
            .and_then(|commands| commands.keys().next().cloned())
            .expect("command name");
        manifest["commands"][&command_name]["allowedNetwork"] = serde_json::Value::Bool(true);

        let result = parse_runtime_pack_manifest(&manifest.to_string());

        assert!(
            matches!(result, Err(PackError::InvalidManifest(message)) if message.contains("allowedNetwork"))
        );
    }

    #[test]
    fn missing_pack_returns_clear_error() {
        let (_temp, manager) = temp_pack_manager();
        let result = manager
            .resolver
            .resolve_runtime("example.runtime.missing", "0.1.0");

        assert!(matches!(
            result,
            Err(PackError::MissingPack { kind: PackKind::Runtime, id, version })
                if id == "example.runtime.missing" && version == "0.1.0"
        ));
    }

    #[test]
    fn pack_cache_path_traversal_is_blocked() {
        let (_temp, manager) = temp_pack_manager();
        let result = manager
            .cache
            .pack_dir(PackKind::Runtime, "example.runtime.test", "../0.1.0");
        assert!(matches!(result, Err(PackError::InvalidVersion(_))));

        let result = manager
            .cache
            .pack_dir(PackKind::Runtime, "../example.runtime.test", "0.1.0");
        assert!(matches!(result, Err(PackError::InvalidPackId(_))));
    }

    #[test]
    fn cached_pack_versions_are_immutable() {
        let (_temp, manager) = temp_pack_manager();
        let mut manifest = first_runtime_manifest();
        manager
            .cache
            .install_runtime_manifest(&manifest)
            .expect("install original");
        manifest.description = "changed".to_string();

        let result = manager.cache.install_runtime_manifest(&manifest);

        assert!(matches!(result, Err(PackError::ImmutableVersion(_))));
    }

    #[test]
    fn resolver_rejects_cached_runtime_manifest_with_unsafe_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache =
            LocalPackCache::from_root(temp.path().join("data").join("packs")).expect("cache");
        let mut manifest = first_runtime_manifest();
        manifest.id = "example.runtime.unsafe".to_string();
        let pack_dir = cache
            .pack_dir(PackKind::Runtime, &manifest.id, &manifest.version)
            .expect("pack dir");
        fs::create_dir_all(&pack_dir).expect("pack dir");
        let mut value = serde_json::to_value(&manifest).expect("json");
        value["runtime"]["bind"] = serde_json::Value::String("0.0.0.0".to_string());
        fs::write(pack_dir.join(MANIFEST_FILE_NAME), value.to_string()).expect("write");

        let resolver = PackResolver::new(cache, Vec::new());
        let result = resolver.resolve_runtime(&manifest.id, &manifest.version);

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
        let manifest = first_runtime_manifest();
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).expect("outside");
        fs::write(
            outside.join(MANIFEST_FILE_NAME),
            serde_json::to_string_pretty(&manifest).expect("manifest"),
        )
        .expect("manifest");

        let runtime_parent = cache
            .root()
            .join(PackKind::Runtime.cache_dir_name())
            .join("example.runtime.linked");
        fs::create_dir_all(&runtime_parent).expect("runtime parent");
        std::os::unix::fs::symlink(&outside, runtime_parent.join(&manifest.version))
            .expect("symlink");
        let resolver = PackResolver::new(cache, Vec::new());
        let result = resolver.resolve_runtime("example.runtime.linked", &manifest.version);

        assert!(matches!(result, Err(PackError::PathEscape(_))));
    }
}
