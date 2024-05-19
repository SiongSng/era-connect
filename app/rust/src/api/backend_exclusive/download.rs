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
use futures::{future::BoxFuture, StreamExt};
use log::{debug, error};
use reqwest::Url;
use tokio::{
    fs::File,
    io::{AsyncReadExt, BufReader},
    time::{self, Instant},
};

use crate::api::shared_resources::{
    collection::CollectionId,
    entry::{DOWNLOAD_PROGRESS, STORAGE},
};

pub type HandlesType<'a> = Vec<BoxFuture<'a, anyhow::Result<()>>>;

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

#[derive(PartialEq, Clone, Debug)]
pub struct Progress {
    pub name: Arc<str>,
    pub percentages: f64,
    pub speed: Option<f64>,
    pub current_size: Option<f64>,
    pub total_size: Option<f64>,
    pub bias: DownloadBias,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            name: String::new().into(),
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
#[derive(PartialEq, Clone, Copy, Debug)]
pub struct DownloadBias {
    pub start: f64,
    pub end: f64,
}
impl Default for DownloadBias {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 100.0,
        }
    }
}

// get progress and and launch download, if HandlesType doesn't exist, does not calculate speed
pub async fn execute_and_progress(
    id: CollectionId,
    download_args: DownloadArgs<'_>,
    bias: DownloadBias,
    name: String,
) -> anyhow::Result<()> {
    debug!("receiving download request");
    let handles = download_args.handles;
    let calculate_speed = download_args.is_size;
    let download_complete = Arc::new(AtomicBool::new(false));
    let arc_id = Arc::new(id);

    let download = &STORAGE.global_settings.read().await.download;
    let max_simultaenous_download = download.max_simultatneous_download;

    let download_complete_clone = Arc::clone(&download_complete);
    let current_size_clone = Arc::clone(&download_args.current);
    let total_size_clone = Arc::clone(&download_args.total);
    let id_clone = Arc::clone(&arc_id);
    let output = tokio::spawn(async move {
        rolling_average(
            name,
            download_complete_clone,
            current_size_clone,
            total_size_clone,
            id_clone,
            bias,
            calculate_speed,
        )
        .await
    });

    join_futures(handles, max_simultaenous_download).await?;
    download_complete.store(true, Ordering::Release);
    let progress = output.await?;

    DOWNLOAD_PROGRESS
        .insert(Arc::clone(&arc_id), progress)
        .await?;
    DOWNLOAD_PROGRESS.remove(arc_id).await?;

    debug!("finish download request");

    Ok(())
}

pub async fn rolling_average(
    name: impl Into<Arc<str>>,
    download_complete: Arc<AtomicBool>,
    current: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    id: Arc<CollectionId>,
    bias: DownloadBias,
    calculate_speed: bool,
) -> Progress {
    let mut instant = Instant::now();
    let mut prev_bytes = 0.0;
    let mut completed = false;
    let m_progress;
    let sleep_time = 250;
    let rolling_average_window = 5000 / sleep_time; // 5000/250 = 20
    let mut average_speed = VecDeque::with_capacity(rolling_average_window);
    let name = name.into();
    loop {
        let name = name.clone();
        let multiplier = bias.end - bias.start;

        time::sleep(Duration::from_millis(sleep_time.try_into().unwrap())).await;
        let current = current.load(Ordering::Relaxed) as f64;
        let total = total.load(Ordering::Relaxed) as f64;
        let percentages = (current / total).mul_add(multiplier, bias.start);

        let progress = if calculate_speed {
            let speed = (current - prev_bytes) / instant.elapsed().as_secs_f64() / 1_000_000.0;

            if average_speed.len() < rolling_average_window {
                average_speed.push_back(speed);
            } else {
                average_speed.pop_front();
                average_speed.push_back(speed);
            }

            let average_speed = average_speed.iter().sum::<f64>() / average_speed.len() as f64;

            let global_settings = &STORAGE.global_settings.read().await;
            let speed_limit = global_settings.download.download_speed_limit.as_ref();

            Progress {
                name,
                percentages,
                speed: Some(average_speed),
                current_size: Some(current),
                total_size: Some(total),
                bias,
            }
        } else {
            Progress {
                name,
                percentages,
                speed: None,
                current_size: None,
                total_size: None,
                bias,
            }
        };

        if download_complete.load(Ordering::Acquire) {
            if !completed {
                completed = true;
            } else {
                m_progress = progress;
                break;
            }
        } else {
            prev_bytes = current;
            instant = Instant::now();
            DOWNLOAD_PROGRESS
                .insert(Arc::clone(&id), progress)
                .await
                .unwrap();
        }
    }
    m_progress
}

pub async fn join_futures(
    handles: HandlesType<'_>,
    concurrency_limit: usize,
) -> Result<(), anyhow::Error> {
    let mut download_stream = tokio_stream::iter(handles).buffer_unordered(concurrency_limit);
    while let Some(x) = download_stream.next().await {
        x?;
    }
    Ok(())
}

pub struct DownloadArgs<'a> {
    pub current: Arc<AtomicUsize>,
    pub total: Arc<AtomicUsize>,
    pub handles: HandlesType<'a>,
    pub is_size: bool,
}
