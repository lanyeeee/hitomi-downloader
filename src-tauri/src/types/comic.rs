use std::path::{Path, PathBuf};

use anyhow::Context;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dir_name: String,
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
            dir_name: String::new(),
        };

        comic.update_fields(app).context(format!(
            "Failed to update fields for comic `{}`",
            comic.title
        ))?;

        Ok(comic)
    }

    pub fn from_metadata(app: &AppHandle, metadata_path: &Path) -> anyhow::Result<Comic> {
        let comic_json = std::fs::read_to_string(metadata_path).context(format!(
            "Failed to convert metadata to Comic, failed to read metadata file `{}`",
            metadata_path.display()
        ))?;
        let mut comic: Comic = serde_json::from_str(&comic_json).context(format!(
            "Failed to convert metadata to Comic, failed to deserialize `{}` to Comic",
            metadata_path.display()
        ))?;
        // The `is_downloaded` and `dir_name` fields are not serialized in the metadata file
        // call `update_fields` to set them
        comic.update_fields(app).context(format!(
            "Failed to update fields for comic `{}`",
            comic.title
        ))?;
        Ok(comic)
    }

    /// Update fields based on the metadata file in the download directory
    ///
    /// Update fields and logic:
    /// - `dir_name`: Update to the directory name where the metadata file is located by matching the current comic id
    /// - `is_downloaded`: Set to true if the corresponding comic metadata is found
    pub fn update_fields(&mut self, app: &AppHandle) -> anyhow::Result<()> {
        let download_dir = app.state::<RwLock<Config>>().read().download_dir.clone();
        if !download_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&download_dir)
            .context(format!(
                "Failed to read the download directory `{}`",
                download_dir.display()
            ))?
            .filter_map(Result::ok)
        {
            let metadata_path = entry.path().join("metadata.json");
            if !metadata_path.exists() {
                continue;
            }

            let metadata_str = std::fs::read_to_string(&metadata_path)
                .context(format!("Failed to read `{}`", metadata_path.display()))?;

            let comic_json: serde_json::Value =
                serde_json::from_str(&metadata_str).context(format!(
                    "Failed to deserialize `{}` to serde_json::Value",
                    metadata_path.display()
                ))?;

            let id = comic_json
                .get("id")
                .and_then(|id| id.as_number())
                .context(format!(
                    "`id` field not found in `{}`",
                    metadata_path.display()
                ))?
                .as_i64()
                .context(format!(
                    "`id` field in `{}` is not an integer",
                    metadata_path.display()
                ))?;
            #[allow(clippy::cast_possible_truncation)]
            let id = id as i32;

            if id != self.id {
                continue;
            }

            self.dir_name = entry.file_name().to_string_lossy().to_string();
            self.is_downloaded = Some(true);
            break;
        }

        Ok(())
    }

    pub fn get_download_dir(&self, app: &AppHandle) -> PathBuf {
        app.state::<RwLock<Config>>()
            .read()
            .download_dir
            .join(&self.dir_name)
    }

    pub fn get_export_dir(&self, app: &AppHandle) -> PathBuf {
        app.state::<RwLock<Config>>()
            .read()
            .export_dir
            .join(&self.dir_name)
    }

    pub fn get_temp_download_dir(&self, app: &AppHandle) -> PathBuf {
        let dir_name = &self.dir_name;

        app.state::<RwLock<Config>>()
            .read()
            .download_dir
            .join(format!(".downloading-{dir_name}"))
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
