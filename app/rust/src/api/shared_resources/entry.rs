use anyhow::Context;
use flutter_rust_bridge::setup_default_user_utils;
use log::{info, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fs::create_dir_all, time::Duration};
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

use uuid::Uuid;

use crate::api::backend_exclusive::mod_management::mods::ModOverride;
pub use crate::api::backend_exclusive::storage::{
    account_storage::{AccountStorage, AccountStorageKey, AccountStorageValue},
    global_settings::{UILayout, UILayoutKey, UILayoutValue},
};

use crate::api::backend_exclusive::vanilla::version::get_versions;
use crate::api::backend_exclusive::{
    download::Progress,
    storage::{storage_loader::StorageInstance, storage_state::StorageState},
};
use crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent;
use crate::api::shared_resources::authentication::{self, account::MinecraftSkin};

use crate::api::backend_exclusive::vanilla;
use crate::api::backend_exclusive::vanilla::version::VersionMetadata;

use crate::api::shared_resources::authentication::msa_flow::LoginFlowErrors;
use crate::api::shared_resources::collection::Collection;
use crate::api::shared_resources::collection::ModLoaderType;
use crate::api::shared_resources::collection::{AdvancedOptions, CollectionId, ModLoader};

pub static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    dirs::data_dir()
        .expect("Can't find data_dir")
        .join("era-connect")
});

pub static STORAGE: Lazy<StorageState> = Lazy::new(StorageState::new);

/// A globally accessible sender for updating download progress, utilizing an unbounded channel for asynchronous message passing.
///
/// This static variable enables various parts of an application to send updates about download progress or request data
/// regarding current progress states. It leverages `HashMapMessage` to perform actions like insertions, removals, and queries
/// against a hashmap that tracks the progress of each download uniquely identified by `CollectionId`.
/// # Examples
/// ```rust
///     let (tx, mut rx) = unbounded_channel();
///     DOWNLOAD_PROGRESS.send(HashMapMessage::Get(Arc::clone(&id), tx))?;
///     if let Some(Some(x)) = rx.recv().await {
///         debug!("{:#?}", x);
///     }
/// ```
pub static DOWNLOAD_PROGRESS: DownloadProgress = DownloadProgress::new();

pub struct DownloadProgress(Lazy<UnboundedSender<HashMapMessage>>);

impl DownloadProgress {
    const fn new() -> Self {
        Self(Lazy::new(|| {
            let (sender, receiver) = unbounded_channel();
            spawn_hashmap_manager_thread(receiver);
            sender
        }))
    }

    pub fn get_continuous_progress_handle(
        &'static self,
        collection: &Collection,
        sender: UnboundedSender<String>,
    ) -> flutter_rust_bridge::JoinHandle<Result<(), anyhow::Error>> {
        let id = Arc::new(collection.get_collection_id());
        tokio::spawn(async move {
            loop {
                if let Some(x) = self.get(Arc::clone(&id)).await? {
                    if x.percentages >= 100.0 {
                        break;
                    }
                    sender.send(format!("{:#?}", x))?;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Ok::<(), anyhow::Error>(())
        })
    }

    pub async fn get(
        &self,
        id: impl Into<Arc<CollectionId>>,
    ) -> anyhow::Result<Option<Arc<Progress>>> {
        let id = id.into();
        let (tx, mut rx) = unbounded_channel();
        self.0.send(HashMapMessage::Get(id, tx))?;
        Ok(rx.recv().await.flatten())
    }
    pub async fn insert(
        &self,
        id: impl Into<Arc<CollectionId>>,
        value: Progress,
    ) -> anyhow::Result<()> {
        self.0.send(HashMapMessage::Insert(id.into(), value))?;
        Ok(())
    }
    pub async fn remove(&self, id: impl Into<Arc<CollectionId>>) -> anyhow::Result<()> {
        self.0.send(HashMapMessage::Remove(id.into()))?;
        Ok(())
    }
}

enum HashMapMessage {
    Insert(Arc<CollectionId>, Progress),
    Remove(Arc<CollectionId>),
    Get(Arc<CollectionId>, UnboundedSender<Option<Arc<Progress>>>),
}

fn spawn_hashmap_manager_thread(mut receiver: UnboundedReceiver<HashMapMessage>) {
    flutter_rust_bridge::spawn(async move {
        let mut hashmap = HashMap::new();

        loop {
            if let Some(message) = receiver.recv().await {
                match message {
                    HashMapMessage::Insert(key, value) => {
                        hashmap.insert(key, Arc::new(value));
                    }
                    HashMapMessage::Remove(key) => {
                        hashmap.remove(&key);
                    }
                    HashMapMessage::Get(key, response_sender) => {
                        let response = hashmap.get(&key).map(Arc::clone);
                        response_sender.send(response).expect("receiver dead.");
                    }
                }
            }
        }
    });
}

pub fn init_app() -> anyhow::Result<()> {
    setup_default_user_utils();
    setup_logger()?;
    Ok(())
}

fn setup_logger() -> anyhow::Result<()> {
    use chrono::Local;

    let file_name = format!("{}.log", Local::now().format("%Y-%m-%d-%H-%M-%S"));
    let file_path = DATA_DIR.join("logs").join(file_name);
    let parent = file_path
        .parent()
        .context("Failed to get the parent directory of logs directory")?;
    create_dir_all(parent)?;

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] {} | {}:{} | {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.file().unwrap_or_else(|| record.target()),
                record.line().unwrap_or(0),
                message
            ));
        })
        .chain(std::io::stdout())
        .chain(fern::log_file(file_path)?)
        .filter(|metadata| {
            if cfg!(debug_assertions) {
                metadata.level() <= log::LevelFilter::Debug
            } else {
                metadata.level() <= log::LevelFilter::Info
            }
        })
        .apply()?;

    info!("Successfully setup logger");
    Ok(())
}

pub fn get_ui_layout_storage(key: UILayoutKey) -> UILayoutValue {
    let value = STORAGE
        .global_settings
        .blocking_read()
        .ui_layout
        .get_value(key);
    value
}

pub async fn get_vanilla_versions() -> anyhow::Result<Vec<VersionMetadata>> {
    vanilla::version::get_versions().await
}

pub fn set_ui_layout_storage(value: UILayoutValue) -> anyhow::Result<()> {
    let global_settings = &mut *STORAGE.global_settings.blocking_write();
    let ui_layout = &mut global_settings.ui_layout;
    ui_layout.set_value(value);
    global_settings.save()?;
    Ok(())
}

pub fn get_account_storage(key: AccountStorageKey) -> AccountStorageValue {
    STORAGE.account_storage.blocking_read().get_value(key)
}

pub fn get_skin_file_path(skin: MinecraftSkin) -> String {
    skin.get_head_file_path().to_string_lossy().to_string()
}

pub async fn remove_minecraft_account(uuid: Uuid) -> anyhow::Result<()> {
    let mut storage = STORAGE.account_storage.write().await;
    storage.remove_account(uuid);
    storage.save()?;
    Ok(())
}

pub async fn minecraft_login_flow(skin: UnboundedSender<LoginFlowEvent>) -> anyhow::Result<()> {
    let result = authentication::msa_flow::login_flow(skin.clone()).await;
    match result {
        Ok(account) => {
            if let Some(skin) = account.skins.first() {
                skin.download_skin().await?;
            }

            let mut storage = STORAGE.account_storage.write().await;
            storage.add_account(account.clone(), true);
            storage.save()?;

            skin.send(LoginFlowEvent::Success(account))?;
            info!("Successfully login minecraft account");
        }
        Err(e) => {
            skin.send(LoginFlowEvent::Error(LoginFlowErrors::UnknownError(
                format!("{e:#}"),
            )))?;
            warn!("Failed to login minecraft account: {:#}", e);
        }
    }

    Ok(())
}

pub async fn create_collection(
    display_name: impl Into<String>,
    version_metadata: VersionMetadata,
    mod_loader: Option<ModLoader>,
    advanced_options: Option<AdvancedOptions>,
) -> anyhow::Result<()> {
    let display_name = display_name.into();
    let mod_loader = Some(ModLoader {
        mod_loader_type: ModLoaderType::Fabric,
        version: None,
    });
    let version_metadata = get_versions()
        .await?
        .into_iter()
        .find(|x| x.id == "1.20.2")
        .unwrap();
    let mut collection =
        Collection::create(display_name, version_metadata, mod_loader, advanced_options)?;

    collection
        .add_multiple_modrinth_mod(
            vec![
                "fabric-api",
                "distanthorizons",
                "sodium",
                "modmenu",
                "ferrite-core",
                "lazydfu",
                "iris",
                "continuity",
                "indium",
                "rpmtw-update-mod",
            ],
            vec![],
            Some(vec![ModOverride::IgnoreMinorGameVersion]),
        )
        .await?;

    info!(
        "Successfully created collection basic file at {}",
        collection.entry_path.display()
    );

    info!(
        "Successfully created collection basic file at {}",
        collection.entry_path.display()
    );
    // collection.launch_game().await?;
    collection.verify_and_download_game().await?;
    collection.download_mods().await?;

    let (tx, mut rx) = unbounded_channel();
    let download_handle = DOWNLOAD_PROGRESS.get_continuous_progress_handle(&collection, tx);

    while let Some(x) = rx.recv().await {
        info!("{:#?}", x);
    }

    // dbg!(&collection.mod_manager.mods);

    collection.launch_game().await?;

    info!("Successfully finished downloading game");

    download_handle.await??;

    Ok(())
}
