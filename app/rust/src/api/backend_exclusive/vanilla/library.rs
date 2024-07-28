use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use dioxus_logger::tracing::error;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::api::backend_exclusive::{
    download::{download_file, validate_sha1, DownloadError, HandlesType},
    errors::{self, ManifestProcessingError},
};

use super::rules::{ActionType, OsName, Rule};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Library {
    pub downloads: LibraryArtifact,
    pub name: String,
    pub rules: Option<Vec<Rule>>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Metadata {
    pub path: Option<String>,
    pub sha1: String,
    pub size: usize,
    pub url: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct LibraryArtifact {
    pub artifact: Metadata,
}

pub fn os_match(library: &Library, current_os_type: OsName) -> (bool, bool, &str) {
    let mut process_native = false;
    let mut is_native_library = false;
    let mut library_extension_type = "";
    if let Some(rule) = &library.rules {
        for x in rule {
            if let Some(os) = &x.os {
                if let Some(name) = os.name {
                    if current_os_type == name && x.action == ActionType::Allow {
                        process_native = true;
                    }
                    is_native_library = true;
                    library_extension_type = match current_os_type {
                        OsName::Osx | OsName::OsxArm64 => ".dylib",
                        OsName::Linux | OsName::LinuxArm64 | OsName::LinuxArm32 => ".so",
                        OsName::Windows => ".dll",
                    }
                }
            }
        }
    }
    (process_native, is_native_library, library_extension_type)
}

use snafu::prelude::*;

#[derive(Snafu, Debug)]
pub enum LibraryError {
    #[snafu(display("OS {os} not supported"))]
    OsNotSupported { os: String },
    #[snafu(display("{str}"))]
    OsDetection { str: String },
    #[snafu(transparent)]
    Zip { source: zip::result::ZipError },
    #[snafu(transparent)]
    Download { source: DownloadError },
    #[snafu(display("Could not read file at: {path:?}"))]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },
}

pub async fn parallel_library(
    library_list: Arc<[Library]>,
    folder: Arc<Path>,
    native_folder: Arc<Path>,
    current: Arc<AtomicUsize>,
    total_size: Arc<AtomicUsize>,
    library_download_handles: &mut HandlesType,
) -> Result<(), LibraryError> {
    let current_size = current;

    let current_os =
        os_version::detect().map_err(|x| LibraryError::OsDetection { str: x.to_string() })?;
    let current_os_type = match current_os {
        os_version::OsVersion::Linux(_) => Some(OsName::Linux),
        os_version::OsVersion::Windows(_) => Some(OsName::Windows),
        os_version::OsVersion::MacOS(_) => Some(OsName::Osx),
        _ => None,
    }
    .context(OsNotSupportedSnafu {
        os: current_os.to_string(),
    })?;

    for index in 0..(library_list).len() {
        let library_list = Arc::clone(&library_list);
        let folder = Arc::clone(&folder);
        let native_folder = Arc::clone(&native_folder);
        let total_size = Arc::clone(&total_size);
        let current_size = Arc::clone(&current_size);
        let current_os = current_os_type.clone();
        let library = &library_list[index];
        let path = library.downloads.artifact.path.clone();
        let (process_native, is_native_library, library_extension) = os_match(&library, current_os);
        if let Some(path) = &path {
            let non_native_download_path = folder.join(&path);
            let non_native_redownload = if non_native_download_path.exists() {
                if let Err(x) = if !process_native && !is_native_library {
                    validate_sha1(&non_native_download_path, &library.downloads.artifact.sha1).await
                } else {
                    Ok(())
                } {
                    error!("{x}, \nredownloading.");
                    true
                } else {
                    false
                }
            } else {
                true
            };

            if !process_native && non_native_redownload {
                if let Some(parent_dir) = non_native_download_path.parent() {
                    fs::create_dir_all(parent_dir)
                        .await
                        .context(IoSnafu { path: parent_dir })?;
                    total_size.fetch_add(library.downloads.artifact.size, Ordering::Relaxed);
                }
            }
            if process_native {
                total_size.fetch_add(library.downloads.artifact.size, Ordering::Relaxed);
            }

            let url = library.downloads.artifact.url.clone();
            let library_extension = library_extension.to_string();

            let handle = Box::pin(async move {
                if !process_native && non_native_redownload {
                    let bytes = download_file(url.clone(), Arc::clone(&current_size)).await?;
                    fs::write(&non_native_download_path, bytes)
                        .await
                        .context(errors::IoSnafu {
                            path: non_native_download_path,
                        })?;
                }
                // always download(kinda required for now.)
                if process_native {
                    native_download(
                        &url.clone(),
                        current_size,
                        native_folder,
                        &library_extension,
                    )
                    .await
                    .map_err(Into::<ManifestProcessingError>::into)?;
                }
                Ok(())
            });
            library_download_handles.push(handle);
        }
    }

    Ok(())
}

async fn native_download(
    url: &String,
    current_size: Arc<AtomicUsize>,
    native_folder: Arc<Path>,
    library_extension: &str,
) -> Result<(), LibraryError> {
    let current_size_clone = Arc::clone(&current_size);
    let bytes = download_file(url, current_size_clone).await?;

    let reader = std::io::Cursor::new(bytes);
    if let Ok(mut archive) = zip::ZipArchive::new(reader) {
        let native_temp_dir = native_folder.join("temp");
        archive.extract(native_temp_dir.as_path())?;
        for x in archive.file_names() {
            if (x.contains(library_extension) || x.contains(".sha1")) && !x.contains(".git") {
                let to =
                    native_folder.join(PathBuf::from(x).file_name().expect("Somhow no filename"));
                tokio::fs::rename(native_temp_dir.join(x), to)
                    .await
                    .context(IoSnafu {
                        path: &native_temp_dir,
                    })?;
            }
        }
    }
    Ok(())
}
