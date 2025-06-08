use std::{
    io::{Cursor, Read},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use indexmap::IndexSet;
use regex::Regex;
use reqwest::{header::RANGE, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use tokio::sync::OnceCell;

use crate::hitomi_client::HitomiClient;

use super::{DOMAIN, NOZOMI_EXTENSION, PROTOCOL};

//searchlib.js
const SEPARATOR: &str = "-";
const EXTENSION: &str = ".html";
const INDEX_DIR: &str = "tagindex";
const GALLERIES_INDEX_DIR: &str = "galleriesindex";
const MAX_NODE_SIZE: u64 = 464;
const B: usize = 16;
const COMPRESSED_NOZOMI_PREFIX: &str = "n";
const TAG_INDEX_DOMAIN: &str = "tagindex.hitomi.la";

static TAG_INDEX_VERSION: OnceCell<String> = OnceCell::const_new();
static GALLERIES_INDEX_VERSION: OnceCell<String> = OnceCell::const_new();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Node {
    keys: Vec<Vec<u8>>,
    datas: Vec<(i64, i32)>,
    sub_node_addresses: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Suggestion {
    s: String,
    t: i32,
    u: String,
    n: String,
}

fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

fn hash_term(term: &str) -> Vec<u8> {
    sha256(term.as_bytes())[..4].to_vec()
}

fn sanitize(input: &str) -> String {
    let re = Regex::new(r"[/#]").unwrap();
    re.replace_all(input, "").to_string()
}

async fn get_index_version(name: &str) -> anyhow::Result<String> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let client = HitomiClient::get_api_client();
    let url = format!("{PROTOCOL}//{DOMAIN}/{name}/version?_={timestamp}");
    let request = client.read().get(&url);
    Ok(request.send().await?.text().await?)
}

fn encode_search_query_for_url(s: char) -> String {
    match s {
        ' ' => "_".to_string(),
        '/' => "slash".to_string(),
        '.' => "dot".to_string(),
        _ => s.to_string(),
    }
}

async fn get_url_at_range(url: &str, range: std::ops::Range<u64>) -> anyhow::Result<Vec<u8>> {
    let start = range.start;
    let end = range.end - 1;
    let range_header = format!("bytes={start}-{end}");

    let client = HitomiClient::get_api_client();

    let request = client.read().get(url).header(RANGE, range_header);
    let http_resp = request.send().await?;

    Ok(http_resp.bytes().await?.to_vec())
}

fn decode_node(data: &[u8]) -> anyhow::Result<Node> {
    let mut cursor = Cursor::new(data);

    let number_of_keys = cursor.read_i32::<BigEndian>()?;
    let mut keys = Vec::new();

    for _ in 0..number_of_keys {
        let key_size = cursor.read_i32::<BigEndian>()?;
        if key_size == 0 || key_size > 32 {
            return Err(anyhow!("fatal: !keySize || keySize > 32"));
        }
        #[allow(clippy::cast_sign_loss)]
        let mut key = vec![0u8; key_size as usize];
        cursor.read_exact(&mut key)?;
        keys.push(key);
    }

    let number_of_datas = cursor.read_i32::<BigEndian>()?;
    let mut datas = Vec::new();

    for _ in 0..number_of_datas {
        let offset = cursor.read_i64::<BigEndian>()?;
        let length = cursor.read_i32::<BigEndian>()?;
        datas.push((offset, length));
    }

    let mut sub_node_addresses = Vec::new();
    for _ in 0..=B {
        let address = cursor.read_i64::<BigEndian>()?;
        sub_node_addresses.push(address);
    }

    Ok(Node {
        keys,
        datas,
        sub_node_addresses,
    })
}

async fn get_node_at_address(field: &str, address: i64) -> anyhow::Result<Option<Node>> {
    let tag_index_version = TAG_INDEX_VERSION
        .get_or_init(|| async { get_index_version(INDEX_DIR).await.unwrap_or_default() })
        .await;

    let galleries_index_version = GALLERIES_INDEX_VERSION
        .get_or_init(|| async {
            get_index_version(GALLERIES_INDEX_DIR)
                .await
                .unwrap_or_default()
        })
        .await;

    let url = match field {
        "galleries" => format!(
            "{PROTOCOL}//{DOMAIN}/{GALLERIES_INDEX_DIR}/galleries.{galleries_index_version}.index"
        ),
        "languages" => format!(
            "{PROTOCOL}//{DOMAIN}/{GALLERIES_INDEX_DIR}/languages.{galleries_index_version}.index"
        ),
        "nozomiurl" => format!(
            "{PROTOCOL}//{DOMAIN}/{GALLERIES_INDEX_DIR}/nozomiurl.{galleries_index_version}.index"
        ),
        _ => format!("{PROTOCOL}//{DOMAIN}/{INDEX_DIR}/{field}.{tag_index_version}.index"),
    };

    #[allow(clippy::cast_sign_loss)]
    let nodedata = get_url_at_range(&url, address as u64..(address as u64 + MAX_NODE_SIZE)).await?;
    Ok(Some(decode_node(&nodedata)?))
}

fn compare_arrays(a: &[u8], b: &[u8]) -> i32 {
    let top = std::cmp::min(a.len(), b.len());
    for i in 0..top {
        #[allow(clippy::comparison_chain)]
        if a[i] < b[i] {
            return -1;
        } else if a[i] > b[i] {
            return 1;
        }
    }
    0
}

fn locate_key(key: &[u8], node: &Node) -> (bool, usize) {
    for (i, node_key) in node.keys.iter().enumerate() {
        let cmp_result = compare_arrays(key, node_key);
        if cmp_result <= 0 {
            return (cmp_result == 0, i);
        }
    }
    (false, node.keys.len())
}

fn is_leaf(node: &Node) -> bool {
    node.sub_node_addresses.iter().all(|&addr| addr == 0)
}

async fn b_search(field: &str, key: &[u8], node: &Node) -> anyhow::Result<Option<(i64, i32)>> {
    if node.keys.is_empty() {
        return Ok(None);
    }

    let (there, where_idx) = locate_key(key, node);
    if there {
        return Ok(Some(node.datas[where_idx]));
    } else if is_leaf(node) {
        return Ok(None);
    }

    if let Some(next_node) = get_node_at_address(field, node.sub_node_addresses[where_idx]).await? {
        // use Box::pin to wrap the Future of the recursive call
        return Box::pin(b_search(field, key, &next_node)).await;
    }
    Ok(None)
}

async fn get_gallery_ids_from_data(data: (i64, i32)) -> anyhow::Result<IndexSet<i32>> {
    let galleries_index_version = GALLERIES_INDEX_VERSION
        .get_or_init(|| async {
            get_index_version(GALLERIES_INDEX_DIR)
                .await
                .unwrap_or_default()
        })
        .await;

    let url = format!(
        "{PROTOCOL}//{DOMAIN}/{GALLERIES_INDEX_DIR}/galleries.{galleries_index_version}.data"
    );
    let (offset, length) = data;

    if length > 100_000_000 || length <= 0 {
        return Err(anyhow!("length `{length}` is too long"));
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_lossless)]
    let inbuf = get_url_at_range(&url, offset as u64..((offset + length as i64) as u64)).await?;

    let mut cursor = Cursor::new(inbuf);
    let number_of_gallery_ids = cursor.read_i32::<BigEndian>()?;

    if number_of_gallery_ids > 10_000_000 || number_of_gallery_ids <= 0 {
        return Err(anyhow!(
            "number_of_galleryids `{number_of_gallery_ids}` is too long"
        ));
    }

    let mut gallery_ids = IndexSet::new();
    for _ in 0..number_of_gallery_ids {
        gallery_ids.insert(cursor.read_i32::<BigEndian>()?);
    }

    Ok(gallery_ids)
}

pub(crate) async fn get_gallery_ids_from_nozomi(
    area: Option<&str>,
    tag: &str,
    language: &str,
) -> anyhow::Result<IndexSet<i32>> {
    let nozomi_address = match area {
        None => format!(
            "{PROTOCOL}//{DOMAIN}/{COMPRESSED_NOZOMI_PREFIX}/{tag}-{language}{NOZOMI_EXTENSION}"
        ),
        Some(area) => format!(
            "{PROTOCOL}//{DOMAIN}/{COMPRESSED_NOZOMI_PREFIX}/{area}/{tag}-{language}{NOZOMI_EXTENSION}"
        ),
    };

    let client = HitomiClient::get_api_client();

    let request = client.read().get(&nozomi_address);
    let http_resp = request.send().await?;
    if http_resp.status() != 200 {
        return Ok(IndexSet::new());
    }

    let bytes = http_resp.bytes().await?;

    let mut cursor = Cursor::new(bytes);
    let mut nozomi = IndexSet::new();

    while let Ok(id) = cursor.read_i32::<BigEndian>() {
        nozomi.insert(id);
    }

    Ok(nozomi)
}

pub(crate) async fn get_gallery_ids_for_query(query: &str) -> anyhow::Result<IndexSet<i32>> {
    let query = query.replace('_', " ");

    if let Some(colon_idx) = query.find(':') {
        let (ns, tag) = query.split_at(colon_idx);
        let tag = &tag[1..];

        let (area, language, tag) = match ns {
            "female" | "male" => (Some("tag"), "all", query.to_string()),
            "language" => (None, tag, "index".to_string()),
            _ => (Some(ns), "all", tag.to_string()),
        };

        return get_gallery_ids_from_nozomi(area, &tag, language).await;
    }

    let key = hash_term(&query);
    let field = "galleries";

    if let Some(node) = get_node_at_address(field, 0).await? {
        if let Some(data) = b_search(field, &key, &node).await? {
            return get_gallery_ids_from_data(data).await;
        }
    }

    Ok(IndexSet::new())
}

pub async fn get_suggestions_for_query(query: &str) -> anyhow::Result<Vec<Suggestion>> {
    let query = query.replace('_', " ");
    let (field, term) = if let Some(colon_idx) = query.find(':') {
        let (field, term) = query.split_at(colon_idx);
        (field, &term[1..])
    } else {
        ("global", query.as_str())
    };

    let chars_path_segment = term
        .chars()
        .map(encode_search_query_for_url)
        .collect::<Vec<String>>()
        .join("/");

    let url = if chars_path_segment.is_empty() {
        format!("{PROTOCOL}//{TAG_INDEX_DOMAIN}/{field}.json")
    } else {
        format!("{PROTOCOL}//{TAG_INDEX_DOMAIN}/{field}/{chars_path_segment}.json")
    };

    let client = HitomiClient::get_api_client();
    let request = client.read().get(&url);

    let http_resp = request.send().await?;
    let status = http_resp.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    } else if status != StatusCode::OK {
        let body = http_resp.text().await?;
        return Err(anyhow!("Unexpected status code({status}): {body}"));
    }
    let body = http_resp.text().await?;

    let suggestions: serde_json::Value = serde_json::from_str(&body)?;

    let mut result = Vec::new();
    if let serde_json::Value::Array(suggestions) = suggestions {
        for suggestion in suggestions {
            if let serde_json::Value::Array(suggestion) = suggestion {
                if suggestion.len() < 3 {
                    continue;
                }

                let ns = suggestion[2].as_str().unwrap_or("").to_string();
                let tagname = sanitize(suggestion[0].as_str().unwrap_or(""));

                let url = match ns.as_str() {
                    "female" | "male" => format!("/tag/{ns}:{tagname}{SEPARATOR}1{EXTENSION}"),
                    "language" => format!("/index-{tagname}{SEPARATOR}1{EXTENSION}"),
                    _ => format!("/{ns}/{tagname}{SEPARATOR}all{SEPARATOR}1{EXTENSION}"),
                };

                result.push(Suggestion {
                    s: suggestion[0].as_str().unwrap_or("").to_string(),
                    #[allow(clippy::cast_possible_truncation)]
                    t: suggestion[1].as_i64().unwrap_or(0) as i32,
                    u: url,
                    n: ns,
                });
            }
        }
    }

    Ok(result)
}
