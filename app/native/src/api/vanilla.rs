pub mod assets;
pub mod library;
pub mod manifest;
pub mod rules;
pub mod version;

use chrono::{DateTime, Utc};
use color_eyre::eyre::{bail, eyre, Context, ContextCompat};
use color_eyre::Result;
use futures::future::BoxFuture;
use log::error;
use serde::{Deserialize, Serialize};
use std::path::Path;
pub use std::path::PathBuf;
use std::sync::{atomic::AtomicUsize, atomic::Ordering, Arc};
use tokio::fs::{self, create_dir_all};

use crate::api::collection::Collection;

use self::assets::{extract_assets, parallel_assets};
use self::library::{os_match, parallel_library, Library};
use self::manifest::{Argument, Downloads, GameManifest};
use self::rules::OsName;
use super::download::{
    download_file, extract_filename, validate_sha1, DownloadArgs, DownloadError,
};
use super::{DATA_DIR, STORAGE};

#[derive(Debug, PartialEq, Deserialize)]
struct GameFlags {
    arguments: Vec<String>,
    additional_arguments: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct JvmFlags {
    arguments: Vec<String>,
    additional_arguments: Option<Vec<String>>,
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

#[derive(Clone, Debug)]
pub struct LaunchArgs {
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub game_args: Vec<String>,
}

use thiserror::Error;
#[derive(Error, Debug)]
pub enum VanillaLaunchError {
    #[error("Token expired")]
    TokenExpire(DateTime<Utc>),
    #[error("Io error")]
    Io(CustomIoErrorKind),
    #[error("Downloading error, {0}")]
    DownloadError(#[from] DownloadError),
    #[error("some weird error you know")]
    Other(String),
}
// we don't use the mirror feature because it will force us to use unstable rust
#[derive(Debug, Clone)]
pub enum CustomIoErrorKind {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidInput,
    TimedOut,
    Other,
}

impl From<color_eyre::Report> for VanillaLaunchError {
    fn from(value: color_eyre::Report) -> Self {
        Self::Other(format!("{value:?}"))
    }
}

impl From<std::io::Error> for VanillaLaunchError {
    fn from(value: std::io::Error) -> Self {
        let error = match value.kind() {
            std::io::ErrorKind::NotFound => CustomIoErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied => CustomIoErrorKind::PermissionDenied,
            std::io::ErrorKind::AlreadyExists => CustomIoErrorKind::AlreadyExists,
            std::io::ErrorKind::InvalidInput => CustomIoErrorKind::InvalidInput,
            std::io::ErrorKind::TimedOut => CustomIoErrorKind::TimedOut,
            _ => CustomIoErrorKind::Other,
        };
        Self::Io(error)
    }
}

pub async fn launch_game(launch_args: LaunchArgs) -> Result<()> {
    let mut launch_vec = Vec::new();
    launch_vec.extend(launch_args.jvm_args);
    launch_vec.push(launch_args.main_class);
    launch_vec.extend(launch_args.game_args);
    let b = tokio::process::Command::new("java")
        .args(launch_vec)
        .spawn();
    b?.wait().await?;
    Ok(())
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

pub type HandlesType<'a> = Vec<BoxFuture<'a, Result<()>>>;

pub async fn prepare_vanilla_download<'a>(
    collection: Collection,
    game_manifest: GameManifest,
) -> Result<(DownloadArgs<'a>, ProcessedArguments), VanillaLaunchError> {
    let version_id = collection.minecraft_version.id;
    let entry_path = &collection.entry_path;
    let shared_path = get_global_shared_path();

    let game_directory = entry_path.join("game");
    let asset_directory = shared_path.join("assets");
    let library_directory = shared_path.join("libraries");
    let version_directory = shared_path.join(format!("versions/{version_id}"));
    let native_directory = version_directory.join("natives");

    create_dir_all(&library_directory).await?;
    create_dir_all(&asset_directory).await?;
    create_dir_all(&game_directory).await?;
    create_dir_all(&native_directory).await?;

    let game_flags = setup_game_flags(&game_manifest.arguments.game)?;
    let jvm_flags = setup_jvm_flags(&game_manifest.arguments.jvm)?;

    let (game_options, game_flags) = setup_game_option(
        version_id,
        &game_directory,
        &asset_directory,
        game_manifest.asset_index.id.clone(),
        game_flags,
    )
    .await?;

    let downloads_list = game_manifest.downloads;
    let library_list: Arc<[Library]> = game_manifest.libraries.into();

    let (client_jar, jvm_options) = setup_jvm_options(
        entry_path,
        &downloads_list,
        &library_directory,
        &game_directory,
        &native_directory,
    )?;

    let (jvm_options, mut jvm_flags) = add_jvm_rules(
        Arc::clone(&library_list),
        &library_directory,
        &client_jar,
        jvm_options,
        jvm_flags,
    )?;

    jvm_flags.arguments = jvm_args_parse(&jvm_flags.arguments, &jvm_options);

    let mut handles = Vec::new();

    let current_size = Arc::new(AtomicUsize::new(0));

    let (library_path, native_library_path) = (
        Arc::clone(&jvm_options.library_directory),
        Arc::clone(&jvm_options.native_directory),
    );

    let total_size = parallel_library(
        Arc::clone(&library_list),
        library_path,
        native_library_path,
        Arc::clone(&current_size),
        &mut handles,
    )
    .await?;

    let client_jar_filename = version_directory.join(extract_filename(&downloads_list.client.url)?);

    if !client_jar_filename.exists() {
        let bytes =
            download_file(&downloads_list.client.url, Some(Arc::clone(&current_size))).await?;
        fs::write(client_jar_filename, &bytes).await?;
        total_size.fetch_add(downloads_list.client.size, Ordering::Relaxed);
    } else if let Err(x) = validate_sha1(&client_jar_filename, &downloads_list.client.sha1).await {
        error!("{x}\n redownloading.");
        let bytes =
            download_file(&downloads_list.client.url, Some(Arc::clone(&current_size))).await?;
        fs::write(client_jar_filename, &bytes).await?;
        total_size.fetch_add(downloads_list.client.size, Ordering::Relaxed);
    }

    let asset_settings = extract_assets(&game_manifest.asset_index, asset_directory).await?;
    parallel_assets(asset_settings, &current_size, &total_size, &mut handles).await?;

    if let Some(advanced_option) = collection.advanced_options {
        if let Some(max_memory) = advanced_option.jvm_max_memory {
            jvm_flags.arguments.push(format!("-Xmx{}M", max_memory))
        }
    }

    let launch_args = LaunchArgs {
        jvm_args: jvm_flags.arguments,
        main_class: game_manifest.main_class,
        game_args: game_flags.arguments,
    };

    Ok((
        DownloadArgs {
            current_size: Arc::clone(&current_size),
            total_size: Arc::clone(&total_size),
            handles,
        },
        ProcessedArguments {
            launch_args,
            jvm_args: jvm_options,
            game_args: game_options,
        },
    ))
}

fn add_jvm_rules(
    library_list: Arc<[Library]>,
    library_path: impl AsRef<Path>,
    client_jar: impl AsRef<Path>,
    mut jvm_options: JvmOptions,
    jvm_flags: JvmFlags,
) -> Result<(JvmOptions, JvmFlags)> {
    let current_os = os_version::detect().map_err(|x| eyre!(x))?;
    let current_os_type = match current_os {
        os_version::OsVersion::Linux(_) => OsName::Linux,
        os_version::OsVersion::Windows(_) => OsName::Windows,
        os_version::OsVersion::MacOS(_) => OsName::Osx,
        _ => bail!("not supported"),
    };

    let mut parsed_library_list = Vec::new();
    for library in library_list.iter() {
        let (process_native, is_native_library, _) = os_match(library, &current_os_type);
        if !process_native && !is_native_library {
            parsed_library_list.push(library);
        }
    }

    let mut classpath_list = parsed_library_list
        .iter()
        .map(|x| {
            library_path
                .as_ref()
                .join(&x.downloads.artifact.path)
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<String>>();

    classpath_list.push(client_jar.as_ref().to_string_lossy().to_string());
    jvm_options.classpath = classpath_list.join(":");
    // for x in &jvm_flags.rules {
    //     if x.action == ActionType::Allow
    //         && x.os
    //             .as_ref()
    //             .map_or(false, |os| os.name == Some(current_os_type))
    //     {
    //         jvm_flags
    //             .arguments
    //             .extend_from_slice(x.value.as_ref().context("rules value doesn't exist")?);
    //     }
    // }
    Ok((jvm_options, jvm_flags))
}

fn setup_jvm_options(
    entry_path: impl AsRef<Path>,
    downloads_list: &Downloads,
    library_directory: impl AsRef<Path>,
    game_directory: impl AsRef<Path>,
    native_directory: impl AsRef<Path>,
) -> Result<(PathBuf, JvmOptions)> {
    let client_jar = entry_path
        .as_ref()
        .join(extract_filename(&downloads_list.client.url)?);
    let jvm_options = JvmOptions {
        launcher_name: String::from("era-connect"),
        launcher_version: String::from("0.0.1"),
        classpath: String::new(),
        classpath_separator: String::from(":"),
        primary_jar: client_jar.to_string_lossy().to_string(),
        library_directory: Arc::from(
            library_directory
                .as_ref()
                .canonicalize()
                .context("Fail to canonicalize library_directory(jvm_options)")?,
        ),
        game_directory: Arc::from(
            game_directory
                .as_ref()
                .canonicalize()
                .context("Fail to canonicalize game_directory(jvm_options)")?,
        ),
        native_directory: Arc::from(
            native_directory
                .as_ref()
                .canonicalize()
                .context("Fail to canonicalize native_directory(jvm_options)")?,
        ),
    };
    Ok((client_jar, jvm_options))
}

fn setup_jvm_flags(jvm_argument: &Vec<Argument>) -> Result<JvmFlags> {
    let jvm_flags = JvmFlags {
        // rules: get_rules(jvm_argument)?,
        // arguments: jvm_argument
        //     .iter()
        //     .filter_map(Value::as_str)
        //     .map(ToString::to_string)
        //     .collect::<Vec<_>>(),
        arguments: vec![],
        additional_arguments: None,
    };
    Ok(jvm_flags)
}

fn setup_game_flags(game_arguments: &Vec<Argument>) -> Result<GameFlags> {
    let game_flags = GameFlags {
        // rules: get_rules(game_argument)?,
        // arguments: game_argument
        //     .iter()
        //     .filter_map(Value::as_str)
        //     .map(ToString::to_string)
        //     .collect::<Vec<_>>(),
        arguments: vec![],
        additional_arguments: None,
    };
    Ok(game_flags)
}

async fn setup_game_option(
    current_version: String,
    game_directory: impl AsRef<Path> + Send + Sync,
    asset_directory: impl AsRef<Path> + Send + Sync,
    asset_index_id: String,
    mut game_flags: GameFlags,
) -> Result<(GameOptions, GameFlags), VanillaLaunchError> {
    let storage = STORAGE.account_storage.read().await;
    let uuid = storage
        .main_account
        .context("Can't launch game without main account")?;
    let minecraft_account = storage
        .accounts
        .iter()
        .find(|x| x.uuid == uuid)
        .context("Somehow fail to find the main account uuid in accounts")?;

    let expires_at = minecraft_account.access_token.expires_at;
    if Utc::now() > expires_at {
        return Err(VanillaLaunchError::TokenExpire(expires_at));
    }

    let game_options = GameOptions {
        auth_player_name: minecraft_account.username.clone(),
        game_version_name: current_version,
        game_directory: Arc::from(
            game_directory
                .as_ref()
                .canonicalize()
                .context("Fail to canonicalize game_directory(game_options)")?,
        ),
        assets_root: Arc::from(
            asset_directory
                .as_ref()
                .canonicalize()
                .context("Fail to canonicalize asset_directory(game_options)")?,
        ),
        assets_index_name: asset_index_id,
        auth_access_token: minecraft_account.access_token.token.clone(),
        auth_uuid: uuid.to_string(),
        user_type: String::from("mojang"),
        version_type: String::from("release"),
    };
    game_flags.arguments = game_args_parse(&game_flags, &game_options);

    if let Some(x) = game_flags.additional_arguments.as_ref() {
        game_flags.arguments.extend_from_slice(x);
    }
    Ok((game_options, game_flags))
}

fn jvm_args_parse(jvm_flags: &[String], jvm_options: &JvmOptions) -> Vec<String> {
    let mut parsed_argument = Vec::new();

    for argument in jvm_flags.iter() {
        let mut s = argument.as_str();
        let mut buf = String::with_capacity(s.len());

        while let Some(pos) = s.find("${") {
            buf.push_str(&s[..pos]);
            let start = &s[pos + 2..];

            let Some(closing) = start.find('}') else { panic!("missing closing brace"); };
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

fn game_args_parse(game_flags: &GameFlags, game_arguments: &GameOptions) -> Vec<String> {
    let mut modified_arguments = Vec::new();

    for argument in &game_flags.arguments {
        let mut s = argument.as_str();
        let mut buf = String::with_capacity(s.len());

        while let Some(pos) = s.find("${") {
            buf.push_str(&s[..pos]);
            let start = &s[pos + 2..];

            let Some(closing) = start.find('}') else { panic!("missing closing brace"); };
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

fn get_global_shared_path() -> PathBuf {
    DATA_DIR.join("shared")
}
