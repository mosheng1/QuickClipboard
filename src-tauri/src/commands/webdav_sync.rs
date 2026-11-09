use crate::services;

#[tauri::command]
pub async fn webdav_test_connection() -> Result<(), String> {
    services::webdav_sync::test_connection().await
}

#[tauri::command]
pub async fn webdav_upload() -> Result<services::webdav_sync::SyncReport, String> {
    services::webdav_sync::upload().await
}

#[tauri::command]
pub async fn webdav_download() -> Result<services::webdav_sync::SyncReport, String> {
    services::webdav_sync::download(false).await
}

#[tauri::command]
pub async fn webdav_download_all() -> Result<services::webdav_sync::SyncReport, String> {
    services::webdav_sync::download(true).await
}

#[tauri::command]
pub fn webdav_get_status() -> Result<services::webdav_sync::WebdavStatus, String> {
    Ok(services::webdav_sync::status())
}

#[tauri::command]
pub fn webdav_get_last_report(
) -> Result<Option<services::webdav_sync::sync_scheduler::WebdavSyncReportEvent>, String> {
    Ok(services::webdav_sync::sync_scheduler::get_last_report())
}

#[tauri::command]
pub fn webdav_start_scheduler() -> Result<(), String> {
    services::webdav_sync::start_scheduler();
    Ok(())
}

#[tauri::command]
pub fn webdav_stop_scheduler() -> Result<(), String> {
    services::webdav_sync::stop_scheduler();
    Ok(())
}

#[tauri::command]
pub fn webdav_has_saved_password(url: String, username: String) -> Result<bool, String> {
    if url.trim().is_empty() || username.trim().is_empty() {
        return Ok(false);
    }
    services::secure_credentials::has_webdav_password(&url, &username)
}

#[tauri::command]
pub fn webdav_set_password(
    url: String,
    username: String,
    password: String,
) -> Result<bool, String> {
    if password.is_empty() {
        if url.trim().is_empty() || username.trim().is_empty() {
            return Ok(false);
        }
        services::secure_credentials::delete_webdav_password(&url, &username)?;
        return Ok(false);
    }
    services::secure_credentials::set_webdav_password(&url, &username, &password)?;
    Ok(true)
}

#[tauri::command]
pub fn webdav_has_saved_encryption_password(
    url: String,
    username: String,
    root_path: String,
) -> Result<bool, String> {
    if url.trim().is_empty() {
        return Ok(false);
    }
    services::secure_credentials::has_webdav_encryption_password(&url, &username, &root_path)
}

#[tauri::command]
pub fn webdav_set_encryption_password(
    url: String,
    username: String,
    root_path: String,
    password: String,
) -> Result<bool, String> {
    services::webdav_sync::crypto::clear_cached_keys();
    if password.is_empty() {
        if url.trim().is_empty() {
            return Ok(false);
        }
        services::secure_credentials::delete_webdav_encryption_password(
            &url, &username, &root_path,
        )?;
        return Ok(false);
    }
    services::secure_credentials::set_webdav_encryption_password(
        &url, &username, &root_path, &password,
    )?;
    Ok(true)
}
