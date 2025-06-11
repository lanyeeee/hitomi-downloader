use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use bytes::Bytes;
use parking_lot::RwLock;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    hitomi::{self},
    types::{Comic, SearchResult},
    utils::get_app_handle,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResp {
    pub ret: bool,
    pub html: String,
}

#[derive(Clone)]
pub struct HitomiClient {
    app: AppHandle,
    api_client: Arc<RwLock<ClientWithMiddleware>>,
    img_client: Arc<RwLock<ClientWithMiddleware>>,
}

impl HitomiClient {
    pub fn new(app: AppHandle) -> Self {
        let api_client = create_api_client(&app);
        let api_client = Arc::new(RwLock::new(api_client));

        let img_client = create_img_client(&app);
        let img_client = Arc::new(RwLock::new(img_client));

        Self {
            app,
            api_client,
            img_client,
        }
    }

    pub fn get_api_client() -> Arc<RwLock<ClientWithMiddleware>> {
        let app = get_app_handle();
        let hitomi_client = app.state::<HitomiClient>();
        hitomi_client.api_client.clone()
    }
    pub async fn search(
        &self,
        query: &str,
        page_num: usize,
        sort_by_popularity: bool,
    ) -> anyhow::Result<SearchResult> {
        let ids = hitomi::do_search(query.to_string(), sort_by_popularity).await?;

        let search_result = self.get_page(ids.into_iter().collect(), page_num).await?;

        Ok(search_result)
    }

    pub async fn get_page(&self, ids: Vec<i32>, page_num: usize) -> anyhow::Result<SearchResult> {
        const PAGE_SIZE: usize = 25;

        // Calculate total pages by ceiling division
        let total_page = ids.len().div_ceil(PAGE_SIZE);

        let get_gallery_info_tasks = ids
            .iter()
            .skip((page_num - 1) * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|id| async move {
                hitomi::get_gallery_info(*id)
                    .await
                    .context(format!("Failed to get gallery info for `{id}`"))
            });
        let gallery_infos = futures::future::try_join_all(get_gallery_info_tasks).await?;

        let search_result =
            SearchResult::from_gallery_infos(&self.app, gallery_infos, page_num, total_page, ids)
                .await?;

        Ok(search_result)
    }

    pub async fn get_comic(&self, id: i32) -> anyhow::Result<Comic> {
        let gallery = hitomi::get_gallery_info(id)
            .await
            .context(format!("Failed to get gallery info for `{id}`"))?;

        let comic = Comic::from_gallery_info(&self.app, gallery).await?;
        Ok(comic)
    }

    pub async fn get_img_data(&self, url: &str) -> anyhow::Result<Bytes> {
        let request = self
            .img_client
            .read()
            .get(url)
            .header("referer", "https://hitomi.la/");
        let http_resp = request.send().await?;
        // check http response status code
        let status = http_resp.status();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            return Err(anyhow!("Failed after multiple retries, try again later"));
        } else if status != StatusCode::OK {
            let body = http_resp.text().await?;
            return Err(anyhow!("Unexpected status code({status}): {body}"));
        }
        // get image data
        let img_data = http_resp.bytes().await?;
        Ok(img_data)
    }
}

fn create_api_client(_app: &AppHandle) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .base(1)
        .jitter(Jitter::Bounded)
        .build_with_total_retry_duration(Duration::from_secs(5));

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

fn create_img_client(_app: &AppHandle) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .base(1)
        .jitter(Jitter::Bounded)
        .build_with_max_retries(20);

    let client = reqwest::ClientBuilder::new().build().unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
