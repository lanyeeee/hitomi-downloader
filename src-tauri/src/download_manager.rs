use std::{
    collections::HashMap,
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::{
    sync::{watch, Semaphore, SemaphorePermit},
    task::JoinSet,
};

use crate::{
    config::Config,
    events::{DownloadSpeedEvent, DownloadTaskEvent},
    extensions::AnyhowErrorToStringChain,
    hitomi::{image_url_from_image, Ext},
    hitomi_client::HitomiClient,
    types::{Comic, DownloadFormat},
    utils::filename_filter,
};

/// Used to manage download tasks
///
/// Cloning `DownloadManager` has minimal overhead, and the performance overhead is almost negligible.
/// You can safely clone and use it in multiple threads.
///
/// Specifically:
/// - `app` is of type `AppHandle`, according to the `Tauri` documentation, the overhead of cloning it is minimal.
/// - Other fields are wrapped in `Arc`, and the cloning operation of these fields only increases the reference count.
#[derive(Clone)]
pub struct DownloadManager {
    app: AppHandle,
    comic_sem: Arc<Semaphore>,
    img_sem: Arc<Semaphore>,
    byte_per_sec: Arc<AtomicU64>,
    download_tasks: Arc<RwLock<HashMap<i32, DownloadTask>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum DownloadTaskState {
    Pending,
    Downloading,
    Paused,
    Cancelled,
    Completed,
    Failed,
}

impl DownloadManager {
    pub fn new(app: &AppHandle) -> Self {
        let manager = DownloadManager {
            app: app.clone(),
            comic_sem: Arc::new(Semaphore::new(2)),
            img_sem: Arc::new(Semaphore::new(4)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
            download_tasks: Arc::new(RwLock::new(HashMap::new())),
        };

        tauri::async_runtime::spawn(manager.clone().emit_download_speed_loop());

        manager
    }

    pub fn create_download_task(&self, comic: Comic) -> anyhow::Result<()> {
        use DownloadTaskState::{Downloading, Paused, Pending};
        let id = comic.id;
        let mut tasks = self.download_tasks.write();
        if let Some(task) = tasks.get(&id) {
            // If the task already exists and the state is `Pending`, `Downloading`, or `Paused`, a new task will not be created
            let state = *task.state_sender.borrow();
            if matches!(state, Pending | Downloading | Paused) {
                return Ok(());
            }
        }
        let task = DownloadTask::new(self.app.clone(), comic)
            .context(format!("Failed to create download task with id `{id}`",))?;
        tauri::async_runtime::spawn(task.clone().process());
        tasks.insert(id, task);
        Ok(())
    }

    pub fn pause_download_task(&self, id: i32) -> anyhow::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&id) else {
            return Err(anyhow!("Can't find download task with id `{id}`"));
        };
        task.set_state(DownloadTaskState::Paused);
        Ok(())
    }

    pub fn resume_download_task(&self, id: i32) -> anyhow::Result<()> {
        use DownloadTaskState::{Cancelled, Completed, Failed, Pending};
        let comic = {
            let tasks = self.download_tasks.read();
            let Some(task) = tasks.get(&id) else {
                return Err(anyhow!("Can't find download task with id `{id}`"));
            };
            let task_state = *task.state_sender.borrow();

            if matches!(task_state, Failed | Cancelled | Completed) {
                // If the task state is `Failed`, `Cancelled`, or `Completed`, get the comic to recreate the download task
                Some(task.comic.as_ref().clone())
            } else {
                task.set_state(Pending);
                None
            }
        };
        // If comic is not None, recreate the download task
        if let Some(comic) = comic {
            self.create_download_task(comic)
                .context(format!("Failed to recreate download task with id `{id}`"))?;
        }
        Ok(())
    }

    pub fn cancel_download_task(&self, id: i32) -> anyhow::Result<()> {
        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(&id) else {
            return Err(anyhow!("Can't find download task with id `{id}`"));
        };
        task.set_state(DownloadTaskState::Cancelled);
        Ok(())
    }

    #[allow(clippy::cast_precision_loss)]
    async fn emit_download_speed_loop(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let byte_per_sec = self.byte_per_sec.swap(0, Ordering::Relaxed);
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2} MB/s");

            let _ = DownloadSpeedEvent { speed }.emit(&self.app);
        }
    }
}

#[derive(Clone)]
struct DownloadTask {
    app: AppHandle,
    download_manager: DownloadManager,
    comic: Arc<Comic>,
    state_sender: watch::Sender<DownloadTaskState>,
    downloaded_img_count: Arc<AtomicU32>,
    total_img_count: Arc<AtomicU32>,
    download_format: DownloadFormat,
}

impl DownloadTask {
    pub fn new(app: AppHandle, mut comic: Comic) -> anyhow::Result<Self> {
        comic.update_dir_name_fields_by_fmt(&app).context(format!(
            "Failed to update directory name fields by fmt of `{}`",
            comic.title
        ))?;

        let download_manager = app.state::<DownloadManager>().inner().clone();
        let (state_sender, _) = watch::channel(DownloadTaskState::Pending);
        let download_format = app.state::<RwLock<Config>>().read().download_format;

        let task = Self {
            app,
            download_manager,
            comic: Arc::new(comic),
            state_sender,
            downloaded_img_count: Arc::new(AtomicU32::new(0)),
            total_img_count: Arc::new(AtomicU32::new(0)),
            download_format,
        };

        Ok(task)
    }

    async fn process(self) {
        self.emit_download_task_create_event();

        let download_comic_task = self.download_comic();
        tokio::pin!(download_comic_task);

        let mut state_receiver = self.state_sender.subscribe();
        state_receiver.mark_changed();
        let mut permit = None;
        loop {
            let state_is_downloading = *state_receiver.borrow() == DownloadTaskState::Downloading;
            let state_is_pending = *state_receiver.borrow() == DownloadTaskState::Pending;
            tokio::select! {
                () = &mut download_comic_task, if state_is_downloading && permit.is_some() => break,
                control_flow = self.acquire_comic_permit(&mut permit), if state_is_pending => {
                    match control_flow {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                },
                _ = state_receiver.changed() => {
                    match self.handle_state_change(&mut permit, &mut state_receiver) {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                }
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    async fn download_comic(&self) {
        let id = self.comic.id;
        let comic_title = &self.comic.title;
        // Get the url of each image of this comic
        let Some(img_urls) = self.get_img_urls().await else {
            return;
        };
        // the total number of images that need to be downloaded
        self.total_img_count
            .store(img_urls.len() as u32, Ordering::Relaxed);
        // create temporary download directory
        let Some(temp_download_dir) = self.create_temp_download_dir() else {
            return;
        };
        // get the download format from the config
        let download_format = self.app.state::<RwLock<Config>>().read().download_format;
        // image download paths
        let save_paths: Vec<PathBuf> = img_urls
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let extension = download_format.to_extension();
                temp_download_dir.join(format!("{:04}.{extension}", i + 1))
            })
            .collect();
        // delete files in the temporary download directory that do not match `config.download_format`
        if let Err(err) = self.clean_temp_download_dir(&temp_download_dir, &save_paths) {
            let err_title =
                format!("Failed to clean temporary download directory of `{comic_title}`");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return;
        }

        let mut join_set = JoinSet::new();
        // create download tasks one by one
        for (i, url) in img_urls.into_iter().enumerate() {
            let url = url.clone();
            let temp_download_dir = temp_download_dir.clone();
            let download_img_task = DownloadImgTask::new(self, url, temp_download_dir, i);
            // create download task
            join_set.spawn(download_img_task.process());
        }
        // wait for all download tasks to complete
        join_set.join_all().await;
        tracing::trace!(id, comic_title, "All images downloaded");
        // check if all images of this comic are downloaded successfully
        let downloaded_img_count = self.downloaded_img_count.load(Ordering::Relaxed);
        let total_img_count = self.total_img_count.load(Ordering::Relaxed);
        if downloaded_img_count != total_img_count {
            // not all images of this comic are downloaded successfully
            let err_title = format!("`{comic_title}` download incomplete");
            let err_msg = format!(
                "There are `{total_img_count}` images in total, but only `{downloaded_img_count}` images are downloaded"
            );
            tracing::error!(err_title, message = err_msg);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return;
        }
        // all images of this comic are downloaded successfully
        let download_dir = match self.rename_temp_download_dir(&temp_download_dir) {
            Ok(download_dir) => download_dir,
            Err(err) => {
                let err_title =
                    format!("Failed to rename temp download directory of `{comic_title}`");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);

                self.set_state(DownloadTaskState::Failed);
                self.emit_download_task_update_event();

                return;
            }
        };
        // finally, save the metadata of this comic
        if let Err(err) = self.save_metadata(&download_dir) {
            let err_title = format!("Failed to save metadata of `{comic_title}`");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }
        tracing::info!(id, comic_title, "Comic download successfully");

        self.set_state(DownloadTaskState::Completed);
        self.emit_download_task_update_event();
    }

    async fn get_img_urls(&self) -> Option<Vec<String>> {
        let id = self.comic.id;
        let comic_title = &self.comic.title;

        let get_img_urls_task = self.comic.files.iter().map(|file| {
            let ext = match self.download_format {
                DownloadFormat::Webp => Ext::Webp,
                DownloadFormat::Avif => Ext::Avif,
            };
            image_url_from_image(id, file, ext)
        });
        let img_urls = match futures::future::try_join_all(get_img_urls_task).await {
            Ok(img_urls) => img_urls,
            Err(err) => {
                let err_title = format!("Failed to get image urls of `{comic_title}`");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                return None;
            }
        };
        Some(img_urls)
    }

    fn create_temp_download_dir(&self) -> Option<PathBuf> {
        let id = self.comic.id;
        let comic_title = &self.comic.title;

        let temp_download_dir = self.comic.get_temp_download_dir(&self.app);

        if let Err(err) = std::fs::create_dir_all(&temp_download_dir).map_err(anyhow::Error::from) {
            let err_title = format!(
                "Failed to create directory of `{comic_title}`: `{}`",
                temp_download_dir.display()
            );
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);

            self.set_state(DownloadTaskState::Failed);
            self.emit_download_task_update_event();

            return None;
        };

        tracing::trace!(
            id,
            comic_title,
            "Create temporary download directory `{}` successfully",
            temp_download_dir.display()
        );

        Some(temp_download_dir)
    }

    /// Delete files in the temporary download directory that do not match `config.download_format`
    fn clean_temp_download_dir(
        &self,
        temp_download_dir: &Path,
        save_paths: &[PathBuf],
    ) -> anyhow::Result<()> {
        let id = self.comic.id;
        let comic_title = &self.comic.title;

        let entries = std::fs::read_dir(temp_download_dir).context(format!(
            "Failed to read temporary download directory `{}`",
            temp_download_dir.display()
        ))?;

        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if !save_paths.contains(&path) {
                std::fs::remove_file(&path)
                    .context(format!("Failed to delete file `{}`", path.display()))?;
            }
        }

        tracing::trace!(
            id,
            comic_title,
            "Clean temporary download directory `{}` successfully",
            temp_download_dir.display()
        );

        Ok(())
    }

    async fn acquire_comic_permit<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
    ) -> ControlFlow<()> {
        let id = self.comic.id;
        let comic_title = &self.comic.title;

        tracing::debug!(id, comic_title, "Comic is pending");

        self.emit_download_task_update_event();

        *permit = match permit.take() {
            // If there is a permit, use it directly
            Some(permit) => Some(permit),
            // If there is no permit, get the permit
            None => match self
                .download_manager
                .comic_sem
                .acquire()
                .await
                .map_err(anyhow::Error::from)
            {
                Ok(permit) => Some(permit),
                Err(err) => {
                    let err_title =
                        format!("Failed to get the permit to download the comic `{comic_title}`");
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);

                    self.set_state(DownloadTaskState::Failed);
                    self.emit_download_task_update_event();

                    return ControlFlow::Break(());
                }
            },
        };
        // If the current task state is not `Pending`, the task state will not be set to `Downloading`
        if *self.state_sender.borrow() != DownloadTaskState::Pending {
            return ControlFlow::Continue(());
        }
        // Set the task state to `Downloading`
        if let Err(err) = self
            .state_sender
            .send(DownloadTaskState::Downloading)
            .map_err(anyhow::Error::from)
        {
            let err_title = format!("Failed to send state `Downloading` to `{comic_title}`");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) -> ControlFlow<()> {
        let id = self.comic.id;
        let comic_title = &self.comic.title;

        self.emit_download_task_update_event();
        let state = *state_receiver.borrow();
        match state {
            DownloadTaskState::Paused => {
                tracing::debug!(id, comic_title, "Comic is paused");
                if let Some(permit) = permit.take() {
                    drop(permit);
                };
                ControlFlow::Continue(())
            }
            DownloadTaskState::Cancelled => {
                tracing::debug!(id, comic_title, "Comic is cancelled");
                ControlFlow::Break(())
            }
            _ => ControlFlow::Continue(()),
        }
    }

    fn set_state(&self, state: DownloadTaskState) {
        let comic_title = &self.comic.title;
        if let Err(err) = self.state_sender.send(state).map_err(anyhow::Error::from) {
            let err_title = format!("Failed to send state `{state:?}` to `{comic_title}`");
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
        }
    }

    fn emit_download_task_update_event(&self) {
        let _ = DownloadTaskEvent::Update {
            comic_id: self.comic.id,
            state: *self.state_sender.borrow(),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    fn emit_download_task_create_event(&self) {
        let _ = DownloadTaskEvent::Create {
            state: *self.state_sender.borrow(),
            comic: Box::new(self.comic.as_ref().clone()),
            downloaded_img_count: self.downloaded_img_count.load(Ordering::Relaxed),
            total_img_count: self.total_img_count.load(Ordering::Relaxed),
        }
        .emit(&self.app);
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn save_metadata(&self, download_dir: &Path) -> anyhow::Result<()> {
        let mut comic = self.comic.as_ref().clone();
        // Set the `is_downloaded` field of all comics to `None`, so that the `is_downloaded` field is ignored during serialization
        comic.is_downloaded = None;
        // Set the `dir_name` field of all comics to empty, so that the `dir_name` field is ignored during serialization
        comic.dir_name = String::new();

        let comic_title = &comic.title;
        let comic_json = serde_json::to_string_pretty(&comic).context(format!(
            "Failed to save metadata of `{comic_title}`, Failed to serialize Comic to json"
        ))?;

        let metadata_path = download_dir.join("metadata.json");

        std::fs::write(&metadata_path, comic_json).context(format!(
            "Failed to save metadata of `{comic_title}`, Failed to write json to `{}`",
            metadata_path.display()
        ))?;

        Ok(())
    }

    /// Rename the temporary download directory to the download directory, return the download directory
    fn rename_temp_download_dir(&self, temp_download_dir: &Path) -> anyhow::Result<PathBuf> {
        let id = self.comic.id;
        let comic_title = &self.comic.title;

        let download_dir = self.comic.get_download_dir(&self.app);

        if download_dir.exists() {
            std::fs::remove_dir_all(&download_dir).context(format!(
                "Failed to delete directory `{}`",
                download_dir.display()
            ))?;
        }

        std::fs::rename(temp_download_dir, &download_dir).context(format!(
            "Failed to rename `{}` to `{}`",
            temp_download_dir.display(),
            download_dir.display()
        ))?;

        tracing::trace!(
            id,
            comic_title,
            "Rename temp download directory of `{}` successfully",
            temp_download_dir.display()
        );

        Ok(download_dir)
    }
}

#[derive(Clone)]
struct DownloadImgTask {
    app: AppHandle,
    download_manager: DownloadManager,
    download_task: DownloadTask,
    url: String,
    temp_download_dir: PathBuf,
    index: usize,
}

impl DownloadImgTask {
    pub fn new(
        download_task: &DownloadTask,
        url: String,
        temp_download_dir: PathBuf,
        index: usize,
    ) -> Self {
        Self {
            app: download_task.app.clone(),
            download_manager: download_task.download_manager.clone(),
            download_task: download_task.clone(),
            url,
            temp_download_dir,
            index,
        }
    }

    async fn process(self) {
        let download_img_task = self.download_img();
        tokio::pin!(download_img_task);

        let mut state_receiver = self.download_task.state_sender.subscribe();
        state_receiver.mark_changed();
        let mut permit = None;

        loop {
            let state_is_downloading = *state_receiver.borrow() == DownloadTaskState::Downloading;
            tokio::select! {
                () = &mut download_img_task, if state_is_downloading && permit.is_some() => break,
                control_flow = self.acquire_img_permit(&mut permit), if state_is_downloading && permit.is_none() => {
                    match control_flow {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                },
                _ = state_receiver.changed() => {
                    match self.handle_state_change(&mut permit, &mut state_receiver) {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => break,
                    }
                }
            }
        }
    }

    async fn download_img(&self) {
        let url = &self.url;
        let id = self.download_task.comic.id;
        let comic_title = &self.download_task.comic.title;

        tracing::trace!(id, comic_title, url, "Start downloading images");

        let extension = self.download_task.download_format.to_extension();
        let save_path = self
            .temp_download_dir
            .join(format!("{:04}.{extension}", self.index + 1));
        if save_path.exists() {
            // If the image already exists, skip it
            self.download_task
                .downloaded_img_count
                .fetch_add(1, Ordering::Relaxed);

            self.download_task.emit_download_task_update_event();

            tracing::trace!(id, comic_title, url, "Image already exists, skip download");
            return;
        }
        // download image
        let img_data = match self.hitomi_client().get_img_data(url).await {
            Ok(img_data) => img_data,
            Err(err) => {
                let err_title = format!("Failed to download image `{url}`");
                let string_chain = err.to_string_chain();
                tracing::error!(err_title, message = string_chain);
                return;
            }
        };

        tracing::trace!(id, comic_title, url, "Image downloaded to memory");
        // save image
        if let Err(err) = std::fs::write(&save_path, &img_data).map_err(anyhow::Error::from) {
            let err_title = format!("Failed to save image `{}`", save_path.display());
            let string_chain = err.to_string_chain();
            tracing::error!(err_title, message = string_chain);
            return;
        }

        tracing::trace!(
            id,
            url,
            comic_title,
            "Image successfully saved to `{}`",
            save_path.display()
        );
        // Record the number of bytes downloaded
        self.download_manager
            .byte_per_sec
            .fetch_add(img_data.len() as u64, Ordering::Relaxed);

        self.download_task
            .downloaded_img_count
            .fetch_add(1, Ordering::Relaxed);

        self.download_task.emit_download_task_update_event();
    }

    async fn acquire_img_permit<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
    ) -> ControlFlow<()> {
        let url = &self.url;
        let id = self.download_task.comic.id;
        let comic_title = &self.download_task.comic.title;

        tracing::trace!(id, comic_title, url, "Image is pending");

        *permit = match permit.take() {
            // If there is a permit, use it directly
            Some(permit) => Some(permit),
            // If there is no permit, get the permit
            None => match self
                .download_manager
                .img_sem
                .acquire()
                .await
                .map_err(anyhow::Error::from)
            {
                Ok(permit) => Some(permit),
                Err(err) => {
                    let err_title =
                        format!("Failed to get the permit to download the image `{comic_title}`");
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);
                    return ControlFlow::Break(());
                }
            },
        };
        ControlFlow::Continue(())
    }

    fn handle_state_change<'a>(
        &'a self,
        permit: &mut Option<SemaphorePermit<'a>>,
        state_receiver: &mut watch::Receiver<DownloadTaskState>,
    ) -> ControlFlow<()> {
        let url = &self.url;
        let id = self.download_task.comic.id;
        let comic_title = &self.download_task.comic.title;

        let state = *state_receiver.borrow();
        match state {
            DownloadTaskState::Paused => {
                tracing::trace!(id, comic_title, url, "Image is paused");
                if let Some(permit) = permit.take() {
                    drop(permit);
                };
                ControlFlow::Continue(())
            }
            DownloadTaskState::Cancelled => {
                tracing::trace!(id, comic_title, url, "Image is cancelled");
                ControlFlow::Break(())
            }
            _ => ControlFlow::Continue(()),
        }
    }

    fn hitomi_client(&self) -> HitomiClient {
        self.app.state::<HitomiClient>().inner().clone()
    }
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize, Type)]
pub struct DirNameFmtParams {
    id: i32,
    title: String,
    language: String,
    language_localname: String,
    artists: String,
}

impl Comic {
    /// Update the `dir_name` fields based on the fmt
    fn update_dir_name_fields_by_fmt(&mut self, app: &AppHandle) -> anyhow::Result<()> {
        let comic_title = &self.title;

        let fmt_params = DirNameFmtParams {
            id: self.id,
            title: self.title.clone(),
            language: self.language.clone(),
            language_localname: self.language_localname.clone(),
            artists: self.artists.join(", "),
        };
        self.dir_name = Comic::get_dir_name_by_fmt(app, &fmt_params).context(format!(
            "Failed to get directory name by fmt of `{comic_title}`"
        ))?;

        Ok(())
    }

    fn get_dir_name_by_fmt(
        app: &AppHandle,
        fmt_params: &DirNameFmtParams,
    ) -> anyhow::Result<String> {
        use strfmt::strfmt;

        let fmt = app.state::<RwLock<Config>>().read().dir_name_fmt.clone();

        let json_value = serde_json::to_value(fmt_params)
            .context("Failed to convert DirNameFmtParams to serde_json::Value")?;

        let json_map = json_value
            .as_object()
            .context("DirNameFmtParams is not a JSON object")?;

        let vars: HashMap<String, String> = json_map
            .into_iter()
            .map(|(k, v)| {
                let key = k.clone();
                let value = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (key, value)
            })
            .collect();

        let dir_name = strfmt(&fmt, &vars).context("Failed to format directory name")?;
        let dir_name = filename_filter(&dir_name);

        Ok(dir_name)
    }
}
