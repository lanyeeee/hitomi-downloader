mod commands;
mod config;
mod download_manager;
mod errors;
mod events;
mod export;
mod extensions;
mod hitomi;
mod hitomi_client;
mod logger;
mod types;
mod utils;

use anyhow::Context;
use config::Config;
use download_manager::DownloadManager;
use events::{DownloadSpeedEvent, DownloadTaskEvent, ExportCbzEvent, ExportPdfEvent, LogEvent};
use hitomi_client::HitomiClient;
use parking_lot::RwLock;
use tauri::{Manager, Wry};

use crate::commands::*;

fn generate_context() -> tauri::Context<Wry> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri_specta::Builder::<Wry>::new()
        .commands(tauri_specta::collect_commands![
            greet,
            get_config,
            save_config,
            search,
            get_page,
            get_comic,
            create_download_task,
            pause_download_task,
            resume_download_task,
            cancel_download_task,
            get_downloaded_comics,
            export_pdf,
            export_cbz,
            get_search_suggestions,
            get_logs_dir_size,
            show_path_in_file_manager,
            get_cover_data,
            get_synced_comic,
        ])
        .events(tauri_specta::collect_events![
            LogEvent,
            DownloadTaskEvent,
            DownloadSpeedEvent,
            ExportPdfEvent,
            ExportCbzEvent,
        ]);

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .formatter(specta_typescript::formatter::prettier)
                .header("// @ts-nocheck"), // disable typescript checks
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            utils::APP_HANDLE.get_or_init(|| app.handle().clone());

            builder.mount_events(app);

            let app_data_dir = app
                .path()
                .app_data_dir()
                .context("get app_data_dir failed")?;

            std::fs::create_dir_all(&app_data_dir).context(format!(
                "create app_data_dir `{}` failed",
                app_data_dir.display()
            ))?;

            let config = RwLock::new(Config::new(app.handle())?);
            app.manage(config);

            let hitomi_client = HitomiClient::new(app.handle().clone());
            app.manage(hitomi_client);

            let download_manager = DownloadManager::new(app.handle());
            app.manage(download_manager);

            logger::init(app.handle())?;

            Ok(())
        })
        .run(generate_context())
        .expect("error while running tauri application");
}
