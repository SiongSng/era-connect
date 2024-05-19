use std::sync::Arc;

use tokio::sync::RwLock;

use crate::api::shared_resources::collection::Collection;

use super::{
    account_storage::AccountStorage, global_settings::GlobalSettings,
    storage_loader::StorageInstance,
};

#[derive(Clone)]
pub struct StorageState {
    pub account_storage: Arc<RwLock<AccountStorage>>,
    pub collections: Arc<RwLock<Vec<Collection>>>,
    pub global_settings: Arc<RwLock<GlobalSettings>>,
}

// impl PartialEq for StorageState {
//     fn eq(&self, other: &Self) -> bool {
//         // We need to acquire the read locks to access the data inside the RwLock.
//         // This assumes that we don't care about the state of the lock itself,
//         // but rather the data it protects.
//         // Note: The use of async block and await to handle async locking
//         tokio::task::block_in_place(|| {
//             tokio::runtime::Handle::current().block_on(async {
//                 let self_account_storage = self.account_storage.read().await;
//                 let other_account_storage = other.account_storage.read().await;

//                 let self_collections = self.collections.read().await;
//                 let other_collections = self.collections.read().await;

//                 let self_global_settings = self.global_settings.read().await;
//                 let other_global_settings = self.global_settings.read().await;

//                 *self_account_storage == *other_account_storage
//                     && *self_collections == *other_collections
//                     && *self_global_settings == *other_global_settings
//             })
//         })
//     }
// }

impl StorageState {
    pub fn new() -> Self {
        let account_storage = AccountStorage::load().unwrap_or_default();
        let collections = Collection::scan().unwrap_or_default();
        let global_settings = GlobalSettings::load().unwrap_or_default();

        Self {
            account_storage: Arc::new(RwLock::new(account_storage)),
            global_settings: Arc::new(RwLock::new(global_settings)),
            collections: Arc::new(RwLock::new(collections)),
        }
    }
}
