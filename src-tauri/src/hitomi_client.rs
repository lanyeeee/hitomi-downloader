use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use bytes::Bytes;
use parking_lot::RwLock;
use reqwest::{Client, StatusCode};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, Jitter, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    config::Config,
    extensions::AnyhowErrorToStringChain,
    hitomi::{self, Suggestion},
    types::{Comic, ProxyMode, SearchResult},
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
    cover_client: Arc<RwLock<Client>>,
}

impl HitomiClient {
    pub fn new(app: AppHandle) -> Self {
        let api_client = create_api_client(&app);
        let api_client = Arc::new(RwLock::new(api_client));

        let img_client = create_img_client(&app);
        let img_client = Arc::new(RwLock::new(img_client));

        let cover_client = create_cover_client(&app);
        let cover_client = Arc::new(RwLock::new(cover_client));

        Self {
            app,
            api_client,
            img_client,
            cover_client,
        }
    }

    pub fn get_api_client() -> Arc<RwLock<ClientWithMiddleware>> {
        let app = get_app_handle();
        let hitomi_client = app.state::<HitomiClient>();
        hitomi_client.api_client.clone()
    }

    pub fn reload_client(&self) {
        let api_client = create_api_client(&self.app);
        *self.api_client.write() = api_client;

        let img_client = create_img_client(&self.app);
        *self.img_client.write() = img_client;

        let cover_client = create_cover_client(&self.app);
        *self.cover_client.write() = cover_client;
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

    pub async fn get_search_suggestions(&self, query: &str) -> anyhow::Result<Vec<Suggestion>> {
        let suggestion = hitomi::get_suggestions_for_query(query).await?;
        Ok(suggestion)
    }

    pub async fn get_cover_data(&self, cover_url: &str) -> anyhow::Result<Bytes> {
        let request = self
            .cover_client
            .read()
            .get(cover_url)
            .header("referer", "https://hitomi.la/");
        let http_resp = request.send().await?;
        // check http response status code
        let status = http_resp.status();
        if status != StatusCode::OK {
            let body = http_resp.text().await?;
            return Err(anyhow!("Unexpected status code({status}): {body}"));
        }
        let cover_data = http_resp.bytes().await?;
        Ok(cover_data)
    }
}

fn create_api_client(app: &AppHandle) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .base(1)
        .jitter(Jitter::Bounded)
        .build_with_total_retry_duration(Duration::from_secs(5));

    let client = reqwest::ClientBuilder::new()
        .set_proxy(app, "api_client")
        .timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

fn create_img_client(app: &AppHandle) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .base(1)
        .jitter(Jitter::Bounded)
        .build_with_max_retries(20);

    let client = reqwest::ClientBuilder::new()
        .set_proxy(app, "img_client")
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

fn create_cover_client(app: &AppHandle) -> Client {
    reqwest::ClientBuilder::new()
        .set_proxy(app, "cover_client")
        .build()
        .unwrap()
}

trait ClientBuilderExt {
    fn set_proxy(self, app: &AppHandle, client_name: &str) -> Self;
}

impl ClientBuilderExt for reqwest::ClientBuilder {
    fn set_proxy(self, app: &AppHandle, client_name: &str) -> reqwest::ClientBuilder {
        let proxy_mode = app.state::<RwLock<Config>>().read().proxy_mode;
        match proxy_mode {
            ProxyMode::System => self,
            ProxyMode::NoProxy => self.no_proxy(),
            ProxyMode::Custom => {
                let config = app.state::<RwLock<Config>>();
                let config = config.read();
                let proxy_host = &config.proxy_host;
                let proxy_port = &config.proxy_port;
                let proxy_url = format!("http://{proxy_host}:{proxy_port}");

                match reqwest::Proxy::all(&proxy_url).map_err(anyhow::Error::from) {
                    Ok(proxy) => self.proxy(proxy),
                    Err(err) => {
                        let err_title =
                            format!("{client_name} failed to set proxy `{proxy_url}`, use system proxy instead");
                        let string_chain = err.to_string_chain();
                        tracing::error!(err_title, message = string_chain);
                        self
                    }
                }
            }
        }
    }
}
