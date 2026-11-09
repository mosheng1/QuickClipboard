use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use super::storage::{
    self, ShelfFilePersisted, ShelfGeometryPersisted, ShelfPersisted, ShelfStatePersisted,
};
use super::types::{describe_path, label_for, ShelfFileInfo, ShelfSummary, STATE_CHANGED_EVENT};
use super::window::create_shelf_window;

#[derive(Clone, Debug)]
struct ShelfRecord {
    id: String,
    name: String,
}

static SHELVES: Lazy<Mutex<Vec<ShelfRecord>>> = Lazy::new(|| Mutex::new(Vec::new()));
static SHELF_MUTATION_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static STARTUP_RESTORE_STARTED: AtomicBool = AtomicBool::new(false);

const DEFAULT_SHELF_NAME_PREFIX: &str = "文件盒_";
const STARTUP_RESTORE_DELAY_MS: u64 = 800;
const STARTUP_RESTORE_INTERVAL_MS: u64 = 220;
const DROP_PROXY_RESOURCE_CLEANUP_MIN_AGE_MS: u64 = 5000;
const DROP_PROXY_RESOURCE_CLEANUP_DELAY_MS: u64 = DROP_PROXY_RESOURCE_CLEANUP_MIN_AGE_MS + 1000;

fn ensure_state_loaded() -> ShelfStatePersisted {
    storage::load()
}

fn default_shelf_name_index(name: &str) -> Option<u32> {
    let suffix = name.strip_prefix(DEFAULT_SHELF_NAME_PREFIX)?;
    let index = suffix.parse::<u32>().ok()?;
    if index == 0 || index.to_string() != suffix {
        return None;
    }
    Some(index)
}

fn next_default_shelf_name(persisted: &[ShelfPersisted], active: &[ShelfRecord]) -> String {
    let mut used = HashSet::new();
    for name in persisted
        .iter()
        .map(|item| item.name.as_str())
        .chain(active.iter().map(|item| item.name.as_str()))
    {
        if let Some(index) = default_shelf_name_index(name) {
            used.insert(index);
        }
    }

    let mut index = 1;
    while used.contains(&index) {
        index += 1;
    }
    format!("{}{}", DEFAULT_SHELF_NAME_PREFIX, index)
}

/// 创建一个新的文件盒窗口，自动分配 id 与默认名称。
pub fn open_or_create_shelf(app: &AppHandle) -> Result<ShelfSummary, String> {
    let _mutation_guard = SHELF_MUTATION_LOCK.lock();
    let state = ensure_state_loaded();

    let id = Uuid::new_v4().to_string();
    let (stagger_index, name) = {
        let guard = SHELVES.lock();
        (
            guard.len() as u32,
            next_default_shelf_name(&state.shelves, &guard),
        )
    };

    create_shelf_window(app, &id, &name, stagger_index, None, true)?;

    let record = ShelfRecord {
        id: id.clone(),
        name: name.clone(),
    };
    SHELVES.lock().push(record.clone());

    let _ = storage::upsert_shelf(ShelfPersisted {
        id: id.clone(),
        name: name.clone(),
        files: Vec::new(),
        selected_peer_ids: Vec::new(),
    });

    Ok(ShelfSummary {
        label: label_for(&id),
        id,
        name,
    })
}

/// 启动后延迟恢复，避免在 setup 阶段连续创建多个 WebView 导致首个窗口偶发不可见。
pub fn schedule_startup_restore_persisted_shelves(app: AppHandle) {
    if STARTUP_RESTORE_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(STARTUP_RESTORE_DELAY_MS)).await;

        let state = ensure_state_loaded();
        let geometries = state.geometries;
        for (index, persisted) in state.shelves.into_iter().enumerate() {
            let geometry = geometries.get(&persisted.id).cloned();
            if let Err(err) = restore_persisted_shelf_on_main_thread(
                app.clone(),
                persisted.clone(),
                index,
                geometry,
            ) {
                eprintln!(
                    "[transfer_shelf] 恢复文件盒窗口失败 {}: {}",
                    persisted.id, err
                );
            }
            tokio::time::sleep(Duration::from_millis(STARTUP_RESTORE_INTERVAL_MS)).await;
        }
    });
}

fn restore_persisted_shelf_on_main_thread(
    app: AppHandle,
    persisted: ShelfPersisted,
    index: usize,
    geometry: Option<ShelfGeometryPersisted>,
) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    let app_for_restore = app.clone();
    app.run_on_main_thread(move || {
        let result =
            restore_persisted_shelf(&app_for_restore, &persisted, index, geometry.as_ref());
        let _ = sender.send(result);
    })
    .map_err(|e| format!("调度文件盒恢复到主线程失败: {}", e))?;

    receiver
        .recv_timeout(Duration::from_secs(8))
        .map_err(|e| format!("等待文件盒恢复结果超时: {}", e))?
}

fn restore_persisted_shelf(
    app: &AppHandle,
    persisted: &ShelfPersisted,
    index: usize,
    geometry: Option<&ShelfGeometryPersisted>,
) -> Result<(), String> {
    let _mutation_guard = SHELF_MUTATION_LOCK.lock();
    if persisted.id.trim().is_empty() {
        return Err("文件盒 id 为空".to_string());
    }
    if SHELVES.lock().iter().any(|item| item.id == persisted.id) {
        return Ok(());
    }

    create_shelf_window(
        app,
        &persisted.id,
        &persisted.name,
        index as u32,
        geometry,
        false,
    )?;
    SHELVES.lock().push(ShelfRecord {
        id: persisted.id.clone(),
        name: persisted.name.clone(),
    });
    Ok(())
}

pub fn list_shelves() -> Vec<ShelfSummary> {
    SHELVES
        .lock()
        .iter()
        .map(|item| ShelfSummary {
            label: label_for(&item.id),
            id: item.id.clone(),
            name: item.name.clone(),
        })
        .collect()
}

pub fn append_files_to_recent_or_new_shelf(
    app: &AppHandle,
    paths: Vec<String>,
) -> Result<ShelfSummary, String> {
    let paths = paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("没有可加入文件盒的文件".to_string());
    }

    let summary = match SHELVES.lock().last().cloned() {
        Some(record) => ShelfSummary {
            label: label_for(&record.id),
            id: record.id,
            name: record.name,
        },
        None => open_or_create_shelf(app)?,
    };

    let mut persisted = load_shelf_state(&summary.id);
    let now = chrono::Utc::now().timestamp_millis();
    let mut existing = persisted
        .files
        .iter()
        .map(|item| item.path.clone())
        .collect::<HashSet<_>>();
    for path in paths {
        let info = describe_path(&path);
        if !info.exists || info.is_dir || !existing.insert(info.path.clone()) {
            continue;
        }
        persisted.files.push(ShelfFilePersisted {
            path: info.path,
            added_at_ms: now,
        });
    }
    storage::upsert_shelf(persisted)?;

    let _ = app.emit(
        STATE_CHANGED_EVENT,
        serde_json::json!({
            "shelfId": summary.id,
        }),
    );
    let _ = focus_shelf(app, &summary.id);
    Ok(summary)
}

pub fn append_files_to_shelf(
    app: &AppHandle,
    id: &str,
    paths: Vec<String>,
) -> Result<ShelfSummary, String> {
    let paths = paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("没有可加入文件盒的文件".to_string());
    }

    let summary = SHELVES
        .lock()
        .iter()
        .find(|item| item.id == id)
        .map(|record| ShelfSummary {
            label: label_for(&record.id),
            id: record.id.clone(),
            name: record.name.clone(),
        })
        .ok_or_else(|| format!("找不到文件盒窗口: {}", id))?;

    let mut persisted = load_shelf_state(&summary.id);
    let now = chrono::Utc::now().timestamp_millis();
    let mut existing = persisted
        .files
        .iter()
        .map(|item| item.path.clone())
        .collect::<HashSet<_>>();
    for path in paths {
        let info = describe_path(&path);
        if !info.exists || info.is_dir || !existing.insert(info.path.clone()) {
            continue;
        }
        persisted.files.push(ShelfFilePersisted {
            path: info.path,
            added_at_ms: now,
        });
    }
    storage::upsert_shelf(persisted)?;

    let _ = app.emit(
        STATE_CHANGED_EVENT,
        serde_json::json!({
            "shelfId": summary.id,
        }),
    );
    let _ = focus_shelf(app, &summary.id);
    Ok(summary)
}

pub fn focus_shelf(app: &AppHandle, id: &str) -> Result<(), String> {
    let label = label_for(id);
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("找不到文件盒窗口: {}", id))?;
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();
    window
        .set_focus()
        .map_err(|e| format!("聚焦文件盒窗口失败: {}", e))
}

pub fn rename_shelf(app: &AppHandle, id: &str, name: String) -> Result<ShelfSummary, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("文件盒名称不能为空".to_string());
    }
    let next_name = trimmed.chars().take(48).collect::<String>();

    {
        let mut guard = SHELVES.lock();
        let record = guard
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| format!("找不到文件盒窗口: {}", id))?;
        record.name = next_name.clone();
    }

    let mut persisted = load_shelf_state(id);
    persisted.name = next_name.clone();
    let _ = storage::upsert_shelf(persisted);

    let label = label_for(id);
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.set_title(&next_name);
    }

    Ok(ShelfSummary {
        label,
        id: id.to_string(),
        name: next_name,
    })
}

pub fn close_shelf(app: &AppHandle, id: &str) -> Result<(), String> {
    let label = label_for(id);
    if let Some(window) = app.get_webview_window(&label) {
        window
            .close()
            .map_err(|e| format!("关闭文件盒窗口失败: {}", e))?;
    }

    {
        let mut guard = SHELVES.lock();
        guard.retain(|item| item.id != id);
    }

    let _ = storage::remove_shelf(id);
    crate::windows::drop_proxy::schedule_cleanup_orphan_resources(
        DROP_PROXY_RESOURCE_CLEANUP_MIN_AGE_MS,
        DROP_PROXY_RESOURCE_CLEANUP_DELAY_MS,
    );
    Ok(())
}

/// 读取持久化的 shelf 暂存数据。
pub fn load_shelf_state(id: &str) -> ShelfPersisted {
    storage::load()
        .shelves
        .into_iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| ShelfPersisted {
            id: id.to_string(),
            ..Default::default()
        })
}

pub fn persisted_file_paths() -> Vec<String> {
    storage::load()
        .shelves
        .into_iter()
        .flat_map(|shelf| shelf.files.into_iter().map(|file| file.path))
        .filter(|path| !path.trim().is_empty())
        .collect()
}

/// 写入 shelf 文件队列与目标设备。
pub fn save_shelf_state(
    id: &str,
    files: Vec<ShelfFileInfo>,
    selected_peer_ids: Vec<String>,
) -> Result<(), String> {
    let mut existing = load_shelf_state(id);

    let now = chrono::Utc::now().timestamp_millis();
    let previous: std::collections::HashMap<String, i64> = existing
        .files
        .iter()
        .map(|item| (item.path.clone(), item.added_at_ms))
        .collect();

    existing.files = files
        .into_iter()
        .map(|file| ShelfFilePersisted {
            added_at_ms: previous.get(&file.path).copied().unwrap_or(now),
            path: file.path,
        })
        .collect();
    existing.selected_peer_ids = selected_peer_ids;
    if existing.name.is_empty() {
        if let Some(record) = SHELVES.lock().iter().find(|item| item.id == id) {
            existing.name = record.name.clone();
        }
    }

    storage::upsert_shelf(existing)
}
