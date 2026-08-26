use crate::services::database::{
    clear_clipboard_history as db_clear_clipboard_history,
    delete_clipboard_item as db_delete_clipboard_item,
    delete_clipboard_items as db_delete_clipboard_items, get_clipboard_count,
    get_clipboard_data_items, get_clipboard_item_by_id,
    increment_paste_counts as db_increment_paste_counts, limit_clipboard_history,
    move_clipboard_item_by_id as db_move_clipboard_item_by_id, move_clipboard_item_to_top,
    query_clipboard_items, toggle_pin_clipboard_item as db_toggle_pin,
    update_clipboard_item as db_update_clipboard_item, ClipboardItem, PaginatedResult, QueryParams,
};
use crate::services::paste::FilesData;
use std::path::Path;
use tauri::Emitter;

fn fill_file_exists(items: &mut [ClipboardItem]) {
    for item in items.iter_mut() {
        if item.content_type == "file" || item.content_type == "image" {
            check_and_fill_file_exists(item);
        }
    }
}

fn check_and_fill_file_exists(item: &mut ClipboardItem) {
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

pub fn hydrate_clipboard_item_for_ui(item: &mut ClipboardItem) {
    if item.content_type == "file" || item.content_type == "image" {
        check_and_fill_file_exists(item);
    }
}

// 路径解析函数
fn resolve_stored_path(stored_path: &str) -> String {
    crate::services::resolve_stored_path(stored_path)
}

// 分页查询剪贴板历史
#[tauri::command]
pub async fn get_clipboard_history(
    offset: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
    content_type: Option<String>,
    paste_status: Option<String>,
) -> Result<PaginatedResult<ClipboardItem>, String> {
    tokio::task::spawn_blocking(move || {
        let params = QueryParams {
            offset: offset.unwrap_or(0),
            limit: limit.unwrap_or(50),
            search,
            content_type,
            paste_status,
        };
        let result = query_clipboard_items(params)?;
        let mut items = result.items;
        fill_file_exists(&mut items);

        Ok::<PaginatedResult<ClipboardItem>, String>(PaginatedResult::new(
            result.total_count,
            items,
            result.offset,
            result.limit,
        ))
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

// 另存图片
#[tauri::command]
pub async fn save_image_from_path(
    file_path: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    use std::path::Path;
    use tauri_plugin_dialog::DialogExt;

    let path = Path::new(&file_path);
    if !path.exists() {
        return Err("图片文件不存在".to_string());
    }

    let filename = format!(
        "QC_{}.png",
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("image")
    );

    let save_path = app
        .dialog()
        .file()
        .set_file_name(filename)
        .blocking_save_file()
        .ok_or("用户取消保存")?;

    let dest = save_path.as_path().ok_or("无效的文件路径")?;
    std::fs::copy(&file_path, dest).map_err(|e| format!("保存失败: {}", e))?;

    Ok(dest.to_string_lossy().to_string())
}

// 获取剪贴板总数
#[tauri::command]
pub fn get_clipboard_total_count() -> Result<i64, String> {
    get_clipboard_count()
}

// 移动剪贴板项到顶部（粘贴后置顶使用）
#[tauri::command]
pub fn move_clipboard_item(id: i64) -> Result<(), String> {
    let result = move_clipboard_item_to_top(id);
    if result.is_ok() {
        notify_lan_change("clipboard");
    }
    result
}

// 移动剪贴板项（拖拽排序，按 ID，用于搜索/筛选时）
#[tauri::command]
pub fn move_clipboard_item_by_id(from_id: i64, to_id: i64) -> Result<(), String> {
    let result = db_move_clipboard_item_by_id(from_id, to_id);
    if result.is_ok() {
        notify_lan_change("clipboard");
    }
    result
}

// 应用历史记录数量限制
#[tauri::command]
pub fn apply_history_limit(limit: u64) -> Result<(), String> {
    limit_clipboard_history(limit)
}

// 粘贴参数
#[derive(Debug, serde::Deserialize)]
pub struct PasteParams {
    #[serde(default)]
    pub clipboard_id: Option<i64>,
    #[serde(default)]
    pub favorite_id: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
}

// 粘贴剪贴板项或收藏项
#[tauri::command]
pub async fn paste_content(params: PasteParams, app: tauri::AppHandle) -> Result<(), String> {
    use crate::services::database::get_favorite_by_id;
    use crate::services::paste::paste_handler::{
        paste_clipboard_item_with_format, paste_clipboard_item_with_update,
        paste_favorite_item_with_format, paste_favorite_item_with_update,
    };
    use crate::services::paste::PasteAction;

    let paste_action = match params.action.as_deref() {
        Some(action) if !action.trim().is_empty() => Some(
            PasteAction::from_id(action).ok_or_else(|| format!("不支持的粘贴动作: {}", action))?,
        ),
        _ => None,
    };

    // 先隐藏窗口，让用户感知粘贴是即时的
    if !crate::get_window_state().is_pinned {
        if let Some(window) = crate::get_main_window(&app) {
            crate::hide_main_window(&window);
        }
    }

    let params_for_task = params;
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        // 根据参数类型处理粘贴
        if let Some(clipboard_id) = params_for_task.clipboard_id {
            let item = get_clipboard_item_by_id(clipboard_id)?
                .ok_or_else(|| format!("剪贴板项不存在: {}", clipboard_id))?;

            if paste_action.is_some() {
                paste_clipboard_item_with_format(&item, paste_action)?;
            } else {
                paste_clipboard_item_with_update(&item)?;
            }
        } else if let Some(favorite_id) = params_for_task.favorite_id {
            let favorite = get_favorite_by_id(&favorite_id)?
                .ok_or_else(|| format!("收藏项不存在: {}", favorite_id))?;

            // 将收藏项转换为剪贴板项格式
            let item = ClipboardItem {
                id: 0,
                uuid: None,
                favorite_id: Some(favorite_id.clone()),
                source_device_id: None,
                is_remote: false,
                content: favorite.content,
                html_content: favorite.html_content,
                content_type: favorite.content_type,
                image_id: favorite.image_id,
                item_order: favorite.item_order,
                is_pinned: false,
                paste_count: 0,
                source_app: None,
                source_icon_hash: None,
                char_count: favorite.char_count,
                created_at: favorite.created_at,
                updated_at: favorite.updated_at,
            };

            if paste_action.is_some() {
                paste_favorite_item_with_format(&item, &favorite_id, paste_action)?;
            } else {
                paste_favorite_item_with_update(&item, &favorite_id)?;
            }
        } else {
            return Err("必须 clipboard_id 或 favorite_id".to_string());
        };

        Ok(())
    })
    .await
    .map_err(|e| format!("粘贴任务执行失败: {}", e))??;

    Ok(())
}

// 删除单个剪贴板项
#[tauri::command]
pub fn delete_clipboard_item(id: i64) -> Result<(), String> {
    let result = db_delete_clipboard_item(id);
    if result.is_ok() {
        crate::services::clipboard::clear_last_content_cache();
        notify_lan_change("clipboard");
    }
    result
}

#[tauri::command]
pub fn delete_clipboard_items(ids: Vec<i64>) -> Result<(), String> {
    let result = db_delete_clipboard_items(&ids);
    if result.is_ok() {
        crate::services::clipboard::clear_last_content_cache();
        notify_lan_change("clipboard");
    }
    result
}

// 清空剪贴板历史
#[tauri::command]
pub fn clear_clipboard_history() -> Result<(), String> {
    let result = db_clear_clipboard_history();
    if result.is_ok() {
        crate::services::clipboard::clear_last_content_cache();
        notify_lan_change("clipboard");
    }
    result
}
// 根据 ID 获取单个剪贴板项
#[tauri::command]
pub fn get_clipboard_item_by_id_cmd(
    id: i64,
    max_length: Option<usize>,
) -> Result<ClipboardItem, String> {
    use crate::services::database::get_clipboard_item_by_id_with_limit;
    let mut item = get_clipboard_item_by_id_with_limit(id, max_length)?
        .ok_or_else(|| format!("剪贴板项不存在: {}", id))?;

    hydrate_clipboard_item_for_ui(&mut item);

    Ok(item)
}

// 更新剪贴板项内容
#[tauri::command]
pub fn update_clipboard_item_cmd(
    id: i64,
    content: String,
    html_content: Option<String>,
) -> Result<(), String> {
    let result = db_update_clipboard_item(id, content, html_content);
    if result.is_ok() {
        notify_lan_change("clipboard");
    }
    result
}

// 切换剪贴板项置顶状态
#[tauri::command]
pub fn toggle_pin_clipboard_item(id: i64) -> Result<bool, String> {
    let result = db_toggle_pin(id);
    if result.is_ok() {
        notify_lan_change("clipboard");
    }
    result
}

// 复制图片文件到剪贴板
#[tauri::command]
pub fn copy_image_to_clipboard(file_path: String) -> Result<(), String> {
    use image::ImageFormat;
    use sha2::{Digest, Sha256};
    use std::io::Cursor;

    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("图片文件不存在: {}", file_path));
    }

    // 统一转为 PNG 缓存，避免剪贴板直接依赖会被销毁的源文件
    let image_data = std::fs::read(path).map_err(|e| format!("读取图片失败: {}", e))?;

    let image = image::load_from_memory(&image_data).map_err(|e| format!("解码图片失败: {}", e))?;
    let mut png_data = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png)
        .map_err(|e| format!("编码 PNG 失败: {}", e))?;

    let hash = format!("{:x}", Sha256::digest(&png_data));
    let filename = format!("{}.png", &hash[..16]);

    let data_dir = crate::services::get_data_directory()?;
    let clipboard_images_dir = data_dir.join("clipboard_images");
    std::fs::create_dir_all(&clipboard_images_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let saved_path = clipboard_images_dir.join(&filename);
    if !saved_path.exists() {
        std::fs::write(&saved_path, &png_data).map_err(|e| format!("保存图片失败: {}", e))?;
    }

    crate::services::paste::clipboard_content::set_clipboard_image_file_recordable(
        &saved_path.to_string_lossy(),
    )
}

// 复制文件列表到剪贴板
#[tauri::command]
pub fn copy_files_to_clipboard(paths: Vec<String>) -> Result<(), String> {
    use clipboard_rs::{Clipboard, ClipboardContext};

    if paths.is_empty() {
        return Err("没有可复制的文件".to_string());
    }

    let normalized_paths = paths
        .into_iter()
        .map(|path| {
            let trimmed = path.trim().to_string();
            if trimmed.is_empty() {
                return Err("文件路径不能为空".to_string());
            }
            if !Path::new(&trimmed).exists() {
                return Err(format!("文件不存在: {}", trimmed));
            }
            Ok(trimmed)
        })
        .collect::<Result<Vec<_>, String>>()?;

    let ctx = ClipboardContext::new().map_err(|e| format!("创建剪贴板上下文失败: {}", e))?;

    ctx.set_files(normalized_paths)
        .map_err(|e| format!("设置文件到剪贴板失败: {}", e))
}

// 复制剪贴板项内容（不记录到历史）
#[tauri::command]
pub fn copy_clipboard_item(id: i64) -> Result<(), String> {
    use crate::services::paste::paste_handler::copy_clipboard_item as do_copy;

    let item = get_clipboard_item_by_id(id)?.ok_or_else(|| format!("剪贴板项不存在: {}", id))?;

    do_copy(&item)
}

#[tauri::command]
pub fn get_clipboard_item_paste_options_cmd(
    id: i64,
) -> Result<Vec<crate::services::database::PasteOption>, String> {
    let item = get_clipboard_item_by_id(id)?.ok_or_else(|| format!("剪贴板项不存在: {}", id))?;
    let raw_formats = get_clipboard_data_items("clipboard", &id.to_string())?;
    Ok(crate::services::paste::options::build_paste_options(
        &item,
        &raw_formats,
    ))
}

#[tauri::command]
pub async fn merge_copy_clipboard_items(ids: Vec<i64>) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        if ids.is_empty() {
            return Err("至少需要选择一项内容".to_string());
        }

        let items = ids
            .iter()
            .map(|id| {
                get_clipboard_item_by_id(*id)?.ok_or_else(|| format!("剪贴板项不存在: {}", id))
            })
            .collect::<Result<Vec<_>, _>>()?;

        crate::services::paste::copy_merged_items(&items)
    })
    .await
    .map_err(|e| format!("批量合并复制任务执行失败: {}", e))?
}

#[tauri::command]
pub async fn merge_paste_clipboard_items(
    ids: Vec<i64>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let ids_for_emit = ids.clone();
    tokio::task::spawn_blocking(move || {
        if ids.is_empty() {
            return Err("至少需要选择一项内容".to_string());
        }

        let items = ids
            .iter()
            .map(|id| {
                get_clipboard_item_by_id(*id)?.ok_or_else(|| format!("剪贴板项不存在: {}", id))
            })
            .collect::<Result<Vec<_>, _>>()?;

        crate::services::paste::paste_merged_items(&items, &app)?;
        db_increment_paste_counts(&ids)?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("批量合并粘贴任务执行失败: {}", e))??;

    if let Some(app_handle) = crate::services::clipboard::get_app_handle() {
        for id in ids_for_emit {
            let _ = app_handle.emit("paste-count-updated", id);
        }
    }

    Ok(())
}

// 直接粘贴文本
#[tauri::command]
pub async fn paste_text_direct(text: String, app: tauri::AppHandle) -> Result<(), String> {
    use crate::services::paste::paste_handler::paste_text_direct as do_paste;

    if !crate::get_window_state().is_pinned {
        if let Some(window) = crate::get_main_window(&app) {
            crate::hide_main_window(&window);
        }
    }

    tokio::task::spawn_blocking(move || do_paste(&text))
        .await
        .map_err(|e| format!("粘贴文本任务执行失败: {}", e))??;

    Ok(())
}

// 粘贴图片文件
#[tauri::command]
pub async fn paste_image_file(file_path: String, app: tauri::AppHandle) -> Result<(), String> {
    use crate::services::paste::paste_handler::paste_image_file as do_paste;

    if !crate::get_window_state().is_pinned {
        if let Some(window) = crate::get_main_window(&app) {
            crate::hide_main_window(&window);
        }
    }

    tokio::task::spawn_blocking(move || do_paste(&file_path))
        .await
        .map_err(|e| format!("粘贴图片任务执行失败: {}", e))??;

    Ok(())
}

#[tauri::command]
pub fn resolve_image_path(stored_path: String) -> Result<String, String> {
    Ok(resolve_stored_path(&stored_path))
}

fn notify_lan_change(reason: &'static str) {
    if let Some(app) = crate::services::clipboard::get_app_handle() {
        crate::services::sync_transfer::lan_notify_local_change(app, reason);
    }
}
