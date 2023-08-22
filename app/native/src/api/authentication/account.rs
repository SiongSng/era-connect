use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use flutter_rust_bridge::frb;
use image::{imageops, DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::DATA_DIR;

#[frb(dart_metadata=("freezed"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftAccount {
    pub username: String,
    pub uuid: Uuid,
    pub access_token: AccountToken,
    pub refresh_token: AccountToken,
    pub skins: Vec<MinecraftSkin>,
    pub capes: Vec<MinecraftCape>,
}

#[frb(dart_metadata=("freezed"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftSkin {
    pub id: Uuid,
    pub state: String,
    pub url: String,
    pub variant: MinecraftSkinVariant,
}

#[frb(dart_metadata=("freezed"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MinecraftSkinVariant {
    #[serde(rename = "CLASSIC")]
    Classic,
    #[serde(rename = "SLIM")]
    Slim,
}

#[frb(dart_metadata=("freezed"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftCape {
    pub id: Uuid,
    pub state: String,
    pub url: String,
    pub alias: String,
}

#[frb(dart_metadata=("freezed"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountToken {
    pub token: String,
    pub expires_at: i64,
}

impl MinecraftSkin {
    pub async fn download_skin(&self) -> anyhow::Result<()> {
        let response = reqwest::get(&self.url)
            .await
            .context("Failed to download skin")?;

        let raw_image = response.bytes().await.context("Failed to get skin bytes")?;
        let head_image = Self::obtain_player_head(&raw_image).await?;

        fs::create_dir_all(self.get_skin_directory()).context("Failed to create skin directory")?;
        fs::write(self.get_raw_file_path(), raw_image).context("Failed to save raw skin")?;
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
    async fn obtain_player_head(buffer: &[u8]) -> Result<DynamicImage, image::ImageError> {
        let image = image::load_from_memory_with_format(buffer, ImageFormat::Png)?
            .crop_imm(8, 8, 8, 8)
            .resize_to_fill(50, 50, imageops::FilterType::Nearest);

        Ok(image)
    }
}
