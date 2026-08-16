use std::sync::Arc;
use tauri::Emitter;

use crate::services;

pub const SYNC_TRANSFER_LAN_FILE_PROGRESS_EVENT: &str = "sync-transfer-lan-file-progress";

#[tauri::command]
pub fn sync_transfer_get_mode_infos(
) -> Result<Vec<services::sync_transfer::SyncTransferModeInfo>, String> {
    Ok(services::sync_transfer::mode_infos())
}

#[tauri::command]
pub fn sync_transfer_lan_get_status(
) -> Result<services::sync_transfer::lan::LanRuntimeStatus, String> {
    Ok(services::sync_transfer::lan_status())
}

#[tauri::command]
pub async fn sync_transfer_lan_start_http_server(app: tauri::AppHandle) -> Result<u16, String> {
    services::sync_transfer::lan_start_http_server(app).await
}

#[tauri::command]
pub async fn sync_transfer_lan_stop_http_server() -> Result<(), String> {
    services::sync_transfer::lan_stop_http_server().await;
    Ok(())
}

#[tauri::command]
pub async fn sync_transfer_lan_refresh_pairing_code(
    app: tauri::AppHandle,
) -> Result<services::sync_transfer::lan::PairingCodeView, String> {
    services::sync_transfer::lan_start_http_server(app).await?;
    Ok(services::sync_transfer::lan_refresh_pairing_code())
}

#[tauri::command]
pub fn sync_transfer_lan_list_paired_peers(
) -> Result<Vec<services::sync_transfer::lan::PairedPeerInfo>, String> {
    Ok(services::sync_transfer::lan_list_paired_peers())
}

#[tauri::command]
pub fn sync_transfer_lan_remove_paired_peer(device_id: String) -> Result<bool, String> {
    services::sync_transfer::lan_remove_paired_peer(&device_id)
}

#[tauri::command]
pub async fn sync_transfer_lan_pair_with_peer(
    base_url: String,
    pairing_code: String,
    app: tauri::AppHandle,
) -> Result<services::sync_transfer::lan::PairedPeerInfo, String> {
    services::sync_transfer::lan_start_http_server(app).await?;
    services::sync_transfer::lan_pair_with_peer(base_url, pairing_code).await
}

#[tauri::command]
pub async fn sync_transfer_lan_fetch_peer_snapshot(
    device_id: String,
) -> Result<services::sync_transfer::lan::LanSyncSnapshot, String> {
    services::sync_transfer::lan_fetch_peer_snapshot(&device_id).await
}

#[tauri::command]
pub fn sync_transfer_lan_get_local_snapshot(
) -> Result<services::sync_transfer::lan::LanSyncSnapshot, String> {
    services::sync_transfer::lan_snapshot()
}

#[tauri::command]
pub async fn sync_transfer_lan_discover_peers(
    timeout_ms: Option<u64>,
) -> Result<Vec<services::sync_transfer::lan::DiscoveredLanPeer>, String> {
    services::sync_transfer::lan_discover_peers(timeout_ms.unwrap_or(1200)).await
}

#[tauri::command]
pub fn sync_transfer_lan_get_auto_sync_status(
) -> Result<services::sync_transfer::lan::LanAutoSyncStatus, String> {
    Ok(services::sync_transfer::lan_auto_sync_status())
}

#[tauri::command]
pub async fn sync_transfer_lan_update_auto_sync_settings(
    settings: services::sync_transfer::lan::LanAutoSyncSettings,
    app: tauri::AppHandle,
) -> Result<services::sync_transfer::lan::LanAutoSyncSettings, String> {
    let settings = services::sync_transfer::lan_update_auto_sync_settings(settings)?;
    if settings.receive_enabled {
        services::sync_transfer::lan_start_http_server(app.clone()).await?;
    } else {
        services::sync_transfer::lan_stop_http_server().await;
    }
    Ok(settings)
}

#[tauri::command]
pub async fn sync_transfer_lan_pull_from_peer(
    device_id: String,
    app: tauri::AppHandle,
) -> Result<services::webdav_sync::SyncReport, String> {
    let report = services::sync_transfer::lan_pull_from_peer(&device_id).await?;
    if report.pulled_clipboard > 0 {
        crate::windows::main_window::mark_clipboard_refresh_pending();
    }
    if report.pulled_favorites > 0 {
        crate::windows::main_window::mark_favorites_refresh_pending();
    }
    if report.pulled_groups > 0 {
        crate::windows::main_window::mark_groups_refresh_pending();
        crate::windows::main_window::mark_favorites_refresh_pending();
    }
    if report.pulled > 0 && crate::windows::main_window::is_main_window_visible_for_updates() {
        let _ = crate::commands::window::emit_main_window_refresh_needed_event(&app);
    }
    Ok(report)
}

#[tauri::command]
pub async fn sync_transfer_lan_push_to_peer(
    device_id: String,
) -> Result<services::webdav_sync::SyncReport, String> {
    services::sync_transfer::lan_push_to_peer(&device_id).await
}

#[tauri::command]
pub async fn sync_transfer_lan_send_file_to_peer(
    device_id: String,
    file_path: String,
    transfer_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<services::sync_transfer::lan::FileTransferResult, String> {
    let progress_app = app.clone();
    let callback: services::sync_transfer::lan::FileTransferProgressCallback =
        Arc::new(move |payload| {
            let _ = progress_app.emit(SYNC_TRANSFER_LAN_FILE_PROGRESS_EVENT, payload);
        });
    services::sync_transfer::lan_send_file_to_peer_with_progress(
        &device_id,
        &file_path,
        transfer_id,
        Some(callback),
    )
    .await
}
