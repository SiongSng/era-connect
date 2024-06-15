use anyhow::Context;
use chrono::{DateTime, Utc};
use futures::executor::block_on;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::api::backend_exclusive::download::download_file;

const VERSION_MANIFEST_URL: &str = "https://meta.modrinth.com/minecraft/v0/manifest.json";

#[derive(Debug, Deserialize)]
pub struct VersionsManifest {
    pub latest: LatestVersion,
    pub versions: Vec<VersionMetadata>,
}

#[derive(Debug, Deserialize)]
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
    pub async fn from_id(id: &str) -> anyhow::Result<Self> {
        VERSION_MANIFEST
            .versions
            .iter()
            .find(|x| x.id == id)
            .context("Can't find version_metadata with this id")
            .cloned()
    }
    pub async fn latest_release() -> anyhow::Result<Self> {
        Self::from_id(&VERSION_MANIFEST.latest.release).await
    }
    pub async fn latest_snapshot() -> anyhow::Result<Self> {
        Self::from_id(&VERSION_MANIFEST.latest.snapshot).await
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

pub static VERSION_MANIFEST: Lazy<VersionsManifest> = Lazy::new(|| {
    let response = block_on(download_file(VERSION_MANIFEST_URL, None)).unwrap();
    serde_json::from_slice(&response).unwrap()
});
