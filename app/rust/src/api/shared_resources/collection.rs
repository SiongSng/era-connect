use std::convert::TryInto;
use std::fmt::Display;
use std::future::Future;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
pub use std::path::PathBuf;
use std::{borrow::Cow, fs::create_dir_all};

use chrono::{DateTime, Duration, Utc};
use dioxus::signals::{AnyStorage, ReadableRef, Signal, SyncSignal, UnsyncStorage};
use dioxus_logger::tracing::info;
use dioxus_radio::hooks::{use_radio, Radio, RadioChannel};
use futures::FutureExt;
use ordermap::OrderMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use snafu::prelude::*;
use tokio::fs::File;
use tokio::task::JoinHandle;

use crate::api::backend_exclusive::mod_management::mods::ModError;
use crate::api::backend_exclusive::storage::storage_loader::StorageError;
pub use crate::api::backend_exclusive::storage::storage_loader::StorageLoader;

use crate::api::backend_exclusive::vanilla::launcher::{
    LaunchGameError, LoggerEvent, LoggerEventError,
};
use crate::api::{
    backend_exclusive::{
        download::{execute_and_progress, DownloadBias, DownloadType},
        mod_management::mods::{ModManager, ModOverride, Tag, FERINTH, FURSE},
        modding::forge::fetch_launch_args_modded,
        vanilla::{
            self,
            launcher::{full_vanilla_download, LaunchArgs},
            version::VersionMetadata,
        },
    },
    shared_resources::entry::DATA_DIR,
};

#[derive(Debug, Snafu)]
pub enum CollectionError {
    // #[snafu(display("Failed to deserializae"))]
    // Desearialization { source: serde_json::Error },
    #[snafu(display("Could not read file at: {path:?}"))]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },

    #[snafu(transparent)]
    ModError {
        source: ModError,
    },
    #[snafu(display("Project {id} cannot be found on Modrinth"))]
    ModrinthNotAProject {
        id: String,
        source: ferinth::Error,
    },
    #[snafu(display("Project {id} cannot be found on Curseforge"))]
    CurseForgeNotAPorject {
        id: String,
        source: furse::Error,
    },
    StorageError {
        source: StorageError,
    },
    LaunchGameError {
        source: LaunchGameError,
    },
    #[snafu(display("Some error happenend within `with_async_mut`, error: {source:#?}"))]
    AsyncWrite {
        source: anyhow::Error,
    },
}

#[serde_with::serde_as]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
pub struct Collection {
    pub display_name: String,
    pub minecraft_version: VersionMetadata,
    pub mod_controller: Option<ModController>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde_as(as = "serde_with::DurationSeconds<i64>")]
    pub played_time: Duration,
    pub advanced_options: Option<AdvancedOptions>,
    pub picture_path: PathBuf,

    entry_path: PathBuf,
    launch_args: Option<LaunchArgs>,
}

#[derive(Default, Clone)]
pub struct Collections(pub OrderMap<CollectionId, Collection>);

// Channels used to identify the subscribers of the State
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum CollectionRadioChannel {
    WholeListSubscription,
    WithId(CollectionId),
}

pub type CollectionsRadio = Radio<Collections, CollectionRadioChannel>;

impl RadioChannel<Collections> for CollectionRadioChannel {}

#[derive(Clone, Copy)]
pub struct CollectionRadio {
    id: CollectionId,
    radio: CollectionsRadio,
}

/// The return type of `.with_async_mut()` must either be
/// a. `Base`, for a typical return that does not need to provide any extra information
/// b. if any information is needed, you can use `(Base, T)`
pub trait ClosureReturn<Base> {
    type Value;
    fn get(s: Self) -> (Base, Self::Value)
    where
        Self: Sized;
}

impl<T, Base> ClosureReturn<Base> for (Base, T) {
    type Value = T;
    fn get(s: Self) -> (Base, Self::Value) {
        s
    }
}

impl<Base> ClosureReturn<Base> for Base {
    type Value = ();
    fn get(s: Self) -> (Base, Self::Value) {
        (s, ())
    }
}

impl CollectionRadio {
    #[must_use]
    pub fn read(&self) -> ReadableRef<Signal<Collection>> {
        UnsyncStorage::try_map(self.radio.read(), |x| x.0.get(&self.id))
            .unwrap_or_else(|| panic!("id does not exist in radio, {}", self.id))
    }

    #[must_use]
    // fake alais
    pub fn peek(&self) -> ReadableRef<Signal<Collection>> {
        UnsyncStorage::try_map(self.radio.read(), |x| x.0.get(&self.id))
            .unwrap_or_else(|| panic!("id does not exist in radio, {}", self.id))
    }

    #[must_use]
    pub fn read_owned(&self) -> Collection {
        self.read().clone()
    }

    pub fn with_mut(&mut self, f: impl FnOnce(&mut Collection)) -> Result<(), CollectionError> {
        let id = self.id;
        let mut err = None;
        self.radio.write_with(|mut v| {
            if let Err(x) = v.0.get_mut(&id).unwrap().with_mut(f) {
                err = Some(x);
            }
        });
        info!("Finish radio writing");
        err.map_or_else(|| Ok(()), Err)
    }

    pub async fn with_async_mut<
        Ret: ClosureReturn<Collection>,
        Fut: Future<Output = Result<Ret, anyhow::Error>>,
    >(
        &mut self,
        f: impl FnOnce(Collection) -> Fut + Send,
    ) -> Result<Ret::Value, CollectionError> {
        let return_value = f(self.peek().clone()).await.context(AsyncWriteSnafu)?;

        let (collection, value) = Ret::get(return_value);

        self.with_mut(|x| *x = collection)?;

        Ok(value)
    }
}

#[derive(
    Debug,
    Deserialize,
    Serialize,
    Clone,
    Eq,
    PartialEq,
    Hash,
    PartialOrd,
    Ord,
    derive_more::Display,
    Copy,
)]
pub struct CollectionId(u64);

#[must_use]
pub fn use_collections_radio() -> CollectionsRadio {
    use_radio(CollectionRadioChannel::WholeListSubscription)
}

#[must_use]
pub fn use_keys() -> Vec<CollectionId> {
    use_radio(CollectionRadioChannel::WholeListSubscription)
        .read()
        .0
        .keys()
        .copied()
        .collect()
}

impl CollectionId {
    #[must_use]
    pub fn use_collection_radio(self) -> CollectionRadio {
        let radio = use_radio(CollectionRadioChannel::WithId(self));
        CollectionRadio { id: self, radio }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
pub struct ModController {
    pub loader: ModLoader,
    pub manager: ModManager,
}

impl ModController {
    pub async fn finished_downloading(&self) -> bool {
        self.manager.all_downloaded().await
    }
}

impl ModController {
    #[must_use]
    pub const fn new(loader: ModLoader, manager: ModManager) -> Self {
        Self { loader, manager }
    }
}

pub struct ScreenShot {
    pub path: PathBuf,
}

impl ScreenShot {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn get_size(&self) -> std::io::Result<Size> {
        let file = std::fs::File::open(&self.path)?;
        let size = Size::Bytes(file.metadata()?.size() as usize);
        Ok(size)
    }

    pub fn get_creation_date(&self) -> std::io::Result<DateTime<Utc>> {
        let file = std::fs::File::open(&self.path)?;
        let time = file.metadata()?.mtime();

        Ok(DateTime::from_timestamp(time, 0).unwrap())
    }
}

const COLLECTION_FILE_NAME: &str = "collection.json";
const COLLECTION_BASE: &str = "collections";

impl Collection {
    #[must_use]
    pub const fn display_name(&self) -> &String {
        &self.display_name
    }
    #[must_use]
    pub const fn minecraft_version(&self) -> &VersionMetadata {
        &self.minecraft_version
    }
    #[must_use]
    pub const fn mod_controller(&self) -> Option<&ModController> {
        self.mod_controller.as_ref()
    }
    #[must_use]
    pub const fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
    #[must_use]
    pub const fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
    #[must_use]
    pub const fn advanced_options(&self) -> Option<&AdvancedOptions> {
        self.advanced_options.as_ref()
    }
    #[must_use]
    pub fn picture_path(&self) -> &Path {
        self.picture_path.as_path()
    }
    #[must_use]
    pub fn entry_path(&self) -> &Path {
        self.entry_path.as_path()
    }

    pub(crate) fn with_mut<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> T,
    ) -> Result<T, CollectionError> {
        let v = f(self);
        self.save_collection_file()?;
        Ok(v)
    }

    /// Creates a collection and return a collection with its loader attached
    pub fn create(
        display_name: String,
        version_metadata: VersionMetadata,
        mod_loader: impl Into<Option<ModLoader>> + Send,
        picture_path: impl Into<PathBuf> + Send,
        advanced_options: Option<AdvancedOptions>,
        mut collections_radio: CollectionsRadio,
    ) -> Result<Self, CollectionError> {
        let now_time = Utc::now();
        let loader = Self::create_loader(&display_name).context(StorageSnafu)?;
        let entry_path = loader.base_path;
        let mod_controller = mod_loader.into().map(|loader| {
            let mod_manager = ModManager::new(
                entry_path.join("minecraft_root"),
                entry_path.join("mod_images"),
                loader.clone(),
                version_metadata.clone(),
            );
            ModController::new(loader, mod_manager)
        });

        let collection = Self {
            display_name,
            minecraft_version: version_metadata,
            mod_controller,
            created_at: now_time,
            updated_at: now_time,
            played_time: Duration::seconds(0),
            advanced_options,
            entry_path,
            launch_args: None,
            picture_path: picture_path.into(),
        };

        collection.save_collection_file()?;

        collections_radio.write_with(|mut v| {
            v.0.insert(collection.get_collection_id(), collection.clone());
            info!("Finish inserting collection");
        });

        Ok(collection)
    }

    pub async fn get_screenshots(&self) -> Result<Vec<ScreenShot>, CollectionError> {
        let game_directory = self.game_directory();
        let Ok(x) = game_directory.join("screenshots").read_dir() else {
            return Ok(vec![]);
        };
        x.map(|x| {
            x.context(IoSnafu {
                path: &game_directory,
            })
        })
        .map(|x| Ok::<_, CollectionError>(x?.path()))
        .map(|x| Ok(ScreenShot::new(x?)))
        .collect()
    }

    /// use project id(slug also works) to add mod, will deal with dependencies insertion
    pub async fn add_modrinth_mod(
        &mut self,
        project_id: impl AsRef<str> + Send + Sync,
        tag: Vec<Tag>,
        mod_override: Option<Vec<ModOverride>>,
    ) -> Result<(), CollectionError> {
        let project_id = project_id.as_ref();
        let project = FERINTH
            .get_project(project_id)
            .await
            .context(ModrinthNotAProjectSnafu { id: project_id })?;
        if let Some(manager) = self.mod_manager_mut() {
            manager
                .add_project(project.into(), tag, mod_override.unwrap_or_default())
                .await?;
        }
        Ok(())
    }

    pub async fn add_multiple_modrinth_mod(
        &mut self,
        project_ids: Vec<&str>,
        tag: Vec<Tag>,
        mod_override: impl Into<Option<Vec<ModOverride>>> + Send,
    ) -> Result<(), CollectionError> {
        let project = FERINTH.get_multiple_projects(&project_ids).await.context(
            ModrinthNotAProjectSnafu {
                id: project_ids.join(" "),
            },
        )?;
        if let Some(mod_controller) = self.mod_controller.as_mut() {
            mod_controller
                .manager
                .add_multiple_project(
                    project.into_iter().map(Into::into).collect::<Vec<_>>(),
                    tag.clone(),
                    mod_override.into().unwrap_or(Vec::new()),
                )
                .await?;
        }
        Ok(())
    }
    pub async fn add_curseforge_mod(
        &mut self,
        project_id: i32,
        tag: Vec<Tag>,
        mod_override: Option<Vec<ModOverride>>,
    ) -> Result<(), CollectionError> {
        let project = FURSE
            .get_mod(project_id)
            .await
            .context(CurseForgeNotAPorjectSnafu {
                id: project_id.to_string(),
            })?;
        if let Some(mod_manager) = self.mod_manager_mut() {
            mod_manager
                .add_project(project.into(), tag, mod_override.unwrap_or_default())
                .await?;
        }
        Ok(())
    }

    pub async fn download_mods(&self) -> Result<(), CollectionError> {
        let id = self.get_collection_id();
        if let Some(mod_manager) = self.mod_manager() {
            let download_args = mod_manager.get_download().await?;
            execute_and_progress(
                id,
                download_args,
                DownloadBias::default(),
                DownloadType::mods_download(),
            )
            .await;
        }
        Ok(())
    }

    /// SIDE-EFFECT: put `launch_args` into Struct
    pub async fn launch_game(
        &mut self,
        signal: SyncSignal<LoggerEvent>,
    ) -> Result<JoinHandle<Result<(), LoggerEventError>>, CollectionError> {
        if self.launch_args.is_none() {
            self.launch_args = Some(self.verify_and_download_game().await?);
        }
        vanilla::launcher::launch_game(self.launch_args.as_ref().unwrap(), signal)
            .await
            .context(LaunchGameSnafu)
    }

    pub async unsafe fn launch_game_unchecked(
        &self,
        signal: SyncSignal<LoggerEvent>,
    ) -> Result<JoinHandle<Result<(), LoggerEventError>>, CollectionError> {
        vanilla::launcher::launch_game(self.launch_args.as_ref().unwrap_unchecked(), signal)
            .await
            .context(LaunchGameSnafu)
    }

    /// Downloads game(also verifies)
    pub async fn verify_and_download_game(&self) -> Result<LaunchArgs, CollectionError> {
        if self.mod_loader().is_some() {
            fetch_launch_args_modded(self)
                .await
                .context(LaunchGameSnafu)
        } else {
            full_vanilla_download(self).await.context(LaunchGameSnafu)
        }
    }

    #[must_use]
    pub fn game_directory(&self) -> PathBuf {
        self.entry_path.join("minecraft_root")
    }

    pub fn get_base_path() -> PathBuf {
        DATA_DIR.join(COLLECTION_BASE)
    }

    #[must_use]
    pub fn get_collection_id(&self) -> CollectionId {
        let id = self.entry_path.file_name().unwrap().to_string_lossy();
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        CollectionId(hasher.finish())
    }

    pub fn save_collection_file(&self) -> Result<(), CollectionError> {
        StorageLoader::new(
            COLLECTION_FILE_NAME.to_owned(),
            Cow::Borrowed(&self.entry_path),
        )
        .save(&self)
        .context(StorageSnafu)?;
        Ok(())
    }

    #[must_use]
    pub fn mod_loader(&self) -> Option<&ModLoader> {
        self.mod_controller.as_ref().map(|x| &x.loader)
    }
    #[must_use]
    pub fn mod_manager(&self) -> Option<&ModManager> {
        self.mod_controller.as_ref().map(|x| &x.manager)
    }
    pub fn mod_loader_mut(&mut self) -> Option<&mut ModLoader> {
        self.mod_controller.as_mut().map(|x| &mut x.loader)
    }
    pub fn mod_manager_mut(&mut self) -> Option<&mut ModManager> {
        self.mod_controller.as_mut().map(|x| &mut x.manager)
    }

    fn create_loader(display_name: &str) -> Result<StorageLoader, StorageError> {
        // Windows file and directory name restrictions.
        let invalid_chars_regex = Regex::new(r#"[\\/:*?\"<>|]"#).unwrap();
        let reserved_names = vec![
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];

        let mut dir_name = invalid_chars_regex
            .replace_all(display_name, "")
            .to_string();
        if reserved_names.contains(&dir_name.to_uppercase().as_str()) {
            dir_name = format!("{dir_name}_");
        }
        if dir_name.is_empty() {
            dir_name = Self::gen_random_string();
        }

        let entry_path = Self::handle_duplicate_dir(Self::get_base_path(), &dir_name);
        create_dir_all(&entry_path).map_err(|x| StorageError::Io {
            source: x,
            path: entry_path.clone(),
        })?;
        let loader =
            StorageLoader::new(COLLECTION_FILE_NAME.to_owned(), Cow::Borrowed(&entry_path));

        Ok(loader)
    }

    fn handle_duplicate_dir(base_dir: PathBuf, dir_name: &str) -> PathBuf {
        let path = base_dir.join(dir_name);
        if !path.exists() {
            return path;
        }

        let new_dir_name = format!("{dir_name}_{}", Self::gen_random_string());

        Self::handle_duplicate_dir(base_dir, &new_dir_name)
    }

    fn gen_random_string() -> String {
        use rand::{distributions::Alphanumeric, Rng};

        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(5)
            .map(char::from)
            .collect()
    }

    pub fn scan() -> Result<Vec<Result<Self, StorageError>>, CollectionError> {
        let mut collections = Vec::new();
        let collection_base_dir = Self::get_base_path();
        create_dir_all(&collection_base_dir).context(IoSnafu {
            path: &collection_base_dir,
        })?;
        let dirs = collection_base_dir.read_dir().context(IoSnafu {
            path: &collection_base_dir,
        })?;

        for base_entry in dirs {
            let base_entry_path = base_entry
                .context(IoSnafu {
                    path: PathBuf::new(),
                })?
                .path();
            for file in base_entry_path.read_dir().context(IoSnafu {
                path: &base_entry_path,
            })? {
                let file_name = file
                    .context(IoSnafu {
                        path: PathBuf::new(),
                    })?
                    .file_name()
                    .to_string_lossy()
                    .to_string();

                if file_name == COLLECTION_FILE_NAME {
                    let path = collection_base_dir.join(&base_entry_path);
                    let loader = StorageLoader::new(file_name.clone(), Cow::Borrowed(&path));
                    let mut collection = loader.load::<Self>();
                    if let Ok(ref mut collection) = collection {
                        let entry_name =
                            collection.entry_path.file_name().unwrap().to_string_lossy();

                        info!("Collection {entry_name} has been successfully read");
                        if collection.entry_path != path {
                            collection.entry_path = path;
                            loader.save(&collection).context(StorageSnafu)?;
                        }
                    } else {
                        dioxus_logger::tracing::error!("{path:?} has invalid data");
                    }
                    collections.push(collection);
                }
            }
        }
        Ok(collections)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
pub struct ModLoader {
    #[serde(rename = "type")]
    pub mod_loader_type: ModLoaderType,
    pub version: Option<String>,
}

impl ModLoader {
    pub fn new(mod_loader_type: ModLoaderType, version: impl Into<Option<String>>) -> Self {
        Self {
            mod_loader_type,
            version: version.into(),
        }
    }
}

impl Display for ModLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mod_loader_type)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, derive_more::Display, Hash)]
pub enum ModLoaderType {
    Forge,
    NeoForge,
    Fabric,
    Quilt,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
pub struct AdvancedOptions {
    pub jvm_max_memory: Option<Size>,
    pub java_arguments: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Size {
    Bytes(usize),
    Megabytes(usize),
    Gigabytes(usize),
}

impl Size {
    pub fn to_java_size(&self) -> String {
        match self {
            Self::Bytes(m) => format!("{}M", m / 1024 / 1024),
            Size::Megabytes(m) => format!("{m}M"),
            Size::Gigabytes(m) => format!("{m}G"),
        }
    }
    pub fn to_bytes(&self) -> f64 {
        match self {
            Size::Bytes(x) => *x as f64,
            Size::Megabytes(x) => *x as f64 * 1024. * 1024.,
            Size::Gigabytes(x) => *x as f64 * 1024. * 1024. * 1024.,
        }
    }
    pub fn to_megabytes(&self) -> f64 {
        match self {
            Size::Bytes(x) => *x as f64 / 1024. / 1024.,
            Size::Megabytes(x) => *x as f64,
            Size::Gigabytes(x) => *x as f64 * 1024.,
        }
    }
}
