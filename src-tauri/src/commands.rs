use parking_lot::RwLock;
use tauri::{AppHandle, State};

use crate::{
    config::Config,
    download_manager::DownloadManager,
    errors::{CommandError, CommandResult},
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
    config_state: State<RwLock<Config>>,
    config: Config,
) -> CommandResult<()> {
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
