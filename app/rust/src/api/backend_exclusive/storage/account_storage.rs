use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::shared_resources::authentication::account::MinecraftAccount;

use super::storage_loader::{StorageError, StorageInstance, StorageLoader};

const ACCOUNT_FILE_NAME: &str = "accounts.json";

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AccountStorage {
    pub accounts: Vec<MinecraftAccount>,
    /// The uuid of the main account in the [accounts] vector
    pub main_account: Option<Uuid>,
}

impl StorageInstance<Self> for AccountStorage {
    fn file_name() -> &'static str {
        ACCOUNT_FILE_NAME
    }

    fn save(&self) -> Result<(), StorageError> {
        let storage =
            StorageLoader::new(Self::file_name().to_owned(), Cow::Owned(Self::base_path()));
        storage.save(self)
    }
}

impl AccountStorage {
    pub fn add_account(&mut self, account: MinecraftAccount, is_main_account: bool) {
        // remove all accounts that have the same uuid
        self.accounts.retain(|a| a.uuid != account.uuid);
        let uuid = account.uuid;
        self.accounts.push(account);

        let set_as_main_account =
            is_main_account || self.accounts.len() == 1 || self.main_account.is_none();

        if set_as_main_account {
            self.main_account = Some(uuid);
        }
    }

    pub fn remove_account(&mut self, uuid: Uuid) {
        self.accounts.retain(|a| a.uuid != uuid);
        if self.main_account == Some(uuid) {
            self.main_account = None;
        }
    }
}
