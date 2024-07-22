use std::{
    collections::VecDeque,
    convert::TryInto,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{bail, Context};
use bytes::{BufMut, Bytes, BytesMut};
use dioxus::{prelude::spawn, signals::Readable};
use dioxus_logger::tracing::{debug, error};
use futures::{future::BoxFuture, StreamExt};
use reqwest::Url;
use tokio::{
    fs::File,
    io::{AsyncReadExt, BufReader},
    task,
    time::{self, Instant},
};

use crate::api::shared_resources::{
    collection::CollectionId,
    entry::{DOWNLOAD_PROGRESS, STORAGE},
};

pub type HandlesType<Out = anyhow::Result<()>> = Vec<BoxFuture<'static, Out>>;

pub async fn download_file(
    url: impl AsRef<str> + Send + Sync,
    current_size: impl Into<Option<Arc<AtomicUsize>>>,
) -> Result<Bytes, anyhow::Error> {
    let url = url.as_ref();
    let current_size = current_size.into();
    let client = reqwest::Client::builder()
        .tcp_keepalive(Some(std::time::Duration::from_secs(10)))
        .http2_keep_alive_timeout(std::time::Duration::from_secs(10))
        .build()?;
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
    }?;

    let bytes = match chunked_download(response, current_size.clone()).await {
        Ok(x) => x,
        Err(err) => {
            error!("{err:?} you fucked up.");
            if let Some(ref size) = current_size {
                size.fetch_sub(err.1, Ordering::Relaxed);
            }
            chunked_download(client.get(url).send().await?, current_size)
                .await
                .map_err(|err| err.0)?
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

pub fn extract_filename(url: impl AsRef<str> + Send + Sync) -> anyhow::Result<String> {
    let parsed_url = Url::parse(url.as_ref())?;
    let path_segments = parsed_url.path_segments().context("Invalid URL")?;
    let filename = path_segments.last().context("No filename found in URL")?;
    Ok(filename.to_owned())
}

pub async fn validate_sha1(
    file_path: impl AsRef<Path> + Send + Sync,
    sha1: &str,
) -> anyhow::Result<()> {
    use sha1::{Digest, Sha1};
    let file_path = file_path.as_ref();
    let file = File::open(file_path).await?;
    let mut buffer = Vec::new();
    let mut reader = BufReader::new(file);
    reader.read_to_end(&mut buffer).await?;

    let mut hasher = Sha1::new();
    hasher.update(&buffer);
    let result = &hasher.finalize()[..];

    if result != hex::decode(sha1)? {
        bail!(
            r#"
{} hash don't fit!
get: {}
expected: {}
"#,
            file_path.to_string_lossy(),
            hex::encode(result),
            sha1
        );
    }

    Ok(())
}
pub async fn get_hash(file_path: impl AsRef<Path> + Send + Sync) -> anyhow::Result<Vec<u8>> {
    use sha1::{Digest, Sha1};
    let file_path = file_path.as_ref();
    let file = File::open(file_path).await?;
    let mut buffer = Vec::new();
    let mut reader = BufReader::new(file);
    reader.read_to_end(&mut buffer).await?;
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
}

impl Progress {
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
        self.collection_id.partial_cmp(&other.collection_id)
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

    pub fn is_download_type(&self, download_type: &DownloadType) -> bool {
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
    pub fn is_vanilla_assets(&self) -> bool {
        matches!(self, Self::VanillaAssets(..))
    }

    /// Returns `true` if the download type is [`ModLoaderAssets`].
    ///
    /// [`ModLoaderAssets`]: DownloadType::ModLoaderAssets
    #[must_use]
    pub fn is_mod_loader_assets(&self) -> bool {
        matches!(self, Self::ModLoaderAssets(..))
    }

    /// Returns `true` if the download type is [`ModLoaderProcess`].
    ///
    /// [`ModLoaderProcess`]: DownloadType::ModLoaderProcess
    #[must_use]
    pub fn is_mod_loader_process(&self) -> bool {
        matches!(self, Self::ModLoaderProcess(..))
    }

    /// Returns `true` if the download type is [`ModsDownload`].
    ///
    /// [`ModsDownload`]: DownloadType::ModsDownload
    #[must_use]
    pub fn is_mods_download(&self) -> bool {
        matches!(self, Self::ModsDownload(..))
    }
}

impl Default for DownloadType {
    fn default() -> Self {
        Self::VanillaAssets(String::new())
    }
}

impl DownloadId {
    pub fn from_progress(id: CollectionId, progress: &Progress) -> DownloadId {
        DownloadId {
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
) -> anyhow::Result<()> {
    debug!("receiving download request");
    let calculate_speed = download_args.is_size;
    let download_complete = Arc::new(AtomicBool::new(false));

    spawn(async move {
        let handles = download_args.handles;
        let value = download_type.clone();
        let download_complete_clone = Arc::clone(&download_complete);
        let current_size_clone = Arc::clone(&download_args.current);
        let total_size_clone = Arc::clone(&download_args.total);

        let local = task::LocalSet::new();

        local
            .run_until(async move {
                let output = tokio::task::spawn_local(rolling_average(
                    value,
                    download_complete_clone,
                    current_size_clone,
                    total_size_clone,
                    id.clone(),
                    bias,
                    calculate_speed,
                ));

                join_futures(handles).await.unwrap();

                download_complete.store(true, Ordering::Release);
                output.await.unwrap();
            })
            .await;
    });
    debug!("finish download request");
    Ok(())
}

pub async fn rolling_average(
    download_type: impl Into<Arc<DownloadType>>,
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
        time::sleep(Duration::from_millis(sleep_time.try_into().unwrap())).await;
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
            };
            let download_id = DownloadId::from_progress(id.clone(), &progress);
            DOWNLOAD_PROGRESS.write().0.insert(download_id, progress);
            return;
        }

        let progress = if calculate_speed {
            let speed = (current - prev_bytes) / instant.elapsed().as_secs_f64();

            if average_speed.len() < rolling_average_window {
                average_speed.push_back(speed);
            } else {
                average_speed.pop_front();
                average_speed.push_back(speed);
            }

            let average_speed = average_speed.iter().sum::<f64>() / average_speed.len() as f64;
            debug!("{}", average_speed);

            Progress {
                download_type,
                percentages,
                speed: Some(average_speed),
                current_size: Some(current),
                total_size: Some(total),
                bias,
            }
        } else {
            Progress {
                download_type,
                percentages,
                speed: None,
                current_size: None,
                total_size: None,
                bias,
            }
        };

        if download_complete.load(Ordering::Relaxed) {
            if !completed {
                completed = true;
            } else {
                let download_id = DownloadId::from_progress(id.clone(), &progress);
                DOWNLOAD_PROGRESS.write().0.insert(download_id, progress);
                break;
            }
        } else {
            prev_bytes = current;
            instant = Instant::now();
            let download_id = DownloadId::from_progress(id.clone(), &progress);
            DOWNLOAD_PROGRESS.write().0.insert(download_id, progress);
        }
    }
}

pub async fn join_futures(handles: HandlesType) -> Result<(), anyhow::Error> {
    let concurrent_limit = STORAGE
        .global_settings
        .read()
        .download
        .max_simultatneous_download;
    let mut download_stream = tokio_stream::iter(handles)
        .map(|x| tokio::spawn(x))
        .buffer_unordered(concurrent_limit);
    while let Some(x) = download_stream.next().await {
        x??;
    }
    Ok(())
}

pub struct DownloadArgs {
    pub current: Arc<AtomicUsize>,
    pub total: Arc<AtomicUsize>,
    pub handles: HandlesType,
    pub is_size: bool,
}
