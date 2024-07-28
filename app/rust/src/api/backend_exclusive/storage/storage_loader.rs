use std::{
    borrow::Cow,
    fs::{create_dir_all, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::Serialize;
use snafu::prelude::*;

use crate::api::{backend_exclusive::download::DownloadError, shared_resources::entry::DATA_DIR};

pub struct StorageLoader {
    pub file_name: String,
    pub base_path: PathBuf,
}

#[must_use]
pub fn get_global_shared_path() -> PathBuf {
    DATA_DIR.join("shared")
}

#[derive(Snafu, Debug)]
pub enum StorageError {
    #[snafu(transparent)]
    Download { source: DownloadError },
    #[snafu(display("Failed to deserializae"))]
    Desearialization { source: serde_json::Error },
    #[snafu(display("Could not read file at: {path:?}"))]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },
}

impl StorageLoader {
    #[must_use]
    pub fn new(file_name: String, base_path: Cow<Path>) -> Self {
        let base_path = match base_path {
            Cow::Borrowed(c) => c.to_path_buf(),
            Cow::Owned(x) => x,
        };

        Self {
            file_name,
            base_path,
        }
    }

    pub fn load<T: DeserializeOwned + Serialize>(&self) -> Result<T, StorageError> {
        let file = File::open(self.get_path_buf()).context(IoSnafu {
            path: self.get_path_buf(),
        })?;
        let reader = BufReader::new(file);
        let storage = serde_json::from_reader(reader).context(DesearializationSnafu)?;

        Ok(storage)
    }

    pub fn load_with_default<T: Default + DeserializeOwned + Serialize>(
        &self,
    ) -> Result<T, StorageError> {
        let path = self.get_path_buf();
        if !path.exists() {
            let storage = T::default();
            self.save(&storage)?;
            return Ok(storage);
        }

        self.load()
    }

    pub fn save<T: Serialize>(&self, storage: &T) -> Result<(), StorageError> {
        create_dir_all(self.get_storage_directory()).context(IoSnafu {
            path: self.get_storage_directory(),
        })?;

        let file = File::create(self.get_path_buf()).context(IoSnafu {
            path: self.get_path_buf(),
        })?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, storage).context(DesearializationSnafu)?;
        Ok(())
    }

    fn get_path_buf(&self) -> PathBuf {
        self.get_storage_directory().join(&self.file_name)
    }

    fn get_storage_directory(&self) -> PathBuf {
        DATA_DIR.join(&self.base_path)
    }
}

pub trait StorageInstance<T: Default + DeserializeOwned + Serialize> {
    fn file_name() -> &'static str;

    fn base_path() -> PathBuf {
        PathBuf::from("storages")
    }

    fn save(&self) -> Result<(), StorageError>;

    fn load() -> Result<T, StorageError> {
        let loader =
            StorageLoader::new(Self::file_name().to_owned(), Cow::Owned(Self::base_path()));
        loader.load_with_default::<T>()
    }
}
