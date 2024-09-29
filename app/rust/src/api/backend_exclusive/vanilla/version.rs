use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::api::backend_exclusive::{download::download_file, errors::ManifestProcessingError};

const VERSION_MANIFEST_URL: &str = "https://meta.modrinth.com/minecraft/v0/manifest.json";

#[derive(Debug, Deserialize, Clone)]
pub struct VersionsManifest {
    pub latest: LatestVersion,
    pub versions: Vec<VersionMetadata>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LatestVersion {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
    /// A unique identifier of the version, for example `1.20.1` or `23w33a`.
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: VersionType,
    /// A direct link to the detailed metadata file for this version.
    pub url: String,
    #[serde(rename = "time")]
    pub uploaded_time: DateTime<Utc>,
    pub release_time: DateTime<Utc>,
    pub sha1: String,
    pub compliance_level: u32,
}

impl VersionMetadata {
    pub async fn from_id(id: &str) -> Result<Option<Self>, ManifestProcessingError> {
        Ok(get_version_manifest()
            .await?
            .versions
            .iter()
            .find(|x| x.id == id)
            .cloned())
    }
    pub async fn latest_release() -> Result<Option<Self>, ManifestProcessingError> {
        Self::from_id(&get_version_manifest().await?.latest.release).await
    }
    pub async fn latest_snapshot() -> Result<Option<Self>, ManifestProcessingError> {
        Self::from_id(&get_version_manifest().await?.latest.snapshot).await
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}
impl Ord for VersionMetadata {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.release_time.cmp(&other.release_time)
    }
}

impl PartialOrd for VersionMetadata {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

use crate::api::backend_exclusive::errors::*;
use snafu::prelude::*;

static TEMP: Mutex<Option<VersionsManifest>> = const { Mutex::const_new(None) };

pub async fn get_version_manifest() -> Result<VersionsManifest, ManifestProcessingError> {
    let mut tmp = TEMP.lock().await;
    if let Some(x) = &*tmp {
        Ok(x.clone())
    } else {
        let response = download_file(VERSION_MANIFEST_URL, None).await?;
        let slice: VersionsManifest =
            serde_json::from_slice(&response).context(DesearializationSnafu)?;
        *tmp = Some(slice.clone());
        Ok(slice)
    }
}
