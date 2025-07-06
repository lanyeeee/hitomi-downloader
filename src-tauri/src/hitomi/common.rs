use anyhow::{anyhow, Context};
use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::hitomi_client::HitomiClient;

use super::gg::GG;
//common.js
pub const PROTOCOL: &str = "https:";
pub const DOMAIN: &str = "ltn.gold-usergeneratedcontent.net";
#[allow(dead_code)]
pub const GALLERY_BLOCK_EXTENSION: &str = ".html";
#[allow(dead_code)]
pub const GALLERY_BLOCK_DIR: &str = "galleryblock";
pub const NOZOMI_EXTENSION: &str = ".nozomi";

#[derive(Debug, Serialize, Deserialize)]
pub struct Artist {
    pub artist: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub group: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Parody {
    pub parody: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Character {
    pub character: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct Tag {
    pub tag: String,
    pub url: String,
    #[serde(default, deserialize_with = "string_to_i32")]
    pub female: i32,
    #[serde(default, deserialize_with = "string_to_i32")]
    pub male: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct Language {
    #[serde(default, deserialize_with = "string_to_i32")]
    pub galleryid: i32,
    pub url: String,
    pub language_localname: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GalleryFiles {
    pub width: i32,
    pub hash: String,
    #[serde(default)]
    pub haswebp: i32,
    #[serde(default)]
    pub hasavif: i32,
    #[serde(default)]
    pub hasjxl: i32,
    pub name: String,
    pub height: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GalleryInfo {
    #[serde(default, deserialize_with = "string_to_i32")]
    pub id: i32,
    pub title: String,
    pub japanese_title: Option<String>,
    pub language: Option<String>,
    pub language_localname: Option<String>,
    #[serde(rename = "type")]
    pub type_field: String,
    pub date: String,
    pub artists: Option<Vec<Artist>>,
    pub groups: Option<Vec<Group>>,
    pub parodys: Option<Vec<Parody>>,
    pub tags: Option<Vec<Tag>>,
    #[serde(default)]
    pub related: Vec<i32>,
    #[serde(default)]
    pub languages: Vec<Language>,
    pub characters: Option<Vec<Character>>,
    #[serde(default)]
    pub scene_indexes: Vec<i32>,
    #[serde(default)]
    pub files: Vec<GalleryFiles>,
}

fn string_to_i32<'de, D>(d: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde_json::Value;
    let value: Value = serde::Deserialize::deserialize(d)?;

    match value {
        #[allow(clippy::cast_possible_truncation)]
        Value::Number(n) => Ok(n.as_i64().unwrap_or(0) as i32),
        Value::String(s) => Ok(s.parse().unwrap_or(0)),
        _ => Err(serde::de::Error::custom(
            "`string_to_i32` failed, value type is not `Number` or `String`",
        )),
    }
}

#[allow(clippy::cast_sign_loss)]
pub async fn subdomain_from_url(
    url: &str,
    base: Option<&str>,
    dir: Option<&str>,
) -> anyhow::Result<String> {
    let base_is_none_or_empty = match base {
        None => true,
        Some(base) => base.is_empty(),
    };

    let mut retval = String::new();

    if base_is_none_or_empty {
        match dir {
            Some("webp") => retval = "w".to_string(),
            Some("avif") => retval = "a".to_string(),
            _ => {}
        }
    }

    let re = Regex::new(r"/[0-9a-f]{61}([0-9a-f]{2})([0-9a-f])")?;

    let Some(caps) = re.captures(url) else {
        return Ok(String::new());
    };

    let g = i32::from_str_radix(&format!("{}{}", &caps[2], &caps[1]), 16);

    if let Ok(g) = g {
        let mut gg = GG::inst().lock().await;
        let m_result = gg.m(g).await?;

        retval = if base_is_none_or_empty {
            format!("{retval}{}", 1 + m_result)
        } else {
            let c = char::from_u32((97 + m_result) as u32).context("Invalid character value")?;
            format!("{c}{}", base.unwrap_or(""))
        };
    };

    Ok(retval)
}

pub async fn url_from_url(
    url: &str,
    base: Option<&str>,
    dir: Option<&str>,
) -> anyhow::Result<String> {
    let re = Regex::new(r"//..?\.(?:gold-usergeneratedcontent\.net|hitomi\.la)/")?;
    let subdomain = subdomain_from_url(url, base, dir).await?;
    let res_url = re.replace(url, format!("//{subdomain}.gold-usergeneratedcontent.net/"));
    Ok(res_url.into_owned())
}

pub async fn full_path_from_hash(hash: &str) -> anyhow::Result<String> {
    let mut gg = GG::inst().lock().await;
    let b = gg.b().await?;
    let s = gg.s(hash)?;
    Ok(format!("{b}{s}/{hash}"))
}

pub fn real_full_path_from_hash(hash: &str) -> anyhow::Result<String> {
    let re = Regex::new(r"^.*(..)(.)$")?;
    let real_full_path = re.replace(hash, format!("$2/$1/{hash}")).to_string();
    Ok(real_full_path)
}

pub async fn url_from_hash(
    _gallery_id: i32,
    image: &GalleryFiles,
    dir: Option<&str>,
    ext: Option<&str>,
) -> anyhow::Result<String> {
    let ext = match (ext, dir) {
        (Some(e), _) => e,
        (None, Some(d)) => d,
        (None, None) => image.name.rsplit('.').next().unwrap_or(""),
    };

    let mut url = String::from("https://a.gold-usergeneratedcontent.net/");

    if let Some(dir) = dir {
        if dir != "webp" && dir != "avif" {
            url.push_str(dir);
            url.push('/');
        }
    }

    url.push_str(&full_path_from_hash(&image.hash).await?);
    url.push('.');
    url.push_str(ext);

    Ok(url)
}

pub async fn url_from_url_from_hash(
    gallery_id: i32,
    image: &GalleryFiles,
    dir: Option<&str>,
    ext: Option<&str>,
    base: Option<&str>,
) -> anyhow::Result<String> {
    if base == Some("tn") {
        let real_path = real_full_path_from_hash(&image.hash)?;

        let Some(dir) = dir else {
            return Err(anyhow!(r#"if base is "tn", dir must not be None"#));
        };

        let Some(ext) = ext else {
            return Err(anyhow!(r#"if base is "tn", ext must not be None"#));
        };

        let url = format!("https://a.gold-usergeneratedcontent.net/{dir}/{real_path}.{ext}");
        url_from_url(&url, base, None).await
    } else {
        let url = url_from_hash(gallery_id, image, dir, ext).await?;
        url_from_url(&url, base, dir).await
    }
}

pub enum Ext {
    Webp,
    Avif,
}

pub async fn image_url_from_image(
    gallery_id: i32,
    image: &GalleryFiles,
    ext: Ext,
) -> anyhow::Result<String> {
    match ext {
        Ext::Webp => url_from_url_from_hash(gallery_id, image, Some("webp"), None, None).await,
        Ext::Avif => url_from_url_from_hash(gallery_id, image, Some("avif"), None, None).await,
    }
}

pub async fn get_gallery_info(gallery_id: i32) -> anyhow::Result<GalleryInfo> {
    let client = HitomiClient::get_api_client();

    let url = format!("{PROTOCOL}//{DOMAIN}/galleries/{gallery_id}.js");
    let request = client.read().get(&url);
    let body = request.send().await?.text().await?;

    let json_str = body.replace("var galleryinfo = ", "");
    let gallery_info: GalleryInfo = serde_json::from_str(&json_str)
        .context(format!("Failed to parse gallery info: {json_str}"))?;
    Ok(gallery_info)
}
