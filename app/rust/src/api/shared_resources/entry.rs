use anyhow::anyhow;
use anyhow::Context;
use flutter_rust_bridge::frb;
use flutter_rust_bridge::setup_default_user_utils;
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fs::create_dir_all, time::Duration};
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

use uuid::Uuid;

pub use crate::api::backend_exclusive::storage::{
    account_storage::{AccountStorage, AccountStorageKey, AccountStorageValue},
    global_settings::{UILayout, UILayoutKey, UILayoutValue},
};

use crate::api::backend_exclusive::{
    download::Progress,
    storage::{storage_loader::StorageInstance, storage_state::StorageState},
};
use crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent;
use crate::api::shared_resources::authentication::{self, account::MinecraftSkin};

use crate::api::backend_exclusive::vanilla;
pub use crate::api::backend_exclusive::vanilla::version::VersionMetadata;

use crate::api::shared_resources::authentication::msa_flow::LoginFlowErrors;
use crate::api::shared_resources::collection::ModLoaderType;
use crate::api::shared_resources::collection::{
    AdvancedOptions, Collection, CollectionId, ModLoader,
};
use crate::frb_generated::StreamSink;

pub static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    dirs::data_dir()
        .expect("Can't find data_dir")
        .join("era-connect")
});

pub static DOWNLOAD_PROGRESS: Lazy<UnboundedSender<HashMapMessage>> = Lazy::new(|| {
    let (sender, receiver) = unbounded_channel();
    spawn_hashmap_manager_thread(receiver);
    sender
});

pub static STORAGE: Lazy<StorageState> = Lazy::new(StorageState::new);

pub enum HashMapMessage {
    Insert(Arc<CollectionId>, Progress),
    Remove(Arc<CollectionId>),
    Get(Arc<CollectionId>, UnboundedSender<Option<Arc<Progress>>>),
}

fn spawn_hashmap_manager_thread(mut receiver: UnboundedReceiver<HashMapMessage>) {
    flutter_rust_bridge::spawn(async move {
        let mut hashmap: HashMap<Arc<CollectionId>, Arc<Progress>> = HashMap::new();

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
                        let _ = response_sender.send(response);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
}

#[frb(init)]
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

// #[frb(sync)]
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

#[frb(sync)]
pub fn get_account_storage(key: AccountStorageKey) -> AccountStorageValue {
    STORAGE.account_storage.blocking_read().get_value(key)
}

#[frb(sync)]
pub fn get_skin_file_path(skin: MinecraftSkin) -> String {
    skin.get_head_file_path().to_string_lossy().to_string()
}

pub async fn remove_minecraft_account(uuid: Uuid) -> anyhow::Result<()> {
    let mut storage = STORAGE.account_storage.write().await;
    storage.remove_account(uuid);
    storage.save()?;
    Ok(())
}

pub async fn minecraft_login_flow(skin: StreamSink<LoginFlowEvent>) -> anyhow::Result<()> {
    let result = authentication::msa_flow::login_flow(&skin).await;
    match result {
        Ok(account) => {
            if let Some(skin) = account.skins.first() {
                skin.download_skin().await?;
            }

            let mut storage = STORAGE.account_storage.write().await;
            storage.add_account(account.clone(), true);
            storage.save()?;

            skin.add(LoginFlowEvent::Success(account))
                .map_err(|x| anyhow!(x))?;
            info!("Successfully login minecraft account");
        }
        Err(e) => {
            skin.add(LoginFlowEvent::Error(LoginFlowErrors::UnknownError(
                format!("{e:#}"),
            )))
            .map_err(|x| anyhow!(x))?;
            warn!("Failed to login minecraft account: {:#}", e);
        }
    }

    Ok(())
}

pub async fn create_collection(
    display_name: String,
    version_metadata: VersionMetadata,
    mod_loader: Option<ModLoader>,
    advanced_options: Option<AdvancedOptions>,
) -> anyhow::Result<()> {
    let mod_loader = Some(ModLoader {
        mod_loader_type: ModLoaderType::Quilt,
        version: String::new(),
    });
    let mut collection =
        Collection::create(display_name, version_metadata, mod_loader, advanced_options)?;

    info!(
        "Successfully created collection basic file at {}",
        collection.entry_path.display()
    );
    let id = Arc::new(collection.get_collection_id());
    let download_handle = tokio::spawn(async move {
        loop {
            let (tx, mut rx) = unbounded_channel();
            DOWNLOAD_PROGRESS.send(HashMapMessage::Get(Arc::clone(&id), tx))?;
            if let Some(Some(x)) = rx.recv().await {
                if x.percentages >= 100.0 {
                    break;
                }
                debug!("{:#?}", x);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok::<(), anyhow::Error>(())
    });

    collection.launch_game().await?;

    download_handle.await??;

    info!("Successfully finished downloading game");

    Ok(())
}
