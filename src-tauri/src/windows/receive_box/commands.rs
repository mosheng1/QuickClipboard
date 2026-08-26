use tauri::AppHandle;

use super::manager;
use super::types::{ReceiveBoxCloudFile, ReceiveBoxLanFile};

#[tauri::command]
pub async fn receive_box_open(app: AppHandle) -> Result<(), String> {
    manager::open_receive_box(&app)
}

#[tauri::command]
pub async fn receive_box_focus(app: AppHandle) -> Result<(), String> {
    manager::focus_receive_box(&app)
}

#[tauri::command]
pub fn receive_box_list_lan_files() -> Result<Vec<ReceiveBoxLanFile>, String> {
    manager::list_lan_files()
}

#[tauri::command]
pub async fn receive_box_list_cloud_files() -> Result<Vec<ReceiveBoxCloudFile>, String> {
    manager::list_cloud_files().await
}

#[tauri::command]
pub async fn receive_box_download_cloud_file(
    file_id: String,
) -> Result<ReceiveBoxCloudFile, String> {
    manager::download_cloud_file(file_id).await
}

#[tauri::command]
pub fn receive_box_open_local_file(path: String) -> Result<(), String> {
    manager::open_local_file(path)
}

#[tauri::command]
pub fn receive_box_reveal_local_file(path: String) -> Result<(), String> {
    manager::reveal_local_file(path)
}

#[tauri::command]
pub fn receive_box_delete_local_file(path: String) -> Result<(), String> {
    manager::delete_local_file(path)
}

#[tauri::command]
pub async fn receive_box_delete_cloud_file(app: AppHandle, file_id: String) -> Result<(), String> {
    manager::delete_cloud_file(file_id).await?;
    super::emit_cloud_files_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn receive_box_add_to_transfer_shelf(app: AppHandle, path: String) -> Result<(), String> {
    manager::add_to_transfer_shelf(&app, path)
}
