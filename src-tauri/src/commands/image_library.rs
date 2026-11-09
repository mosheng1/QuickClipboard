use crate::services::image_library;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
pub struct SaveImagePayload {
    group: String,
    filename: String,
    data: Vec<u8>,
}

#[derive(Deserialize)]
pub struct GetImageListPayload {
    group: String,
    offset: usize,
    limit: usize,
}

#[derive(Deserialize)]
pub struct GetImageCountPayload {
    group: String,
}

#[derive(Deserialize)]
pub struct DeleteImagePayload {
    group: String,
    filename: String,
}

#[derive(Deserialize)]
pub struct RenameImagePayload {
    group: String,
    old_filename: String,
    new_filename: String,
}

#[derive(Deserialize)]
pub struct ImageGroupPayload {
    name: String,
    icon: String,
    color: String,
}

#[derive(Deserialize)]
pub struct RenameImageGroupPayload {
    old_name: String,
    new_name: String,
    icon: String,
    color: String,
}

#[derive(Deserialize)]
pub struct MoveImagePayload {
    source_group: String,
    filename: String,
    target_group: String,
}

#[derive(Deserialize)]
pub struct DeleteImageGroupPayload {
    name: String,
    move_images_to_default: bool,
}

#[tauri::command]
pub fn il_init() -> Result<(), String> {
    image_library::init_image_library()
}

#[tauri::command]
pub async fn il_save_image(payload: SaveImagePayload) -> Result<image_library::ImageInfo, String> {
    let group = payload.group;
    let filename = payload.filename;
    let data = payload.data;

    let handle =
        tokio::task::spawn_blocking(move || image_library::save_image(&group, &filename, &data));
    match tokio::time::timeout(Duration::from_secs(15), handle).await {
        Ok(join_result) => join_result.map_err(|e| format!("任务执行失败: {}", e))?,
        Err(_) => Err("保存图片超时".to_string()),
    }
}

#[tauri::command]
pub fn il_get_image_list(
    payload: GetImageListPayload,
) -> Result<image_library::ImageListResult, String> {
    image_library::get_image_list(&payload.group, payload.offset, payload.limit)
}

#[tauri::command]
pub fn il_get_image_count(payload: GetImageCountPayload) -> Result<usize, String> {
    image_library::get_image_count(&payload.group)
}

#[tauri::command]
pub fn il_delete_image(payload: DeleteImagePayload) -> Result<(), String> {
    image_library::delete_image(&payload.group, &payload.filename)
}

#[tauri::command]
pub fn il_rename_image(payload: RenameImagePayload) -> Result<image_library::ImageInfo, String> {
    image_library::rename_image(&payload.group, &payload.old_filename, &payload.new_filename)
}

#[tauri::command]
pub fn il_get_images_dir() -> Result<String, String> {
    let path = image_library::get_images_dir()?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn il_get_gifs_dir() -> Result<String, String> {
    let path = image_library::get_gifs_dir()?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn il_get_groups() -> Result<Vec<image_library::ImageGroupInfo>, String> {
    image_library::list_groups()
}

#[tauri::command]
pub fn il_add_group(payload: ImageGroupPayload) -> Result<image_library::ImageGroupInfo, String> {
    image_library::add_group(&payload.name, &payload.icon, &payload.color)
}

#[tauri::command]
pub fn il_update_group(
    payload: RenameImageGroupPayload,
) -> Result<image_library::ImageGroupInfo, String> {
    image_library::update_group(
        &payload.old_name,
        &payload.new_name,
        &payload.icon,
        &payload.color,
    )
}

#[tauri::command]
pub fn il_move_image_to_group(
    payload: MoveImagePayload,
) -> Result<image_library::ImageInfo, String> {
    image_library::move_image_to_group(
        &payload.source_group,
        &payload.filename,
        &payload.target_group,
    )
}

#[tauri::command]
pub fn il_delete_group(
    payload: DeleteImageGroupPayload,
) -> Result<Vec<image_library::ImageGroupInfo>, String> {
    image_library::delete_group(&payload.name, payload.move_images_to_default)
}
