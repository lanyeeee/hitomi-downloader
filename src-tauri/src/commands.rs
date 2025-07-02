use std::path::PathBuf;

use anyhow::{anyhow, Context};
use indexmap::IndexMap;
use parking_lot::RwLock;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use walkdir::WalkDir;

use crate::{
    config::Config,
    download_manager::DownloadManager,
    errors::{CommandError, CommandResult},
    export,
    extensions::AnyhowErrorToStringChain,
    hitomi::Suggestion,
    hitomi_client::HitomiClient,
    logger,
    types::{Comic, SearchResult},
};

#[tauri::command]
#[specta::specta]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn get_config(config: State<RwLock<Config>>) -> Config {
    let config = config.read().clone();
    tracing::debug!("get config success");
    config
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn save_config(
    app: AppHandle,
    hitomi_client: State<HitomiClient>,
    config_state: State<RwLock<Config>>,
    config: Config,
) -> CommandResult<()> {
    let proxy_changed = {
        let config_state = config_state.read();
        config_state.proxy_mode != config.proxy_mode
            || config_state.proxy_host != config.proxy_host
            || config_state.proxy_port != config.proxy_port
    };

    let enable_file_logger = config.enable_file_logger;
    let enable_file_logger_changed = config_state
        .read()
        .enable_file_logger
        .ne(&enable_file_logger);

    {
        // Wrapped in braces to automatically release the write lock
        let mut config_state = config_state.write();
        *config_state = config;
        config_state
            .save(&app)
            .map_err(|err| CommandError::from("save config failed", err))?;
        tracing::debug!("save config success");
    }

    if proxy_changed {
        hitomi_client.reload_client();
    }

    if enable_file_logger_changed {
        if enable_file_logger {
            logger::reload_file_logger()
                .map_err(|err| CommandError::from("reload file logger failed", err))?;
        } else {
            logger::disable_file_logger()
                .map_err(|err| CommandError::from("disable file logger failed", err))?;
        }
    }

    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
pub async fn search(
    hitomi_client: State<'_, HitomiClient>,
    query: &str,
    page_num: usize,
    sort_by_popularity: bool,
) -> CommandResult<SearchResult> {
    let search_result = hitomi_client
        .search(query, page_num, sort_by_popularity)
        .await
        .map_err(|err| CommandError::from("search failed", err))?;
    tracing::debug!("search success");
    Ok(search_result)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_page(
    hitomi_client: State<'_, HitomiClient>,
    ids: Vec<i32>,
    page_num: usize,
) -> CommandResult<SearchResult> {
    let search_result = hitomi_client
        .get_page(ids, page_num)
        .await
        .map_err(|err| CommandError::from("get page failed", err))?;
    tracing::debug!("get page success");
    Ok(search_result)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_comic(hitomi_client: State<'_, HitomiClient>, id: i32) -> CommandResult<Comic> {
    let comic = hitomi_client
        .get_comic(id)
        .await
        .map_err(|err| CommandError::from("get comic failed", err))?;
    tracing::debug!("get comic success");
    Ok(comic)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn create_download_task(
    download_manager: State<DownloadManager>,
    comic: Comic,
) -> CommandResult<()> {
    let id = comic.id;
    download_manager
        .create_download_task(comic)
        .map_err(|err| {
            let err_msg = format!("Failed to create download task with ID `{id}`");
            CommandError::from(&err_msg, err)
        })?;
    tracing::debug!("Created download task with ID `{id}` successfully");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn pause_download_task(download_manager: State<DownloadManager>, id: i32) -> CommandResult<()> {
    download_manager.pause_download_task(id).map_err(|err| {
        let err_msg = format!("Failed to pause download task with ID `{id}`");
        CommandError::from(&err_msg, err)
    })?;
    tracing::debug!("Paused download task with ID `{id}` successfully");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn resume_download_task(
    download_manager: State<DownloadManager>,
    id: i32,
) -> CommandResult<()> {
    download_manager.resume_download_task(id).map_err(|err| {
        let err_msg = format!("Failed to resume download task with ID `{id}`");
        CommandError::from(&err_msg, err)
    })?;
    tracing::debug!("Resumed download task with ID `{id}` successfully");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn cancel_download_task(
    download_manager: State<DownloadManager>,
    id: i32,
) -> CommandResult<()> {
    download_manager.cancel_download_task(id).map_err(|err| {
        let err_msg = format!("Failed to cancel download task with ID `{id}`");
        CommandError::from(&err_msg, err)
    })?;
    tracing::debug!("Canceled download task with ID `{id}` successfully");
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn get_downloaded_comics(config: State<RwLock<Config>>) -> Vec<Comic> {
    let download_dir = config.read().download_dir.clone();
    // Traverse the download directory to get the path and modification time of all metadata files
    let mut metadata_path_with_modify_time = Vec::new();
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
        let metadata = match path
            .metadata()
            .map_err(anyhow::Error::from)
            .context(format!(
                "Failed to get file metadata of `{}`",
                path.display()
            )) {
            Ok(metadata) => metadata,
            Err(err) => {
                let err_title = "An error occurred while getting downloaded comics, skipped";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                continue;
            }
        };

        let modify_time = match metadata
            .modified()
            .map_err(anyhow::Error::from)
            .context(format!(
                "Failed to get file modification time of `{}`",
                path.display()
            )) {
            Ok(modify_time) => modify_time,
            Err(err) => {
                let err_title = "An error occurred while getting downloaded comics, skipped";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                continue;
            }
        };

        metadata_path_with_modify_time.push((path.to_path_buf(), modify_time));
    }
    // Sort by file modification time, with the newest at the front
    metadata_path_with_modify_time.sort_by(|(_, a), (_, b)| b.cmp(a));
    // Create Comic from metadata file
    let mut downloaded_comics = Vec::new();
    for (metadata_path, _) in metadata_path_with_modify_time {
        match Comic::from_metadata(&metadata_path).context(format!(
            "Failed to create Comic from metadata `{}`",
            metadata_path.display()
        )) {
            Ok(comic) => downloaded_comics.push(comic),
            Err(err) => {
                let err_title = "An error occurred while getting downloaded comics, skipped";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
            }
        }
    }

    tracing::debug!("get downloaded comics success");

    // Group comics by their ID to facilitate deduplication
    let mut comics_by_id: IndexMap<i32, Vec<Comic>> = IndexMap::new();
    for comic in downloaded_comics {
        comics_by_id.entry(comic.id).or_default().push(comic);
    }

    let mut unique_comics = Vec::new();
    for (_comic_id, mut comics) in comics_by_id {
        // The download directories for all comics with the same ID, which may have multiple versions, so we need to deduplicate
        let comic_download_dirs: Vec<&PathBuf> = comics
            .iter()
            .filter_map(|comic| comic.comic_download_dir.as_ref())
            .collect();

        if comic_download_dirs.is_empty() {
            // This situation should not actually happen, because the comic metadata file should always have a download directory
            continue;
        }

        // Choose the first one as the retained comic
        let chosen_download_dir = comic_download_dirs[0];

        if comics.len() > 1 {
            let dir_paths_string = comic_download_dirs
                .iter()
                .map(|path| format!("`{}`", path.display()))
                .collect::<Vec<String>>()
                .join(", ");
            // If there are duplicate comics, report an error
            let comic_title = &comics[0].title;
            let err_title = "An error occurred while getting downloaded comics";
            let string_chain = anyhow!("All version paths: [{dir_paths_string}]")
                .context(format!(
                    "To proceed, temporarily selected only the version '{}' from the multiple versions found",
                    chosen_download_dir.display()
                ))
                .context(format!(
                    "Comic `{comic_title}` has multiple versions in the download directory. Please handle this manually, keeping only one",
                ))
                .to_string_chain();
            tracing::error!(err_title, message = string_chain);
        }
        // Choose the first one as the retained comic
        let chosen_comic = comics.remove(0);
        unique_comics.push(chosen_comic);
    }

    unique_comics
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_pdf(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let title = &comic.title;
    export::pdf(&app, &comic).map_err(|err| {
        CommandError::from(&format!("Failed to export pdf for comic `{title}`"), err)
    })?;
    tracing::debug!("Exported pdf for comic `{title}` successfully");
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub fn export_cbz(app: AppHandle, comic: Comic) -> CommandResult<()> {
    let title = &comic.title;
    export::cbz(&app, &comic).map_err(|err| {
        CommandError::from(&format!("Failed to export cbz for comic `{title}`"), err)
    })?;
    tracing::debug!("Exported cbz for comic `{title}` successfully");
    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub async fn get_search_suggestions(
    hitomi_client: State<'_, HitomiClient>,
    query: String,
) -> CommandResult<Vec<Suggestion>> {
    let suggestions = hitomi_client
        .get_search_suggestions(&query)
        .await
        .map_err(|err| CommandError::from("Failed to get search suggestions", err))?;
    tracing::debug!("get search suggestions success");
    Ok(suggestions)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_logs_dir_size(app: AppHandle) -> CommandResult<u64> {
    let logs_dir = logger::logs_dir(&app)
        .context("Failed to get logs directory")
        .map_err(|err| CommandError::from("Failed to get logs directory size", err))?;
    let logs_dir_size = std::fs::read_dir(&logs_dir)
        .context(format!(
            "Failed to read logs directory `{}`",
            logs_dir.display()
        ))
        .map_err(|err| CommandError::from("Failed to get logs directory size", err))?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum::<u64>();
    tracing::debug!("get logs directory size success");
    Ok(logs_dir_size)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn show_path_in_file_manager(app: AppHandle, path: &str) -> CommandResult<()> {
    app.opener()
        .reveal_item_in_dir(path)
        .context(format!("Failed to open `{path}` in file manager"))
        .map_err(|err| CommandError::from("Failed to open in file manager", err))?;
    tracing::debug!("Opened in file manager successfully");
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub async fn get_cover_data(
    hitomi_client: State<'_, HitomiClient>,
    cover_url: String,
) -> CommandResult<Vec<u8>> {
    let cover_data = hitomi_client
        .get_cover_data(&cover_url)
        .await
        .map_err(|err| CommandError::from("Failed to get cover", err))?;
    Ok(cover_data.to_vec())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
#[specta::specta]
pub fn get_synced_comic(app: AppHandle, mut comic: Comic) -> CommandResult<Comic> {
    comic.update_fields(&app).map_err(|err| {
        let err_title = format!("Failed to update fields for comic `{}`", comic.title);
        CommandError::from(&err_title, err)
    })?;

    Ok(comic)
}
