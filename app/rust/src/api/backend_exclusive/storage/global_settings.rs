use std::{borrow::Cow, path::PathBuf};

use serde::{Deserialize, Serialize};
use struct_key_value_pair::VariantStruct;

use super::storage_loader::{StorageInstance, StorageLoader};

const GLOBAL_SETTINGS_FILENAME: &str = "global_setings.json";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct GlobalSettings {
    pub appearances: Appearances,
    pub ui_layout: UILayout,
    pub download: Download,
}

impl StorageInstance<Self> for GlobalSettings {
    fn file_name() -> &'static str {
        GLOBAL_SETTINGS_FILENAME
    }

    fn save(&self) -> anyhow::Result<()> {
        let storage =
            StorageLoader::new(Self::file_name().to_owned(), Cow::Owned(Self::base_path()));
        storage.save(self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Speed {
    MegabytePerSecond(f64),
    MebibytePerSecond(f64),
    KilobytePerSecond(f64),
    KibiBytePerSecond(f64),
}

impl Speed {
    // FIXME: Naive translation
    pub fn to_mebibyte(&self) -> f64 {
        match self {
            Speed::MegabytePerSecond(x) => *x,
            Speed::MebibytePerSecond(x) => *x,
            Speed::KilobytePerSecond(x) => *x / 1024.,
            Speed::KibiBytePerSecond(x) => *x / 1000.,
        }
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self::MegabytePerSecond(0.0)
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub enum DarkLightMode {
    #[default]
    Dark,
    Light,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, VariantStruct)]
#[serde(default)]
pub struct UILayout {
    pub completed_setup: bool,
    pub shows_recommendation: bool,
    pub sidebar_preivew: bool,
}
