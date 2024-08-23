use chrono::DateTime;
use chrono::Utc;
use dioxus_logger::tracing::warn;
use ferinth::structures::search::Facet;
use ferinth::structures::search::Sort;
use ferinth::structures::version::DependencyType;
use furse::structures::file_structs::FileRelationType;
use furse::structures::file_structs::HashAlgo;
use futures::StreamExt;
use futures::TryFutureExt;
use once_cell::sync::Lazy;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::Reverse;
use std::fs::create_dir_all;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use snafu::prelude::*;

use async_recursion::async_recursion;

use crate::api::backend_exclusive::download::extract_filename;
use crate::api::backend_exclusive::download::save_url;
use crate::api::backend_exclusive::download::DownloadError;
use crate::api::backend_exclusive::errors::ManifestProcessingError;
use crate::api::backend_exclusive::vanilla::version::get_version_manifest;
use crate::api::backend_exclusive::vanilla::version::VersionMetadata;
use crate::api::shared_resources::collection::ModLoader;
use crate::api::{
    backend_exclusive::{
        download::{get_hash, validate_sha1, DownloadArgs, HandlesType},
        vanilla::version::VersionType,
    },
    shared_resources::collection::ModLoaderType,
};

#[derive(Snafu, Debug)]
pub enum ModError {
    #[snafu(display("Could not read file at: {path:?}"))]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },
    #[snafu(context(false), display("Failed to download"))]
    Download { source: DownloadError },

    #[snafu(transparent)]
    ManifestProcessingError { source: ManifestProcessingError },

    #[snafu(display("Mod: {minecraft_mod} has disabled sharing"))]
    SharingDisabled { minecraft_mod: String },
    #[snafu(display("{name} has failed to be look up using modrinth"))]
    HashScanFailrue { source: FetchError, name: String },
    #[snafu(display("look up of {name} failed"))]
    ModNotExist { source: FetchError, name: String },
    #[snafu(display("Fail to Search"))]
    SearchFails { source: FetchError },
    #[snafu(transparent)]
    GeneralFetchError { source: FetchError },
    #[snafu(display("Tokio spawning Errors"))]
    TokioSpawning { source: tokio::task::JoinError },
    #[snafu(display(
        "Can not find a suitable mod for {mod_loader} modloader and {project} project"
    ))]
    ProjectModloaderLookup {
        mod_loader: ModLoaderType,
        project: String,
    },
}

#[derive(Snafu, Debug)]
pub enum FetchError {
    #[snafu(transparent)]
    Ferinth { source: ferinth::Error },
    #[snafu(transparent)]
    Furse { source: furse::Error },
}

pub type ModrinthSearchResponse = ferinth::structures::search::Response;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModMetadata {
    pub name: String,
    pub project_id: ProjectId,
    pub enabled: bool,
    pub authors: Vec<String>,
    pub long_description: String,
    pub short_description: String,
    pub mod_version: Option<String>,
    pub tag: Vec<Tag>,
    pub overrides: Vec<ModOverride>,
    pub incompatiable_mods: Option<Vec<ModMetadata>>,
    pub icon_url: Option<Url>,
    pub last_updated: DateTime<Utc>,
    // pub project: Project,
    icon_directory: PathBuf,
    game_directory: PathBuf,
    mod_data: RawModData,
}

pub enum Platform {
    Modrinth,
    Curseforge,
}

impl ModMetadata {
    pub fn get_icon_path(&self) -> Option<PathBuf> {
        self.icon_url.as_ref().map(|icon| {
            self.icon_directory.join(format!(
                "{}_{}",
                self.project_id.to_string(),
                extract_filename(icon.to_string()).unwrap()
            ))
        })
    }

    pub fn platform(&self) -> Platform {
        match self.mod_data {
            RawModData::Modrinth(_) => Platform::Modrinth,
            RawModData::Curseforge { .. } => Platform::Curseforge,
        }
    }

    pub async fn disable(&mut self) -> io::Result<()> {
        for file in self
            .get_filepaths()?
            .into_iter()
            .filter(|x| x.extension() != Some("disabled".as_ref()))
        {
            let new = file
                .extension()
                .map(|original| {
                    let mut os_str = original.to_os_string();
                    os_str.push(".disabled");
                    file.with_extension(os_str)
                })
                .unwrap();
            tokio::fs::rename(&file, new).await?;
        }
        self.enabled = false;
        Ok(())
    }

    pub async fn enable(&mut self) -> io::Result<()> {
        for file in self
            .get_filepaths()?
            .into_iter()
            .filter(|x| x.extension() == Some("disabled".as_ref()))
        {
            tokio::fs::rename(&file, file.with_extension("")).await?;
        }
        self.enabled = true;
        Ok(())
    }

    pub fn get_filepaths(&self) -> io::Result<Vec<PathBuf>> {
        let mut read = std::fs::read_dir(self.base_path())?
            .map(|x| x.map(|x| x.path()))
            .collect::<io::Result<Vec<_>>>()?;
        let target = match &self.mod_data {
            RawModData::Modrinth(x) => x
                .files
                .iter()
                .map(|x| self.base_path().join(&x.filename))
                .collect(),
            RawModData::Curseforge { data, .. } => {
                vec![self.base_path().join(&data.file_name)]
            }
        };
        read.retain(|x| target.contains(x) || target.contains(&x.with_extension("")));
        Ok(read)
    }

    pub fn base_path(&self) -> PathBuf {
        self.game_directory.join("mods")
    }

    pub async fn needs_update(&self) -> Result<bool, ModError> {
        let ferinth = &FERINTH;
        let furse = &FURSE;
        Ok(match &self.mod_data {
            RawModData::Modrinth(modrinth) => {
                let new = ferinth
                    .get_project(&modrinth.project_id)
                    .await
                    .map_err(Into::into)
                    .context(ModNotExistSnafu {
                        name: &modrinth.project_id,
                    })?;
                new.updated > self.last_updated
            }
            RawModData::Curseforge { data, .. } => {
                let new = furse
                    .get_mod(data.mod_id)
                    .await
                    .map_err(Into::into)
                    .context(ModNotExistSnafu {
                        name: data.mod_id.to_string(),
                    })?;
                new.date_modified > self.last_updated
            }
        })
    }

    pub async fn icon_downloaded(&self) -> Option<bool> {
        if let Some(path) = self.get_icon_path() {
            Some(path.exists())
        } else {
            None
        }
    }

    /// game_directory is from `collection.game_directory`
    pub async fn mod_is_downloaded(&self) -> bool {
        match &self.mod_data {
            RawModData::Modrinth(x) => {
                tokio_stream::iter(x.files.iter())
                    .all(|file| async move {
                        let base_path = self.base_path();
                        let hash = &file.hashes.sha1;
                        let filename = &file.filename;
                        let path = base_path.join(filename);
                        validate_sha1(&path, hash).await.is_ok()
                    })
                    .await
            }
            RawModData::Curseforge { data: file, .. } => {
                let hashes = file
                    .hashes
                    .iter()
                    .filter(|x| x.algo == HashAlgo::Sha1)
                    .map(|x| x.value.clone())
                    .collect::<Vec<_>>();
                let url = file.download_url.as_ref();
                if url.is_some() {
                    let filename = &file.file_name;
                    let base_path = self.base_path();
                    let path = base_path.join(filename);
                    hashes_validate(path, &hashes).await
                } else {
                    // FIXME: returns true for non
                    true
                }
            }
        }
    }
}

impl PartialOrd for ModMetadata {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if let (Some(x), Some(y)) = (&self.mod_version, &other.mod_version) {
            if let (Ok(x), Ok(y)) = (semver::Version::parse(x), semver::Version::parse(y)) {
                return Some(x.cmp(&y));
            }
        }
        Some(self.last_updated.cmp(&other.last_updated))
    }
}

impl PartialEq for ModMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.project_id == other.project_id
    }
}

impl Eq for ModMetadata {}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ProjectId {
    Modrinth(String),
    Curseforge(i32),
}

impl ToString for ProjectId {
    fn to_string(&self) -> String {
        match self {
            Self::Modrinth(x) => x.to_string(),
            Self::Curseforge(x) => x.to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, derive_more::From)]
pub enum RawModData {
    Modrinth(ferinth::structures::version::Version),
    Curseforge {
        data: furse::structures::file_structs::File,
        metadata: furse::structures::file_structs::FileIndex,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, derive_more::From)]
pub enum Project {
    Modrinth(ferinth::structures::project::Project),
    Curseforge(furse::structures::mod_structs::Mod),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModManager {
    pub mods: Vec<ModMetadata>,
    #[serde(skip)]
    cache: Option<Vec<VersionMetadata>>,

    game_directory: PathBuf,
    icon_directory: PathBuf,
    mod_loader: ModLoader,
    target_game_version: VersionMetadata,
}

pub static FERINTH: Lazy<ferinth::Ferinth> = Lazy::new(ferinth::Ferinth::default);
pub static FURSE: Lazy<furse::Furse> =
    Lazy::new(|| furse::Furse::new("$2a$10$bL4bIL5pUWqfcO7KQtnMReakwtfHbNKh6v1uTpKlzhwoueEJQnPnm"));

impl PartialEq for ModManager {
    fn eq(&self, other: &Self) -> bool {
        self.mods == other.mods
    }
}
impl Eq for ModManager {}

impl ModManager {
    #[must_use]
    pub fn new(
        game_directory: PathBuf,
        icon_directory: PathBuf,
        mod_loader: ModLoader,
        target_game_version: VersionMetadata,
    ) -> Self {
        Self {
            mods: Vec::new(),
            cache: None,
            game_directory,
            mod_loader,
            target_game_version,
            icon_directory,
        }
    }

    pub async fn all_downloaded(&self) -> bool {
        tokio_stream::iter(self.mods.iter())
            .all(|minecraft_mod| minecraft_mod.mod_is_downloaded())
            .await
    }

    pub async fn get_download(&self) -> Result<DownloadArgs, ModError> {
        let current_size = Arc::new(AtomicUsize::new(0));
        let total_size = Arc::new(AtomicUsize::new(0));
        let base_path = self.game_directory.join("mods");
        let icon_base_path = &self.icon_directory;
        if !base_path.exists() {
            create_dir_all(&base_path).context(IoSnafu {
                path: base_path.clone(),
            })?;
        }
        if !icon_base_path.exists() {
            create_dir_all(&icon_base_path).context(IoSnafu {
                path: icon_base_path,
            })?;
        }
        let mut handles = HandlesType::new();
        for minecraft_mod in &self.mods {
            let icon = minecraft_mod.icon_url.clone();
            let icon_path = minecraft_mod.get_icon_path();
            let current_size_clone = Arc::clone(&current_size);
            handles.push(async move {
                if let (Some(icon), Some(icon_path)) = (icon, icon_path) {
                    if !icon_path.exists() {
                        save_url(icon, current_size_clone, icon_path).await?;
                    }
                }
                Ok(())
            });
            match &minecraft_mod.mod_data {
                RawModData::Modrinth(minecraft_mod) => {
                    for file in &minecraft_mod.files {
                        let hash = file.hashes.sha1.clone();
                        let url = file.url.clone();
                        let filename = file.filename.clone();
                        let path = base_path.join(filename.clone());
                        let size = file.size;
                        let current_size_clone = Arc::clone(&current_size);
                        let total_size_clone = Arc::clone(&total_size);
                        let mod_writer =
                            save_url(url, current_size_clone, path.clone()).map_err(Into::into);
                        if !path.exists() {
                            total_size_clone.fetch_add(size, Ordering::Relaxed);
                            handles.push(mod_writer);
                        } else if let Err(x) = validate_sha1(&path, &hash).await {
                            warn!("Redownloading {x}");
                            total_size_clone.fetch_add(size, Ordering::Relaxed);
                            handles.push(mod_writer);
                        }
                    }
                }
                RawModData::Curseforge { data: file, .. } => {
                    let hashes = file
                        .hashes
                        .clone()
                        .into_iter()
                        .filter(|x| x.algo == HashAlgo::Sha1)
                        .map(|x| x.value)
                        .collect::<Vec<_>>();
                    let filename = file.file_name.clone();
                    let url = file.download_url.clone().context(SharingDisabledSnafu {
                        minecraft_mod: filename.clone(),
                    })?;
                    let path = base_path.join(filename);
                    let size = file.file_length;
                    let current_size_clone = Arc::clone(&current_size);
                    let total_size_clone = Arc::clone(&total_size);
                    let mod_writer =
                        save_url(url, current_size_clone, path.clone()).map_err(Into::into);
                    if !path.exists() {
                        total_size_clone.fetch_add(size, Ordering::Relaxed);
                        handles.push(mod_writer);
                    } else if !hashes_validate(&path, &hashes).await {
                        total_size_clone.fetch_add(size, Ordering::Relaxed);
                        handles.push(mod_writer);
                    }
                }
            }
        }

        let download_args = DownloadArgs {
            current: Arc::clone(&current_size),
            total: Arc::clone(&total_size),
            handles,
            is_size: true,
        };

        Ok(download_args)
    }

    // TODO: furse mode not implemetned yet
    pub async fn scan(&mut self) -> Result<(), ModError> {
        let ferinth = &FERINTH;
        let mod_path = self.game_directory.join("mods");
        if !mod_path.exists() {
            create_dir_all(&mod_path).context(IoSnafu {
                path: mod_path.clone(),
            })?;
        }
        let dirs = mod_path.read_dir().context(IoSnafu {
            path: mod_path.clone(),
        })?;
        for mod_entry in dirs {
            let mod_entry = mod_entry.context(IoSnafu {
                path: PathBuf::new(),
            })?;
            let path = mod_entry.path();
            let raw_hash = get_hash(&path).await?;
            let hash = hex::encode(raw_hash);
            let already_contained_in_collection = self.mods.iter().any(|x| match &x.mod_data {
                RawModData::Modrinth(x) => x.files.iter().any(|x| x.hashes.sha1 == hash),
                RawModData::Curseforge { data, .. } => data
                    .hashes
                    .iter()
                    .filter(|x| x.algo == HashAlgo::Sha1)
                    .any(|x| x.value == hash),
            });
            if !already_contained_in_collection {
                let path_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let version = ferinth
                    .get_version_from_hash(&hash)
                    .await
                    .map_err(Into::into)
                    .context(HashScanFailrueSnafu { name: path_name })?;
                self.add_mod(version.into(), vec![Tag::Explicit], Vec::new())
                    .await?;
            }
        }

        Ok(())
    }

    #[async_recursion]
    async fn mod_dependencies_resolve(
        &mut self,
        minecraft_mod: &RawModData,
        mod_override: Vec<ModOverride>,
    ) -> Result<(), ModError> {
        match minecraft_mod {
            RawModData::Modrinth(minecraft_mod) => {
                for dept in &minecraft_mod.dependencies {
                    let mod_override = mod_override.clone();
                    if dept.dependency_type == DependencyType::Required {
                        if let Some(dependency) = &dept.version_id {
                            let ver = FERINTH
                                .get_version(dependency)
                                .await
                                .map_err(Into::into)
                                .context(ModNotExistSnafu {
                                    name: dependency.clone(),
                                })?;
                            self.add_mod(ver.into(), vec![Tag::Dependencies], mod_override)
                                .await?;
                        } else if let Some(project) = &dept.project_id {
                            let ver = FERINTH
                                .get_project(project)
                                .await
                                .map_err(Into::into)
                                .context(ModNotExistSnafu {
                                    name: project.clone(),
                                })?;
                            self.add_project(ver.into(), vec![Tag::Dependencies], mod_override)
                                .await?;
                        }
                    }
                }
            }
            RawModData::Curseforge {
                data: minecraft_mod,
                ..
            } => {
                for dept in &minecraft_mod.dependencies {
                    let mod_override = mod_override.clone();
                    if dept.relation_type == FileRelationType::RequiredDependency {
                        let project = FURSE
                            .get_mod(dept.mod_id)
                            .await
                            .map_err(Into::into)
                            .context(ModNotExistSnafu {
                                name: dept.mod_id.to_string(),
                            })?;
                        self.add_project(project.into(), vec![Tag::Dependencies], mod_override)
                            .await?;
                    }
                }
            }
        }
        Ok(())
    }

    #[async_recursion]
    async fn add_mod(
        &mut self,
        minecraft_mod_data: RawModData,
        tag: Vec<Tag>,
        mod_override: Vec<ModOverride>,
    ) -> Result<(), ModError> {
        let (new, is_fabric_api) = Self::raw_mod_transformation(
            self.icon_directory.clone(),
            self.game_directory.clone(),
            minecraft_mod_data,
            mod_override.clone(),
            tag,
        )
        .await?;

        if let Some(previous) = self.mods.iter_mut().find(|x| **x == new) {
            if new > *previous {
                *previous = new;
            }
        } else {
            self.mod_dependencies_resolve(&new.mod_data, mod_override)
                .await?;
            if self.mod_loader.mod_loader_type == ModLoaderType::Quilt && is_fabric_api {
                let project = (&FERINTH)
                    .get_project("qsl")
                    .await
                    .map_err(Into::into)
                    .context(ModNotExistSnafu {
                        name: "qsl".to_string(),
                    })?;
                self.add_project(project.into(), vec![Tag::Dependencies], vec![])
                    .await?;
            } else {
                self.mods.push(new);
            }
        }
        Ok(())
    }

    async fn add_multiple_mods(
        &mut self,
        minecraft_mod_data: Vec<RawModData>,
        tag: Vec<Tag>,
        mod_override: Vec<ModOverride>,
    ) -> Result<(), ModError> {
        let buffered = tokio_stream::iter(minecraft_mod_data.into_iter())
            .map(|mod_metadata| {
                let mod_override_cloned = mod_override.clone();
                let icon_directory_cloned = self.icon_directory.clone();
                let game_directory_cloned = self.game_directory.clone();
                let tag_cloned = tag.clone();
                tokio::spawn(async move {
                    let (mod_metadata, is_fabric_api) = Self::raw_mod_transformation(
                        icon_directory_cloned,
                        game_directory_cloned,
                        mod_metadata,
                        mod_override_cloned,
                        tag_cloned,
                    )
                    .await?;
                    Ok((mod_metadata, is_fabric_api))
                })
            })
            .buffered(10)
            .collect::<Vec<_>>();
        let minecraft_mod_data = buffered
            .await
            .into_iter()
            .flatten()
            .collect::<Result<Vec<_>, ModError>>()?;
        for (mod_metadata, is_fabric_api) in minecraft_mod_data {
            if !self.mods.contains(&mod_metadata) {
                self.mod_dependencies_resolve(&mod_metadata.mod_data, mod_override.clone())
                    .await?;
                if self.mod_loader.mod_loader_type == ModLoaderType::Quilt && is_fabric_api {
                    let project = (&FERINTH)
                        .get_project("qsl")
                        .await
                        .map_err(Into::into)
                        .context(ModNotExistSnafu {
                            name: "qsl".to_string(),
                        })?;
                    self.add_project(project.into(), vec![Tag::Dependencies], vec![])
                        .await?;
                } else {
                    self.mods.push(mod_metadata);
                }
            }
        }
        Ok(())
    }

    async fn raw_mod_transformation(
        icon_directory: PathBuf,
        game_directory: PathBuf,
        minecraft_mod_data: RawModData,
        mod_override: Vec<ModOverride>,
        tag: Vec<Tag>,
    ) -> Result<(ModMetadata, bool), ModError> {
        let mut is_fabric_api = false;
        let mod_metadata = match &minecraft_mod_data {
            RawModData::Modrinth(minecraft_mod) => {
                if minecraft_mod.project_id == "P7dR8mSH" {
                    is_fabric_api = true;
                }
                let project = FERINTH
                    .get_project(&minecraft_mod.project_id)
                    .await
                    .map_err(Into::into)
                    .context(ModNotExistSnafu {
                        name: minecraft_mod.project_id.to_string(),
                    })?;
                let memebers = FERINTH
                    .list_team_members(&project.team)
                    .await
                    .map_err(Into::<FetchError>::into)?;
                let authors = memebers.into_iter().map(|x| x.user.username).collect();
                ModMetadata {
                    name: project.title.clone(),
                    project_id: ProjectId::Modrinth(minecraft_mod.project_id.clone()),
                    long_description: project.body.clone(),
                    short_description: project.description.clone(),
                    mod_version: Some(minecraft_mod.version_number.clone()),
                    tag,
                    incompatiable_mods: None,
                    overrides: mod_override,
                    mod_data: minecraft_mod_data,
                    icon_url: project.icon_url.clone(),
                    icon_directory,
                    last_updated: project.updated,
                    authors,
                    enabled: true,
                    game_directory,
                    // project: Project::Modrinth(project),
                }
            }
            RawModData::Curseforge {
                data: minecraft_mod,
                ..
            } => {
                let project = FURSE
                    .get_mod(minecraft_mod.mod_id)
                    .await
                    .map_err(Into::into)
                    .context(ModNotExistSnafu {
                        name: minecraft_mod.mod_id.to_string(),
                    })?;
                let icon_url = project
                    .logo
                    .clone()
                    .map(|x| Url::parse(&x.thumbnail_url))
                    .transpose()
                    .expect("not valid url");
                let authors = project.authors.into_iter().map(|x| x.name).collect();
                ModMetadata {
                    name: project.name.clone(),
                    project_id: ProjectId::Curseforge(minecraft_mod.mod_id),
                    long_description: FURSE
                        .get_mod_description(project.id)
                        .await
                        .map_err(Into::<FetchError>::into)?,
                    short_description: project.summary.clone(),
                    mod_version: None,
                    tag,
                    incompatiable_mods: None,
                    overrides: mod_override,
                    mod_data: minecraft_mod_data,
                    icon_url,
                    icon_directory,
                    last_updated: project.date_modified,
                    authors,
                    enabled: true,
                    game_directory,
                    // project: Project::Curseforge(project),
                }
            }
        };
        Ok((mod_metadata, is_fabric_api))
    }

    pub async fn search_modrinth_project(
        &self,
        query: &str,
    ) -> Result<ModrinthSearchResponse, ModError> {
        let mod_loader = &self.mod_loader;
        let mod_loader_facet = Facet::Categories(mod_loader.to_string());
        let version_facet = Facet::Versions(self.target_game_version.id.to_string());
        let search_hits = FERINTH
            .search(
                query,
                &Sort::Relevance,
                vec![vec![mod_loader_facet, version_facet]],
            )
            .await
            .map_err(Into::into)
            .context(SearchFailsSnafu)?;
        Ok(search_hits)
    }

    async fn all_game_versions(&mut self) -> Result<&[VersionMetadata], ModError> {
        let version = get_version_manifest().await;

        if let None = self.cache {
            self.cache = Some(version?.versions);
        }

        // SAFETY: a `None` variant for `self.cache` would have been replaced by a `Some`
        // variant in the code above.
        Ok(unsafe { self.cache.as_mut().unwrap_unchecked() })
    }

    // NOTE: Hacky way of doing game version check
    #[async_recursion]
    pub async fn add_project(
        &mut self,
        project: Project,
        tag: Vec<Tag>,
        mod_override: Vec<ModOverride>,
    ) -> Result<(), ModError> {
        let target_game_version = self.target_game_version.id.clone();
        let collection_mod_loader = self.mod_loader.mod_loader_type;

        let all_game_versions = self.all_game_versions().await?;

        let collection_game_version = all_game_versions
            .iter()
            .find(|x| x.id == target_game_version)
            .expect("somehow can't find game versions");

        let name = match &project {
            Project::Modrinth(x) => x.title.clone(),
            Project::Curseforge(x) => x.name.clone(),
        };

        let versions = get_mod_version(project).await?;

        let version = fetch_version_modloader_constraints(
            &name,
            all_game_versions,
            collection_game_version,
            collection_mod_loader,
            versions,
            &mod_override,
        )?;

        self.add_mod(
            version
                .context(ProjectModloaderLookupSnafu {
                    mod_loader: collection_mod_loader,
                    project: name,
                })?
                .into(),
            tag,
            mod_override,
        )
        .await?;

        Ok(())
    }

    pub async fn add_multiple_project(
        &mut self,
        projects: Vec<Project>,
        tag: Vec<Tag>,
        mod_override: Vec<ModOverride>,
    ) -> Result<(), ModError> {
        let target_game_id = self.target_game_version.id.clone();
        let collection_mod_loader = self.mod_loader.mod_loader_type;
        let all_game_versions = self.all_game_versions().await?;

        let buffered_iterator = tokio_stream::iter(projects.into_iter().map(|project| {
            tokio::spawn(async {
                let name = match &project {
                    Project::Modrinth(x) => x.title.clone(),
                    Project::Curseforge(x) => x.name.clone(),
                };
                let version = get_mod_version(project).await;
                match version {
                    Ok(v) => Ok((name, v)),
                    Err(err) => Err(err),
                }
            })
        }))
        .buffered(10)
        .map(|x| {
            let (name, versions) = x.context(TokioSpawningSnafu)??;

            let collection_game_version = all_game_versions
                .iter()
                .find(|x| x.id == target_game_id)
                .expect("somehow can't find game versions");

            let version = fetch_version_modloader_constraints(
                &name,
                all_game_versions,
                collection_game_version,
                collection_mod_loader,
                versions,
                &mod_override,
            )?
            .context(ProjectModloaderLookupSnafu {
                mod_loader: collection_mod_loader,
                project: name,
            })?;
            Ok(version)
        })
        .collect::<Vec<_>>();

        let multiple_mods = buffered_iterator
            .await
            .into_iter()
            .collect::<Result<Vec<_>, ModError>>()?;

        self.add_multiple_mods(multiple_mods, tag, mod_override)
            .await?;

        Ok(())
    }
}

fn fetch_version_modloader_constraints(
    name: &str,
    all_game_versions: &[VersionMetadata],
    collection_game_version: &VersionMetadata,
    collection_mod_loader: ModLoaderType,
    versions: Vec<RawModData>,
    mod_override: &Vec<ModOverride>,
) -> Result<Option<RawModData>, ModError> {
    let mut mod_loader_filter = versions
        .into_iter()
        .map(|x| match x {
            RawModData::Modrinth(mut ver) => {
                ver.files = ver
                    .files
                    .into_iter()
                    .filter(|x| x.primary)
                    .collect::<Vec<_>>();
                ver.into()
            }
            RawModData::Curseforge { .. } => x,
        })
        .filter(|x| {
            if mod_override.contains(&ModOverride::IgnoreModLoader) {
                true
            } else {
                match &x {
                    RawModData::Modrinth(x) => x.loaders.iter().any(|loader| {
                        let loader = match loader.as_str() {
                            "forge" => Some(ModLoaderType::Forge),
                            "quilt" => Some(ModLoaderType::Quilt),
                            "fabric" => Some(ModLoaderType::Fabric),
                            "neoforge" => Some(ModLoaderType::NeoForge),
                            _ => None,
                        };

                        let target_quilt_compatibility =
                            mod_override.contains(&ModOverride::QuiltFabricCompatibility);

                        let neo_forge_compatibility = collection_mod_loader
                            == ModLoaderType::NeoForge
                            && loader == Some(ModLoaderType::Forge);

                        let quilt_compatible = target_quilt_compatibility
                            && collection_mod_loader == ModLoaderType::Quilt
                            && loader == Some(ModLoaderType::Fabric);
                        loader.is_some_and(|x| x == collection_mod_loader)
                            || quilt_compatible
                            || neo_forge_compatibility
                    }),
                    RawModData::Curseforge { metadata, .. } => {
                        if let Some(loader) = &metadata.mod_loader {
                            let mut any = false;
                            let loader = match loader {
                                furse::structures::common_structs::ModLoaderType::Any => {
                                    any = true;
                                    None
                                }
                                furse::structures::common_structs::ModLoaderType::Forge => {
                                    Some(ModLoaderType::Forge)
                                }
                                furse::structures::common_structs::ModLoaderType::Cauldron => None,
                                furse::structures::common_structs::ModLoaderType::LiteLoader => {
                                    None
                                }
                                furse::structures::common_structs::ModLoaderType::Fabric => {
                                    Some(ModLoaderType::Fabric)
                                }
                                furse::structures::common_structs::ModLoaderType::Quilt => {
                                    Some(ModLoaderType::Quilt)
                                }
                                furse::structures::common_structs::ModLoaderType::NeoForge => {
                                    Some(ModLoaderType::NeoForge)
                                }
                            };

                            let neo_forge_compatibility = collection_mod_loader
                                == ModLoaderType::NeoForge
                                && loader == Some(ModLoaderType::Forge);

                            let quilt_compatible = mod_override
                                .contains(&ModOverride::QuiltFabricCompatibility)
                                && collection_mod_loader == ModLoaderType::Quilt
                                && loader == Some(ModLoaderType::Fabric);
                            loader.is_some_and(|x| x == collection_mod_loader)
                                || quilt_compatible
                                || any
                                || neo_forge_compatibility
                        } else {
                            false
                        }
                    }
                }
            }
        })
        .peekable();
    if mod_loader_filter.peek().is_none() {
        return Err(ModError::ProjectModloaderLookup {
            mod_loader: collection_mod_loader,
            project: name.to_string(),
        });
    }
    let mut possible_versions = mod_loader_filter
        .filter(|x| {
            let supported_game_versions = match x {
                RawModData::Modrinth(x) => x
                    .game_versions
                    .iter()
                    .filter_map(|x| all_game_versions.iter().find(|y| &y.id == x))
                    .collect::<Vec<_>>(),
                RawModData::Curseforge { data, .. } => data
                    .game_versions
                    .iter()
                    .filter_map(|x| all_game_versions.iter().find(|y| &y.id == x))
                    .collect::<Vec<_>>(),
            };

            match &**mod_override {
                x if x.contains(&ModOverride::IgnoreAllGameVersion) => true,
                x if x.contains(&ModOverride::IgnoreMinorGameVersion)
                    && collection_game_version.version_type == VersionType::Release =>
                {
                    supported_game_versions
                        .iter()
                        .any(|x| minor_game_check(x, &collection_game_version.id))
                }
                _ => supported_game_versions
                    .iter()
                    .any(|x| x.id == collection_game_version.id),
            }
        })
        .collect::<Vec<_>>();
    possible_versions.sort_by_key(|x| match x {
        RawModData::Modrinth(x) => Reverse(x.date_published),
        RawModData::Curseforge { data, .. } => Reverse(data.file_date),
    });
    let version = if mod_override.contains(&ModOverride::IgnoreMinorGameVersion) {
        // strict game check
        let strict_check_version = possible_versions.iter().find(|x| {
            let supported_game_versions = match x {
                RawModData::Modrinth(x) => x
                    .game_versions
                    .iter()
                    .filter_map(|x| all_game_versions.iter().find(|y| &y.id == x))
                    .collect::<Vec<_>>(),
                RawModData::Curseforge { data, .. } => data
                    .game_versions
                    .iter()
                    .filter_map(|x| all_game_versions.iter().find(|y| &y.id == x))
                    .collect::<Vec<_>>(),
            };
            supported_game_versions
                .iter()
                .any(|x| x.id == collection_game_version.id)
        });
        if let Some(x) = strict_check_version {
            Some(x.clone())
        } else {
            possible_versions.into_iter().next()
        }
    } else {
        possible_versions.into_iter().next()
    };
    Ok(version)
}

async fn get_mod_version(project: Project) -> Result<Vec<RawModData>, ModError> {
    let projects = match project {
        Project::Modrinth(project) => FERINTH
            .get_multiple_versions(
                project
                    .versions
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .await
            .map_err(Into::<FetchError>::into)?
            .into_iter()
            .map(|x| x.into())
            .collect::<Vec<_>>(),
        Project::Curseforge(x) => x
            .latest_files
            .into_iter()
            .zip(x.latest_files_indexes.into_iter())
            .map(|x| x.into())
            .collect::<Vec<_>>(),
    };
    Ok(projects)
}

fn minor_game_check(version: &&VersionMetadata, game_id: &str) -> bool {
    let target_semver = semver::Version::parse(game_id).map(|x| x.minor);
    if let (Ok(supported_version), Ok(target)) = (
        semver::Version::parse(&version.id).map(|x| x.minor),
        target_semver.as_ref(),
    ) {
        &supported_version == target
    } else {
        version.id == game_id
    }
}
async fn hashes_validate(path: impl AsRef<Path>, vec: &[String]) -> bool {
    let path = path.as_ref();
    tokio_stream::iter(vec)
        .any(|x| async move { validate_sha1(path, x).await.is_ok() })
        .await
}

/// `IgnoreMinorGameVersion` will behave like `IgnoreAllGameVersion` if operated on snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ModOverride {
    // ignore minor game version, using a latest-fully-compatiable mod available
    IgnoreMinorGameVersion,
    IgnoreAllGameVersion,
    QuiltFabricCompatibility,
    IgnoreModLoader,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Tag {
    Dependencies,
    Explicit,
    Custom(String),
}
