use dioxus::signals::{Readable, SyncSignal, Writable};
use dioxus_logger::tracing::{error, info, Level};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::sync::{atomic::AtomicUsize, atomic::Ordering, Arc};
use tokio::fs::create_dir_all;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStdout;
use tokio::task::{JoinError, JoinHandle};
use xml::reader::XmlEvent;
use xml::EventReader;

use super::assets::{extract_assets, parallel_assets_download};
use super::library::{os_match, parallel_library, Library};
use super::manifest::{self, Argument, GameManifest};
use super::rules::{ActionType, OsName};
use crate::api::backend_exclusive::download::{
    execute_and_progress, save_url, DownloadBias, DownloadType, HandlesType,
};
use crate::api::backend_exclusive::errors::ManifestProcessingError;
use crate::api::backend_exclusive::errors::*;
use crate::api::backend_exclusive::vanilla::manifest::fetch_game_manifest;
use crate::api::backend_exclusive::{
    download::{extract_filename, validate_sha1, DownloadArgs},
    storage::storage_loader::get_global_shared_path,
};
use crate::api::shared_resources::collection::Collection;
use crate::api::shared_resources::entry::STORAGE;
use snafu::prelude::*;

#[derive(Debug, PartialEq)]
struct GameFlagsProcessed {
    arguments: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct JvmFlagsProcessed {
    arguments: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct RawGameFlags {
    arguments: Vec<Argument>,
}

#[derive(Debug, PartialEq)]
struct RawJvmFlags {
    arguments: Vec<Argument>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct LogFile {
    id: String,
    sha1: String,
    size: usize,
    url: String,
}

#[derive(Clone, Debug)]
pub struct JvmOptions {
    pub launcher_name: String,
    pub launcher_version: String,
    pub classpath: String,
    pub classpath_separator: String,
    pub primary_jar: String,
    pub library_directory: Arc<Path>,
    pub game_directory: Arc<Path>,
    pub native_directory: Arc<Path>,
}

#[derive(Clone, Debug)]
pub struct GameOptions {
    pub auth_player_name: String,
    pub game_version_name: String,
    pub game_directory: Arc<Path>,
    pub assets_root: Arc<Path>,
    pub assets_index_name: String,
    pub auth_access_token: String,
    pub auth_uuid: String,
    pub user_type: String,
    pub version_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct LaunchArgs {
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub game_args: Vec<String>,
}

#[derive(Snafu, Debug)]
pub enum LaunchGameError {
    #[snafu(display("Could not read tokio spawn task"))]
    TokioSpawnTask { source: std::io::Error },
    #[snafu(transparent)]
    JoinError { source: JoinError },
    #[snafu(transparent)]
    Logger { source: LoggerEventError },
    #[snafu(transparent)]
    ManifestProcessing { source: ManifestProcessingError },
}

pub struct Logger {
    pub level: Level,
    reader: BufReader<ChildStdout>,
}

#[derive(derive_builder::Builder, Debug)]
pub struct LoggerEvent {
    level: Level,
    logger: String,
    thread: String,
    timestamp: String,
    #[builder(setter(into, strip_option), default)]
    message: Option<String>,
}

impl LoggerEvent {
    pub fn level(&self) -> &Level {
        &self.level
    }
    pub fn logger(&self) -> &str {
        &self.logger
    }
    pub fn thread(&self) -> &str {
        &self.thread
    }
    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

impl Default for LoggerEvent {
    fn default() -> Self {
        Self {
            level: Level::TRACE,
            message: None,
            logger: String::new(),
            thread: String::new(),
            timestamp: String::new(),
        }
    }
}

impl Display for LoggerEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}/{}]: {}",
            self.thread,
            self.level,
            self.message.clone().unwrap_or_default()
        )
    }
}

#[derive(Snafu, Debug)]
pub enum LoggerEventError {
    #[snafu(display("Could not fetch the next line of lines"))]
    NextLine { source: std::io::Error },
    #[snafu(display("xml: {xml}, why: {source}"))]
    Builder {
        source: LoggerEventBuilderError,
        xml: String,
    },

    #[snafu(display("Unhandled XML attribute: {attribute}"))]
    UnhandlededAttribute { attribute: String },

    #[snafu(display("Failed to parse log level: {level}"))]
    InvalidLevel {
        source: <Level as FromStr>::Err,
        level: String,
    },
}

struct XmlNotFinished;

impl Logger {
    #[must_use]
    pub const fn new(reader: BufReader<ChildStdout>) -> Self {
        Self {
            level: Level::INFO,
            reader,
        }
    }

    #[must_use]
    pub fn get_event(
        self,
        mut signal: SyncSignal<LoggerEvent>,
    ) -> JoinHandle<Result<(), LoggerEventError>> {
        let reader = self.reader;
        let mut lines = reader.lines();
        let mut total = String::new();
        tokio::spawn(async move {
            while let Some(mut x) = lines.next_line().await.context(NextLineSnafu)? {
                if !x.contains("log4j") {
                    continue;
                }
                x = x.replace("log4j:", "");
                total.push_str(&x);

                if let Ok(event) = Self::try_parse_xml(&total)? {
                    signal.set(event);
                    total.clear();
                }
            }
            Ok(())
        })
    }

    fn try_parse_xml(xml: &str) -> Result<Result<LoggerEvent, XmlNotFinished>, LoggerEventError> {
        let parser = EventReader::from_str(xml);
        let mut is_complete = false;
        let mut builder = LoggerEventBuilder::create_empty();

        for event in parser {
            match event {
                Ok(XmlEvent::StartElement { attributes, .. }) => {
                    for attr in &attributes {
                        match &*attr.name.local_name {
                            "logger" => {
                                builder.logger(attr.value.clone());
                            }
                            "timestamp" => {
                                builder.timestamp(attr.value.clone());
                            }
                            "level" => {
                                let level = Level::from_str(&attr.value)
                                    .context(InvalidLevelSnafu { level: &attr.value })?;
                                builder.level(level);
                            }
                            "thread" => {
                                builder.thread(attr.value.clone());
                            }
                            x => {
                                return UnhandlededAttributeSnafu { attribute: x }.fail();
                            }
                        }
                    }
                }
                Ok(XmlEvent::CData(x)) => {
                    builder.message(x);
                }
                Ok(XmlEvent::EndElement { .. }) => {
                    is_complete = true;
                }
                Ok(_) => {}
                Err(_) => return Ok(Err(XmlNotFinished)),
            }
        }

        if is_complete {
            builder.build().context(BuilderSnafu { xml }).map(|x| Ok(x))
        } else {
            Ok(Err(XmlNotFinished))
        }
    }
}

pub async fn launch_game(
    launch_args: &LaunchArgs,
    signal: SyncSignal<LoggerEvent>,
) -> Result<JoinHandle<Result<(), LoggerEventError>>, LaunchGameError> {
    let mut launch_vec = Vec::new();
    launch_vec.extend(&launch_args.jvm_args);
    launch_vec.push(&launch_args.main_class);
    launch_vec.extend(&launch_args.game_args);
    let mut child = tokio::process::Command::new("java")
        .args(launch_vec)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(TokioSpawnTaskSnafu)?;

    let stdout = child.stdout.take().expect("stdout does not exist.");
    let reader = BufReader::new(stdout);
    let logger = Logger::new(reader);

    Ok(logger.get_event(signal))
}

pub async fn full_vanilla_download(collection: &Collection) -> Result<LaunchArgs, LaunchGameError> {
    info!("Starts Vanilla Downloading");
    let collection_id = collection.get_collection_id();
    let vanilla_bias = DownloadBias {
        start: 0.0,
        end: 100.0,
    };
    let game_manifest = fetch_game_manifest(&collection.minecraft_version().url).await?;
    let (vanilla_download_args, vanilla_arguments) =
        prepare_vanilla_download(collection, game_manifest.clone()).await?;
    execute_and_progress(
        collection_id,
        vanilla_download_args,
        vanilla_bias,
        DownloadType::vanilla(),
    )
    .await;

    Ok(vanilla_arguments.launch_args)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuiltLibrary {
    name: String,
    url: String,
}

pub struct ProcessedArguments {
    pub launch_args: LaunchArgs,
    pub jvm_args: JvmOptions,
    pub game_args: GameOptions,
}

#[allow(clippy::too_many_lines)]
pub async fn prepare_vanilla_download(
    collection: &Collection,
    game_manifest: GameManifest,
) -> Result<(DownloadArgs, ProcessedArguments), ManifestProcessingError> {
    let version_id = collection.minecraft_version().id.clone();
    let shared_path = get_global_shared_path();

    let game_directory = collection.game_directory();
    let asset_directory = shared_path.join("assets");
    let library_directory = shared_path.join("libraries");
    let version_directory = shared_path.join(format!("versions/{version_id}"));
    let native_directory = version_directory.join("natives");

    create_dir_all(&library_directory).await.context(IoSnafu {
        path: &library_directory,
    })?;
    create_dir_all(&asset_directory).await.context(IoSnafu {
        path: &asset_directory,
    })?;
    create_dir_all(&game_directory).await.context(IoSnafu {
        path: &game_directory,
    })?;
    create_dir_all(&native_directory).await.context(IoSnafu {
        path: &native_directory,
    })?;

    info!("Setting up game flags");

    let game_flags = setup_game_flags(game_manifest.arguments.game);

    info!("Setting up jvm flags");
    let jvm_flags = setup_jvm_flags(game_manifest.arguments.jvm);

    let (game_options, game_flags) = setup_game_option(
        version_id,
        &game_directory,
        &asset_directory,
        game_manifest.asset_index.id.clone(),
        game_flags,
    )
    .await
    .context(GameOptionsSnafu)?;

    let downloads_list = game_manifest.downloads;
    let library_list: Arc<[Library]> = game_manifest.libraries.into();

    let client_jar_filename = version_directory.join(
        extract_filename(&downloads_list.client.url).context(ExtractDownloadingFilenameSnafu {
            str: &downloads_list.client.url,
        })?,
    );

    info!("Setting up jvm options");
    let jvm_options = setup_jvm_options(
        client_jar_filename.to_string_lossy().to_string(),
        &library_directory,
        &game_directory,
        &native_directory,
    )?;

    info!("Setting up jvm rules");
    let (jvm_options, mut jvm_flags) = add_jvm_rules(
        &library_list,
        &library_directory,
        &client_jar_filename,
        jvm_options,
        jvm_flags,
    )
    .context(JvmRulesSnafu)?;

    jvm_flags.arguments = jvm_args_parse(&jvm_flags.arguments, &jvm_options);

    let mut handles = HandlesType::new();

    let current_size = Arc::new(AtomicUsize::new(0));

    let (library_path, native_library_path) = (
        Arc::clone(&jvm_options.library_directory),
        Arc::clone(&jvm_options.native_directory),
    );

    info!("Preping library downloading");
    let total_size = Arc::new(AtomicUsize::new(0));
    parallel_library(
        Arc::clone(&library_list),
        library_path,
        native_library_path,
        Arc::clone(&current_size),
        Arc::clone(&total_size),
        &mut handles,
    )
    .await?;

    info!("Downloads client jar");
    if !client_jar_filename.exists() {
        total_size.fetch_add(downloads_list.client.size, Ordering::Relaxed);
        let current_size = Arc::clone(&current_size);
        handles.push(Box::pin(
            save_url(downloads_list.client.url, current_size, client_jar_filename)
                .map_err(Into::into),
        ));
    } else if let Err(x) = validate_sha1(&client_jar_filename, &downloads_list.client.sha1).await {
        total_size.fetch_add(downloads_list.client.size, Ordering::Relaxed);
        error!("{x}\n redownloading.");
        let current_size = Arc::clone(&current_size);
        handles.push(Box::pin(
            save_url(downloads_list.client.url, current_size, client_jar_filename)
                .map_err(Into::into),
        ));
    }

    info!("Preping for assets download");
    let asset_settings = extract_assets(&game_manifest.asset_index, asset_directory).await?;
    info!("Asset information grepped, now turning them into downloadable args");
    parallel_assets_download(asset_settings, &current_size, &total_size, &mut handles).await?;

    if let Some(advanced_option) = &collection.advanced_options() {
        if let Some(max_memory) = advanced_option.jvm_max_memory {
            jvm_flags.arguments.push(format!("-Xmx{max_memory}M"));
        }
        jvm_flags.arguments.extend(
            advanced_option
                .java_arguments
                .split(' ')
                .map(ToOwned::to_owned),
        );
    }

    info!("Setup logging");

    let xml = build_log_xml(&game_directory);
    let path = version_directory.join("log4j2.xml");

    jvm_flags
        .arguments
        .push(format!("-Dlog4j.configurationFile={}", path.display()));

    tokio::fs::write(&path, xml)
        .await
        .context(IoSnafu { path })?;

    let launch_args = LaunchArgs {
        jvm_args: jvm_flags.arguments,
        main_class: game_manifest.main_class,
        game_args: game_flags.arguments,
    };

    info!("Finished vanilla preping");

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
    ))
}

fn build_log_xml(path: &Path) -> String {
    format!(
        r#"
            <Configuration status="WARN">
<Appenders>
<Console name="SysOut" target="SYSTEM_OUT">
<LegacyXMLLayout/>
</Console>
<RollingRandomAccessFile name="File" fileName="{0}/logs/latest.log" filePattern="{0}/logs/%d{{yyyy-MM-dd}}-%i.log.gz">
<PatternLayout pattern="[%d{{HH:mm:ss}}] [%t/%level]: %msg{{nolookups}}%n"/>
<Policies>
<TimeBasedTriggeringPolicy/>
<OnStartupTriggeringPolicy/>
</Policies>
</RollingRandomAccessFile>
<Listener name="Tracy">
<PatternLayout pattern="(%F:%L): %msg{{nolookups}}%n"/>
</Listener>
</Appenders>
<Loggers>
<Root level="info">
<filters>
<MarkerFilter marker="NETWORK_PACKETS" onMatch="DENY" onMismatch="NEUTRAL"/>
</filters>
<AppenderRef ref="SysOut"/>
<AppenderRef ref="File"/>
<AppenderRef ref="Tracy"/>
</Root>
</Loggers>
</Configuration>
        "#,
        path.to_string_lossy()
    )
}

#[derive(Snafu, Debug)]
pub enum JvmRuleError {
    #[snafu(display("Os {os} is not supported"))]
    OsNotSupported { os: String },
    #[snafu(display("Fails to detect os_version, error: {str}"))]
    OsDetection { str: String },
}

fn add_jvm_rules(
    library_list: &[Library],
    library_path: impl AsRef<Path>,
    client_jar: impl AsRef<Path>,
    mut jvm_options: JvmOptions,
    jvm_flags: RawJvmFlags,
) -> Result<(JvmOptions, JvmFlagsProcessed), JvmRuleError> {
    let current_os =
        os_version::detect().map_err(|x| JvmRuleError::OsDetection { str: x.to_string() })?;
    let current_os_type = match current_os {
        os_version::OsVersion::Linux(_) => Some(OsName::Linux),
        os_version::OsVersion::Windows(_) => Some(OsName::Windows),
        os_version::OsVersion::MacOS(_) => Some(OsName::Osx),
        _ => None,
    }
    .context(OsNotSupportedSnafu {
        os: current_os.to_string(),
    })?;

    let mut parsed_library_list = Vec::new();
    for library in library_list {
        let (process_native, is_native_library, _) = os_match(library, current_os_type);
        if !process_native && !is_native_library {
            parsed_library_list.push(library);
        }
    }

    let mut classpath_list = parsed_library_list
        .iter()
        .map(|x| {
            if let Some(ref path) = x.downloads.artifact.path {
                library_path.as_ref().join(path)
            } else {
                library_path.as_ref().to_path_buf()
            }
            .to_string_lossy()
            .to_string()
        })
        .collect::<Vec<String>>();

    classpath_list.push(client_jar.as_ref().to_string_lossy().to_string());
    jvm_options.classpath = classpath_list.join(":");
    let mut new_arguments = Vec::new();
    let current_architecture = std::env::consts::ARCH.to_owned();

    for p in jvm_flags.arguments {
        match p {
            Argument::General(p) => {
                new_arguments.push(p);
            }
            Argument::Ruled { rules, value } => {
                for x in rules {
                    if x.action == ActionType::Allow
                        && x.os.as_ref().map_or(false, |os| {
                            os.name == Some(current_os_type)
                                || os.arch.as_ref() == Some(&current_architecture)
                        })
                    {
                        match value {
                            manifest::ArgumentRuledValue::Single(ref x) => {
                                new_arguments.push(x.clone());
                            }
                            manifest::ArgumentRuledValue::Multiple(ref x) => {
                                new_arguments.extend_from_slice(x);
                            }
                        }
                    }
                }
            }
        }
    }
    let jvm_flags = JvmFlagsProcessed {
        arguments: new_arguments,
    };
    Ok((jvm_options, jvm_flags))
}

fn setup_jvm_options(
    client_jar: String,
    library_directory: impl AsRef<Path>,
    game_directory: impl AsRef<Path>,
    native_directory: impl AsRef<Path>,
) -> Result<JvmOptions, ManifestProcessingError> {
    Ok(JvmOptions {
        launcher_name: String::from("era-connect"),
        launcher_version: String::from("0.0.1"),
        classpath: String::new(),
        classpath_separator: String::from(":"),
        primary_jar: client_jar,
        library_directory: Arc::from(library_directory.as_ref().canonicalize().context(
            IoSnafu {
                path: "Fail to canonicalize library_directory",
            },
        )?),
        game_directory: Arc::from(game_directory.as_ref().canonicalize().context(IoSnafu {
            path: "Fail to canonicalize game_directory",
        })?),
        native_directory: Arc::from(native_directory.as_ref().canonicalize().context(IoSnafu {
            path: "Fail to canonicalize native_directory",
        })?),
    })
}

const fn setup_jvm_flags(jvm_argument: Vec<Argument>) -> RawJvmFlags {
    RawJvmFlags {
        arguments: jvm_argument,
    }
}

const fn setup_game_flags(game_arguments: Vec<Argument>) -> RawGameFlags {
    RawGameFlags {
        arguments: game_arguments,
    }
}

#[derive(Snafu, Debug)]
pub enum GameOptionError {
    #[snafu(display("Can't launch game without having main account set"))]
    NoMainAccount,
    #[snafu(display("Failed to canonicalization for {path:?}"))]
    Canonicalization {
        source: std::io::Error,
        path: PathBuf,
    },
}

async fn setup_game_option(
    current_version: String,
    game_directory: impl AsRef<Path> + Send + Sync,
    asset_directory: impl AsRef<Path> + Send + Sync,
    asset_index_id: String,
    game_flags: RawGameFlags,
) -> Result<(GameOptions, GameFlagsProcessed), GameOptionError> {
    let storage = &STORAGE.account_storage.read();
    let uuid = storage.main_account.context(NoMainAccountSnafu)?;
    let minecraft_account = storage
        .accounts
        .iter()
        .find(|x| x.uuid == uuid)
        .expect("main account not in accounts(should be)");
    let game_options = GameOptions {
        auth_player_name: minecraft_account.username.clone(),
        game_version_name: current_version,
        game_directory: Arc::from(game_directory.as_ref().canonicalize().context(
            CanonicalizationSnafu {
                path: game_directory.as_ref(),
            },
        )?),
        assets_root: Arc::from(asset_directory.as_ref().canonicalize().context(
            CanonicalizationSnafu {
                path: asset_directory.as_ref(),
            },
        )?),
        assets_index_name: asset_index_id,
        auth_access_token: minecraft_account.access_token.token.clone(),
        auth_uuid: uuid.to_string(),
        user_type: String::from("mojang"),
        version_type: String::from("release"),
    };

    // NOTE: IGNORE CHECK
    let mut game_flags = GameFlagsProcessed {
        arguments: game_flags
            .arguments
            .into_iter()
            .flat_map(|x| match x {
                Argument::General(x) => vec![x],
                _ => vec![String::new()],
                // Argument::Ruled { value, .. } => match value {
                //     manifest::ArgumentRuledValue::Single(x) => vec![x],
                //     manifest::ArgumentRuledValue::Multiple(x) => x,
                // },
            })
            .collect(),
    };

    game_flags.arguments = game_args_parse(&game_flags, &game_options);

    Ok((game_options, game_flags))
}

fn jvm_args_parse(jvm_flags: &[String], jvm_options: &JvmOptions) -> Vec<String> {
    let mut parsed_argument = Vec::new();

    for argument in jvm_flags {
        let mut s = argument.as_str();
        let mut buf = String::with_capacity(s.len());

        while let Some(pos) = s.find("${") {
            buf.push_str(&s[..pos]);
            let start = &s[pos + 2..];

            let Some(closing) = start.find('}') else {
                panic!("missing closing brace");
            };
            let var = &start[..closing];
            // make processing string to next part
            s = &start[closing + 1..];

            let natives_directory = jvm_options.native_directory.to_string_lossy().to_string();
            buf.push_str(match var {
                "natives_directory" => &natives_directory,
                "launcher_name" => &jvm_options.launcher_name,
                "launcher_version" => &jvm_options.launcher_version,
                "classpath" => &jvm_options.classpath,
                _ => "",
            });
        }
        buf.push_str(s);
        parsed_argument.push(buf);
    }
    parsed_argument
}

fn game_args_parse(game_flags: &GameFlagsProcessed, game_arguments: &GameOptions) -> Vec<String> {
    let mut modified_arguments = Vec::new();

    for argument in &game_flags.arguments {
        let mut s = argument.as_str();
        let mut buf = String::with_capacity(s.len());

        while let Some(pos) = s.find("${") {
            buf.push_str(&s[..pos]);
            let start = &s[pos + 2..];

            let Some(closing) = start.find('}') else {
                panic!("missing closing brace");
            };
            let var = &start[..closing];
            // make processing string to next part
            s = &start[closing + 1..];

            let game_directory = game_arguments.game_directory.to_string_lossy().to_string();
            let assets_root = game_arguments.assets_root.to_string_lossy().to_string();

            buf.push_str(match var {
                "auth_player_name" => &game_arguments.auth_player_name,
                "version_name" => &game_arguments.game_version_name,
                "game_directory" => &game_directory,
                "assets_root" => &assets_root,
                "assets_index_name" => &game_arguments.assets_index_name,
                "auth_uuid" => &game_arguments.auth_uuid,
                "auth_access_token" => &game_arguments.auth_access_token,
                "version_type" => &game_arguments.version_type,
                _ => "",
            });
        }
        buf.push_str(s);
        modified_arguments.push(buf);
    }
    modified_arguments
}
