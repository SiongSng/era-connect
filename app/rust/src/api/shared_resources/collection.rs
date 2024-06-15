pub use std::path::PathBuf;
use std::{borrow::Cow, fs::create_dir_all, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use log::info;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

pub use crate::api::backend_exclusive::storage::storage_loader::StorageLoader;

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

use super::entry::STORAGE;

#[serde_with::serde_as]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
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

    pub entry_path: PathBuf,
    launch_args: Option<LaunchArgs>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
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
    pub fn new(loader: ModLoader, manager: ModManager) -> Self {
        Self { loader, manager }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct CollectionId(pub Arc<str>);

impl Default for CollectionId {
    fn default() -> Self {
        Self(Arc::from(""))
    }
}
const COLLECTION_FILE_NAME: &str = "collection.json";
const COLLECTION_BASE: &str = "collections";

/// if a method has `&mut self`, remember to call `self.save()` to actually save it!
impl Collection {
    /// Creates a collection and return a collection with its loader attached
    pub async fn create(
        display_name: String,
        version_metadata: VersionMetadata,
        mod_loader: impl Into<Option<ModLoader>>,
        picture_path: impl Into<PathBuf>,
        advanced_options: Option<AdvancedOptions>,
    ) -> anyhow::Result<Collection> {
        let now_time = Utc::now();
        let loader = Self::create_loader(&display_name)?;
        let entry_path = loader.base_path.clone();
        let mod_controller = mod_loader.into().map(|loader| {
            let mod_manager = ModManager::new(
                entry_path.join("minecraft_root"),
                entry_path.join("mod_images"),
                loader.clone(),
                version_metadata.clone(),
            );
            ModController::new(loader, mod_manager)
        });

        let collection = Collection {
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

        collection.save().await?;

        Ok(collection)
    }

    /// use project id(slug also works) to add mod, will deal with dependencies insertion
    pub async fn add_modrinth_mod(
        &mut self,
        project_id: impl AsRef<str> + Send + Sync,
        tag: Vec<Tag>,
        mod_override: Option<Vec<ModOverride>>,
    ) -> anyhow::Result<()> {
        let project_id = project_id.as_ref();
        let project = (&FERINTH).get_project(project_id).await?;
        if let Some(manager) = self.mod_manager_mut() {
            manager
                .add_project(project.into(), tag, mod_override.unwrap_or(Vec::new()))
                .await?;
        }
        self.save().await?;
        Ok(())
    }

    pub async fn add_multiple_modrinth_mod(
        &mut self,
        project_ids: Vec<&str>,
        tag: Vec<Tag>,
        mod_override: impl Into<Option<Vec<ModOverride>>>,
    ) -> anyhow::Result<()> {
        let project = (&FERINTH).get_multiple_projects(&project_ids).await?;
        if let Some(mod_controller) = self.mod_controller.as_mut() {
            mod_controller
                .manager
                .add_multiple_project(
                    project.into_iter().map(|x| x.into()).collect::<Vec<_>>(),
                    tag.clone(),
                    mod_override.into().unwrap_or(Vec::new()),
                )
                .await?;
        }
        self.save().await?;
        Ok(())
    }
    pub async fn add_curseforge_mod(
        &mut self,
        project_id: i32,
        tag: Vec<Tag>,
        mod_override: Option<Vec<ModOverride>>,
    ) -> anyhow::Result<()> {
        let project = (&FURSE).get_mod(project_id).await?;
        if let Some(mod_manager) = self.mod_manager_mut() {
            mod_manager
                .add_project(project.into(), tag, mod_override.unwrap_or(Vec::new()))
                .await?;
        }
        self.save().await?;
        Ok(())
    }

    pub async fn download_mods(&self) -> anyhow::Result<()> {
        let id = self.get_collection_id();
        if let Some(mod_manager) = self.mod_manager() {
            let download_args = mod_manager.get_download()?;
            execute_and_progress(
                id,
                download_args,
                DownloadBias::default(),
                DownloadType::mods_download(),
            )
            .await?;
        }
        Ok(())
    }

    /// SIDE-EFFECT: put `launch_args` into Struct
    pub async fn launch_game(&mut self) -> anyhow::Result<()> {
        if self.launch_args.is_none() {
            self.launch_args = Some(self.verify_and_download_game().await?);
            self.save().await?;
        }
        vanilla::launcher::launch_game(&self.launch_args.as_ref().unwrap()).await
    }

    pub async unsafe fn launch_game_unchecked(&self) -> anyhow::Result<()> {
        vanilla::launcher::launch_game(&self.launch_args.as_ref().unwrap_unchecked()).await
    }

    /// Downloads game(also verifies)
    pub async fn verify_and_download_game(&self) -> anyhow::Result<LaunchArgs> {
        if let Some(mod_loader) = self.mod_loader() {
            match mod_loader.mod_loader_type {
                ModLoaderType::Forge
                | ModLoaderType::NeoForge
                | ModLoaderType::Quilt
                | ModLoaderType::Fabric => fetch_launch_args_modded(self).await,
                // _ => bail!("Modloader not yet implemented"),
            }
        } else {
            full_vanilla_download(self).await
        }
    }

    pub fn game_directory(&self) -> PathBuf {
        self.entry_path.join("minecraft_root")
    }

    pub fn get_base_path() -> PathBuf {
        DATA_DIR.join(COLLECTION_BASE)
    }

    pub fn get_collection_id(&self) -> CollectionId {
        let id: Arc<str> = Arc::from(
            self.entry_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .as_ref(),
        );
        CollectionId(id)
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        StorageLoader::new(
            COLLECTION_FILE_NAME.to_string(),
            Cow::Borrowed(&self.entry_path),
        )
        .save(&self)?;
        {
            let binding = &mut STORAGE.write().collections;
            if let Some(x) = binding
                .iter_mut()
                .filter(|x| x.get_collection_id() == self.get_collection_id())
                .find(|x| &**x != self)
            {
                *x = self.clone();
            }
            if binding
                .iter()
                .all(|x| x.get_collection_id() != self.get_collection_id())
            {
                binding.push(self.clone());
            }
        }
        Ok(())
    }

    pub fn mod_loader(&self) -> Option<&ModLoader> {
        self.mod_controller.as_ref().map(|x| &x.loader)
    }
    pub fn mod_manager(&self) -> Option<&ModManager> {
        self.mod_controller.as_ref().map(|x| &x.manager)
    }
    pub fn mod_loader_mut(&mut self) -> Option<&mut ModLoader> {
        self.mod_controller.as_mut().map(|x| &mut x.loader)
    }
    pub fn mod_manager_mut(&mut self) -> Option<&mut ModManager> {
        self.mod_controller.as_mut().map(|x| &mut x.manager)
    }

    fn create_loader(display_name: &str) -> std::io::Result<StorageLoader> {
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
            dir_name = format!("{}_", dir_name);
        }
        if dir_name.is_empty() {
            dir_name = Self::gen_random_string();
        }

        let entry_path = Self::handle_duplicate_dir(Collection::get_base_path(), &dir_name);
        create_dir_all(&entry_path)?;
        let loader =
            StorageLoader::new(COLLECTION_FILE_NAME.to_string(), Cow::Borrowed(&entry_path));

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

    pub fn scan() -> anyhow::Result<Vec<Collection>> {
        let mut collections = Vec::new();
        let collection_base_dir = Self::get_base_path();
        create_dir_all(&collection_base_dir)?;
        let dirs = collection_base_dir.read_dir()?;

        for base_entry in dirs {
            let base_entry_path = base_entry?.path();
            for file in base_entry_path.read_dir()? {
                let file_name = file?.file_name().to_string_lossy().to_string();

                if file_name == COLLECTION_FILE_NAME {
                    let path = collection_base_dir.join(&base_entry_path);
                    let loader = StorageLoader::new(file_name.clone(), Cow::Borrowed(&path));
                    let mut collection = loader.load::<Collection>()?;

                    // block_on(collection.mod_manager.scan())?;
                    info!("Succesfully scanned the mods");
                    if collection.entry_path != path {
                        collection.entry_path = path;
                        loader.save(&collection)?;
                    }
                    collections.push(collection);
                }
            }
        }
        Ok(collections)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
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

impl ToString for ModLoader {
    fn to_string(&self) -> String {
        match self.mod_loader_type {
            ModLoaderType::Forge => String::from("forge"),
            ModLoaderType::NeoForge => String::from("neoforge"),
            ModLoaderType::Fabric => String::from("fabric"),
            ModLoaderType::Quilt => String::from("quilt"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum ModLoaderType {
    Forge,
    NeoForge,
    Fabric,
    Quilt,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct AdvancedOptions {
    pub jvm_max_memory: Option<usize>,
}
