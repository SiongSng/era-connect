use std::{borrow::Cow, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::storage_loader::{StorageError, StorageInstance, StorageLoader};

const GLOBAL_SETTINGS_FILENAME: &str = "global_setings.json";

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct GlobalSettings {
    pub appearances: Appearances,
    pub ui_layout: UILayout,
    pub download: Download,
}

impl StorageInstance<Self> for GlobalSettings {
    fn file_name() -> &'static str {
        GLOBAL_SETTINGS_FILENAME
    }

    fn save(&self) -> Result<(), StorageError> {
        let storage =
            StorageLoader::new(Self::file_name().to_owned(), Cow::Owned(Self::base_path()));
        storage.save(self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Download {
    pub max_simultatneous_download: usize,
    pub download_speed_limit: Option<Speed>,
}

impl Default for Download {
    fn default() -> Self {
        Self {
            max_simultatneous_download: 64,
            download_speed_limit: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Speed {
    MegabytePerSecond(f64),
    MebibytePerSecond(f64),
    KilobytePerSecond(f64),
    KibiBytePerSecond(f64),
}

impl Speed {
    // FIXME: Naive translation
    #[must_use]
    pub fn to_mebibyte(&self) -> f64 {
        match self {
            Self::MebibytePerSecond(x) | Self::MegabytePerSecond(x) => *x,
            Self::KilobytePerSecond(x) => *x / 1024.,
            Self::KibiBytePerSecond(x) => *x / 1000.,
        }
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self::MegabytePerSecond(0.0)
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Appearances {
    pub dark_lightmode: DarkLightMode,
    pub day_light_darkmode: bool,
    pub default_background_picture: PathBuf,
    pub blur: bool,
    pub adaptive_background: bool,
    pub high_contrast: bool,
    pub greyscale: bool,
    pub disable_background_picture: bool,
    pub animation: bool,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub enum DarkLightMode {
    #[default]
    Dark,
    Light,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct UILayout {
    pub completed_setup: bool,
    pub shows_recommendation: bool,
    pub sidebar_preivew: bool,
}
