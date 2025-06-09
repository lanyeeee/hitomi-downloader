use parking_lot::RwLock;
use tauri::{AppHandle, State};

use crate::{
    config::Config,
    errors::{CommandError, CommandResult},
    hitomi_client::HitomiClient,
    logger,
    types::SearchResult,
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
