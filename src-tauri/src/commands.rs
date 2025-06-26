use std::{path::PathBuf, time::SystemTime};

use anyhow::Context;
use parking_lot::RwLock;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

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
pub fn get_downloaded_comics(
    app: AppHandle,
    config: State<RwLock<Config>>,
) -> CommandResult<Vec<Comic>> {
    let download_dir = config.read().download_dir.clone();
    // Traverse the download directory to get the path and modification time of all metadata files
    let entries = std::fs::read_dir(&download_dir);
    let mut metadata_path_with_modify_time: Vec<(PathBuf, SystemTime)> = entries
        .map_err(|err| {
            let err_title = format!(
                "Failed to get downloaded comics, failed to read download directory {}",
                download_dir.display()
            );
            CommandError::from(&err_title, err)
        })?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let filename = entry.file_name();
            if filename.to_string_lossy().starts_with(".downloading-") {
                return None;
            }
            let metadata_path = entry.path().join("metadata.json");
            if !metadata_path.exists() {
                return None;
            }
            let modify_time = metadata_path.metadata().ok()?.modified().ok()?;
            Some((metadata_path, modify_time))
        })
        .collect();
    // Sort by file modification time, with the newest at the front
    metadata_path_with_modify_time.sort_by(|(_, a), (_, b)| b.cmp(a));
    // Read Comic from metadata file
    let downloaded_comics: Vec<Comic> = metadata_path_with_modify_time
        .iter()
        .filter_map(|(metadata_path, _)| {
            match Comic::from_metadata(&app, metadata_path).map_err(anyhow::Error::from) {
                Ok(comic) => Some(comic),
                Err(err) => {
                    let err_title =
                        format!("Failed to read metadata file `{}`", metadata_path.display());
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);
                    None
                }
            }
        })
        .collect();

    tracing::debug!("get downloaded comics success");
    Ok(downloaded_comics)
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
