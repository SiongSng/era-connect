use anyhow::Context;
use dioxus::signals::GlobalSignal;
use flutter_rust_bridge::setup_default_user_utils;
use log::{info, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub use crate::api::backend_exclusive::storage::{
    account_storage::{AccountStorage, AccountStorageKey, AccountStorageValue},
    global_settings::{UILayout, UILayoutKey, UILayoutValue},
};
use uuid::Uuid;

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
use crate::api::shared_resources::collection::{AdvancedOptions, CollectionId, ModLoader};

pub static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    dirs::data_dir()
        .expect("Can't find data_dir")
        .join("era-connect")
});

pub static STORAGE: GlobalSignal<StorageState> = GlobalSignal::new(StorageState::new);

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
pub static DOWNLOAD_PROGRESS: GlobalSignal<DownloadProgress> =
    GlobalSignal::new(DownloadProgress::new);

#[derive(PartialEq, Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct DownloadProgress(pub HashMap<Arc<CollectionId>, Progress>);

impl DownloadProgress {
    fn new() -> Self {
        Self(HashMap::new())
    }

    pub async fn get_all(self) -> Vec<Progress> {
        self.0.into_values().collect()
    }

    // pub async fn get_all_continous_handle(
    //     &'static self,
    // ) -> anyhow::Result<Vec<UnboundedReceiver<Arc<Progress>>>> {
    //     tokio_stream::iter(
    //         STORAGE()
    //             .collections
    //             .iter()
    //             .map(Collection::get_collection_id),
    //     )
    //     .then(|x| async move {
    //         let (tx, rx) = unbounded_channel();
    //         self.get_continuous_progress_handle(x, tx).await??;
    //         Ok(rx)
    //     })
    //     .collect::<anyhow::Result<Vec<_>>>()
    //     .await
    // }

    // pub fn get_continuous_progress_handle(
    //     &'static self,
    //     collection_id: CollectionId,
    //     sender: UnboundedSender<Arc<Progress>>,
    // ) -> flutter_rust_bridge::JoinHandle<Result<(), anyhow::Error>> {
    //     let id = Arc::new(collection_id);
    //     tokio::spawn(async move {
    //         loop {
    //             if let Some(x) = self.get(&id) {
    //                 if x.percentages >= 100.0 {
    //                     break;
    //                 }
    //                 sender.send(x)?;
    //             }
    //             tokio::time::sleep(Duration::from_millis(300)).await;
    //         }
    //         Ok::<(), anyhow::Error>(())
    //     })
    // }
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

pub async fn get_vanilla_versions() -> anyhow::Result<Vec<VersionMetadata>> {
    vanilla::version::get_versions().await
}

pub async fn set_ui_layout_storage(value: UILayoutValue) -> anyhow::Result<()> {
    let global_settings = &mut STORAGE.write().global_settings;
    let ui_layout = &mut global_settings.ui_layout;
    ui_layout.set_value(value);
    global_settings.save()?;
    Ok(())
}

pub fn get_skin_file_path(skin: MinecraftSkin) -> String {
    skin.get_head_file_path().to_string_lossy().to_string()
}

pub async fn remove_minecraft_account(uuid: Uuid) -> anyhow::Result<()> {
    let storage = &mut STORAGE.write().account_storage;
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

            let storage = &mut STORAGE.write().account_storage;
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
    mod_loader: impl Into<Option<ModLoader>>,
    advanced_options: impl Into<Option<AdvancedOptions>>,
) -> anyhow::Result<Collection> {
    let display_name = display_name.into();
    let mut collection = Collection::create(
        display_name,
        version_metadata,
        mod_loader.into(),
        advanced_options.into(),
    )
    .await?;

    info!(
        "Successfully created collection basic file at {}",
        collection.entry_path.display()
    );

    // collection.launch_game().await?;
    collection.verify_and_download_game().await?;
    collection.download_mods().await?;

    // dbg!(&collection.mod_manager.mods);

    info!("Successfully finished downloading game");

    Ok(collection)
}
