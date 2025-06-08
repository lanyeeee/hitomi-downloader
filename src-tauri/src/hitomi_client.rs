use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use parking_lot::RwLock;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    hitomi::{self},
    types::SearchResult,
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
}

impl HitomiClient {
    pub fn new(app: AppHandle) -> Self {
        let api_client = create_api_client(&app);
        let api_client = Arc::new(RwLock::new(api_client));

        Self { app, api_client }
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
