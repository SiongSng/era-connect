use dioxus::signals::GlobalSignal;
use dioxus_logger::tracing::info;
use once_cell::sync::Lazy;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

use uuid::Uuid;

use crate::api::backend_exclusive::{
    download::{DownloadId, Progress},
    storage::{storage_loader::StorageInstance, storage_state::StorageState},
};
use crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent;
use crate::api::shared_resources::authentication::{self, account::MinecraftSkin};

use crate::api::backend_exclusive::vanilla::version::VersionMetadata;

use crate::api::shared_resources::authentication::msa_flow::LoginFlowError;
use crate::api::shared_resources::collection::Collection;
use crate::api::shared_resources::collection::{AdvancedOptions, ModLoader};

pub static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    dirs::data_dir()
        .expect("Can't find data_dir")
        .join("era-connect")
});

pub static STORAGE: StorageState = StorageState::new();

pub static DOWNLOAD_PROGRESS: GlobalSignal<DownloadProgress> =
    GlobalSignal::new(DownloadProgress::new);

#[derive(PartialEq, Clone)]
pub struct DownloadProgress(pub BTreeMap<DownloadId, Progress>);

impl DownloadProgress {
    fn new() -> Self {
        Self(BTreeMap::new())
    }
}

pub fn get_skin_file_path(skin: MinecraftSkin) -> String {
    skin.get_head_file_path().to_string_lossy().to_string()
}

pub async fn remove_minecraft_account(uuid: Uuid) -> anyhow::Result<()> {
    let storage = &mut STORAGE.account_storage.write();
    storage.remove_account(uuid);
    storage.save()?;
    Ok(())
}

pub async fn minecraft_login_flow(
    skin: UnboundedSender<LoginFlowEvent>,
) -> Result<(), LoginFlowError> {
    let result = authentication::msa_flow::login_flow(skin.clone()).await;
    match result {
        Ok(account) => {
            if let Some(skin) = account.skins.first() {
                skin.download_skin().await?;
            }

            let storage = &mut STORAGE.account_storage.write();
            storage.add_account(account.clone(), true);
            storage.save()?;

            skin.send(LoginFlowEvent::Success(account))?;
            info!("Successfully login minecraft account");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub async fn create_collection(
    display_name: impl Into<String>,
    picture_path: impl Into<PathBuf>,
    version_metadata: VersionMetadata,
    mod_loader: impl Into<Option<ModLoader>>,
    advanced_options: impl Into<Option<AdvancedOptions>>,
) -> anyhow::Result<Collection> {
    let display_name = display_name.into();
    let collection = Collection::create(
        display_name,
        version_metadata,
        mod_loader.into(),
        picture_path,
        advanced_options.into(),
    )
    .await?;

    info!(
        "Successfully created collection basic file at {}",
        collection.entry_path().display()
    );

    collection.verify_and_download_game().await?;

    info!("Successfully finished downloading game");

    Ok(collection)
}
