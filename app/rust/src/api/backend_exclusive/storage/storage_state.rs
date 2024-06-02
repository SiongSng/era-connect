use crate::api::shared_resources::collection::Collection;

use super::{
    account_storage::AccountStorage, global_settings::GlobalSettings,
    storage_loader::StorageInstance,
};

#[derive(Clone)]
pub struct StorageState {
    pub account_storage: AccountStorage,
    pub collections: Vec<Collection>,
    pub global_settings: GlobalSettings,
}

impl StorageState {
    pub fn new() -> Self {
        let account_storage = AccountStorage::load().unwrap_or_default();
        let collections = Collection::scan().unwrap_or_default();
        let global_settings = GlobalSettings::load().unwrap_or_default();

        Self {
            account_storage,
            global_settings,
            collections,
        }
    }
}
