use dioxus::signals::GlobalSignal;

use crate::api::shared_resources::collection::{Collection, CollectionId};

use super::{
    account_storage::AccountStorage, global_settings::GlobalSettings,
    storage_loader::StorageInstance,
};

// #[derive()]
pub struct StorageState {
    pub account_storage: GlobalSignal<AccountStorage>,
    pub collections: GlobalSignal<ordermap::map::OrderMap<CollectionId, Collection>>,
    pub global_settings: GlobalSignal<GlobalSettings>,
}

impl StorageState {
    pub const fn new() -> Self {
        let account_storage = GlobalSignal::new(|| AccountStorage::load().unwrap_or_default());
        let collections = GlobalSignal::new(|| {
            Collection::scan()
                .unwrap_or_default()
                .into_iter()
                .map(|x| (x.get_collection_id(), x))
                .collect()
        });
        let global_settings = GlobalSignal::new(|| GlobalSettings::load().unwrap_or_default());

        Self {
            account_storage,
            global_settings,
            collections,
        }
    }
}
