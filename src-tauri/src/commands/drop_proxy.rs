use serde::Deserialize;
use tauri::{AppHandle, WebviewWindow};

use crate::windows::drop_proxy::{
    self, DropProxyBounds, DropProxyCleanupPayload, DropProxyCursorPosition,
    DropProxyResourcePayload, DropProxyRouteResult,
};

const DEFAULT_CLEANUP_MIN_AGE_MS: u64 = 5000;

#[tauri::command]
pub async fn drop_proxy_ensure(app: AppHandle) -> Result<(), String> {
    drop_proxy::ensure_drop_proxy(app).await
}

#[tauri::command]
pub async fn drop_proxy_show(window: WebviewWindow, bounds: DropProxyBounds) -> Result<(), String> {
    drop_proxy::show_drop_proxy(window, bounds).await
}

#[tauri::command]
pub fn drop_proxy_hide(app: AppHandle) -> Result<(), String> {
    drop_proxy::hide_drop_proxy(&app)
}

#[tauri::command]
pub fn drop_proxy_dispose(app: AppHandle) -> Result<(), String> {
    drop_proxy::dispose_drop_proxy(&app)
}

#[tauri::command]
pub fn drop_proxy_route_paths_at_cursor(
    app: AppHandle,
    window: WebviewWindow,
    paths: Vec<String>,
    cursor_pos: DropProxyCursorPosition,
) -> Result<DropProxyRouteResult, String> {
    drop_proxy::route_paths_at_cursor(&app, window.label(), paths, cursor_pos)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropProxySaveResourceRequest {
    pub filename: String,
    pub data: Vec<u8>,
}

#[tauri::command]
pub async fn drop_proxy_save_resource(
    payload: DropProxySaveResourceRequest,
) -> Result<DropProxyResourcePayload, String> {
    drop_proxy::save_drop_resource(payload.filename, payload.data).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropProxySaveUrlRequest {
    pub filename: String,
    pub url: String,
}

#[tauri::command]
pub async fn drop_proxy_save_url(
    payload: DropProxySaveUrlRequest,
) -> Result<DropProxyResourcePayload, String> {
    drop_proxy::save_drop_url(payload.filename, payload.url).await
}

#[tauri::command]
pub async fn drop_proxy_cleanup_orphan_resources(
    min_age_ms: Option<u64>,
) -> Result<DropProxyCleanupPayload, String> {
    drop_proxy::cleanup_orphan_resources(min_age_ms.unwrap_or(DEFAULT_CLEANUP_MIN_AGE_MS)).await
}
