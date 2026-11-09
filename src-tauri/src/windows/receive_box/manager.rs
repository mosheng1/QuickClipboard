use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use super::types::{ReceiveBoxCloudFile, ReceiveBoxLanFile};
use super::window::{create_receive_box_window, RECEIVE_BOX_LABEL};

pub fn open_receive_box(app: &AppHandle) -> Result<(), String> {
    create_receive_box_window(app, true).map(|_| ())
}

pub fn focus_receive_box(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(RECEIVE_BOX_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        return window
            .set_focus()
            .map_err(|e| format!("聚焦收件盒窗口失败: {}", e));
    }
    open_receive_box(app)
}

pub fn list_lan_files() -> Result<Vec<ReceiveBoxLanFile>, String> {
    let dir = received_files_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut indexed_paths = HashSet::new();
    for item in crate::services::sync_transfer::lan::files::list_received_file_metadata()? {
        indexed_paths.insert(item.path.clone());
        let exists = PathBuf::from(&item.path).exists();
        let icon = file_icon_if_exists(&item.path, exists);
        files.push(ReceiveBoxLanFile {
            path: item.path,
            name: item.name,
            size: item.size,
            sha256: item.sha256,
            source_device_id: item.source_device_id,
            source_device_name: item.source_device_name,
            received_at: item.received_at,
            exists,
            icon,
        });
    }

    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("读取局域网接收文件目录失败: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() || is_temp_file(&path) {
            continue;
        }
        let path_key = path
            .canonicalize()
            .unwrap_or_else(|_| path.clone())
            .to_string_lossy()
            .to_string();
        if indexed_paths.contains(&path_key) {
            continue;
        }
        let icon = crate::utils::icon::get_file_icon_base64(&path_key);

        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("file")
            .to_string();
        let received_at = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
            .unwrap_or(0);
        files.push(ReceiveBoxLanFile {
            path: path_key,
            name,
            size: metadata.len(),
            sha256: String::new(),
            source_device_id: String::new(),
            source_device_name: String::new(),
            received_at,
            exists: true,
            icon,
        });
    }

    files.sort_by(|a, b| b.received_at.cmp(&a.received_at).then(a.name.cmp(&b.name)));
    Ok(files)
}

pub async fn list_cloud_files() -> Result<Vec<ReceiveBoxCloudFile>, String> {
    let files = crate::services::webdav_sync::list_cloud_files().await?;
    Ok(files
        .into_iter()
        .map(|file| {
            let icon = cloud_file_icon(file.local_path.as_deref(), &file.local_status);
            ReceiveBoxCloudFile {
                id: file.id,
                name: file.name,
                size: file.size,
                sha256: file.sha256,
                source_device_id: file.source_device_id,
                source_device_name: file.source_device_name,
                uploaded_at: file.uploaded_at,
                local_status: file.local_status,
                local_path: file.local_path,
                downloaded_at: file.downloaded_at,
                icon,
            }
        })
        .collect())
}

pub async fn download_cloud_file(file_id: String) -> Result<ReceiveBoxCloudFile, String> {
    let result = crate::services::webdav_sync::download_cloud_file(&file_id).await?;
    let file = result.file;
    let icon = cloud_file_icon(file.local_path.as_deref(), &file.local_status);
    Ok(ReceiveBoxCloudFile {
        id: file.id,
        name: file.name,
        size: file.size,
        sha256: file.sha256,
        source_device_id: file.source_device_id,
        source_device_name: file.source_device_name,
        uploaded_at: file.uploaded_at,
        local_status: file.local_status,
        local_path: file.local_path,
        downloaded_at: file.downloaded_at,
        icon,
    })
}

pub fn open_local_file(path: String) -> Result<(), String> {
    let path = validate_managed_file_path(&path)?;
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| format!("打开文件失败: {}", e))
}

pub fn reveal_local_file(path: String) -> Result<(), String> {
    let path = validate_managed_file_path(&path)?;
    let parent = path
        .parent()
        .ok_or_else(|| "无法定位文件所在目录".to_string())?;
    tauri_plugin_opener::open_path(parent, None::<&str>)
        .map_err(|e| format!("打开文件位置失败: {}", e))
}

pub fn delete_local_file(path: String) -> Result<(), String> {
    let candidate = PathBuf::from(&path);
    if !candidate.exists() {
        forget_missing_lan_file_metadata(&candidate)?;
        return Ok(());
    }

    let path = validate_managed_file_path(&path)?;
    std::fs::remove_file(&path).map_err(|e| format!("删除本地文件失败: {}", e))?;
    forget_lan_file_metadata(&path)?;
    Ok(())
}

pub async fn delete_cloud_file(file_id: String) -> Result<(), String> {
    crate::services::webdav_sync::delete_cloud_file(&file_id).await
}

pub fn add_to_transfer_shelf(app: &AppHandle, path: String) -> Result<(), String> {
    let path = validate_managed_file_path(&path)?;
    crate::windows::transfer_shelf::append_files_to_recent_or_new_shelf(
        app,
        vec![path.to_string_lossy().to_string()],
    )
    .map(|_| ())
}

fn received_files_dir() -> Result<PathBuf, String> {
    crate::services::sync_transfer::lan::files::received_files_dir()
}

fn file_icon_if_exists(path: &str, exists: bool) -> Option<String> {
    if exists {
        crate::utils::icon::get_file_icon_base64(path)
    } else {
        None
    }
}

fn cloud_file_icon(path: Option<&str>, status: &str) -> Option<String> {
    if status == "downloaded" {
        path.and_then(crate::utils::icon::get_file_icon_base64)
    } else {
        None
    }
}

fn validate_managed_file_path(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);
    let canonical = candidate
        .canonicalize()
        .map_err(|e| format!("文件不存在或无法访问: {}", e))?;
    let metadata = std::fs::metadata(&canonical).map_err(|e| format!("读取文件信息失败: {}", e))?;
    if !metadata.is_file() || is_internal_managed_file(&canonical)? {
        return Err("只能操作收件盒内的普通文件".to_string());
    }

    let data_dir = crate::services::get_data_directory()?;
    let roots = [received_files_dir()?, data_dir.join("cloud_file_downloads")];
    for root in roots {
        if let Ok(root) = root.canonicalize() {
            if canonical.starts_with(root) {
                return Ok(canonical);
            }
        }
    }
    Err("只能操作收件盒管理的文件".to_string())
}

fn is_temp_file(path: &Path) -> bool {
    crate::services::sync_transfer::lan::files::is_received_file_internal(path)
}

fn is_internal_managed_file(path: &Path) -> Result<bool, String> {
    let hidden_or_temp = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|name| name.starts_with('.') || name.ends_with(".qcpart"))
        .unwrap_or(false);
    if hidden_or_temp {
        return Ok(true);
    }

    let received_root = received_files_dir()?;
    if let Ok(root) = received_root.canonicalize() {
        if path.starts_with(root) {
            return Ok(crate::services::sync_transfer::lan::files::is_received_file_internal(path));
        }
    }
    Ok(false)
}

fn forget_lan_file_metadata(path: &Path) -> Result<(), String> {
    let received_root = received_files_dir()?;
    if let Ok(root) = received_root.canonicalize() {
        if path.starts_with(root) {
            crate::services::sync_transfer::lan::files::remove_received_file_metadata(path)?;
        }
    }
    Ok(())
}

fn forget_missing_lan_file_metadata(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "只能操作收件盒管理的文件".to_string())?;
    let root = received_files_dir()?
        .canonicalize()
        .map_err(|_| "只能操作收件盒管理的文件".to_string())?;
    let parent = parent
        .canonicalize()
        .map_err(|_| "只能操作收件盒管理的文件".to_string())?;
    if !parent.starts_with(root) {
        return Err("只能操作收件盒管理的文件".to_string());
    }
    crate::services::sync_transfer::lan::files::remove_received_file_metadata(path)
}
