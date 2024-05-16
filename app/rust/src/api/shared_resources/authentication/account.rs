use std::path::PathBuf;

use anyhow::Context;
use image::{imageops, DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use tokio::fs;
pub use uuid::Uuid;

use crate::api::backend_exclusive::download::download_file;
use crate::api::shared_resources::entry::DATA_DIR;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftAccount {
    pub username: String,
    pub uuid: Uuid,
    pub access_token: AccountToken,
    pub refresh_token: AccountToken,
    pub skins: Vec<MinecraftSkin>,
    pub capes: Vec<MinecraftCape>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftSkin {
    pub id: Uuid,
    pub state: String,
    pub url: String,
    pub variant: MinecraftSkinVariant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MinecraftSkinVariant {
    #[serde(rename = "CLASSIC")]
    Classic,
    #[serde(rename = "SLIM")]
    Slim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftCape {
    pub id: Uuid,
    pub state: String,
    pub url: String,
    pub alias: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountToken {
    pub token: String,
    pub expires_at: i64,
}

impl MinecraftSkin {
    pub async fn download_skin(&self) -> anyhow::Result<()> {
        let raw_image = download_file(&self.url, None)
            .await
            .context("Failed to download skin")?;

        let head_image = Self::obtain_player_head(&raw_image)?;

        fs::create_dir_all(self.get_skin_directory())
            .await
            .context("Failed to create skin directory")?;
        fs::write(self.get_raw_file_path(), raw_image)
            .await
            .context("Failed to save raw skin")?;
        head_image.save_with_format(self.get_head_file_path(), ImageFormat::Png)?;

        Ok(())
    }

    pub fn get_head_file_path(&self) -> PathBuf {
        self.get_skin_directory().join("head.png")
    }

    fn get_skin_directory(&self) -> PathBuf {
        DATA_DIR.join("skins").join(self.id.to_string())
    }

    fn get_raw_file_path(&self) -> PathBuf {
        self.get_skin_directory().join("raw.png")
    }

    /// Obtain the player head image from the raw skin buffer and return the head image.
    fn obtain_player_head(buffer: &[u8]) -> Result<DynamicImage, image::ImageError> {
        image::load_from_memory_with_format(buffer, ImageFormat::Png).map(|x| {
            x.crop_imm(8, 8, 8, 8)
                .resize_to_fill(50, 50, imageops::FilterType::Nearest)
        })
    }
}
