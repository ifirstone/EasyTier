use axum::{Json, Router, extract::State, routing::get};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::AppStateInner;

const GITHUB_REPO: &str = "EasyTier/EasyTier";
const PROXY_PREFIX: &str = "https://hub.04510451.xyz";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub prerelease: bool,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseResponse {
    pub tag_name: String,
    pub prerelease: bool,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestReleaseResponse {
    pub release: Option<ReleaseResponse>,
}

fn proxy_download_url(url: &str) -> String {
    if url.starts_with("https://github.com/") {
        format!("{}/{}", PROXY_PREFIX, url)
    } else {
        url.to_string()
    }
}

#[instrument(skip(_state))]
async fn handle_latest_release(State(_state): State<AppStateInner>) -> Result<Json<LatestReleaseResponse>, (axum::http::StatusCode, axum::Json<super::Error>)> {
    let client = Client::builder()
        .user_agent("easytier-web/1.0")
        .build()
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(super::Error { message: e.to_string() })))?;

    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, axum::Json(super::Error { message: e.to_string() })))?;

    if !resp.status().is_success() {
        return Ok(Json(LatestReleaseResponse { release: None }));
    }

    let release: Release = resp
        .json()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, axum::Json(super::Error { message: e.to_string() })))?;

    let mut assets = release.assets;
    for asset in &mut assets {
        asset.browser_download_url = proxy_download_url(&asset.browser_download_url);
    }

    Ok(Json(LatestReleaseResponse {
        release: Some(ReleaseResponse {
            tag_name: release.tag_name,
            prerelease: release.prerelease,
            assets,
        }),
    }))
}

pub fn build_route() -> Router<AppStateInner> {
    Router::new()
        .route("/api/v1/client-releases/latest", get(handle_latest_release))
}
