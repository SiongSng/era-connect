use dioxus_logger::tracing::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snafu::prelude::*;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicUsize, atomic::Ordering, Arc},
};
use tokio::fs;

use crate::api::backend_exclusive::{
    download::{download_file, validate_sha1, HandlesType},
    errors::*,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
    pub url: String,
    pub id: String,
    sha1: String,
    size: usize,
    total_size: usize,
}

pub struct AssetSettings {
    pub asset_download_list: Vec<String>,
    pub asset_download_hash: Vec<String>,
    pub asset_download_path: Vec<PathBuf>,
    pub asset_download_size: Vec<usize>,
}

pub async fn extract_assets(
    asset_index: &AssetIndex,
    folder: PathBuf,
) -> Result<AssetSettings, ManifestProcessingError> {
    let asset_index_path = folder.join(PathBuf::from(format!("indexes/{}.json", &asset_index.id)));
    let asset_index_content: Value = if asset_index_path.exists()
        && validate_sha1(&asset_index_path, asset_index.sha1.as_str())
            .await
            .is_ok()
    {
        let b: &[u8] = &fs::read(&asset_index_path).await.context(IoSnafu {
            path: asset_index_path,
        })?;
        serde_json::from_slice(b).context(DesearializationSnafu)?
    } else {
        let path = asset_index_path.parent().context(AssetPathParentSnafu)?;
        fs::create_dir_all(path).await.context(IoSnafu { path })?;
        let bytes = download_file(&asset_index.url, None).await?;
        fs::write(&asset_index_path, &bytes)
            .await
            .context(IoSnafu {
                path: asset_index_path,
            })?;
        serde_json::from_slice(&bytes).context(DesearializationSnafu)?
    };
    let asset_objects = asset_index_content
        .get("objects")
        .and_then(|x| x.as_object())
        .context(ManifestLookUpSnafu {
            key: "asset_index_contet[objects]",
        })?;
    let mut asset_download_list = Vec::new();
    let mut asset_download_hash = Vec::new();
    let mut asset_download_path = Vec::new();
    let mut asset_download_size = Vec::new();
    for (_, val) in asset_objects {
        let size = val
            .get("size")
            .and_then(|x| x.as_u64())
            .context(ManifestLookUpSnafu {
                key: "asset_objects[_][size]",
            })? as usize;
        let hash = val
            .get("hash")
            .and_then(|x| x.as_str())
            .context(ManifestLookUpSnafu {
                key: "asset_objects[_][hash]",
            })?;
        asset_download_list.push(format!(
            "https://resources.download.minecraft.net/{hash:.2}/{hash}",
        ));
        asset_download_hash.push(hash.to_owned());
        asset_download_path.push(folder.join(PathBuf::from(format!("objects/{hash:.2}/{hash}"))));
        asset_download_size.push(size);
    }
    Ok(AssetSettings {
        asset_download_list,
        asset_download_hash,
        asset_download_path,
        asset_download_size,
    })
}

pub async fn parallel_assets_download(
    assets: AssetSettings,
    current_size: &Arc<AtomicUsize>,
    total_size: &Arc<AtomicUsize>,
    handles: &mut HandlesType,
) -> Result<(), ManifestProcessingError> {
    let asset_download_list = Arc::new(assets.asset_download_list);
    let asset_download_hash = Arc::new(assets.asset_download_hash);
    let asset_download_path = Arc::new(assets.asset_download_path);
    let asset_download_size = Arc::new(assets.asset_download_size);
    for index in 0..asset_download_list.len() {
        debug!("downloading asset");
        let current_size = Arc::clone(current_size);
        let total_size = Arc::clone(total_size);
        let asset_download_list = Arc::clone(&asset_download_list);
        let asset_download_size = Arc::clone(&asset_download_size);
        let asset_download_hash = Arc::clone(&asset_download_hash);
        let asset_download_paths = Arc::clone(&asset_download_path);
        let asset_download_path = &asset_download_paths[index];
        if let Some(path) = asset_download_path.parent() {
            fs::create_dir_all(path).await.context(IoSnafu { path })?;
        }
        let okto_download = if asset_download_path.exists() {
            if let Err(x) = validate_sha1(
                asset_download_path,
                asset_download_hash
                    .get(index)
                    .context(ManifestLookUpSnafu {
                        key: "asset_download_hash[index]",
                    })?,
            )
            .await
            {
                error!("{x}, \nredownloading.");
                true
            } else {
                false
            }
        } else {
            true
        };
        if okto_download {
            total_size.fetch_add(
                *asset_download_size
                    .get(index)
                    .context(ManifestLookUpSnafu {
                        key: "asset_download_size[index]",
                    })?,
                Ordering::Relaxed,
            );
        }
        let asset_download_path = asset_download_path.clone();
        handles.push(Box::pin(async move {
            if okto_download {
                let bytes = download_file(
                    asset_download_list
                        .get(index)
                        .context(ManifestLookUpSnafu {
                            key: "asset_download_list[index]",
                        })?,
                    current_size,
                )
                .await?;
                fs::write(&asset_download_path, bytes)
                    .await
                    .context(IoSnafu {
                        path: asset_download_path,
                    })?;
            }
            Ok(())
        }));
    }
    Ok(())
}
