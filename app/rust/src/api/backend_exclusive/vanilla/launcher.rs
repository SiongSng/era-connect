use anyhow::bail;
use anyhow::{Context, Result};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{atomic::AtomicUsize, atomic::Ordering, Arc};
use tokio::fs::{self, create_dir_all};

use super::assets::{extract_assets, parallel_assets_download};
use super::library::{os_match, parallel_library, Library};
use super::manifest::{self, Argument, GameManifest};
use super::rules::{ActionType, OsName};
use crate::api::backend_exclusive::download::{execute_and_progress, DownloadBias};
use crate::api::backend_exclusive::vanilla::manifest::fetch_game_manifest;
use crate::api::backend_exclusive::{
    download::{download_file, extract_filename, validate_sha1, DownloadArgs},
    storage::storage_loader::get_global_shared_path,
};
use crate::api::shared_resources::collection::Collection;
use crate::api::shared_resources::entry::STORAGE;

#[derive(Debug, PartialEq, Deserialize)]
struct GameFlagsProcessed {
    arguments: Vec<String>,
    additional_arguments: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct JvmFlagsProcessed {
    arguments: Vec<String>,
    additional_arguments: Option<Vec<String>>,
}
#[derive(Debug, PartialEq, Deserialize)]
struct GameFlagsUnprocessed {
    arguments: Vec<Argument>,
    additional_arguments: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct JvmFlagsUnprocessed {
    arguments: Vec<Argument>,
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct LaunchArgs {
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub game_args: Vec<String>,
}

pub async fn launch_game(launch_args: &LaunchArgs) -> Result<()> {
    let mut launch_vec = Vec::new();
    launch_vec.extend(&launch_args.jvm_args);
    launch_vec.push(&launch_args.main_class);
    launch_vec.extend(&launch_args.game_args);
    let b = tokio::process::Command::new("java")
        .args(launch_vec)
        .spawn();
    b?.wait().await?;
    Ok(())
}
pub async fn full_vanilla_download(collection: &Collection) -> anyhow::Result<LaunchArgs> {
    info!("Starts Vanilla Downloading");
    let collection_id = collection.get_collection_id();
    let vanilla_bias = DownloadBias {
        start: 0.0,
        end: 100.0,
    };
    let game_manifest = fetch_game_manifest(&collection.minecraft_version.url).await?;
    let (vanilla_download_args, vanilla_arguments) =
        prepare_vanilla_download(collection, game_manifest.clone()).await?;
    execute_and_progress(
        collection_id,
        vanilla_download_args,
        vanilla_bias,
        String::from("Download Vanilla"),
    )
    .await?;

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

pub async fn prepare_vanilla_download<'a>(
    collection: &Collection,
    game_manifest: GameManifest,
) -> Result<(DownloadArgs<'a>, ProcessedArguments)> {
    let version_id = collection.minecraft_version.id.clone();
    let shared_path = get_global_shared_path();

    let game_directory = collection.game_directory();
    let asset_directory = shared_path.join("assets");
    let library_directory = shared_path.join("libraries");
    let version_directory = shared_path.join(format!("versions/{version_id}"));
    let native_directory = version_directory.join("natives");

    create_dir_all(&library_directory)
        .await
        .context("fail to create library directory (vanilla)")?;
    create_dir_all(&asset_directory)
        .await
        .context("fail to create asset directory (vanilla)")?;
    create_dir_all(&game_directory)
        .await
        .context("fail to create game directory (vanilla)")?;
    create_dir_all(&native_directory)
        .await
        .context("fail to create native directory (vanilla)")?;

    let game_flags = setup_game_flags(game_manifest.arguments.game);
    let jvm_flags = setup_jvm_flags(game_manifest.arguments.jvm);

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

    let client_jar_filename = version_directory.join(extract_filename(&downloads_list.client.url)?);

    let jvm_options = setup_jvm_options(
        client_jar_filename.to_string_lossy().to_string(),
        &library_directory,
        &game_directory,
        &native_directory,
    )?;

    let (jvm_options, mut jvm_flags) = add_jvm_rules(
        Arc::clone(&library_list),
        &library_directory,
        &client_jar_filename,
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

    if !client_jar_filename.exists() {
        total_size.fetch_add(downloads_list.client.size, Ordering::Relaxed);
        let bytes = download_file(&downloads_list.client.url, Arc::clone(&current_size)).await?;
        fs::write(client_jar_filename, &bytes).await?;
    } else if let Err(x) = validate_sha1(&client_jar_filename, &downloads_list.client.sha1).await {
        total_size.fetch_add(downloads_list.client.size, Ordering::Relaxed);
        error!("{x}\n redownloading.");
        let bytes = download_file(&downloads_list.client.url, Arc::clone(&current_size)).await?;
        fs::write(client_jar_filename, &bytes).await?;
    }

    let asset_settings = extract_assets(&game_manifest.asset_index, asset_directory).await?;
    parallel_assets_download(asset_settings, &current_size, &total_size, &mut handles).await?;

    if let Some(advanced_option) = &collection.advanced_options {
        if let Some(max_memory) = advanced_option.jvm_max_memory {
            jvm_flags.arguments.push(format!("-Xmx{max_memory}M"))
        }
    }

    let launch_args = LaunchArgs {
        jvm_args: jvm_flags.arguments,
        main_class: game_manifest.main_class,
        game_args: game_flags.arguments,
    };

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

fn add_jvm_rules(
    library_list: Arc<[Library]>,
    library_path: impl AsRef<Path>,
    client_jar: impl AsRef<Path>,
    mut jvm_options: JvmOptions,
    jvm_flags: JvmFlagsUnprocessed,
) -> Result<(JvmOptions, JvmFlagsProcessed), anyhow::Error> {
    let current_os = os_version::detect()?;
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
    let current_architecture = std::env::consts::ARCH.to_string();

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
                                new_arguments.push(x.clone())
                            }
                            manifest::ArgumentRuledValue::Multiple(ref x) => {
                                new_arguments.extend_from_slice(&x)
                            }
                        }
                    }
                }
            }
        }
    }
    let jvm_flags = JvmFlagsProcessed {
        arguments: new_arguments,
        additional_arguments: jvm_flags.additional_arguments,
    };
    Ok((jvm_options, jvm_flags))
}

fn setup_jvm_options(
    client_jar: String,
    library_directory: impl AsRef<Path>,
    game_directory: impl AsRef<Path>,
    native_directory: impl AsRef<Path>,
) -> Result<JvmOptions, anyhow::Error> {
    Ok(JvmOptions {
        launcher_name: String::from("era-connect"),
        launcher_version: String::from("0.0.1"),
        classpath: String::new(),
        classpath_separator: String::from(":"),
        primary_jar: client_jar,
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
    })
}

const fn setup_jvm_flags(jvm_argument: Vec<Argument>) -> JvmFlagsUnprocessed {
    JvmFlagsUnprocessed {
        arguments: jvm_argument,
        additional_arguments: None,
    }
}

const fn setup_game_flags(game_arguments: Vec<Argument>) -> GameFlagsUnprocessed {
    GameFlagsUnprocessed {
        arguments: game_arguments,
        additional_arguments: None,
    }
}

async fn setup_game_option(
    current_version: String,
    game_directory: impl AsRef<Path> + Send + Sync,
    asset_directory: impl AsRef<Path> + Send + Sync,
    asset_index_id: String,
    game_flags: GameFlagsUnprocessed,
) -> Result<(GameOptions, GameFlagsProcessed), anyhow::Error> {
    let storage = &STORAGE.account_storage.read().await;
    let uuid = storage
        .main_account
        .context("Can't launch game without main account")?;
    let minecraft_account = storage
        .accounts
        .iter()
        .find(|x| x.uuid == uuid)
        .context("Somehow fail to find the main account uuid in accounts")?;
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
        additional_arguments: game_flags.additional_arguments,
    };

    game_flags.arguments = game_args_parse(&game_flags, &game_options);

    if let Some(x) = game_flags.additional_arguments.as_ref() {
        game_flags.arguments.extend_from_slice(x);
    }
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
