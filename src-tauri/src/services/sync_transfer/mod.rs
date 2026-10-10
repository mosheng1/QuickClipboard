pub mod device_identity;
pub mod lan;
pub mod sync_plan;
pub mod types;

pub use types::{mode_infos, SyncTransferModeInfo};

pub fn device_id() -> String {
    device_identity::device_id()
}

pub fn lan_status() -> lan::LanRuntimeStatus {
    lan::runtime::status()
}

pub async fn lan_start_http_server(app: tauri::AppHandle) -> Result<u16, String> {
    lan::http_server::start(app, Default::default()).await
}

pub async fn lan_stop_http_server() {
    lan::http_server::stop().await
}

pub fn lan_refresh_pairing_code() -> lan::PairingCodeView {
    lan::runtime::refresh_pairing_code()
}

pub fn lan_list_paired_peers() -> Vec<lan::PairedPeerInfo> {
    lan::peer_store::list_peer_infos()
}

pub fn lan_remove_paired_peer(device_id: &str) -> Result<bool, String> {
    lan::peer_store::remove_peer(device_id)
}

pub async fn lan_pair_with_peer(
    base_url: String,
    pairing_code: String,
) -> Result<lan::PairedPeerInfo, String> {
    lan::http_client::pair_with_peer(base_url, pairing_code).await
}

pub fn lan_snapshot() -> Result<lan::LanSyncSnapshot, String> {
    lan::snapshot::snapshot()
}

pub async fn lan_fetch_peer_snapshot(device_id: &str) -> Result<lan::LanSyncSnapshot, String> {
    let peer = lan::peer_store::list_peers()
        .into_iter()
        .find(|peer| peer.device_id == device_id)
        .ok_or_else(|| "未找到已配对设备".to_string())?;
    let snapshot = lan::http_client::fetch_peer_snapshot(&peer).await?;
    let _ = lan::peer_store::mark_peer_seen(device_id);
    Ok(snapshot)
}

pub async fn lan_discover_peers(timeout_ms: u64) -> Result<Vec<lan::DiscoveredLanPeer>, String> {
    lan::discovery::discover(timeout_ms).await
}

pub fn lan_auto_sync_status() -> lan::LanAutoSyncStatus {
    lan::auto_sync::status()
}

pub fn lan_update_auto_sync_settings(
    settings: lan::LanAutoSyncSettings,
) -> Result<lan::LanAutoSyncSettings, String> {
    lan::auto_sync::update_settings(settings)
}

pub fn lan_notify_local_change(app: tauri::AppHandle, reason: &'static str) {
    crate::services::webdav_sync::notify_local_change(app.clone(), reason);
    lan::auto_sync::notify_local_change(app, reason);
}

pub async fn lan_start_configured_services(app: tauri::AppHandle) {
    let settings = lan::auto_sync::settings();
    if !settings.receive_enabled {
        return;
    }
    let _ = lan::http_server::start(app, Default::default()).await;
}

pub async fn lan_pull_from_peer(
    device_id: &str,
) -> Result<crate::services::webdav_sync::SyncReport, String> {
    let report = lan::pull::pull_from_peer(device_id).await?;
    let _ = lan::peer_store::mark_peer_seen(device_id);
    Ok(report)
}

pub async fn lan_push_to_peer(
    device_id: &str,
) -> Result<crate::services::webdav_sync::SyncReport, String> {
    let report = lan::push::push_to_peer(device_id).await?;
    let _ = lan::peer_store::mark_peer_seen(device_id);
    Ok(report)
}

pub async fn lan_send_file_to_peer(
    device_id: &str,
    file_path: &str,
) -> Result<lan::FileTransferResult, String> {
    let result = lan::transfer::send_file_to_peer(device_id, file_path).await?;
    let _ = lan::peer_store::mark_peer_seen(device_id);
    Ok(result)
}

pub async fn lan_send_file_to_peer_with_progress(
    device_id: &str,
    file_path: &str,
    transfer_id: Option<String>,
    progress: Option<lan::FileTransferProgressCallback>,
) -> Result<lan::FileTransferResult, String> {
    let result =
        lan::transfer::send_file_to_peer_with_progress(device_id, file_path, transfer_id, progress)
            .await?;
    let _ = lan::peer_store::mark_peer_seen(device_id);
    Ok(result)
}
