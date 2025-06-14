use serde::{Deserialize, Serialize};
use specta::Type;
use yaserde::{YaDeserialize, YaSerialize};

use super::Comic;

/// https://wiki.kavitareader.com/guides/metadata/comics/
#[derive(
    Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type, YaSerialize, YaDeserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ComicInfo {
    #[yaserde(rename = "Manga")]
    pub manga: String,
    /// Comic title
    #[yaserde(rename = "Series")]
    pub series: String,
    #[yaserde(rename = "Writer")]
    pub writer: String,
    #[yaserde(rename = "Publisher")]
    pub publisher: String,
    #[yaserde(rename = "Genre")]
    pub genre: String,
    #[yaserde(rename = "Tags")]
    pub tags: String,
    /// Normal chapter number
    #[yaserde(rename = "Number")]
    pub number: Option<String>,
    /// Volume number
    #[yaserde(rename = "Volume")]
    pub volume: Option<String>,
    /// if the value is `Special`, the chapter will be treated as a special issue by Kavita
    #[yaserde(rename = "Format")]
    pub format: Option<String>,
    /// The number of pages in this chapter
    #[yaserde(rename = "PageCount")]
    pub page_count: i64,
    /// Total number of chapters
    /// - `0` => Ongoing
    /// - `Non-zero` and consistent with `Number` or `Volume` => Completed
    /// - `Other non-zero values` => Ended
    #[yaserde(rename = "Count")]
    pub count: i64,
}

impl From<Comic> for ComicInfo {
    fn from(comic: Comic) -> Self {
        ComicInfo {
            manga: "Yes".to_string(),
            series: comic.title,
            writer: comic.artists.join(", "),
            publisher: "Hitomi".to_string(),
            genre: comic.type_field,
            tags: comic
                .tags
                .into_iter()
                .map(|tag| tag.tag)
                .collect::<Vec<String>>()
                .join(", "),
            number: Some("1".to_string()),
            volume: None,
            format: Some("Special".to_string()),
            #[allow(clippy::cast_possible_wrap)]
            page_count: comic.files.len() as i64,
            count: 1,
        }
    }
}
