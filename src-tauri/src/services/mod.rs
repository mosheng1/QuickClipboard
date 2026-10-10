pub mod clipboard;
pub mod data_management;
pub mod database;
pub mod image_library;
pub mod low_memory;
pub mod memory;
pub mod notification;
pub mod paste;
pub mod secure_credentials;
pub mod settings;
pub mod sound;
pub mod store;
pub mod sync_transfer;
pub mod system;
pub mod webdav_sync;

pub use notification::show_startup_notification;
pub use settings::{get_data_directory, get_settings, update_settings, AppSettings};
pub use sound::{mark_paste_operation, AppSounds, SoundPlayer};
pub use system::hotkey;

pub fn normalize_path_for_hash(path: &str) -> String {
    let normalized = path.replace("\\", "/");
    for prefix in ["clipboard_images/", "pin_images/"] {
        if let Some(idx) = normalized.find(prefix) {
            return normalized[idx..].to_string();
        }
    }
    normalized
}

// 解析存储的路径为实际绝对路径
pub fn resolve_stored_path(stored_path: &str) -> String {
    let normalized_input = stored_path.replace("/", "\\");

    if normalized_input.starts_with("clipboard_images\\")
        || normalized_input.starts_with("pin_images\\")
        || normalized_input.starts_with("image_library\\")
    {
        if let Ok(data_dir) = get_data_directory() {
            return data_dir
                .join(&normalized_input)
                .to_string_lossy()
                .to_string();
        }
    }

    let search_path = stored_path.replace("\\", "/");
    for prefix in ["clipboard_images/", "pin_images/", "image_library/"] {
        if let Some(idx) = search_path.find(prefix) {
            if let Ok(data_dir) = get_data_directory() {
                let relative = search_path[idx..].replace("/", "\\");
                let new_path = data_dir.join(&relative);
                if new_path.exists() {
                    return new_path.to_string_lossy().to_string();
                }
            }
        }
    }

    stored_path.to_string()
}

pub fn is_portable_build() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase())
        })
        .map(|name| name.contains("portable"))
        .unwrap_or(false)
}
