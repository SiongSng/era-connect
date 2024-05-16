use anyhow::Context;
use chrono::{DateTime, Utc};
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

pub async fn get_versions() -> anyhow::Result<Vec<VersionMetadata>> {
    let response = download_file(VERSION_MANIFEST_URL, None)
        .await
        .context("Failed to download version manifest")?;
    let version_manifest: VersionsManifest =
        serde_json::from_slice(&response).context("Failed to parse version manifest")?;

    Ok(version_manifest.versions)
}
