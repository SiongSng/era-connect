use std::{io, path::PathBuf};

use futures::{StreamExt, TryStreamExt};

use super::{ModMetadata, RawModData};

impl ModMetadata {
    pub fn get_download_count(&self) -> usize {
        match &self.mod_data {
            RawModData::Modrinth(x) => x.downloads,
            RawModData::Curseforge { data, .. } => data.download_count,
        }
    }

    pub fn get_formatted_download_count(&self) -> String {
        let thousands = self.get_download_count() as f64 / 1000.;
        if thousands < 1. {
            format!("{}", self.get_download_count())
        } else {
            let millions = thousands / 1000.;
            if millions < 1. {
                format!("{:.1}K", thousands)
            } else {
                format!("{:.1}M", millions)
            }
        }
    }

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

    pub async fn get_accumlated_size(&self) -> io::Result<usize> {
        tokio_stream::iter(self.get_filepaths()?.into_iter())
            .map(|x| tokio::fs::read(x))
            .buffer_unordered(50)
            .try_fold(0, |acc, x| async move { Ok(acc + x.len()) })
            .await
    }

    pub async fn get_formatted_accumlated_size(&self) -> io::Result<String> {
        let thousands = self.get_accumlated_size().await? as f64 / 1000.;
        let string = if thousands < 1. {
            format!("{} B", self.get_accumlated_size().await?)
        } else {
            let millions = thousands / 1000.;
            if millions < 1. {
                format!("{:.2} KB", thousands)
            } else {
                let giga = millions / 1000.;
                if giga < 1. {
                    format!("{:.2} MB", millions)
                } else {
                    format!("{:.2} GB", giga)
                }
            }
        };
        Ok(string)
    }
}
