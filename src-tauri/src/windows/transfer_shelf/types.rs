use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const LABEL_PREFIX: &str = "transfer-shelf-";
pub const TASK_PROGRESS_EVENT: &str = "transfer-shelf-task-progress";
pub const STATE_CHANGED_EVENT: &str = "transfer-shelf-state-changed";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfFileInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub exists: bool,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfSummary {
    pub id: String,
    pub label: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfSendTarget {
    pub peer_id: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfCloudUploadTarget {
    pub path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfSendError {
    pub peer_id: String,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfFileProgress {
    pub path: String,
    pub sent_bytes: u64,
    pub total_bytes: u64,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfSendTaskPayload {
    pub shelf_id: String,
    pub status: String,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub sent_bytes: u64,
    pub total_bytes: u64,
    pub current_path: Option<String>,
    pub current_file_name: Option<String>,
    pub errors: Vec<ShelfSendError>,
    pub file_progresses: Vec<ShelfFileProgress>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfStateSnapshot {
    pub id: String,
    pub name: String,
    pub files: Vec<ShelfFileInfo>,
    pub selected_peer_ids: Vec<String>,
}

pub fn label_for(id: &str) -> String {
    format!("{}{}", LABEL_PREFIX, id)
}

pub fn describe_path(path: &str) -> ShelfFileInfo {
    let normalized_path = normalize_shell_path(path);
    let effective_path = if normalized_path != path && std::fs::metadata(&normalized_path).is_ok() {
        normalized_path.as_str()
    } else {
        path
    };
    let path_buf = PathBuf::from(effective_path);
    let metadata = std::fs::metadata(&path_buf).ok();
    let name = Path::new(effective_path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(effective_path)
        .to_string();
    let icon = metadata
        .as_ref()
        .filter(|value| value.is_file())
        .and_then(|_| {
            crate::utils::icon::get_file_icon_base64(effective_path)
                .or_else(|| crate::utils::icon::get_file_icon_base64(path))
        });

    ShelfFileInfo {
        path: effective_path.to_string(),
        name,
        size: metadata.as_ref().map(|value| value.len()).unwrap_or(0),
        is_dir: metadata
            .as_ref()
            .map(|value| value.is_dir())
            .unwrap_or(false),
        exists: metadata.is_some(),
        icon,
    }
}

#[cfg(windows)]
fn normalize_shell_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!("\\\\{}", rest)
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

#[cfg(not(windows))]
fn normalize_shell_path(path: &str) -> String {
    path.to_string()
}
