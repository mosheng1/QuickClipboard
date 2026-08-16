pub mod chunk_manager;
pub mod cloud_files;
pub mod crypto;
pub mod downloader;
pub mod groups_sync;
pub mod index_manager;
pub mod local_state;
pub mod sync_scheduler;
pub mod tombstones_sync;
pub mod types;
pub mod uploader;
pub mod webdav_client;

pub use types::{SyncReport, WebdavStatus};

use types::WebdavConfig;
use webdav_client::WebdavClient;

pub async fn test_connection() -> Result<(), String> {
    let client = build_client().await?;
    client.test_connection().await
}

pub async fn upload() -> Result<SyncReport, String> {
    let report = sync_scheduler::upload_selected_parts(false)
        .await?
        .unwrap_or_default();
    Ok(sync_scheduler::store_manual_report("push", report))
}

pub(super) async fn download_raw(force_download: bool) -> Result<SyncReport, String> {
    let client = build_client().await?;
    let device_id = crate::services::sync_transfer::device_id();
    downloader::download_all(&client, &device_id, force_download).await
}

pub async fn download(force_download: bool) -> Result<SyncReport, String> {
    let report = download_raw(force_download).await?;
    Ok(sync_scheduler::store_manual_report("pull", report))
}

pub async fn upload_parts(
    upload_clipboard: bool,
    upload_favorites: bool,
    upload_groups: bool,
    upload_tombstones: bool,
) -> Result<SyncReport, String> {
    let client = build_client().await?;
    let device_id = crate::services::sync_transfer::device_id();
    uploader::upload_parts(
        &client,
        &device_id,
        upload_clipboard,
        upload_favorites,
        upload_groups,
        upload_tombstones,
    )
    .await
}

pub async fn upload_cloud_files_with_progress(
    requests: Vec<cloud_files::CloudFileUploadRequest>,
) -> Result<Vec<cloud_files::CloudFileUploadBatchItem>, String> {
    let client = build_client().await?;
    cloud_files::upload_files_with_progress(&client, requests).await
}

pub async fn list_cloud_files() -> Result<Vec<cloud_files::CloudFileListItem>, String> {
    let client = build_client().await?;
    cloud_files::list_files(&client).await
}

pub async fn download_cloud_file(
    file_id: &str,
) -> Result<cloud_files::CloudFileDownloadResult, String> {
    let client = build_client().await?;
    cloud_files::download_file(&client, file_id).await
}

pub async fn delete_cloud_file(file_id: &str) -> Result<(), String> {
    let client = build_client().await?;
    cloud_files::delete_file(&client, file_id).await
}

pub fn status() -> WebdavStatus {
    sync_scheduler::status()
}

pub fn start_scheduler() {
    sync_scheduler::start();
}

pub fn stop_scheduler() {
    sync_scheduler::stop();
}

pub fn notify_local_change(app: tauri::AppHandle, reason: &'static str) {
    sync_scheduler::notify_local_change(app, reason);
}

pub fn notify_main_window_shown(app: tauri::AppHandle) {
    sync_scheduler::notify_main_window_shown(app);
}

async fn build_client() -> Result<WebdavClient, String> {
    let settings = crate::services::get_settings();
    let webdav_url = settings.webdav_url.trim().to_string();
    let webdav_username = settings.webdav_username.trim().to_string();
    let webdav_root_path = if settings.webdav_root_path.trim().is_empty() {
        "quickclipboard".to_string()
    } else {
        settings.webdav_root_path.clone()
    };
    let password = if settings.webdav_username.trim().is_empty() {
        String::new()
    } else {
        crate::services::secure_credentials::get_webdav_password(&webdav_url, &webdav_username)?
            .ok_or_else(|| "请先在设置中保存 WebDAV 密码".to_string())?
    };
    let encryption_password = crate::services::secure_credentials::get_webdav_encryption_password(
        &webdav_url,
        &webdav_username,
        &webdav_root_path,
    )?
    .ok_or_else(|| "请先设置 WebDAV 云端加密密码".to_string())?;
    let config = WebdavConfig {
        url: webdav_url,
        username: webdav_username,
        password,
        root_path: webdav_root_path,
    };
    let mut client = WebdavClient::new(config)?;
    client.enable_encryption(&encryption_password).await?;
    Ok(client)
}
