use anyhow::anyhow;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use bytes::Bytes;
use dioxus_logger::tracing::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs;

use crate::api::{
    backend_exclusive::{
        download::{download_file, validate_sha1, DownloadArgs, HandlesType},
        vanilla::launcher::{GameOptions, JvmOptions, LaunchArgs, ProcessedArguments},
    },
    shared_resources::collection::ModLoaderType,
};

use super::forge::convert_maven_to_path;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModloaderLibrary {
    pub downloads: Option<ModloaderLibraryDownloadMetadata>,
    pub name: String,
    pub url: Option<String>,
    pub include_in_classpath: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModloaderLibraryDownloadMetadata {
    pub artifact: ModloaderLibraryArtifact,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModloaderLibraryArtifact {
    pub path: String,
    pub sha1: String,
    pub size: usize,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModloaderVersionsManifest {
    game_versions: Vec<ModloaderVersion>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ModloaderVersion {
    id: String,
    stable: bool,
    loaders: Vec<ForgeLoaders>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ForgeLoaders {
    id: String,
    url: String,
    stable: bool,
}

async fn fetch_neoforge_manifest(game_version: &str, forge_version: Option<&str>) -> Result<Bytes> {
    let bytes = download_file("https://meta.modrinth.com/neo/v0/manifest.json", None).await?;
    let forge_manifest: ModloaderVersionsManifest = serde_json::from_slice(&bytes)?;
    let loaders = &forge_manifest
        .game_versions
        .iter()
        .find(|x| x.id == game_version)
        .context("Can't find the desired game version")?
        .loaders;
    let loader_url = &match forge_version {
        Some(x) => loaders.iter().find(|y| y.id == x),
        None => loaders.first(),
    }
    .context("Can't find the forge manifest")?
    .url;
    download_file(&loader_url, None).await
}

async fn fetch_forge_manifest(game_version: &str, forge_version: Option<&str>) -> Result<Bytes> {
    let bytes = download_file("https://meta.modrinth.com/forge/v0/manifest.json", None).await?;
    let forge_manifest: ModloaderVersionsManifest = serde_json::from_slice(&bytes)?;
    let loaders = &forge_manifest
        .game_versions
        .iter()
        .find(|x| x.id == game_version)
        .context("Can't find the desired game version")?
        .loaders;
    let loader_url = &match forge_version {
        Some(x) => loaders.iter().find(|y| y.id == x),
        None => loaders.first(),
    }
    .context("Can't find the forge manifest")?
    .url;
    download_file(&loader_url, None).await
}

async fn fetch_quilt_manifest(quilt_version: Option<&str>) -> Result<Bytes> {
    let bytes = download_file("https://meta.modrinth.com/quilt/v0/manifest.json", None).await?;
    let version_manifest: ModloaderVersionsManifest = serde_json::from_slice(&bytes)?;
    let loaders = &version_manifest
        .game_versions
        .iter()
        .next()
        .context("Can't find the desired game version")?
        .loaders;
    let loader_url = &match quilt_version {
        Some(x) => loaders.iter().find(|y| y.id == x),
        None => loaders.first(),
    }
    .context("Can't find the forge manifest")?
    .url;
    download_file(&loader_url, None).await
}

async fn fetch_fabric_manifest(fabric_version: Option<&str>) -> Result<Bytes> {
    let bytes = download_file("https://meta.modrinth.com/fabric/v0/manifest.json", None).await?;
    let version_manifest: ModloaderVersionsManifest = serde_json::from_slice(&bytes)?;
    let loaders = &version_manifest
        .game_versions
        .iter()
        .next()
        .context("Can't find the desired game version")?
        .loaders;
    let loader_url = &match fabric_version {
        Some(x) => loaders.iter().find(|y| y.id == x),
        None => loaders.first(),
    }
    .context("Can't find the forge manifest")?
    .url;
    download_file(&loader_url, None).await
}

pub async fn prepare_modloader_download<'a>(
    mod_loader: &ModLoaderType,
    mut launch_args: LaunchArgs,
    jvm_options: JvmOptions,
    game_options: GameOptions,
) -> Result<(DownloadArgs, ProcessedArguments, Value)> {
    let bytes = match mod_loader {
        ModLoaderType::NeoForge => {
            fetch_neoforge_manifest(&game_options.game_version_name, None).await?
        }
        ModLoaderType::Forge => fetch_forge_manifest(&game_options.game_version_name, None).await?,
        ModLoaderType::Quilt => fetch_quilt_manifest(None).await?,
        ModLoaderType::Fabric => fetch_fabric_manifest(None).await?,
    };

    let current_size = Arc::new(AtomicUsize::new(0));
    let total_size = Arc::new(AtomicUsize::new(0));

    let mod_loader_manifest: Value = serde_json::from_slice(&bytes)?;
    let libraries: Vec<ModloaderLibrary> = Vec::<ModloaderLibrary>::deserialize(
        mod_loader_manifest
            .get("libraries")
            .context("modloader library key doesn't exist")?,
    )?;

    let libraries = if mod_loader == &ModLoaderType::Quilt || mod_loader == &ModLoaderType::Fabric {
        libraries
            .into_iter()
            .map(|mut x| {
                x.name = x
                    .name
                    .replace("${modrinth.gameVersion}", &game_options.game_version_name);
                x
            })
            .collect()
    } else {
        libraries
    };

    let mut handles: HandlesType = Vec::new();
    let mut classpath = HashSet::new();
    let library_directory = &jvm_options.library_directory;

    for library in libraries {
        if let Some(download) = library.downloads {
            let current_size_clone = Arc::clone(&current_size);
            let total_size_clone = Arc::clone(&total_size);
            let artifact = download.artifact;
            let path = library_directory.join(artifact.path.clone());
            let url = artifact.url.clone();
            let sha1 = artifact.sha1.clone();

            if library.include_in_classpath {
                classpath.insert(path.to_string_lossy().to_string());
            }
            if !path.exists() {
                fs::create_dir_all(path.parent().context("forge library path doesn't exist")?)
                    .await?;
                total_size_clone.fetch_add(artifact.size, Ordering::Relaxed);
            } else if let Err(err) = validate_sha1(&path, &sha1).await {
                total_size_clone.fetch_add(artifact.size, Ordering::Relaxed);
                error!("{err}\n redownloading");
            }

            handles.push(Box::pin(async move {
                if !path.exists() {
                    total_size_clone.fetch_add(artifact.size, Ordering::Relaxed);
                    let bytes = download_file(&url, current_size_clone).await?;
                    fs::write(path, bytes).await.map_err(|err| anyhow!(err))
                } else if let Err(_) = validate_sha1(&path, &sha1).await {
                    let bytes = download_file(&url, current_size_clone).await?;
                    fs::write(path, bytes).await.map_err(|err| anyhow!(err))
                } else {
                    debug!("hash verified");
                    Ok(())
                }
            }));
        } else {
            let name = convert_maven_to_path(&library.name, None)?;
            let url = library.url.context("url doesn't exist")? + &name;
            let path = library_directory.join(name);

            if library.include_in_classpath {
                classpath.insert(path.to_string_lossy().to_string());
            }

            if !path.exists() {
                handles.push(Box::pin(async move {
                    fs::create_dir_all(path.parent().context("library parent dir doesn't exist")?)
                        .await?;
                    let bytes = download_file(&url, None).await?;
                    fs::write(path, bytes).await.map_err(|err| anyhow!(err))
                }));
            }
        }
    }

    let pos = launch_args.jvm_args.iter().position(|x| x == "-cp");
    if let Some(pos) = pos {
        let classpaths = launch_args
            .jvm_args
            .get_mut(pos + 1)
            .context("the next argument of -cp doesn't exist")?;
        let mut a = classpaths
            .split(':')
            .map(ToString::to_string)
            .collect::<HashSet<_>>();
        a.extend(classpath);
        *classpaths = a.into_iter().collect::<Vec<_>>().join(":");
    }

    Ok((
        DownloadArgs {
            current: Arc::clone(&current_size),
            total: Arc::clone(&total_size),
            handles,
            is_size: true,
        },
        ProcessedArguments {
            launch_args,
            jvm_args: jvm_options,
            game_args: game_options,
        },
        mod_loader_manifest,
    ))
}
