use std::{io::Write, sync::OnceLock};

use anyhow::Context;
use notify::{RecommendedWatcher, Watcher};
use parking_lot::RwLock;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tracing::{Level, Subscriber};
use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{
    filter::{filter_fn, FilterExt, Targets},
    fmt::{layer, time::LocalTime},
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    Layer, Registry,
};

use crate::{config::Config, events::LogEvent, extensions::AnyhowErrorToStringChain};

struct LogEventWriter {
    app: AppHandle,
}

impl Write for LogEventWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let log_string = String::from_utf8_lossy(buf);
        match serde_json::from_str::<LogEvent>(&log_string) {
            Ok(log_event) => {
                let _ = log_event.emit(&self.app);
            }
            Err(err) => {
                let log_string = log_string.to_string();
                let err_msg = err.to_string();
                tracing::error!(
                    log_string,
                    err_msg,
                    "deserialize log_string to LogEvent failed"
                );
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

static RELOAD_FN: OnceLock<Box<dyn Fn() -> anyhow::Result<()> + Send + Sync>> = OnceLock::new();
static GUARD: OnceLock<parking_lot::Mutex<Option<WorkerGuard>>> = OnceLock::new();

pub fn init(app: &AppHandle) -> anyhow::Result<()> {
    let lib_module_path = module_path!();
    let lib_target = lib_module_path.split("::").next().context(format!(
        "failed to parse lib_target: lib_module_path={lib_module_path}"
    ))?;
    // filter out logs from other libraries
    let target_filter = Targets::new().with_target(lib_target, Level::TRACE);

    let (file_layer, guard) = create_file_layer(app)?;
    let (reloadable_file_layer, reload_handle) = tracing_subscriber::reload::Layer::new(file_layer);

    let console_layer = layer()
        .with_writer(std::io::stdout)
        .with_timer(LocalTime::rfc_3339())
        .with_file(true)
        .with_line_number(true);
    // send to frontend
    let log_event_writer = std::sync::Mutex::new(LogEventWriter { app: app.clone() });
    let log_event_layer = layer()
        .with_writer(log_event_writer)
        .with_timer(LocalTime::rfc_3339())
        .with_file(true)
        .with_line_number(true)
        .json()
        // filter out logs from this file (logs that failed to parse LogEvent) to avoid infinite recursion
        .with_filter(target_filter.clone().and(filter_fn(|metadata| {
            metadata.module_path() != Some(lib_module_path)
        })));

    Registry::default()
        .with(target_filter)
        .with(reloadable_file_layer)
        .with(console_layer)
        .with(log_event_layer)
        .init();

    GUARD.get_or_init(|| parking_lot::Mutex::new(guard));
    RELOAD_FN.get_or_init(move || {
        let app = app.clone();
        Box::new(move || {
            let (file_layer, guard) = create_file_layer(&app)?;
            reload_handle.reload(file_layer).context("reload failed")?;
            *GUARD.get().context("GUARD not initialized")?.lock() = guard;
            Ok(())
        })
    });
    tauri::async_runtime::spawn(file_log_watcher(app.clone()));

    Ok(())
}

pub fn reload_file_logger() -> anyhow::Result<()> {
    RELOAD_FN.get().context("RELOAD_FN not initialized")?()
}

pub fn disable_file_logger() -> anyhow::Result<()> {
    if let Some(guard) = GUARD.get().context("GUARD not initialized")?.lock().take() {
        drop(guard);
    };
    Ok(())
}

fn create_file_layer<S>(
    app: &AppHandle,
) -> anyhow::Result<(Box<dyn Layer<S> + Send + Sync>, Option<WorkerGuard>)>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let enable_file_logger = app.state::<RwLock<Config>>().read().enable_file_logger;
    // If file logging is not enabled, return a placeholder sink layer that is not created or output to a log file
    if !enable_file_logger {
        let sink_layer = layer()
            .with_writer(std::io::sink)
            .with_timer(LocalTime::rfc_3339())
            .with_ansi(false)
            .with_file(true)
            .with_line_number(true);
        return Ok((Box::new(sink_layer), None));
    }
    let logs_dir = logs_dir(app).context("get logs_dir failed")?;
    let file_appender = RollingFileAppender::builder()
        .filename_prefix("hitomi-downloader")
        .filename_suffix("log")
        .rotation(Rotation::DAILY)
        .build(&logs_dir)
        .context("create RollingFileAppender failed")?;
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = layer()
        .with_writer(non_blocking_appender)
        .with_timer(LocalTime::rfc_3339())
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true);
    Ok((Box::new(file_layer), Some(guard)))
}

async fn file_log_watcher(app: AppHandle) {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);

    let event_handler = move |res| {
        tauri::async_runtime::block_on(async {
            if let Err(err) = sender.send(res).await.map_err(anyhow::Error::from) {
                let err_title = "Failed to send log file watcher event";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
            }
        });
    };

    let mut watcher = match RecommendedWatcher::new(event_handler, notify::Config::default())
        .map_err(anyhow::Error::from)
    {
        Ok(watcher) => watcher,
        Err(err) => {
            let err_title = "Failed to create log file watcher";
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }
    };

    let logs_dir = match logs_dir(&app) {
        Ok(logs_dir) => logs_dir,
        Err(err) => {
            let err_title = "Failed to get log directory for log file watcher";
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }
    };

    if let Err(err) = watcher
        .watch(&logs_dir, notify::RecursiveMode::NonRecursive)
        .map_err(anyhow::Error::from)
    {
        let err_title = "Failed to watch log directory for log file watcher";
        let string_chain = err.to_string_chain();
        tracing::error!(err_title, message = string_chain);
        return;
    }

    while let Some(res) = receiver.recv().await {
        match res.map_err(anyhow::Error::from) {
            Ok(event) => {
                if let notify::EventKind::Remove(_) = event.kind {
                    if let Err(err) = reload_file_logger() {
                        let err_title = "Failed to reload log file";
                        let string_chain = err.to_string_chain();
                        tracing::error!(err_title, message = string_chain);
                    }
                }
            }
            Err(err) => {
                let err_title = "Failed to receive log file watcher event";
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
            }
        }
    }
}

pub fn logs_dir(app: &AppHandle) -> anyhow::Result<std::path::PathBuf> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .context("failed to get app_data_dir")?;
    Ok(app_data_dir.join("logs"))
}
