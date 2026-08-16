use crate::services::database::{
    add_clipboard_to_favorites as db_add_clipboard_to_favorites, add_favorite as db_add_favorite,
    delete_favorite as db_delete_favorite, delete_favorites as db_delete_favorites,
    get_clipboard_data_items, get_favorite_by_id, get_favorites_count,
    increment_favorite_paste_counts as db_increment_favorite_paste_counts, move_favorite_item,
    move_favorite_to_group as db_move_favorite_to_group, query_favorites,
    update_favorite as db_update_favorite, FavoriteItem, FavoritesQueryParams, PaginatedResult,
};
use crate::services::paste::FilesData;
use std::path::Path;
use tauri::Emitter;

fn fill_file_exists_for_favorites(items: &mut [FavoriteItem]) {
    for item in items.iter_mut() {
        if item.content_type == "file" || item.content_type == "image" {
            check_and_fill_file_exists(item);
        }
    }
}

fn check_and_fill_file_exists(item: &mut FavoriteItem) {
    if !item.content.starts_with("files:") {
        return;
    }

    if let Ok(mut data) = serde_json::from_str::<FilesData>(&item.content[6..]) {
        for file in &mut data.files {
            let actual_path = resolve_stored_path(&file.path);
            file.exists = Path::new(&actual_path).exists();
            file.actual_path = Some(actual_path);
        }
        if let Ok(json) = serde_json::to_string(&data) {
            item.content = format!("files:{}", json);
        }
    }
}

pub fn hydrate_favorite_item_for_ui(item: &mut FavoriteItem) {
    if item.content_type == "file" || item.content_type == "image" {
        check_and_fill_file_exists(item);
    }
}

fn resolve_stored_path(stored_path: &str) -> String {
    crate::services::resolve_stored_path(stored_path)
}

// 分页查询收藏列表
#[tauri::command]
pub async fn get_favorites_history(
    offset: Option<i64>,
    limit: Option<i64>,
    group_name: Option<String>,
    search: Option<String>,
    content_type: Option<String>,
) -> Result<PaginatedResult<FavoriteItem>, String> {
    tokio::task::spawn_blocking(move || {
        let params = FavoritesQueryParams {
            offset: offset.unwrap_or(0),
            limit: limit.unwrap_or(50),
            group_name,
            search,
            content_type,
        };

        let mut result = query_favorites(params)?;
        fill_file_exists_for_favorites(&mut result.items);
        Ok::<PaginatedResult<FavoriteItem>, String>(result)
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

// 获取收藏总数
#[tauri::command]
pub fn get_favorites_total_count(group_name: Option<String>) -> Result<i64, String> {
    get_favorites_count(group_name)
}

// 移动收藏项（拖拽排序）
#[tauri::command]
pub fn move_favorite_item_cmd(from_id: String, to_id: String) -> Result<(), String> {
    let result = move_favorite_item(from_id, to_id);
    if result.is_ok() {
        notify_lan_change("favorites");
    }
    result
}

// 从剪贴板历史添加到收藏
#[tauri::command]
pub fn add_clipboard_to_favorites(
    id: i64,
    group_name: Option<String>,
) -> Result<FavoriteItem, String> {
    let result = db_add_clipboard_to_favorites(id, group_name);
    if result.is_ok() {
        notify_lan_change("favorites");
    }
    result
}

// 移动收藏项到分组
#[tauri::command]
pub fn move_quick_text_to_group(id: String, group_name: String) -> Result<(), String> {
    let result = db_move_favorite_to_group(id, group_name);
    if result.is_ok() {
        notify_lan_change("favorites");
    }
    result
}

// 删除收藏项
#[tauri::command]
pub fn delete_quick_text(id: String) -> Result<(), String> {
    let result = db_delete_favorite(id);
    if result.is_ok() {
        notify_lan_change("favorites");
    }
    result
}

#[tauri::command]
pub fn delete_favorite_items(ids: Vec<String>) -> Result<(), String> {
    let result = db_delete_favorites(&ids);
    if result.is_ok() {
        notify_lan_change("favorites");
    }
    result
}

// 根据 ID 获取单个收藏项
#[tauri::command]
pub fn get_favorite_item_by_id_cmd(
    id: String,
    max_length: Option<usize>,
) -> Result<FavoriteItem, String> {
    use crate::services::database::get_favorite_by_id_with_limit;
    let mut item = get_favorite_by_id_with_limit(&id, max_length)?
        .ok_or_else(|| format!("收藏项不存在: {}", id))?;

    hydrate_favorite_item_for_ui(&mut item);

    Ok(item)
}

// 添加收藏项
#[tauri::command]
pub fn add_quick_text(
    title: String,
    content: String,
    group_name: Option<String>,
) -> Result<FavoriteItem, String> {
    let result = db_add_favorite(title, content, group_name);
    if result.is_ok() {
        notify_lan_change("favorites");
    }
    result
}

// 更新收藏项
#[tauri::command]
pub fn update_quick_text(
    id: String,
    title: String,
    content: String,
    group_name: Option<String>,
    html_content: Option<String>,
) -> Result<FavoriteItem, String> {
    let result = db_update_favorite(id, title, content, group_name, html_content);
    if result.is_ok() {
        notify_lan_change("favorites");
    }
    result
}

// 复制收藏项内容（不记录到历史）
#[tauri::command]
pub fn copy_favorite_item(id: String) -> Result<(), String> {
    use crate::services::paste::paste_handler::copy_favorite_item as do_copy;

    let item = favorite_to_clipboard_item(&id)?;

    do_copy(&item, &id)
}

#[tauri::command]
pub fn get_favorite_item_paste_options_cmd(
    id: String,
) -> Result<Vec<crate::services::database::PasteOption>, String> {
    let item = favorite_to_clipboard_item(&id)?;
    let raw_formats = get_clipboard_data_items("favorite", &id)?;
    Ok(crate::services::paste::options::build_paste_options(
        &item,
        &raw_formats,
    ))
}

#[tauri::command]
pub async fn merge_copy_favorite_items(ids: Vec<String>) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        if ids.is_empty() {
            return Err("至少需要选择一项内容".to_string());
        }

        let items = ids
            .iter()
            .map(|id| favorite_to_clipboard_item(id))
            .collect::<Result<Vec<_>, _>>()?;

        crate::services::paste::copy_merged_items(&items)
    })
    .await
    .map_err(|e| format!("批量合并复制任务执行失败: {}", e))?
}

#[tauri::command]
pub async fn merge_paste_favorite_items(
    ids: Vec<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let ids_for_emit = ids.clone();
    tokio::task::spawn_blocking(move || {
        if ids.is_empty() {
            return Err("至少需要选择一项内容".to_string());
        }

        let items = ids
            .iter()
            .map(|id| favorite_to_clipboard_item(id))
            .collect::<Result<Vec<_>, _>>()?;

        crate::services::paste::paste_merged_items(&items, &app)?;
        db_increment_favorite_paste_counts(&ids)?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("批量合并粘贴任务执行失败: {}", e))??;

    if let Some(app_handle) = crate::services::clipboard::get_app_handle() {
        for id in ids_for_emit {
            let _ = app_handle.emit("favorite-paste-count-updated", id);
        }
    }

    Ok(())
}

fn favorite_to_clipboard_item(
    id: &str,
) -> Result<crate::services::database::ClipboardItem, String> {
    let favorite = get_favorite_by_id(id)?.ok_or_else(|| format!("收藏项不存在: {}", id))?;

    Ok(crate::services::database::ClipboardItem {
        id: 0,
        uuid: None,
        favorite_id: Some(id.to_string()),
        source_device_id: None,
        is_remote: false,
        content: favorite.content,
        html_content: favorite.html_content,
        content_type: favorite.content_type,
        image_id: favorite.image_id,
        item_order: favorite.item_order,
        is_pinned: false,
        paste_count: favorite.paste_count,
        source_app: None,
        source_icon_hash: None,
        char_count: favorite.char_count,
        created_at: favorite.created_at,
        updated_at: favorite.updated_at,
    })
}

fn notify_lan_change(reason: &'static str) {
    if let Some(app) = crate::services::clipboard::get_app_handle() {
        crate::services::sync_transfer::lan_notify_local_change(app, reason);
    }
}
