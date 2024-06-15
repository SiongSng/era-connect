use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncBufReadExt;

use crate::api::backend_exclusive::{
    download::{execute_and_progress, DownloadArgs, DownloadBias, DownloadType, HandlesType},
    modding::library::prepare_modloader_download,
    vanilla::{
        launcher::{prepare_vanilla_download, JvmOptions, LaunchArgs},
        manifest::fetch_game_manifest,
    },
};
use crate::api::shared_resources::collection::Collection;

use super::library::ModloaderLibrary;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Processors {
    jar: String,
    classpath: Vec<String>,
    args: Vec<String>,
    sides: Option<Vec<String>>,
    outputs: Option<HashMap<String, String>>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct ProcessorData {
    merged_mappings: ProcessorsDataType,
    /// added at 1.20.4
    merged_mappings_sha: Option<ProcessorsDataType>,
    mappings: ProcessorsDataType,
    /// removed from 1.20.4
    mc_extra: Option<ProcessorsDataType>,
    /// removed from 1.20.4
    mc_extra_sha: Option<ProcessorsDataType>,
    /// removed from 1.20.4
    mc_slim: Option<ProcessorsDataType>,
    /// removed from 1.20.4
    mc_slim_sha: Option<ProcessorsDataType>,
    mc_unpacked: ProcessorsDataType,
    mc_srg: ProcessorsDataType,
    /// removed from 1.20.4
    mcp_version: Option<ProcessorsDataType>,
    mojmaps: ProcessorsDataType,
    patched: ProcessorsDataType,
    /// removed from 1.20.4
    patched_sha: Option<ProcessorsDataType>,
    binpatch: ProcessorsDataType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ProcessorsDataType {
    client: String,
    server: String,
}

impl ProcessorsDataType {
    fn convert_maven_to_path(&mut self, folder: &str) -> Result<()> {
        self.client = convert_maven_to_path(&self.client, Some(folder))?;
        self.server = convert_maven_to_path(&self.server, Some(folder))?;
        Ok(())
    }
}

pub fn processers_process<'a>(
    mut launch_args: LaunchArgs,
    jvm_options: JvmOptions,
    manifest: Value,
) -> Result<(LaunchArgs, DownloadArgs<'a>)> {
    let libraries_folder = jvm_options.library_directory.to_string_lossy().to_string();
    let library_manifest = Vec::<ModloaderLibrary>::deserialize(
        manifest
            .get("libraries")
            .context("mod loader manifest library doesn't exist")?,
    )?;
    let processor_data = manifest
        .get("data")
        .map(|x| ProcessorData::deserialize(x))
        .map_or(Ok(None), |v| v.map(Some))?;
    let processors = manifest
        .get("processors")
        .map(|x| Vec::<Processors>::deserialize(x))
        .map_or(Ok(None), |v| v.map(Some))?;

    let forge_installer = library_manifest
        .iter()
        .filter(|x| x.downloads.is_none())
        .find(|x| x.name.contains("client") && x.name.contains("minecraftforge"))
        .map(|x| x.name.clone());

    let index = Arc::new(AtomicUsize::new(0));
    let index_cloned = Arc::clone(&index);
    let mut handles: HandlesType = Vec::new();
    let max = Arc::new(AtomicUsize::new(
        processors.as_ref().map_or(1, |x| x.len()) - 1,
    ));

    if let (Some(mut processor_data), Some(processors), Some(forge_installer)) =
        (processor_data, processors, forge_installer)
    {
        let minecraft_jar = jvm_options.primary_jar.clone();
        let game_directory = jvm_options.game_directory.to_string_lossy().to_string();
        processor_data
            .merged_mappings
            .convert_maven_to_path(&libraries_folder)?;
        processor_data
            .mappings
            .convert_maven_to_path(&libraries_folder)?;
        processor_data
            .mc_extra
            .as_mut()
            .map(|x| x.convert_maven_to_path(&libraries_folder));
        processor_data
            .mc_slim
            .as_mut()
            .map(|x| x.convert_maven_to_path(&libraries_folder));
        processor_data
            .mc_srg
            .convert_maven_to_path(&libraries_folder)?;
        processor_data
            .mc_unpacked
            .convert_maven_to_path(&libraries_folder)?;
        processor_data
            .mojmaps
            .convert_maven_to_path(&libraries_folder)?;
        processor_data
            .patched
            .convert_maven_to_path(&libraries_folder)?;
        processor_data
            .binpatch
            .convert_maven_to_path(&libraries_folder)?;

        handles.push(Box::pin(async move {
            for (i, processor) in processors.into_iter().enumerate() {
                index_cloned.store(i, Ordering::Relaxed);
                let processor_jar = convert_maven_to_path(&processor.jar, Some(&libraries_folder))?;
                let processor_main_class = get_processor_main_class(processor_jar.clone())
                    .await?
                    .context("Processor main class doesn't exist")?;
                let mut processor_classpath = processor
                    .classpath
                    .iter()
                    .map(|x| {
                        convert_maven_to_path(x, Some(&libraries_folder))
                            .context("failed to convert processor classpath maven to path")
                    })
                    .collect::<Result<Vec<_>>>()?;

                processor_classpath.push(processor_jar);

                if let Some(sides) = processor.sides {
                    if sides.contains(&String::from("server")) {
                        continue;
                    }
                }

                let args = process_client(
                    &processor.args,
                    &processor_data,
                    &minecraft_jar,
                    &game_directory,
                    &convert_maven_to_path(&forge_installer, Some(&libraries_folder))?,
                )
                .into_iter()
                .map(|x| {
                    if x.starts_with('[') {
                        convert_maven_to_path(&x, Some(&libraries_folder))
                            .context("failed to convert [] types maven into path")
                    } else {
                        Ok(x)
                    }
                })
                .collect::<Result<Vec<_>>>()?;

                let mut all = Vec::new();
                all.push(String::from("-cp"));
                all.push(processor_classpath.join(":"));
                all.push(processor_main_class);
                all.extend(args);

                let process = tokio::process::Command::new("java")
                    .args(all)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?
                    .stdout
                    .context("stdout doesn't exist")?;
                let read = tokio::io::BufReader::new(process);
                let mut a = read.lines();
                while let Some(x) = a.next_line().await? {
                    debug!("{x}");
                }
            }
            Ok(())
        }));
    }

    let arguments = manifest
        .get("arguments")
        .context("modloader arguments doesn't exist")?;
    let jvm = arguments
        .get("jvm")
        .map(|x| Vec::<String>::deserialize(x))
        .unwrap_or(Ok(vec![]))?;
    let game = arguments
        .get("game")
        .map(|x| Vec::<String>::deserialize(x))
        .unwrap_or(Ok(vec![]))?;
    let jvm = jvm_args_parse(&jvm, &jvm_options);
    launch_args.main_class = manifest
        .get("mainClass")
        .context("failed to get mainClass of Forge")?
        .as_str()
        .context("forge mainClass doesn't existing")?
        .to_owned();
    launch_args.jvm_args.extend(jvm);
    launch_args.game_args.extend(game);
    let download_args = DownloadArgs {
        current: index,
        total: max,
        handles,
        is_size: false,
    };
    Ok((launch_args, download_args))
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

            let library = jvm_options.library_directory.to_string_lossy().to_string();

            let primary_jar_path = PathBuf::from(&jvm_options.primary_jar);
            let primary_jar = primary_jar_path
                .file_stem()
                .expect("can't find file_stem of primary jar")
                .to_string_lossy()
                .to_string();

            buf.push_str(match var {
                "library_directory" => &library,
                "classpath_separator" => &jvm_options.classpath_separator,
                "version_name" => &primary_jar,
                x => {
                    warn!("{x} is not processed, please notify the developer");
                    x
                }
            });
        }
        buf.push_str(s);
        parsed_argument.push(buf);
    }
    parsed_argument
}
fn process_client(
    jvm_flags: &[String],
    processors_data: &ProcessorData,
    minecraft_jar: &str,
    root: &str,
    installer: &str,
) -> Vec<String> {
    let mut parsed_argument = Vec::new();

    for argument in jvm_flags {
        let mut s = argument.as_str();
        let mut buf = String::with_capacity(s.len());

        while let Some(pos) = s.find('{') {
            buf.push_str(&s[..pos]);
            let start = &s[pos + 1..];

            let Some(closing) = start.find('}') else {
                panic!("missing closing brace");
            };
            let var = &start[..closing];
            // make processing string to next part
            s = &start[closing + 1..];

            buf.push_str(match var {
                "MERGED_MAPPINGS" => processors_data.merged_mappings.client.as_str(),
                "MAPPINGS" => processors_data.mappings.client.as_str(),
                "MC_EXTRA" => processors_data.mc_extra.as_ref().unwrap().client.as_str(),
                "MC_EXTRA_SHA" => processors_data
                    .mc_extra_sha
                    .as_ref()
                    .unwrap()
                    .client
                    .as_str(),
                "MC_SLIM" => processors_data.mc_slim.as_ref().unwrap().client.as_str(),
                "MC_SLIM_SHA" => processors_data
                    .mc_slim_sha
                    .as_ref()
                    .unwrap()
                    .client
                    .as_str(),
                "MC_SRG" => processors_data.mc_srg.client.as_str(),
                "MC_UNPACKED" => processors_data.mc_unpacked.client.as_str(),
                "MOJMAPS" => processors_data.mojmaps.client.as_str(),
                "PATCHED" => processors_data.patched.client.as_str(),
                "PATCHED_SHA" => processors_data
                    .patched_sha
                    .as_ref()
                    .unwrap()
                    .client
                    .as_str(),
                "BINPATCH" => processors_data.binpatch.client.as_str(),
                "MCP_VERSION" => processors_data
                    .mcp_version
                    .as_ref()
                    .unwrap()
                    .client
                    .as_str(),
                "MINECRAFT_JAR" => minecraft_jar,
                "ROOT" => root,
                "INSTALLER" => installer,
                "SIDE" => "client",
                _ => "",
            });
        }
        buf.push_str(s);
        parsed_argument.push(buf);
    }
    parsed_argument
}

fn pre_convert_maven_to_path(input: &str, extension: Option<&str>) -> Result<String> {
    let parts: Vec<&str> = input.split(':').collect();
    let org = parts
        .first()
        .context("failed to got org from maven")?
        .replace('.', "/");
    let package = parts[1];
    let path_version = parts[2];
    let mut version = parts[2].to_owned();
    if let Some(x) = parts.get(3) {
        version.push_str(format!("-{x}").as_str());
    }
    if let Some(x) = parts.get(4) {
        version.push_str(format!("-{x}").as_str());
    }
    let extension = extension.unwrap_or("jar");
    let file_name = format!("{package}-{version}.{extension}");
    let path = format!("{org}/{package}/{path_version}");

    Ok(format!("{path}/{file_name}"))
}

pub fn convert_maven_to_path(str: &str, folder: Option<&str>) -> Result<String> {
    let pos = str.find(|x| x == '@');
    let mut pre_maven = pos.map_or_else(
        || pre_convert_maven_to_path(str, None),
        |position| pre_convert_maven_to_path(&str[..position], Some(&str[position + 1..])),
    )?;
    pre_maven.retain(|x| x != '[' && x != ']');
    if let Some(folder) = folder {
        pre_maven.insert_str(0, format!("{folder}/").as_str());
    }
    Ok(pre_maven)
}

pub async fn get_processor_main_class(path: String) -> Result<Option<String>> {
    let zipfile = tokio::fs::File::open(&path)
        .await
        .context(path)?
        .into_std()
        .await;
    let mut archive = zip::ZipArchive::new(zipfile)?;

    let file = archive.by_name("META-INF/MANIFEST.MF")?;

    let reader = BufReader::new(file);

    for line in reader.lines() {
        let mut line = line?;
        line.retain(|c| !c.is_whitespace());

        if line.starts_with("Main-Class:") {
            if let Some(class) = line.split(':').nth(1) {
                return Ok(Some(class.to_owned()));
            }
        }
    }
    Ok(None)
}
pub async fn fetch_launch_args_modded(collection: &Collection) -> anyhow::Result<LaunchArgs> {
    let collection_id = collection.get_collection_id();
    info!("Starts Vanilla Downloading");
    let vanilla_bias = DownloadBias {
        start: 0.0,
        end: 80.0,
    };
    let game_manifest = fetch_game_manifest(&collection.minecraft_version.url).await?;
    let (vanilla_download_args, vanilla_arguments) =
        prepare_vanilla_download(collection, game_manifest.clone()).await?;
    execute_and_progress(
        collection_id.clone(),
        vanilla_download_args,
        vanilla_bias,
        DownloadType::vanilla(),
    )
    .await?;

    info!("Starts Modloader Downloading");
    let modloader_download_bias = DownloadBias {
        start: 80.0,
        end: 90.0,
    };
    let (modloader_download_args, modloader_arguments, manifest) = prepare_modloader_download(
        &collection.mod_loader().unwrap().mod_loader_type,
        vanilla_arguments.launch_args,
        vanilla_arguments.jvm_args,
        vanilla_arguments.game_args,
    )
    .await?;
    execute_and_progress(
        collection_id.clone(),
        modloader_download_args,
        modloader_download_bias,
        DownloadType::mod_loader_assets(),
    )
    .await?;

    info!("Starts Modloader Processing");
    let forge_processor_bias = DownloadBias {
        start: 90.0,
        end: 100.0,
    };

    let (processed_arguments, forge_processor_progress) = processers_process(
        modloader_arguments.launch_args,
        modloader_arguments.jvm_args,
        manifest,
    )?;
    execute_and_progress(
        collection_id,
        forge_processor_progress,
        forge_processor_bias,
        DownloadType::mod_loader_proccess(),
    )
    .await?;
    Ok(processed_arguments)
}
