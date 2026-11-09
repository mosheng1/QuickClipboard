use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager};
use tokio::task::JoinSet;

use super::manager::{
    append_files_to_shelf, close_shelf, focus_shelf, list_shelves, load_shelf_state,
    open_or_create_shelf, rename_shelf, save_shelf_state,
};
use super::storage::{self, ShelfGeometryPersisted};
use super::types::{
    describe_path, label_for, ShelfCloudUploadTarget, ShelfFileInfo, ShelfFileProgress,
    ShelfSendError, ShelfSendTarget, ShelfSendTaskPayload, ShelfStateSnapshot, ShelfSummary,
    TASK_PROGRESS_EVENT,
};
use super::window::{apply_shelf_geometry, resolve_shelf_geometry};

const FILE_SEND_CONCURRENCY: usize = 4;

#[derive(Clone)]
struct SendTargetContext {
    target: ShelfSendTarget,
    file_size: u64,
    file_name: Option<String>,
}

#[derive(Default)]
struct SendProgressState {
    done: usize,
    failed: usize,
    completed_bytes: u64,
    active_transfers: HashMap<String, ActiveTransferProgress>,
    file_progresses: HashMap<String, FileProgressState>,
    errors: Vec<ShelfSendError>,
}

#[derive(Clone)]
struct ActiveTransferProgress {
    path: String,
    sent_bytes: u64,
}

#[derive(Clone, Default)]
struct FileProgressState {
    completed_bytes: u64,
    total_bytes: u64,
    total: usize,
    done: usize,
    failed: usize,
}

#[tauri::command]
pub async fn transfer_shelf_create(app: AppHandle) -> Result<ShelfSummary, String> {
    open_or_create_shelf(&app)
}

#[tauri::command]
pub fn transfer_shelf_list() -> Vec<ShelfSummary> {
    list_shelves()
}

#[tauri::command]
pub fn transfer_shelf_focus(app: AppHandle, id: String) -> Result<(), String> {
    focus_shelf(&app, &id)
}

#[tauri::command]
pub fn transfer_shelf_rename(
    app: AppHandle,
    id: String,
    name: String,
) -> Result<ShelfSummary, String> {
    rename_shelf(&app, &id, name)
}

#[tauri::command]
pub fn transfer_shelf_close(app: AppHandle, id: String) -> Result<(), String> {
    close_shelf(&app, &id)
}

#[tauri::command]
pub fn transfer_shelf_describe_paths(paths: Vec<String>) -> Vec<ShelfFileInfo> {
    paths.into_iter().map(|path| describe_path(&path)).collect()
}

#[tauri::command]
pub fn transfer_shelf_add_paths(
    app: AppHandle,
    id: String,
    paths: Vec<String>,
) -> Result<ShelfSummary, String> {
    append_files_to_shelf(&app, &id, paths)
}

#[tauri::command]
pub async fn transfer_shelf_send(
    app: AppHandle,
    id: String,
    targets: Vec<ShelfSendTarget>,
) -> Result<ShelfSendTaskPayload, String> {
    if targets.is_empty() {
        return Err("没有可发送的文件".to_string());
    }

    let contexts = targets
        .into_iter()
        .map(|target| {
            let file_size = std::fs::metadata(&target.path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            let file_name = std::path::Path::new(&target.path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string());
            SendTargetContext {
                target,
                file_size,
                file_name,
            }
        })
        .collect::<Vec<_>>();
    let total = contexts.len();
    let total_bytes = contexts.iter().map(|item| item.file_size).sum::<u64>();
    let state = Arc::new(Mutex::new(SendProgressState {
        file_progresses: initial_file_progresses(&contexts),
        ..SendProgressState::default()
    }));

    emit_state_progress(
        &app,
        &id,
        "sending",
        total,
        total_bytes,
        state.clone(),
        None,
        None,
    );

    let mut pending = contexts.into_iter();
    let mut tasks = JoinSet::new();
    for _ in 0..FILE_SEND_CONCURRENCY {
        let Some(context) = pending.next() else {
            break;
        };
        spawn_send_target(
            &mut tasks,
            app.clone(),
            id.clone(),
            total,
            total_bytes,
            state.clone(),
            context,
        );
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            if let Ok(mut guard) = state.lock() {
                guard.failed = guard.failed.saturating_add(1);
                guard.errors.push(ShelfSendError {
                    peer_id: String::new(),
                    path: String::new(),
                    message: format!("文件发送任务异常: {}", error),
                });
            }
        }
        emit_state_progress(
            &app,
            &id,
            "sending",
            total,
            total_bytes,
            state.clone(),
            None,
            None,
        );
        if let Some(context) = pending.next() {
            spawn_send_target(
                &mut tasks,
                app.clone(),
                id.clone(),
                total,
                total_bytes,
                state.clone(),
                context,
            );
        }
    }

    let (done, failed, sent_bytes, errors, file_progresses) = match state.lock() {
        Ok(guard) => (
            guard.done,
            guard.failed,
            guard.completed_bytes.saturating_add(
                guard
                    .active_transfers
                    .values()
                    .map(|item| item.sent_bytes)
                    .sum::<u64>(),
            ),
            guard.errors.clone(),
            file_progress_payloads(&guard),
        ),
        Err(_) => (
            0,
            total,
            0,
            vec![ShelfSendError {
                peer_id: String::new(),
                path: String::new(),
                message: "文件发送状态异常".to_string(),
            }],
            Vec::new(),
        ),
    };
    let status = if failed > 0 { "failed" } else { "done" };
    let payload = ShelfSendTaskPayload {
        shelf_id: id.clone(),
        status: status.to_string(),
        total,
        done,
        failed,
        sent_bytes: sent_bytes.min(total_bytes),
        total_bytes,
        current_path: None,
        current_file_name: None,
        errors,
        file_progresses,
    };
    let _ = app.emit(TASK_PROGRESS_EVENT, payload.clone());
    Ok(payload)
}

#[tauri::command]
pub async fn transfer_shelf_upload_cloud(
    app: AppHandle,
    id: String,
    targets: Vec<ShelfCloudUploadTarget>,
) -> Result<ShelfSendTaskPayload, String> {
    if targets.is_empty() {
        return Err("没有可上传的文件".to_string());
    }

    let contexts = targets
        .into_iter()
        .map(|target| {
            let file_size = std::fs::metadata(&target.path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            let file_name = std::path::Path::new(&target.path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string());
            SendTargetContext {
                target: ShelfSendTarget {
                    peer_id: "cloud".to_string(),
                    path: target.path,
                },
                file_size,
                file_name,
            }
        })
        .collect::<Vec<_>>();
    let total = contexts.len();
    let total_bytes = contexts.iter().map(|item| item.file_size).sum::<u64>();
    let state = Arc::new(Mutex::new(SendProgressState {
        file_progresses: initial_file_progresses(&contexts),
        ..SendProgressState::default()
    }));

    emit_state_progress(
        &app,
        &id,
        "uploading",
        total,
        total_bytes,
        state.clone(),
        None,
        None,
    );

    upload_cloud_targets(
        app.clone(),
        id.clone(),
        total,
        total_bytes,
        state.clone(),
        contexts,
    )
    .await;

    let (done, failed, sent_bytes, errors, file_progresses) = match state.lock() {
        Ok(guard) => (
            guard.done,
            guard.failed,
            guard.completed_bytes.saturating_add(
                guard
                    .active_transfers
                    .values()
                    .map(|item| item.sent_bytes)
                    .sum::<u64>(),
            ),
            guard.errors.clone(),
            file_progress_payloads(&guard),
        ),
        Err(_) => (
            0,
            total,
            0,
            vec![ShelfSendError {
                peer_id: "cloud".to_string(),
                path: String::new(),
                message: "云端上传状态异常".to_string(),
            }],
            Vec::new(),
        ),
    };
    let status = if failed > 0 { "failed" } else { "done" };
    let payload = ShelfSendTaskPayload {
        shelf_id: id.clone(),
        status: status.to_string(),
        total,
        done,
        failed,
        sent_bytes: sent_bytes.min(total_bytes),
        total_bytes,
        current_path: None,
        current_file_name: None,
        errors,
        file_progresses,
    };
    let _ = app.emit(TASK_PROGRESS_EVENT, payload.clone());
    Ok(payload)
}

fn spawn_send_target(
    tasks: &mut JoinSet<()>,
    app: AppHandle,
    id: String,
    total: usize,
    total_bytes: u64,
    state: Arc<Mutex<SendProgressState>>,
    context: SendTargetContext,
) {
    tasks.spawn(async move {
        let current_path = context.target.path.clone();
        let peer_id = context.target.peer_id.clone();
        let current_file_name = context.file_name.clone().or_else(|| {
            std::path::Path::new(&current_path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        });
        let progress_app = app.clone();
        let progress_id = id.clone();
        let progress_path = current_path.clone();
        let progress_name = current_file_name.clone();
        let progress_state = state.clone();
        let callback: crate::services::sync_transfer::lan::FileTransferProgressCallback =
            Arc::new(move |progress| {
                if let Ok(mut guard) = progress_state.lock() {
                    let sent_bytes = guard
                        .active_transfers
                        .get(&progress.transfer_id)
                        .map(|item| item.sent_bytes)
                        .unwrap_or(0)
                        .max(progress.sent_bytes);
                    guard.active_transfers.insert(
                        progress.transfer_id.clone(),
                        ActiveTransferProgress {
                            path: progress.file_path.clone(),
                            sent_bytes,
                        },
                    );
                }
                emit_state_progress(
                    &progress_app,
                    &progress_id,
                    &progress.status,
                    total,
                    total_bytes,
                    progress_state.clone(),
                    Some(progress_path.clone()),
                    progress_name.clone(),
                );
            });
        let transfer_id = format!("{}:{}:{}", id, peer_id, current_path);
        match crate::services::sync_transfer::lan_send_file_to_peer_with_progress(
            &peer_id,
            &current_path,
            Some(transfer_id.clone()),
            Some(callback),
        )
        .await
        {
            Ok(_) => {
                if let Ok(mut guard) = state.lock() {
                    guard.done = guard.done.saturating_add(1);
                    guard.completed_bytes = guard.completed_bytes.saturating_add(context.file_size);
                    guard.active_transfers.remove(&transfer_id);
                    if let Some(file_progress) = guard.file_progresses.get_mut(&current_path) {
                        file_progress.done = file_progress.done.saturating_add(1);
                        file_progress.completed_bytes = file_progress
                            .completed_bytes
                            .saturating_add(context.file_size);
                    }
                }
            }
            Err(error) => {
                if let Ok(mut guard) = state.lock() {
                    let sent_bytes = guard
                        .active_transfers
                        .remove(&transfer_id)
                        .map(|item| item.sent_bytes)
                        .unwrap_or(0)
                        .min(context.file_size);
                    guard.failed = guard.failed.saturating_add(1);
                    guard.completed_bytes = guard.completed_bytes.saturating_add(sent_bytes);
                    if let Some(file_progress) = guard.file_progresses.get_mut(&current_path) {
                        file_progress.failed = file_progress.failed.saturating_add(1);
                        file_progress.completed_bytes =
                            file_progress.completed_bytes.saturating_add(sent_bytes);
                    }
                    guard.errors.push(ShelfSendError {
                        peer_id,
                        path: current_path,
                        message: error,
                    });
                }
            }
        }
        emit_state_progress(&app, &id, "sending", total, total_bytes, state, None, None);
    });
}

async fn upload_cloud_targets(
    app: AppHandle,
    id: String,
    total: usize,
    total_bytes: u64,
    state: Arc<Mutex<SendProgressState>>,
    contexts: Vec<SendTargetContext>,
) {
    let mut requests = Vec::with_capacity(contexts.len());
    for context in contexts {
        let current_path = context.target.path.clone();
        let current_file_name = context.file_name.clone().or_else(|| {
            std::path::Path::new(&current_path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        });
        let transfer_id = format!("{}:cloud:{}", id, current_path);
        let progress_app = app.clone();
        let progress_id = id.clone();
        let progress_path = current_path.clone();
        let progress_name = current_file_name.clone();
        let progress_state = state.clone();
        let callback: crate::services::webdav_sync::cloud_files::CloudFileUploadProgressCallback =
            Arc::new(move |progress| {
                if let Ok(mut guard) = progress_state.lock() {
                    let sent_bytes = guard
                        .active_transfers
                        .get(&progress.transfer_id)
                        .map(|item| item.sent_bytes)
                        .unwrap_or(0)
                        .max(progress.sent_bytes.min(progress.total_bytes));
                    guard.active_transfers.insert(
                        progress.transfer_id.clone(),
                        ActiveTransferProgress {
                            path: progress.file_path.clone(),
                            sent_bytes,
                        },
                    );
                }
                emit_state_progress(
                    &progress_app,
                    &progress_id,
                    &progress.status,
                    total,
                    total_bytes,
                    progress_state.clone(),
                    Some(progress_path.clone()),
                    progress_name.clone(),
                );
            });
        if let Ok(mut guard) = state.lock() {
            guard.active_transfers.insert(
                transfer_id.clone(),
                ActiveTransferProgress {
                    path: current_path.clone(),
                    sent_bytes: 0,
                },
            );
        }
        emit_state_progress(
            &app,
            &id,
            "uploading",
            total,
            total_bytes,
            state.clone(),
            Some(current_path.clone()),
            current_file_name.clone(),
        );
        requests.push(
            crate::services::webdav_sync::cloud_files::CloudFileUploadRequest {
                path: current_path,
                transfer_id: Some(transfer_id),
                progress: Some(callback),
            },
        );
    }

    match crate::services::webdav_sync::upload_cloud_files_with_progress(requests).await {
        Ok(results) => {
            let mut changed = false;
            for item in results {
                match item.result {
                    Ok(_) => {
                        if let Ok(mut guard) = state.lock() {
                            guard.done = guard.done.saturating_add(1);
                            let sent_bytes = guard
                                .active_transfers
                                .remove(&format!("{}:cloud:{}", id, item.path))
                                .map(|item| item.sent_bytes)
                                .unwrap_or(0);
                            let total_file_bytes = guard
                                .file_progresses
                                .get(&item.path)
                                .map(|progress| progress.total_bytes)
                                .unwrap_or(sent_bytes);
                            guard.completed_bytes =
                                guard.completed_bytes.saturating_add(total_file_bytes);
                            if let Some(file_progress) = guard.file_progresses.get_mut(&item.path) {
                                file_progress.done = file_progress.done.saturating_add(1);
                                file_progress.completed_bytes = file_progress
                                    .completed_bytes
                                    .saturating_add(total_file_bytes);
                            }
                        }
                        changed |= item.uploaded;
                    }
                    Err(error) => {
                        if let Ok(mut guard) = state.lock() {
                            let transfer_id = format!("{}:cloud:{}", id, item.path);
                            guard.active_transfers.remove(&transfer_id);
                            guard.failed = guard.failed.saturating_add(1);
                            if let Some(file_progress) = guard.file_progresses.get_mut(&item.path) {
                                file_progress.failed = file_progress.failed.saturating_add(1);
                            }
                            guard.errors.push(ShelfSendError {
                                peer_id: "cloud".to_string(),
                                path: item.path,
                                message: error,
                            });
                        }
                    }
                }
                emit_state_progress(
                    &app,
                    &id,
                    "uploading",
                    total,
                    total_bytes,
                    state.clone(),
                    None,
                    None,
                );
            }
            if changed {
                crate::windows::receive_box::emit_cloud_files_changed(&app);
            }
        }
        Err(error) => {
            if let Ok(mut guard) = state.lock() {
                guard.failed = guard.failed.saturating_add(total);
                guard.active_transfers.clear();
                guard.errors.push(ShelfSendError {
                    peer_id: "cloud".to_string(),
                    path: String::new(),
                    message: error,
                });
            }
            emit_state_progress(
                &app,
                &id,
                "uploading",
                total,
                total_bytes,
                state,
                None,
                None,
            );
        }
    }
}

fn emit_state_progress(
    app: &AppHandle,
    shelf_id: &str,
    status: &str,
    total: usize,
    total_bytes: u64,
    state: Arc<Mutex<SendProgressState>>,
    current_path: Option<String>,
    current_file_name: Option<String>,
) {
    let Ok(guard) = state.lock() else {
        return;
    };
    let sent_bytes = guard
        .completed_bytes
        .saturating_add(
            guard
                .active_transfers
                .values()
                .map(|item| item.sent_bytes)
                .sum::<u64>(),
        )
        .min(total_bytes);
    let file_progresses = file_progress_payloads(&guard);
    emit_task_progress(
        app,
        shelf_id,
        status,
        total,
        guard.done,
        guard.failed,
        sent_bytes,
        total_bytes,
        current_path,
        current_file_name,
        &guard.errors,
        file_progresses,
    );
}

fn emit_task_progress(
    app: &AppHandle,
    shelf_id: &str,
    status: &str,
    total: usize,
    done: usize,
    failed: usize,
    sent_bytes: u64,
    total_bytes: u64,
    current_path: Option<String>,
    current_file_name: Option<String>,
    errors: &[ShelfSendError],
    file_progresses: Vec<ShelfFileProgress>,
) {
    let _ = app.emit(
        TASK_PROGRESS_EVENT,
        ShelfSendTaskPayload {
            shelf_id: shelf_id.to_string(),
            status: status.to_string(),
            total,
            done,
            failed,
            sent_bytes,
            total_bytes,
            current_path,
            current_file_name,
            errors: errors.to_vec(),
            file_progresses,
        },
    );
}

fn initial_file_progresses(contexts: &[SendTargetContext]) -> HashMap<String, FileProgressState> {
    let mut out = HashMap::new();
    for context in contexts {
        let entry = out
            .entry(context.target.path.clone())
            .or_insert_with(FileProgressState::default);
        entry.total = entry.total.saturating_add(1);
        entry.total_bytes = entry.total_bytes.saturating_add(context.file_size);
    }
    out
}

fn file_progress_payloads(state: &SendProgressState) -> Vec<ShelfFileProgress> {
    state
        .file_progresses
        .iter()
        .map(|(path, progress)| {
            let active = state
                .active_transfers
                .values()
                .filter(|item| item.path == path.as_str())
                .fold((0usize, 0u64), |(count, bytes), item| {
                    (
                        count.saturating_add(1),
                        bytes.saturating_add(item.sent_bytes),
                    )
                });
            let sent_bytes = progress
                .completed_bytes
                .saturating_add(active.1)
                .min(progress.total_bytes);
            let finished = progress.done.saturating_add(progress.failed) >= progress.total
                && progress.total > 0;
            let status = if finished {
                if progress.failed > 0 {
                    "failed"
                } else {
                    "done"
                }
            } else if active.0 > 0 || progress.done > 0 || progress.failed > 0 {
                active_status_for_progress(state)
            } else {
                "pending"
            };
            ShelfFileProgress {
                path: path.clone(),
                sent_bytes,
                total_bytes: progress.total_bytes,
                total: progress.total,
                done: progress.done,
                failed: progress.failed,
                status: status.to_string(),
            }
        })
        .collect()
}

fn active_status_for_progress(state: &SendProgressState) -> &'static str {
    if state
        .active_transfers
        .keys()
        .any(|transfer_id| transfer_id.contains(":cloud:"))
    {
        "uploading"
    } else {
        "sending"
    }
}

#[tauri::command]
pub fn transfer_shelf_load_state(id: String) -> ShelfStateSnapshot {
    let persisted = load_shelf_state(&id);
    let files = persisted
        .files
        .into_iter()
        .map(|item| describe_path(&item.path))
        .collect();
    ShelfStateSnapshot {
        id: persisted.id,
        name: persisted.name,
        files,
        selected_peer_ids: persisted.selected_peer_ids,
    }
}

#[tauri::command]
pub fn transfer_shelf_save_state(
    id: String,
    files: Vec<ShelfFileInfo>,
    selected_peer_ids: Vec<String>,
) -> Result<(), String> {
    save_shelf_state(&id, files, selected_peer_ids)
}

#[tauri::command]
pub fn transfer_shelf_save_geometry(app: AppHandle, id: String) -> Result<(), String> {
    let label = label_for(&id);
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("找不到文件盒窗口: {}", id))?;
    if window.is_minimized().unwrap_or(false) {
        return Ok(());
    }
    let position = window
        .outer_position()
        .map_err(|e| format!("读取窗口位置失败: {}", e))?;
    let size = window
        .outer_size()
        .map_err(|e| format!("读取窗口尺寸失败: {}", e))?;
    let geometry = ShelfGeometryPersisted {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    let geometry = resolve_shelf_geometry(&app, &geometry).unwrap_or(geometry);
    storage::upsert_geometry(&id, geometry)
}

#[tauri::command]
pub fn transfer_shelf_apply_geometry(app: AppHandle, id: String) -> Result<bool, String> {
    let geometry = match storage::load().geometries.get(&id).cloned() {
        Some(value) => value,
        None => return Ok(false),
    };
    let label = label_for(&id);
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("找不到文件盒窗口: {}", id))?;
    let resolved = apply_shelf_geometry(&app, &window, &geometry)?;
    let _ = storage::upsert_geometry(&id, resolved);
    Ok(true)
}
