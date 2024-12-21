use std::{
    collections::VecDeque,
    future::Future,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use bytes::{BufMut, Bytes, BytesMut};
use dioxus::{
    prelude::{spawn, throw_error, ScopeId},
    signals::Readable,
};
use dioxus_logger::tracing::{debug, error, info};
use futures::{future::BoxFuture, StreamExt};
use oauth2::url::ParseError;
use pausable_future::Pausable;
use reqwest::Url;
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, BufReader},
    sync::Semaphore,
    task::JoinError,
    time::{self, Instant},
};

use crate::api::shared_resources::{
    collection::CollectionId,
    entry::{DOWNLOAD_PROGRESS, STORAGE},
};

use snafu::prelude::*;

use super::{errors::ManifestProcessingError, vanilla::library::LibraryError};

#[derive(Debug, Snafu)]
pub enum HandleError {
    #[snafu(context(false), display("Failed to download {source}"))]
    Download { source: DownloadError },
    #[snafu(transparent)]
    ManifestProcessingError { source: ManifestProcessingError },
    #[snafu(transparent)]
    JoinError { source: JoinError },
}

impl From<LibraryError> for HandleError {
    fn from(value: LibraryError) -> Self {
        Self::ManifestProcessingError {
            source: value.into(),
        }
    }
}

pub struct HandlesType<Out = Result<(), HandleError>>(pub Vec<Pausable<BoxFuture<'static, Out>>>);

impl Default for HandlesType {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlesType {
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn push(&mut self, future: impl Future<Output = Result<(), HandleError>> + Send + 'static) {
        self.0.push(Pausable::new(Box::pin(future)));
    }
}

#[derive(Debug, Snafu)]
pub enum DownloadError {
    #[snafu(display("Internet Error, could not download {url}"))]
    Internet { source: reqwest::Error, url: String },
    #[snafu(display("Filesystem error at: {}", path.display()))]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },
    #[snafu(display("Url parse error, {url}"))]
    Parse { url: String },
    #[snafu(display("Hash does not match, expected: {expected}\n get: {get}"))]
    HashValidation { expected: String, get: String },
    #[snafu(context(false), display("Not hashable"))]
    FromHex { source: hex::FromHexError },
}

/// # Errors
///
/// Will return `Err` if:
/// - Fails to write `filename`
/// - Failure in `download_file`
pub async fn save_url(
    url: impl AsRef<str> + Send + Sync,
    current_size: impl Into<Option<Arc<AtomicUsize>>> + Send,
    filename: impl AsRef<Path> + Send,
) -> Result<(), DownloadError> {
    let bytes = download_file(url, current_size).await?;
    fs::write(filename.as_ref(), &bytes).await.context(IoSnafu {
        path: filename.as_ref(),
    })
}

/// # Errors
///
/// Will return `Err` if:
/// - Fails to establish internet connection
pub async fn download_file(
    url: impl AsRef<str> + Send + Sync,
    current_size: impl Into<Option<Arc<AtomicUsize>>> + Send,
) -> Result<Bytes, DownloadError> {
    let current_size = current_size.into();
    let url = url.as_ref();
    let client = reqwest::Client::builder()
        .tcp_keepalive(Some(std::time::Duration::from_secs(10)))
        .http2_keep_alive_timeout(std::time::Duration::from_secs(10))
        .build()
        .context(InternetSnafu { url })?;
    let response_result = client.get(url).send().await;

    let retry_amount = 3;
    let response = match response_result {
        Ok(x) => Ok(x),
        Err(err) => {
            let mut temp = Err(err);
            for i in 1..=retry_amount {
                // Wait 0.5s before retry.
                time::sleep(Duration::from_millis(500)).await;
                match client.get(url).send().await {
                    Ok(x) => {
                        temp = Ok(x);
                        break;
                    }
                    Err(x) => {
                        error!("{x}, retry count: {i}");
                    }
                }
            }
            temp
        }
    }
    .context(InternetSnafu { url })?;

    let bytes = match chunked_download(response, current_size.clone()).await {
        Ok(x) => x,
        Err(err) => {
            error!("{err:?} you fucked up.");
            if let Some(ref size) = current_size {
                size.fetch_sub(err.1, Ordering::Relaxed);
            }
            chunked_download(
                client
                    .get(url)
                    .send()
                    .await
                    .context(InternetSnafu { url })?,
                current_size,
            )
            .await
            .map_err(|err| err.0)
            .context(InternetSnafu { url })?
        }
    };

    Ok(bytes)
}

async fn chunked_download(
    mut response: reqwest::Response,
    current_size_clone: Option<Arc<AtomicUsize>>,
) -> Result<Bytes, (reqwest::Error, usize)> {
    let mut bytes = BytesMut::new();
    while let Some(chunk) = response.chunk().await.map_err(|err| (err, bytes.len()))? {
        if let Some(ref size) = current_size_clone {
            size.fetch_add(chunk.len(), Ordering::Relaxed);
        }
        bytes.put(chunk);
    }
    Ok(bytes.freeze())
}

#[derive(Debug, Snafu)]
pub enum FileNameError {
    #[snafu(display("Failed to parse url: {url}"))]
    UrlParsingIssues { source: ParseError, url: String },
    #[snafu(display("Parsed url cannot be base: {url}"))]
    CannotBeBase { url: String },
    #[snafu(display("Filename has not been found in the URL: {url}"))]
    AbscenceFilename { url: String },
}

/// # Errors
///
/// This function returns `err` if:
/// - Fails to parse url
/// - Parsed url cannot be base.
/// - Filename has not been found in the url.
pub fn extract_filename(url: impl AsRef<str> + Send + Sync) -> Result<String, FileNameError> {
    let url = url.as_ref();
    let parsed_url = Url::parse(url).context(UrlParsingIssuesSnafu { url })?;
    let path_segments = parsed_url
        .path_segments()
        .context(CannotBeBaseSnafu { url })?;
    let filename = path_segments
        .last()
        .context(AbscenceFilenameSnafu { url })?;
    Ok(filename.to_owned())
}

/// # Errors
///
/// This function returns `err` if:
/// - Fails to open file.
/// - Fails to read file.
pub async fn validate_sha1(
    file_path: impl AsRef<Path> + Send + Sync,
    sha1: &str,
) -> Result<(), DownloadError> {
    use sha1::{Digest, Sha1};
    let file_path = file_path.as_ref();
    let file = File::open(file_path)
        .await
        .context(IoSnafu { path: file_path })?;
    let mut buffer = Vec::new();
    let mut reader = BufReader::new(file);
    reader
        .read_to_end(&mut buffer)
        .await
        .context(IoSnafu { path: file_path })?;

    let mut hasher = Sha1::new();
    hasher.update(&buffer);
    let result = &hasher.finalize()[..];

    if result != hex::decode(sha1)? {
        let err = HashValidationSnafu {
            expected: hex::encode(result),
            get: sha1,
        };
        return Err(err.build());
    }

    Ok(())
}

/// # Errors
///
/// This function returns `err` if:
/// - Fails to open file.
/// - Fails to read file.
pub async fn get_hash(file_path: impl AsRef<Path> + Send + Sync) -> Result<Vec<u8>, DownloadError> {
    use sha1::{Digest, Sha1};
    let path = file_path.as_ref();
    let file = File::open(path).await.context(IoSnafu { path })?;
    let mut buffer = Vec::new();
    let mut reader = BufReader::new(file);
    reader
        .read_to_end(&mut buffer)
        .await
        .context(IoSnafu { path })?;
    let mut hasher = Sha1::new();
    hasher.update(&buffer);
    let mut vec = Vec::new();
    vec.extend_from_slice(&hasher.finalize()[..]);
    Ok(vec)
}

#[derive(PartialEq, Clone, Debug, PartialOrd)]
pub struct Progress {
    pub download_type: Arc<DownloadType>,
    pub percentages: f64,
    pub speed: Option<f64>,
    pub current_size: Option<f64>,
    pub total_size: Option<f64>,
    pub bias: DownloadBias,
    pub paused: bool,
}

impl Progress {
    #[must_use]
    pub fn finished(&self) -> bool {
        self.percentages >= self.bias.end || self.current_size >= self.total_size
    }
}

#[derive(Clone, Debug)]
pub struct DownloadId {
    pub collection_id: CollectionId,
    pub download_type: Arc<DownloadType>,
}

impl PartialEq for DownloadId {
    fn eq(&self, other: &Self) -> bool {
        self.collection_id == other.collection_id
    }
}

impl Eq for DownloadId {}

impl PartialOrd for DownloadId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.collection_id.cmp(&other.collection_id))
    }
}

impl Ord for DownloadId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.collection_id.cmp(&other.collection_id)
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Hash, PartialOrd, Ord, derive_more::Display)]
pub enum DownloadType {
    VanillaAssets(String),
    ModLoaderAssets(String),
    ModLoaderProcess(String),
    ModsDownload(String),
}

impl DownloadType {
    #[must_use]
    pub fn vanilla() -> Self {
        Self::VanillaAssets(String::from("Vanilla Downloads"))
    }
    #[must_use]
    pub fn mod_loader_assets() -> Self {
        Self::ModLoaderAssets(String::from("Modloader Download"))
    }
    #[must_use]
    pub fn mod_loader_proccess() -> Self {
        Self::ModLoaderAssets(String::from("Modloader Processing"))
    }
    #[must_use]
    pub fn mods_download() -> Self {
        Self::ModLoaderAssets(String::from("Mods Download"))
    }

    #[must_use]
    pub const fn is_download_type(&self, download_type: &Self) -> bool {
        match self {
            Self::VanillaAssets(..) => download_type.is_vanilla_assets(),
            Self::ModLoaderAssets(..) => download_type.is_mod_loader_assets(),
            Self::ModLoaderProcess(..) => download_type.is_mod_loader_process(),
            Self::ModsDownload(..) => download_type.is_mods_download(),
        }
    }

    /// Returns `true` if the download type is [`VanillaAssets`].
    ///
    /// [`VanillaAssets`]: DownloadType::VanillaAssets
    #[must_use]
    pub const fn is_vanilla_assets(&self) -> bool {
        matches!(self, Self::VanillaAssets(..))
    }

    /// Returns `true` if the download type is [`ModLoaderAssets`].
    ///
    /// [`ModLoaderAssets`]: DownloadType::ModLoaderAssets
    #[must_use]
    pub const fn is_mod_loader_assets(&self) -> bool {
        matches!(self, Self::ModLoaderAssets(..))
    }

    /// Returns `true` if the download type is [`ModLoaderProcess`].
    ///
    /// [`ModLoaderProcess`]: DownloadType::ModLoaderProcess
    #[must_use]
    pub const fn is_mod_loader_process(&self) -> bool {
        matches!(self, Self::ModLoaderProcess(..))
    }

    /// Returns `true` if the download type is [`ModsDownload`].
    ///
    /// [`ModsDownload`]: DownloadType::ModsDownload
    #[must_use]
    pub const fn is_mods_download(&self) -> bool {
        matches!(self, Self::ModsDownload(..))
    }
}

impl Default for DownloadType {
    fn default() -> Self {
        Self::VanillaAssets(String::new())
    }
}

impl DownloadId {
    pub fn new(collection_id: CollectionId, download_type: impl Into<Arc<DownloadType>>) -> Self {
        let download_type = download_type.into();
        Self {
            collection_id,
            download_type,
        }
    }

    #[must_use]
    pub fn from_progress(id: CollectionId, progress: &Progress) -> Self {
        Self {
            collection_id: id,
            download_type: progress.download_type.clone(),
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            download_type: Arc::new(DownloadType::default()),
            percentages: 100.0,
            speed: None,
            current_size: None,
            total_size: None,
            bias: DownloadBias {
                start: 0.0,
                end: 100.0,
            },
            paused: false,
        }
    }
}

/// set percentages bias
/// example:
/// start: 30.0
/// end: 100.0
#[derive(PartialEq, Clone, Copy, Debug, PartialOrd)]
pub struct DownloadBias {
    pub start: f64,
    pub end: f64,
}
impl Default for DownloadBias {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 100.,
        }
    }
}

// get progress and and launch download, if HandlesType doesn't exist, does not calculate speed
// does not download stuff when download progress already has it
pub async fn execute_and_progress(
    id: CollectionId,
    download_args: DownloadArgs,
    bias: DownloadBias,
    download_type: DownloadType,
) {
    info!("receiving download request");
    let calculate_speed = download_args.is_size;
    let download_complete = Arc::new(AtomicBool::new(false));

    let handles = download_args.handles;
    let value = download_type.clone();
    let download_complete_clone = Arc::clone(&download_complete);
    let current_size_clone = Arc::clone(&download_args.current);
    let total_size_clone = Arc::clone(&download_args.total);

    spawn(async move {
        spawn(rolling_average(
            value,
            download_complete_clone.clone(),
            current_size_clone,
            total_size_clone,
            id,
            bias,
            calculate_speed,
        ));
        let download_id = DownloadId::new(id, download_type);

        if let Err(err) = join_futures(download_id, handles).await {
            throw_error(err);
        };

        download_complete_clone.store(true, Ordering::Release);
    });

    while !download_complete.load(Ordering::Relaxed) {
        time::sleep(Duration::from_millis(250)).await;
        info!("waiting download to be completed");
    }
    info!("finish download request");
}

pub async fn rolling_average(
    download_type: impl Into<Arc<DownloadType>> + Send,
    download_complete: Arc<AtomicBool>,
    current: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    id: CollectionId,
    bias: DownloadBias,
    calculate_speed: bool,
) {
    let mut instant = Instant::now();
    let mut prev_bytes = 0.;
    let mut completed = false;
    let sleep_time = 250;
    let rolling_average_window = 5000 / sleep_time; // 5000/250 = 20
    let mut average_speed = VecDeque::with_capacity(rolling_average_window);
    let download_type = download_type.into();
    loop {
        time::sleep(Duration::from_millis(sleep_time as u64)).await;
        let download_type = download_type.clone();
        let multiplier = bias.end - bias.start;

        let current = current.load(Ordering::Relaxed) as f64;
        let total = total.load(Ordering::Relaxed) as f64;
        let percentages = (current / total).mul_add(multiplier, bias.start);
        if percentages.is_nan() {
            let progress = Progress {
                download_type,
                percentages: 100.0,
                speed: Some(0.0),
                current_size: Some(0.0),
                total_size: Some(0.0),
                bias: DownloadBias::default(),
                paused: false,
            };
            let download_id = DownloadId::from_progress(id.clone(), &progress);
            DOWNLOAD_PROGRESS.write().0.insert(download_id, progress);
            return;
        }

        let progress = if calculate_speed {
            let speed = (current - prev_bytes) / instant.elapsed().as_secs_f64();

            #[allow(clippy::branches_sharing_code)] // easier understanding
            if average_speed.len() < rolling_average_window {
                average_speed.push_back(speed);
            } else {
                average_speed.pop_front();
                average_speed.push_back(speed);
            }

            let average_speed = average_speed.iter().sum::<f64>() / average_speed.len() as f64;

            let progress = Progress {
                download_type,
                percentages,
                speed: Some(average_speed),
                current_size: Some(current),
                total_size: Some(total),
                bias,
                paused: false,
            };
            debug!("progress: {:#?}", progress);
            progress
        } else {
            Progress {
                download_type,
                percentages,
                speed: None,
                current_size: None,
                total_size: None,
                bias,
                paused: false,
            }
        };

        let download_id = DownloadId::from_progress(id.clone(), &progress);

        let insert = || {
            let mut write = DOWNLOAD_PROGRESS.write();
            if let Some(x) = write.0.get_mut(&download_id) {
                let paused = x.paused;
                *x = progress;
                x.paused = paused;
            } else {
                write.0.insert(download_id, progress);
            }
        };

        if download_complete.load(Ordering::Relaxed) {
            if completed {
                insert();
                break;
            }
            completed = true;
        } else {
            prev_bytes = current;
            instant = Instant::now();
            insert();
        }
    }
}

/// # Errors
///
/// Returns `err` if:
/// - Fails to join from tokio runtime
/// - Error happened in task(with `HandleError`)
pub async fn join_futures(
    download_id: DownloadId,
    handles: HandlesType,
) -> Result<(), HandleError> {
    let concurrent_limit = STORAGE
        .global_settings
        .read()
        .download
        .max_simultatneous_download;
    let download_stream = tokio_stream::iter(handles.0)
        .map(|x| {
            let controller = x.controller();
            (x, controller)
        })
        .collect::<(Vec<_>, Vec<_>)>()
        .await;

    let controllers = download_stream.1;
    let controller_future = async move {
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let download_progress = DOWNLOAD_PROGRESS.peek();
            let Some(progress) = download_progress.0.get(&download_id) else {
                continue;
            };
            let paused = progress.paused;
            for (i, controller) in controllers.iter().enumerate() {
                if paused && !controller.is_paused() {
                    controller.pause();
                    debug!("Paused task {i}");
                }
                if !paused && controller.is_paused() {
                    controller.resume();
                    debug!("Resume task {i}");
                }
            }
        }
    };

    let manager = spawn(controller_future);

    let semaphore = Arc::new(Semaphore::new(concurrent_limit));

    let mut v = Vec::new();

    for x in download_stream.0 {
        let semaphore = Arc::clone(&semaphore);
        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await;
            x.await
        });
        v.push(handle);
    }

    for x in v {
        x.await??;
    }

    manager.pause();

    info!("Finished manager!");
    Ok(())
}

pub struct DownloadArgs {
    pub current: Arc<AtomicUsize>,
    pub total: Arc<AtomicUsize>,
    pub handles: HandlesType,
    pub is_size: bool,
}
