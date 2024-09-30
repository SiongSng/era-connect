use std::{io, path::PathBuf};

use futures::{StreamExt, TryStreamExt};

use super::{ModMetadata, RawModData};

impl ModMetadata {
    #[must_use]
    pub const fn get_download_count(&self) -> usize {
        match &self.mod_data {
            RawModData::Modrinth(x) => x.downloads,
            RawModData::Curseforge { data, .. } => data.download_count,
        }
    }

    #[must_use]
    pub fn get_formatted_download_count(&self) -> String {
        let thousands = self.get_download_count() as f64 / 1000.;
        if thousands < 1. {
            format!("{}", self.get_download_count())
        } else {
            let millions = thousands / 1000.;
            if millions < 1. {
                format!("{thousands:.1}K")
            } else {
                format!("{millions:.1}M")
            }
        }
    }

    /// # Errors
    ///
    /// This functions returns `Err` if:
    /// - Fails to read base dir
    pub fn get_filepaths(&self) -> io::Result<Vec<PathBuf>> {
        let mut read = std::fs::read_dir(self.base_path())?
            .map(|x| x.map(|x| x.path()))
            .collect::<io::Result<Vec<_>>>()?;
        let target = match &self.mod_data {
            RawModData::Modrinth(x) => x
                .files
                .iter()
                .map(|x| self.base_path().join(&x.filename))
                .collect(),
            RawModData::Curseforge { data, .. } => {
                vec![self.base_path().join(&data.file_name)]
            }
        };
        read.retain(|x| target.contains(x) || target.contains(&x.with_extension("")));
        Ok(read)
    }

    /// # Errors
    ///
    /// This functions returns `Err` if:
    /// - Fails to read mod size
    pub async fn get_accumlated_size(&self) -> io::Result<usize> {
        tokio_stream::iter(self.get_filepaths()?.into_iter())
            .map(tokio::fs::read)
            .buffer_unordered(50)
            .try_fold(0, |acc, x| async move { Ok(acc + x.len()) })
            .await
    }

    /// # Errors
    ///
    /// This functions returns `Err` if:
    /// - Fails to get mod accumlated size
    pub async fn get_formatted_accumlated_size(&self) -> io::Result<String> {
        let size = self.get_accumlated_size().await?;
        let thousands = size as f64 / 1000.;
        let string = if thousands < 1. {
            format!("{size} B")
        } else {
            let millions = thousands / 1000.;
            if millions < 1. {
                format!("{thousands:.2} KB")
            } else {
                let giga = millions / 1000.;
                if giga < 1. {
                    format!("{millions:.2} MB")
                } else {
                    format!("{giga:.2} GB")
                }
            }
        };
        Ok(string)
    }
}
