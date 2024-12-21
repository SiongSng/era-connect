use dioxus::signals::GlobalSignal;

use super::{
    account_storage::AccountStorage, global_settings::GlobalSettings,
    storage_loader::StorageInstance,
};

pub struct StorageState {
    pub account_storage: GlobalSignal<AccountStorage>,
    // pub collections: GlobalSignal<ordermap::map::OrderMap<CollectionId, Signal<Collection>>>,
    pub global_settings: GlobalSignal<GlobalSettings>,
}

impl Default for StorageState {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageState {
    #[must_use]
    /// ignores invalid json files(due to update or file corruption)
    pub const fn new() -> Self {
        let account_storage = GlobalSignal::new(|| AccountStorage::load().unwrap_or_default());
        let global_settings = GlobalSignal::new(|| GlobalSettings::load().unwrap_or_default());

        Self {
            account_storage,
            // collections,
            global_settings,
        }
    }
}
