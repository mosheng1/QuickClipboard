use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};

use crate::services::webdav_sync::types::SyncReport;

const EVENT_SYNC_SETTINGS_KEY: &str = "sync_transfer_lan_event_sync_settings";
const LEGACY_AUTO_SYNC_SETTINGS_KEY: &str = "sync_transfer_lan_auto_sync_settings";
const FAILED_PEER_BACKOFF_STEPS_MS: [i64; 4] = [1_000, 3_000, 10_000, 30_000];

static LAST_REPORT: Lazy<Mutex<Option<LanAutoSyncReportEvent>>> = Lazy::new(|| Mutex::new(None));
static PEER_SYNC_STATES: Lazy<Mutex<HashMap<String, PeerSyncState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Default)]
struct PeerSyncState {
    running: bool,
    pending: bool,
    retry_after_ms: Option<i64>,
    retry_scheduled: bool,
    failure_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanAutoSyncSettings {
    #[serde(default, alias = "auto_push")]
    pub send_enabled: bool,
    #[serde(default, alias = "auto_pull")]
    pub receive_enabled: bool,
}

impl Default for LanAutoSyncSettings {
    fn default() -> Self {
        Self {
            send_enabled: false,
            receive_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanAutoSyncStatus {
    pub settings: LanAutoSyncSettings,
    pub last_report: Option<LanAutoSyncReportEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanAutoSyncReportEvent {
    pub mode: String,
    pub peer_device_id: String,
    pub result: SyncReport,
    pub automatic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyLanAutoSyncSettings {
    #[serde(default)]
    auto_push: bool,
    #[serde(default)]
    auto_pull: bool,
}

pub fn settings() -> LanAutoSyncSettings {
    if let Some(settings) =
        crate::services::store::get::<LanAutoSyncSettings>(EVENT_SYNC_SETTINGS_KEY)
    {
        return settings;
    }
    crate::services::store::get::<LegacyLanAutoSyncSettings>(LEGACY_AUTO_SYNC_SETTINGS_KEY)
        .map(|legacy| LanAutoSyncSettings {
            send_enabled: legacy.auto_push,
            receive_enabled: legacy.auto_pull,
        })
        .unwrap_or_default()
}

pub fn update_settings(settings: LanAutoSyncSettings) -> Result<LanAutoSyncSettings, String> {
    let normalized = LanAutoSyncSettings {
        send_enabled: settings.send_enabled,
        receive_enabled: settings.receive_enabled,
    };
    crate::services::store::set(EVENT_SYNC_SETTINGS_KEY, &normalized)?;
    Ok(normalized)
}

pub fn status() -> LanAutoSyncStatus {
    LanAutoSyncStatus {
        settings: settings(),
        last_report: LAST_REPORT.lock().clone(),
    }
}

pub fn can_receive() -> bool {
    settings().receive_enabled
}

pub fn notify_local_change(app: AppHandle, reason: &'static str) {
    let settings = settings();
    if !settings.can_send() {
        return;
    }

    let peers = super::peer_store::list_peers();
    if peers.is_empty() {
        return;
    }

    for peer in peers {
        schedule_peer_sync(app.clone(), reason, peer);
    }
}

impl LanAutoSyncSettings {
    pub fn can_send(&self) -> bool {
        self.send_enabled
    }
}

fn schedule_peer_sync(app: AppHandle, reason: &'static str, peer: super::peer_store::PairedPeer) {
    let device_id = peer.device_id.clone();
    let device_name = peer.device_name.clone();
    let base_url = peer.base_url.clone();
    let now = current_time_ms();

    {
        let mut states = PEER_SYNC_STATES.lock();
        let state = states.entry(device_id.clone()).or_default();
        if let Some(retry_at) = state.retry_after_ms {
            if retry_at > now {
                state.pending = true;
                if !state.retry_scheduled {
                    state.retry_scheduled = true;
                    schedule_retry_after(app, reason, peer, retry_at - now);
                }
                return;
            }
            state.retry_after_ms = None;
            state.retry_scheduled = false;
        }
        if state.running {
            state.pending = true;
            return;
        }
        state.running = true;
        state.pending = false;
    }

    tauri::async_runtime::spawn(async move {
        loop {
            let started = std::time::Instant::now();
            let result = super::push::push_to_peer(&device_id).await;
            match result {
                Ok(report) => {
                    if let Some(state) = PEER_SYNC_STATES.lock().get_mut(&device_id) {
                        state.retry_after_ms = None;
                        state.retry_scheduled = false;
                        state.failure_count = 0;
                    }
                    store_report(&app, reason, device_id.clone(), report);
                }
                Err(e) => {
                    eprintln!(
                        "[局域网同步] 推送失败 device_id={} name={} base_url={} 耗时={}ms 错误={}",
                        device_id,
                        device_name,
                        base_url,
                        started.elapsed().as_millis(),
                        e
                    );
                    if e == super::http_client::LAN_UNAUTHORIZED {
                        let _ = super::peer_store::remove_peer(&device_id);
                        PEER_SYNC_STATES.lock().remove(&device_id);
                        break;
                    } else {
                        mark_peer_failed(&device_id);
                    }
                }
            }

            let should_rerun = {
                let mut states = PEER_SYNC_STATES.lock();
                if let Some(state) = states.get_mut(&device_id) {
                    let backoff_active = state
                        .retry_after_ms
                        .map(|retry_at| retry_at > current_time_ms())
                        .unwrap_or(false);
                    if state.pending && settings().can_send() && !backoff_active {
                        state.pending = false;
                        true
                    } else {
                        state.running = false;
                        if !backoff_active {
                            state.pending = false;
                        } else if state.pending && !state.retry_scheduled {
                            let delay_ms = state.retry_after_ms.unwrap_or_else(current_time_ms)
                                - current_time_ms();
                            state.retry_scheduled = true;
                            schedule_retry_after(
                                app.clone(),
                                reason,
                                peer.clone(),
                                delay_ms.max(0),
                            );
                        }
                        false
                    }
                } else {
                    false
                }
            };

            if should_rerun {
                continue;
            }
            break;
        }
    });
}

fn mark_peer_failed(device_id: &str) -> i64 {
    let mut states = PEER_SYNC_STATES.lock();
    let state = states.entry(device_id.to_string()).or_default();
    state.failure_count = state.failure_count.saturating_add(1);
    let index = state.failure_count.saturating_sub(1) as usize;
    let delay_ms = FAILED_PEER_BACKOFF_STEPS_MS
        .get(index)
        .copied()
        .unwrap_or(*FAILED_PEER_BACKOFF_STEPS_MS.last().unwrap_or(&30_000));
    state.retry_after_ms = Some(current_time_ms().saturating_add(delay_ms));
    state.retry_scheduled = false;
    delay_ms
}

fn schedule_retry_after(
    app: AppHandle,
    reason: &'static str,
    peer: super::peer_store::PairedPeer,
    delay_ms: i64,
) {
    tauri::async_runtime::spawn(async move {
        let wait_ms = delay_ms.max(0) as u64;
        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
        let device_id = peer.device_id.clone();
        let should_retry = {
            let mut states = PEER_SYNC_STATES.lock();
            if let Some(state) = states.get_mut(&device_id) {
                state.retry_scheduled = false;
                let retry_due = state
                    .retry_after_ms
                    .map(|retry_at| retry_at <= current_time_ms())
                    .unwrap_or(true);
                state.pending && !state.running && retry_due
            } else {
                false
            }
        };
        if should_retry {
            schedule_peer_sync(app, reason, peer);
        }
    });
}

fn current_time_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn store_report(app: &AppHandle, mode: &'static str, peer_device_id: String, result: SyncReport) {
    let event = LanAutoSyncReportEvent {
        mode: mode.to_string(),
        peer_device_id,
        result,
        automatic: true,
    };
    *LAST_REPORT.lock() = Some(event.clone());
    let _ = app.emit("sync-transfer-lan-report", event);
}
