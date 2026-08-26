use super::capture::ClipboardContent;
use super::processor::process_content;
use super::storage::store_clipboard_item;
use crate::commands::window::{emit_clipboard_updated_event, ClipboardUpdatedEventPayload};
use clipboard_rs::{
    ClipboardContent as RsClipboardContent, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, WatcherShutdown,
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

static IS_RUNNING: AtomicBool = AtomicBool::new(false);

static GENERATION: AtomicU64 = AtomicU64::new(0);

// 监听器状态
struct MonitorState {
    watcher_handle: Option<thread::JoinHandle<()>>,
    watcher_shutdown: Option<WatcherShutdown>,
    current_generation: u64,
}

static MONITOR_STATE: Lazy<Arc<Mutex<MonitorState>>> = Lazy::new(|| {
    Arc::new(Mutex::new(MonitorState {
        watcher_handle: None,
        watcher_shutdown: None,
        current_generation: 0,
    }))
});

// 上一次捕获的内容哈希集合（用于去重）
static LAST_CONTENT_HASHES: Lazy<Arc<Mutex<Vec<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

static MONITOR_PAUSE_COUNT: AtomicU64 = AtomicU64::new(0);
static MONITOR_SUPPRESS_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
static CAPTURE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static CAPTURE_PENDING: AtomicBool = AtomicBool::new(false);
fn current_time_ms() -> u64 {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as u64
}

pub struct ClipboardMonitorPauseGuard;

impl Drop for ClipboardMonitorPauseGuard {
    fn drop(&mut self) {
        MONITOR_PAUSE_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn pause_clipboard_monitor_for(duration_ms: u64) -> ClipboardMonitorPauseGuard {
    MONITOR_PAUSE_COUNT.fetch_add(1, Ordering::SeqCst);
    let until = current_time_ms().saturating_add(duration_ms);
    loop {
        let current = MONITOR_SUPPRESS_UNTIL_MS.load(Ordering::SeqCst);
        if current >= until {
            break;
        }
        if MONITOR_SUPPRESS_UNTIL_MS
            .compare_exchange(current, until, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            break;
        }
    }
    ClipboardMonitorPauseGuard
}

pub fn is_clipboard_monitor_paused() -> bool {
    MONITOR_PAUSE_COUNT.load(Ordering::SeqCst) > 0
        || current_time_ms() < MONITOR_SUPPRESS_UNTIL_MS.load(Ordering::SeqCst)
}

fn hash_clipboard_content(content: &RsClipboardContent) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();

    match content {
        RsClipboardContent::Text(text)
        | RsClipboardContent::Rtf(text)
        | RsClipboardContent::Html(text) => {
            hasher.update(text.as_bytes());
        }
        RsClipboardContent::Files(files) => {
            for file in files {
                let normalized = crate::services::normalize_path_for_hash(file);
                hasher.update(normalized.as_bytes());
                hasher.update([0u8]);
            }
        }
        RsClipboardContent::Other(format_name, data) => {
            hasher.update(format_name.as_bytes());
            hasher.update([0u8]);
            hasher.update(data);
        }
        RsClipboardContent::Image(image) => {
            use clipboard_rs::common::RustImage;
            let (width, height) = image.get_size();
            hasher.update(width.to_le_bytes());
            hasher.update(height.to_le_bytes());
        }
    }

    format!("{:x}", hasher.finalize())
}

// 清除上一次内容缓存（用于删除剪贴板项后允许重新添加相同内容）
pub fn clear_last_content_cache() {
    let mut last_hashes = LAST_CONTENT_HASHES.lock();
    last_hashes.clear();
}

// 预设粘贴后的内容哈希缓存（多格式）
pub fn set_last_hash_contents(contents: &[RsClipboardContent]) {
    let hashes = contents
        .iter()
        .map(hash_clipboard_content)
        .collect::<Vec<_>>();
    let mut last_hashes = LAST_CONTENT_HASHES.lock();
    *last_hashes = hashes;
}

// 剪贴板监听管理器
struct ClipboardMonitorManager {
    generation: u64,
}

impl ClipboardMonitorManager {
    pub fn new(generation: u64) -> Result<Self, String> {
        Ok(ClipboardMonitorManager { generation })
    }
}

impl ClipboardHandler for ClipboardMonitorManager {
    fn on_clipboard_change(&mut self) {
        if !IS_RUNNING.load(Ordering::Relaxed) {
            return;
        }

        if self.generation != GENERATION.load(Ordering::Relaxed) {
            return;
        }

        if let Err(e) = handle_clipboard_change() {
            if !e.contains("重复内容") {
                eprintln!("处理剪贴板内容失败: {}", e);
            }
        }
    }
}

pub fn start_clipboard_monitor() -> Result<(), String> {
    if IS_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }

    // 启动剪贴板来源监控
    #[cfg(target_os = "windows")]
    crate::services::system::start_clipboard_source_monitor();

    let new_generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    let mut watcher = match ClipboardWatcherContext::new() {
        Ok(watcher) => watcher,
        Err(error) => {
            IS_RUNNING.store(false, Ordering::SeqCst);
            #[cfg(target_os = "windows")]
            crate::services::system::stop_clipboard_source_monitor();
            return Err(format!("创建剪贴板监听器失败: {}", error));
        }
    };
    let watcher_shutdown = watcher.get_shutdown_channel();
    watcher.add_handler(ClipboardMonitorManager::new(new_generation)?);

    let mut state = MONITOR_STATE.lock();
    state.current_generation = new_generation;
    state.watcher_shutdown = Some(watcher_shutdown);

    let handle = thread::spawn(move || {
        watcher.start_watch();
        IS_RUNNING.store(false, Ordering::SeqCst);
    });

    state.watcher_handle = Some(handle);
    Ok(())
}

pub fn stop_clipboard_monitor() -> Result<(), String> {
    let was_running = IS_RUNNING.swap(false, Ordering::SeqCst);
    if was_running {
        // 停止剪贴板来源监控
        #[cfg(target_os = "windows")]
        crate::services::system::stop_clipboard_source_monitor();
    }

    let (watcher_shutdown, watcher_handle) = {
        let mut state = MONITOR_STATE.lock();
        (state.watcher_shutdown.take(), state.watcher_handle.take())
    };

    drop(watcher_shutdown);
    if let Some(handle) = watcher_handle {
        let _ = handle.join();
    }
    Ok(())
}

pub fn is_monitor_running() -> bool {
    IS_RUNNING.load(Ordering::Relaxed)
}

fn spawn_capture_worker_loop() {
    thread::spawn(move || {
        loop {
            CAPTURE_PENDING.store(false, Ordering::SeqCst);
            process_clipboard_change_once();

            // 处理运行期间积压的变更，合并成下一轮单次捕获，避免并发访问剪贴板。
            if !CAPTURE_PENDING.swap(false, Ordering::SeqCst) {
                break;
            }
        }

        CAPTURE_IN_FLIGHT.store(false, Ordering::SeqCst);

        // 防止退出与新事件到达之间的竞态，必要时再拉起下一轮 worker。
        if CAPTURE_PENDING.swap(false, Ordering::SeqCst) {
            if CAPTURE_IN_FLIGHT
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                spawn_capture_worker_loop();
            }
        }
    });
}

fn schedule_capture_worker() {
    if CAPTURE_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        spawn_capture_worker_loop();
    } else {
        CAPTURE_PENDING.store(true, Ordering::SeqCst);
    }
}

fn process_clipboard_change_once() {
    let contents = match ClipboardContent::capture() {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!("抓取剪贴板内容失败: {}", e);
            return;
        }
    };

    if contents.is_empty() {
        return;
    }

    let current_hashes: Vec<String> = contents.iter().map(|c| c.calculate_hash()).collect();

    {
        let last_hashes = LAST_CONTENT_HASHES.lock();
        if *last_hashes == current_hashes {
            return;
        }
    }

    let new_contents: Vec<_> = contents
        .into_iter()
        .filter(|c| {
            let hash = c.calculate_hash();
            let last_hashes = LAST_CONTENT_HASHES.lock();
            !last_hashes.contains(&hash)
        })
        .collect();

    {
        let mut last_hashes = LAST_CONTENT_HASHES.lock();
        *last_hashes = current_hashes;
    }

    if new_contents.is_empty() {
        return;
    }

    let mut any_stored = false;
    for content in new_contents {
        match process_content(content) {
            Ok(processed) => match store_clipboard_item(processed) {
                Ok(id) => {
                    any_stored = true;
                    match crate::services::database::get_clipboard_item_by_id(id) {
                        Ok(Some(mut item)) => {
                            crate::commands::clipboard::hydrate_clipboard_item_for_ui(&mut item);
                            let insert_index =
                                crate::services::database::get_clipboard_item_position(id)
                                    .ok()
                                    .flatten();
                            let total_count = crate::services::database::get_clipboard_count().ok();
                            let _ = emit_clipboard_updated(ClipboardUpdatedEventPayload {
                                kind: "created".to_string(),
                                item: Some(item.clone()),
                                insert_index,
                                total_count,
                            });
                        }
                        _ => {
                            let _ = emit_clipboard_updated(ClipboardUpdatedEventPayload {
                                kind: "unknown".to_string(),
                                item: None,
                                insert_index: None,
                                total_count: None,
                            });
                        }
                    }
                }
                Err(e) if e.contains("重复内容") || e.contains("已禁止保存图片") => {}
                Err(e) => eprintln!("存储剪贴板内容失败: {}", e),
            },
            Err(e) => eprintln!("处理剪贴板内容失败: {}", e),
        }
    }

    if any_stored {
        if let Some(app) = get_app_handle() {
            crate::services::sync_transfer::lan_notify_local_change(app, "clipboard");
        }
        crate::AppSounds::play_copy_on_success();
    }
}

fn handle_clipboard_change() -> Result<(), String> {
    if is_clipboard_monitor_paused() {
        return Ok(());
    }
    // 检查应用过滤
    crate::AppSounds::play_copy_immediate();
    let settings = crate::services::get_settings();

    if crate::services::system::is_front_app_globally_disabled(
        settings.app_filter_enabled,
        &settings.app_filter_blocklist,
        &settings.app_filter_effect,
    ) {
        return Ok(());
    }

    if !crate::services::system::is_current_app_allowed(
        settings.app_filter_enabled,
        &settings.app_filter_blocklist,
    ) {
        return Ok(());
    }

    schedule_capture_worker();

    Ok(())
}

static APP_HANDLE: Lazy<Arc<Mutex<Option<tauri::AppHandle>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub fn set_app_handle(handle: tauri::AppHandle) {
    *APP_HANDLE.lock() = Some(handle);
}

pub fn get_app_handle() -> Option<tauri::AppHandle> {
    APP_HANDLE.lock().clone()
}

fn emit_clipboard_updated(payload: ClipboardUpdatedEventPayload) -> Result<(), String> {
    let app_handle = APP_HANDLE.lock();
    let handle = app_handle.as_ref().ok_or("应用未初始化")?;

    if crate::services::low_memory::is_low_memory_mode() {
        let _ = crate::windows::tray::native_menu::update_native_menu(handle);
    }

    emit_clipboard_updated_event(handle, Some(payload))
}

// 预设哈希缓存（文本类型）
pub fn set_last_hash_text(text: &str) {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    let mut last_hashes = LAST_CONTENT_HASHES.lock();
    *last_hashes = vec![hash];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_item() -> crate::services::database::ClipboardItem {
        crate::services::database::ClipboardItem {
            id: 1,
            uuid: Some("u".to_string()),
            favorite_id: None,
            source_device_id: None,
            is_remote: false,
            content: "c".to_string(),
            html_content: None,
            content_type: "text".to_string(),
            image_id: None,
            item_order: 1,
            is_pinned: false,
            paste_count: 0,
            source_app: None,
            source_icon_hash: None,
            char_count: Some(1),
            created_at: 1,
            updated_at: 1,
        }
    }
}

// 预设哈希缓存（文件类型）
pub fn set_last_hash_files(content: &str) {
    use sha2::{Digest, Sha256};

    if let Some(json_str) = content.strip_prefix("files:") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            let mut hasher = Sha256::new();

            if let Some(files) = json["files"].as_array() {
                for file in files {
                    if let Some(path) = file["path"].as_str() {
                        let normalized = crate::services::normalize_path_for_hash(path);
                        hasher.update(normalized.as_bytes());
                    }
                }
            }

            let hash = format!("{:x}", hasher.finalize());
            let mut last_hashes = LAST_CONTENT_HASHES.lock();
            *last_hashes = vec![hash];
        }
    }
}

// 预设哈希缓存（单文件路径）
pub fn set_last_hash_file(file_path: &str) {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    let normalized = crate::services::normalize_path_for_hash(file_path);
    hasher.update(normalized.as_bytes());

    let hash = format!("{:x}", hasher.finalize());
    let mut last_hashes = LAST_CONTENT_HASHES.lock();
    *last_hashes = vec![hash];
}
