pub mod download;
pub mod mod_management;
pub mod modding;
pub mod storage;
pub mod vanilla;

pub mod errors {
    use std::path::PathBuf;

    use snafu::prelude::*;

    use crate::api::{
        backend_exclusive::download::DownloadError, shared_resources::collection::ModLoaderType,
    };

    use super::{
        download::FileNameError,
        vanilla::{
            launcher::{GameOptionError, JvmRuleError},
            library::LibraryError,
        },
    };

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub))]
    pub enum ManifestProcessingError {
        #[snafu(transparent)]
        Download { source: DownloadError },
        #[snafu(display("Failed to deserializae"))]
        Desearialization { source: serde_json::Error },
        #[snafu(display("IO error at: {path:?}"))]
        Io {
            source: std::io::Error,
            path: PathBuf,
        },

        #[snafu(transparent)]
        LibraryProcessing { source: LibraryError },

        #[snafu(display("Failed to lookup {key}"))]
        ManifestLookUp { key: String },
        #[snafu(display("Jvm rules parsing issues"))]
        JvmRules { source: JvmRuleError },
        #[snafu(display("Failed to setup game options"))]
        GameOptions { source: GameOptionError },
        #[snafu(display("Failed to convert maven to path {str}"))]
        MavenPathTranslation { str: String },

        #[snafu(display("Cannot create class path"))]
        NoClassPath,
        #[snafu(display("Can not find {modloader_type} manifest"))]
        NoManifest { modloader_type: ModLoaderType },
        #[snafu(display("Could not locate library url"))]
        LibraryUrlNotExist,
        #[snafu(display("The desired game version does not exist: \n desired: {desired}"))]
        GameVersionNotExist { desired: String },
        #[snafu(display("MainClass does not exist"))]
        MainClass,
        #[snafu(display("{path:?} does not exist"))]
        MissingDir { path: PathBuf },
        #[snafu(display("parent of asset path does not exist"))]
        AssetPathParent,

        #[snafu(display("Could not read tokio spawn task"))]
        TokioSpawnTask { source: std::io::Error },
        #[snafu(display("Failed to extract the filename of {str}"))]
        ExtractDownloadingFilename { source: FileNameError, str: String },
        #[snafu(transparent)]
        Zip { source: zip::result::ZipError },
    }
}
