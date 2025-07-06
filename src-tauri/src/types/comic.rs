use std::path::{Path, PathBuf};

use anyhow::Context;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use walkdir::WalkDir;

use crate::{
    config::Config,
    hitomi::{url_from_url_from_hash, GalleryFiles, GalleryInfo},
};

#[derive(Default, Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct Comic {
    pub id: i32,
    pub title: String,
    pub japanese_title: String,
    pub language: String,
    pub language_localname: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub date: String,
    pub artists: Vec<String>,
    pub groups: Vec<String>,
    pub parodys: Vec<String>,
    pub tags: Vec<Tag>,
    pub related: Vec<i32>,
    pub languages: Vec<Language>,
    pub characters: Vec<String>,
    pub scene_indexes: Vec<i32>,
    pub files: Vec<GalleryFiles>,
    pub cover_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comic_download_dir: Option<PathBuf>,
}

impl Comic {
    pub async fn from_gallery_info(
        app: &AppHandle,
        gallery_info: GalleryInfo,
    ) -> anyhow::Result<Comic> {
        let first_file = gallery_info
            .files
            .first()
            .context(format!("gallery_info has no files: {gallery_info:?}"))?;
        let cover_url = url_from_url_from_hash(
            gallery_info.id,
            first_file,
            Some("webpbigtn"),
            Some("webp"),
            Some("tn"),
        )
        .await
        .context("Get cover url failed")?;

        let artists = gallery_info
            .artists
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.artist)
            .collect();

        let groups = gallery_info
            .groups
            .unwrap_or_default()
            .into_iter()
            .map(|g| g.group)
            .collect();

        let parodys = gallery_info
            .parodys
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.parody)
            .collect();

        let tags = gallery_info
            .tags
            .unwrap_or_default()
            .into_iter()
            .map(|t| Tag {
                tag: t.tag,
                female: t.female,
                male: t.male,
            })
            .collect();

        let languages = gallery_info
            .languages
            .into_iter()
            .map(|l| Language {
                galleryid: l.galleryid,
                language_localname: l.language_localname,
                name: l.name,
            })
            .collect();

        let characters = gallery_info
            .characters
            .unwrap_or_default()
            .into_iter()
            .map(|c| c.character)
            .collect();

        let mut comic = Comic {
            id: gallery_info.id,
            title: gallery_info.title,
            japanese_title: gallery_info.japanese_title.unwrap_or_default(),
            language: gallery_info.language.unwrap_or_default(),
            language_localname: gallery_info.language_localname.unwrap_or_default(),
            type_field: gallery_info.type_field,
            date: gallery_info.date,
            artists,
            groups,
            parodys,
            tags,
            related: gallery_info.related,
            languages,
            characters,
            scene_indexes: gallery_info.scene_indexes,
            files: gallery_info.files,
            cover_url,
            is_downloaded: None,
            comic_download_dir: None,
        };

        comic.update_fields(app).context(format!(
            "Failed to update fields for comic `{}`",
            comic.title
        ))?;

        Ok(comic)
    }

    pub fn from_metadata(metadata_path: &Path) -> anyhow::Result<Comic> {
        let comic_json = std::fs::read_to_string(metadata_path).context(format!(
            "Failed to convert metadata to Comic, failed to read metadata file `{}`",
            metadata_path.display()
        ))?;
        let mut comic: Comic = serde_json::from_str(&comic_json).context(format!(
            "Failed to convert metadata to Comic, failed to deserialize `{}` to Comic",
            metadata_path.display()
        ))?;
        // The `is_downloaded` and `comic_download_dir` fields are not serialized in the metadata file
        let parent = metadata_path.parent().context(format!(
            "Failed to get parent directory of `{}`",
            metadata_path.display()
        ))?;
        comic.comic_download_dir = Some(parent.to_path_buf());
        comic.is_downloaded = Some(true);
        Ok(comic)
    }

    /// Update fields based on the metadata file in the download directory
    ///
    /// Update fields and logic:
    /// - `comic_download_dir`: Update to the directory where the metadata file is located by matching the current comic id
    /// - `is_downloaded`: Set to true if the corresponding comic metadata is found
    pub fn update_fields(&mut self, app: &AppHandle) -> anyhow::Result<()> {
        let download_dir = app.state::<RwLock<Config>>().read().download_dir.clone();
        if !download_dir.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(&download_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            if entry.file_name() != "metadata.json" {
                continue;
            }
            // now the entry is the metadata.json file
            let metadata_str = std::fs::read_to_string(path)
                .context(format!("Failed to read `{}`", path.display()))?;

            let comic_json: serde_json::Value =
                serde_json::from_str(&metadata_str).context(format!(
                    "Failed to deserialize `{}` to serde_json::Value",
                    path.display()
                ))?;

            let id = comic_json
                .get("id")
                .and_then(|id| id.as_number())
                .context(format!("`id` field not found in `{}`", path.display()))?
                .as_i64()
                .context(format!(
                    "`id` field in `{}` is not an integer",
                    path.display()
                ))?;
            #[allow(clippy::cast_possible_truncation)]
            let id = id as i32;

            if id != self.id {
                continue;
            }

            let parent = path.parent().context(format!(
                "Failed to get parent directory of `{}`",
                path.display()
            ))?;

            self.comic_download_dir = Some(parent.to_path_buf());
            self.is_downloaded = Some(true);
            break;
        }

        Ok(())
    }

    pub fn get_comic_download_dir_name(&self) -> anyhow::Result<String> {
        let comic_download_dir = self
            .comic_download_dir
            .as_ref()
            .context("`comic_download_dir` field is `None`")?;

        let comic_download_dir_name = comic_download_dir
            .file_name()
            .context(format!(
                "Failed to get directory name of `{}`",
                comic_download_dir.display()
            ))?
            .to_string_lossy()
            .to_string();

        Ok(comic_download_dir_name)
    }

    pub fn get_comic_export_dir(&self, app: &AppHandle) -> anyhow::Result<PathBuf> {
        let (download_dir, export_dir) = {
            let config = app.state::<RwLock<Config>>();
            let config = config.read();
            (config.download_dir.clone(), config.export_dir.clone())
        };

        let comic_download_dir = self
            .comic_download_dir
            .as_ref()
            .context("`comic_download_dir` field is `None`")?;

        let relative_dir = comic_download_dir
            .strip_prefix(&download_dir)
            .context(format!(
                "Failed to strip prefix `{}` from `{}`",
                comic_download_dir.display(),
                download_dir.display()
            ))?;

        let comic_export_dir = export_dir.join(relative_dir);
        Ok(comic_export_dir)
    }

    pub fn get_temp_download_dir(&self) -> anyhow::Result<PathBuf> {
        let comic_download_dir = self
            .comic_download_dir
            .as_ref()
            .context("`comic_download_dir` field is `None`")?;

        let comic_download_dir_name = self
            .get_comic_download_dir_name()
            .context("Failed to get comic download directory name")?;

        let parent = comic_download_dir.parent().context(format!(
            "Failed to get parent directory of `{}`",
            comic_download_dir.display()
        ))?;

        let temp_download_dir = parent.join(format!(".downloading-{comic_download_dir_name}"));
        Ok(temp_download_dir)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, Type)]
#[allow(clippy::struct_field_names)]
pub struct Tag {
    pub tag: String,
    pub female: i32,
    pub male: i32,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, Type)]
#[allow(clippy::struct_field_names)]
pub struct Language {
    pub galleryid: i32,
    pub language_localname: String,
    pub name: String,
}
