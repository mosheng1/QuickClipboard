use clipboard_rs::{ClipboardContent as RsClipboardContent, ClipboardContext};

use crate::services::database::{get_clipboard_data_items, ClipboardDataItem, ClipboardItem};
use crate::utils::cf_html::generate_cf_html;

use super::clipboard_content::{
    set_clipboard_contents, set_clipboard_files, set_clipboard_image_file,
};
use super::keyboard::simulate_paste;
use super::options::{resolve_default_paste_action, PasteAction};
use super::text::paste_text;

fn emit_paste_count_updated(id: i64) {
    use tauri::Emitter;
    if let Some(app) = crate::services::clipboard::get_app_handle() {
        let _ = app.emit("paste-count-updated", id);
    }
}

fn emit_favorite_paste_count_updated(id: &str) {
    use tauri::Emitter;
    if let Some(app) = crate::services::clipboard::get_app_handle() {
        let _ = app.emit("favorite-paste-count-updated", id);
    }
}

// 直接粘贴文本
pub fn paste_text_direct(text: &str) -> Result<(), String> {
    crate::services::clipboard::set_last_hash_text(text);

    crate::services::mark_paste_operation();
    let _monitor_guard = crate::services::clipboard::pause_clipboard_monitor_for(1000);

    let ctx = ClipboardContext::new().map_err(|e| format!("创建剪贴板上下文失败: {}", e))?;

    paste_text(&ctx, text)?;

    std::thread::sleep(std::time::Duration::from_millis(50));
    simulate_paste()?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    crate::AppSounds::play_paste_on_success();
    Ok(())
}

// 粘贴图片文件（不记录到历史）
pub fn paste_image_file(file_path: &str) -> Result<(), String> {
    use std::path::Path;

    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("图片文件不存在: {}", file_path));
    }

    crate::services::clipboard::set_last_hash_file(file_path);
    crate::services::mark_paste_operation();
    let _monitor_guard = crate::services::clipboard::pause_clipboard_monitor_for(1000);

    set_clipboard_image_file(file_path)?;

    std::thread::sleep(std::time::Duration::from_millis(50));
    simulate_paste()?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    crate::AppSounds::play_paste_on_success();

    Ok(())
}

// 直接复制剪贴板项到系统剪贴板（不触发粘贴）
pub fn copy_clipboard_item(item: &ClipboardItem) -> Result<(), String> {
    paste_item_internal(item, Some(item.id), None, None, false, false)
}

// 直接复制收藏项到系统剪贴板（不触发粘贴）
pub fn copy_favorite_item(item: &ClipboardItem, favorite_id: &str) -> Result<(), String> {
    paste_item_internal(
        item,
        None,
        Some(favorite_id.to_string()),
        None,
        false,
        false,
    )
}

// 粘贴剪贴板项并自动转换旧格式（更新 clipboard 表）
pub fn paste_clipboard_item_with_update(item: &ClipboardItem) -> Result<(), String> {
    let result = paste_item_internal(item, Some(item.id), None, None, true, true);
    if result.is_ok() {
        let _ = crate::services::database::increment_paste_count(item.id);
        emit_paste_count_updated(item.id);
    }
    result
}

// 粘贴收藏项并自动转换旧格式（更新 favorites 表）
pub fn paste_favorite_item_with_update(
    item: &ClipboardItem,
    favorite_id: &str,
) -> Result<(), String> {
    let result = paste_item_internal(item, None, Some(favorite_id.to_string()), None, true, true);
    if result.is_ok() {
        let _ = crate::services::database::increment_favorite_paste_count(favorite_id);
        emit_favorite_paste_count_updated(favorite_id);
    }
    result
}

// 粘贴剪贴板项（指定动作）
pub fn paste_clipboard_item_with_format(
    item: &ClipboardItem,
    action: Option<PasteAction>,
) -> Result<(), String> {
    let result = paste_item_internal(item, Some(item.id), None, action, true, false);
    if result.is_ok() {
        let _ = crate::services::database::increment_paste_count(item.id);
        emit_paste_count_updated(item.id);
    }
    result
}

// 粘贴收藏项（指定动作）
pub fn paste_favorite_item_with_format(
    item: &ClipboardItem,
    favorite_id: &str,
    action: Option<PasteAction>,
) -> Result<(), String> {
    let result = paste_item_internal(
        item,
        None,
        Some(favorite_id.to_string()),
        action,
        true,
        false,
    );
    if result.is_ok() {
        let _ = crate::services::database::increment_favorite_paste_count(favorite_id);
        emit_favorite_paste_count_updated(favorite_id);
    }
    result
}

fn paste_item_internal(
    item: &ClipboardItem,
    clipboard_id: Option<i64>,
    favorite_id: Option<String>,
    action: Option<PasteAction>,
    simulate: bool,
    update_item: bool,
) -> Result<(), String> {
    let raw_formats = load_raw_formats(clipboard_id, favorite_id.as_deref())?;
    let resolved_action = action.unwrap_or_else(|| {
        if simulate {
            resolve_default_paste_action(item, &raw_formats)
        } else {
            resolve_copy_action(item, &raw_formats)
        }
    });

    let payload = build_payload_from_action(item, &raw_formats, resolved_action.clone())?;

    if payload.is_empty() {
        return Err("没有可写入剪贴板的数据".to_string());
    }

    let _monitor_guard =
        crate::services::clipboard::pause_clipboard_monitor_for(if simulate { 1000 } else { 500 });
    crate::services::clipboard::set_last_hash_contents(&payload);
    crate::services::mark_paste_operation();

    if resolved_action == PasteAction::ImageBundle {
        set_clipboard_image_file(&resolve_item_image_path(item)?)?;
    } else {
        let ctx = ClipboardContext::new().map_err(|e| format!("创建剪贴板上下文失败: {}", e))?;
        if let [RsClipboardContent::Files(paths)] = payload.as_slice() {
            set_clipboard_files(&ctx, paths.clone())?;
        } else {
            set_clipboard_contents(&ctx, payload)?;
        }
    }

    if update_item {
        if let Some(id) = clipboard_id {
            if item
                .content_type
                .split(',')
                .next()
                .unwrap_or(&item.content_type)
                == "image"
                && !item.content.starts_with("files:")
            {
                let new_content = convert_legacy_image_format(item)?;
                update_item_content(Some(id), None, &new_content)?;
            }
        } else if let Some(id) = favorite_id.as_deref() {
            if item
                .content_type
                .split(',')
                .next()
                .unwrap_or(&item.content_type)
                == "image"
                && !item.content.starts_with("files:")
            {
                let new_content = convert_legacy_image_format(item)?;
                update_item_content(None, Some(id), &new_content)?;
            }
        }
    }

    if simulate {
        std::thread::sleep(std::time::Duration::from_millis(50));
        simulate_paste()?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        crate::AppSounds::play_paste_on_success();
    }

    Ok(())
}

fn load_raw_formats(
    clipboard_id: Option<i64>,
    favorite_id: Option<&str>,
) -> Result<Vec<ClipboardDataItem>, String> {
    if let Some(id) = clipboard_id {
        return get_clipboard_data_items("clipboard", &id.to_string());
    }

    if let Some(id) = favorite_id {
        return get_clipboard_data_items("favorite", id);
    }

    Ok(Vec::new())
}

fn resolve_copy_action(item: &ClipboardItem, raw_formats: &[ClipboardDataItem]) -> PasteAction {
    let primary_type = item
        .content_type
        .split(',')
        .next()
        .unwrap_or(&item.content_type);

    match primary_type {
        "image" => PasteAction::ImageBundle,
        "file" => PasteAction::File,
        _ => {
            if !raw_formats.is_empty() {
                PasteAction::AllFormats
            } else if item
                .html_content
                .as_deref()
                .map(|html| !html.trim().is_empty())
                .unwrap_or(false)
            {
                PasteAction::Html
            } else {
                PasteAction::PlainText
            }
        }
    }
}

fn build_payload_from_action(
    item: &ClipboardItem,
    raw_formats: &[ClipboardDataItem],
    action: PasteAction,
) -> Result<Vec<RsClipboardContent>, String> {
    match action {
        PasteAction::PlainText => build_plain_text_payload(item, raw_formats),
        PasteAction::Html => build_html_payload(item, raw_formats),
        PasteAction::Rtf => build_rtf_payload(item, raw_formats),
        PasteAction::AllFormats => build_all_formats_payload(item, raw_formats),
        PasteAction::ImageBundle => build_image_bundle_payload(item),
        PasteAction::File => build_file_payload(item),
    }
}

fn build_plain_text_payload(
    item: &ClipboardItem,
    raw_formats: &[ClipboardDataItem],
) -> Result<Vec<RsClipboardContent>, String> {
    if let Some(raw_text) = find_preferred_text_row(raw_formats) {
        return Ok(vec![RsClipboardContent::Other(
            raw_text.format_name.clone(),
            raw_text.raw_data.clone(),
        )]);
    }

    if item.content.starts_with("files:") {
        return Err("当前条目没有可用的纯文本内容".to_string());
    }

    if item.content.trim().is_empty() {
        return Err("当前条目没有可用的纯文本内容".to_string());
    }

    Ok(vec![RsClipboardContent::Text(item.content.clone())])
}

fn build_html_payload(
    item: &ClipboardItem,
    raw_formats: &[ClipboardDataItem],
) -> Result<Vec<RsClipboardContent>, String> {
    let mut payload = build_plain_text_payload(item, raw_formats).unwrap_or_default();

    if let Some(row) = find_raw_row(raw_formats, "HTML Format") {
        payload.push(RsClipboardContent::Other(
            row.format_name.clone(),
            row.raw_data.clone(),
        ));
        return Ok(payload);
    }

    if let Some(html) = item
        .html_content
        .as_deref()
        .filter(|html| !html.trim().is_empty())
    {
        payload.push(RsClipboardContent::Html(generate_cf_html(html)));
        return Ok(payload);
    }

    if payload.is_empty() {
        return Err("当前条目没有可用的 HTML 内容".to_string());
    }

    Ok(payload)
}

fn build_rtf_payload(
    item: &ClipboardItem,
    raw_formats: &[ClipboardDataItem],
) -> Result<Vec<RsClipboardContent>, String> {
    let mut payload = build_plain_text_payload(item, raw_formats).unwrap_or_default();

    if let Some(row) = find_raw_row(raw_formats, "Rich Text Format") {
        payload.push(RsClipboardContent::Other(
            row.format_name.clone(),
            row.raw_data.clone(),
        ));
        return Ok(payload);
    }

    if payload.is_empty() {
        return Err("当前条目没有可用的 RTF 内容".to_string());
    }

    Ok(payload)
}

fn build_all_formats_payload(
    item: &ClipboardItem,
    raw_formats: &[ClipboardDataItem],
) -> Result<Vec<RsClipboardContent>, String> {
    if raw_formats.is_empty() {
        return build_legacy_all_formats_payload(item);
    }

    let mut payload = Vec::new();

    for row in raw_formats {
        if row.format_name == crate::services::clipboard::INTERNAL_IMAGE_PATH_FORMAT {
            continue;
        }

        payload.push(RsClipboardContent::Other(
            row.format_name.clone(),
            row.raw_data.clone(),
        ));
    }

    // 多格式粘贴时兜底写入标准文本，确保只能接收纯文本的目标可粘贴
    if !item.content.starts_with("files:")
        && !item.content.trim().is_empty()
        && !payload_has_plain_text(&payload)
    {
        payload.push(RsClipboardContent::Text(item.content.clone()));
    }

    if item
        .content_type
        .split(',')
        .any(|value| value.trim() == "image")
    {
        append_unique_payload(&mut payload, build_file_payload(item)?);
    }

    if payload.is_empty() {
        return build_legacy_all_formats_payload(item);
    }

    Ok(payload)
}

fn payload_has_plain_text(payload: &[RsClipboardContent]) -> bool {
    payload.iter().any(|entry| match entry {
        RsClipboardContent::Text(_) => true,
        RsClipboardContent::Other(name, _) => {
            matches!(name.as_str(), "CF_UNICODETEXT" | "CF_TEXT")
        }
        _ => false,
    })
}

fn build_legacy_all_formats_payload(
    item: &ClipboardItem,
) -> Result<Vec<RsClipboardContent>, String> {
    let primary_type = item
        .content_type
        .split(',')
        .next()
        .unwrap_or(&item.content_type);

    match primary_type {
        "image" => build_image_bundle_payload(item),
        "file" => build_file_payload(item),
        _ => {
            let mut payload = Vec::new();

            if !item.content.starts_with("files:") && !item.content.trim().is_empty() {
                payload.push(RsClipboardContent::Text(item.content.clone()));
            }

            if let Some(html) = item
                .html_content
                .as_deref()
                .filter(|html| !html.trim().is_empty())
            {
                payload.push(RsClipboardContent::Html(generate_cf_html(html)));
            }

            if item
                .content_type
                .split(',')
                .any(|value| value.trim() == "image")
            {
                append_unique_payload(&mut payload, build_file_payload(item)?);
            }

            if payload.is_empty() {
                Err("没有可写入剪贴板的数据".to_string())
            } else {
                Ok(payload)
            }
        }
    }
}

fn build_image_bundle_payload(item: &ClipboardItem) -> Result<Vec<RsClipboardContent>, String> {
    build_file_payload(item)
}

fn build_file_payload(item: &ClipboardItem) -> Result<Vec<RsClipboardContent>, String> {
    if item.content.starts_with("files:") {
        let paths = super::clipboard_content::parse_files_content_existing(&item.content)?;
        return Ok(vec![RsClipboardContent::Files(paths)]);
    }

    let image_path = resolve_item_image_path(item)?;
    Ok(vec![RsClipboardContent::Files(vec![image_path])])
}

fn resolve_item_image_path(item: &ClipboardItem) -> Result<String, String> {
    if item.content.starts_with("files:") {
        let paths = super::clipboard_content::parse_files_content_existing(&item.content)?;
        if let Some(path) = paths.into_iter().next() {
            return Ok(path);
        }
    }

    let image_id = item
        .image_id
        .as_deref()
        .and_then(|ids| ids.split(',').map(|s| s.trim()).find(|s| !s.is_empty()))
        .ok_or_else(|| "当前条目没有可用的图片缓存".to_string())?;

    let image_path = crate::services::get_data_directory()?
        .join("clipboard_images")
        .join(format!("{}.png", image_id));

    if !image_path.exists() {
        return Err(format!("图片文件不存在: {}", image_path.display()));
    }

    Ok(image_path.to_string_lossy().to_string())
}

fn find_preferred_text_row<'a>(
    raw_formats: &'a [ClipboardDataItem],
) -> Option<&'a ClipboardDataItem> {
    raw_formats
        .iter()
        .find(|row| {
            row.is_primary && matches!(row.format_name.as_str(), "CF_UNICODETEXT" | "CF_TEXT")
        })
        .or_else(|| find_raw_row(raw_formats, "CF_UNICODETEXT"))
        .or_else(|| find_raw_row(raw_formats, "CF_TEXT"))
}

fn find_raw_row<'a>(
    raw_formats: &'a [ClipboardDataItem],
    format_name: &str,
) -> Option<&'a ClipboardDataItem> {
    raw_formats
        .iter()
        .find(|row| row.format_name == format_name)
}

fn append_unique_payload(payload: &mut Vec<RsClipboardContent>, extra: Vec<RsClipboardContent>) {
    for item in extra {
        let exists = payload
            .iter()
            .any(|current| same_payload_kind(current, &item));
        if !exists {
            payload.push(item);
        }
    }
}

fn same_payload_kind(left: &RsClipboardContent, right: &RsClipboardContent) -> bool {
    match (left, right) {
        (RsClipboardContent::Text(_), RsClipboardContent::Text(_)) => true,
        (RsClipboardContent::Html(_), RsClipboardContent::Html(_)) => true,
        (RsClipboardContent::Rtf(_), RsClipboardContent::Rtf(_)) => true,
        (RsClipboardContent::Files(_), RsClipboardContent::Files(_)) => true,
        (RsClipboardContent::Image(_), RsClipboardContent::Image(_)) => true,
        (RsClipboardContent::Other(left_name, _), RsClipboardContent::Other(right_name, _)) => {
            left_name == right_name
        }
        _ => false,
    }
}

// 转换旧格式图片为新格式（更新 clipboard 表）
fn convert_legacy_image_format(item: &ClipboardItem) -> Result<String, String> {
    use crate::services::get_data_directory;

    let image_id = item
        .image_id
        .as_deref()
        .or_else(|| item.content.strip_prefix("image:"))
        .ok_or("无法获取图片ID")?;

    let image_path = get_data_directory()?
        .join("clipboard_images")
        .join(format!("{}.png", image_id));

    if !image_path.exists() {
        return Err(format!("图片文件不存在: {}", image_path.display()));
    }

    let file_data = serde_json::json!({
        "files": [{
            "path": image_path.to_str().ok_or("路径转换失败")?,
            "name": format!("{}.png", image_id),
            "size": std::fs::metadata(&image_path).map(|m| m.len()).unwrap_or(0),
            "is_directory": false,
            "file_type": "PNG"
        }],
        "operation": "copy"
    });

    Ok(format!("files:{}", file_data))
}

// 更新条目内容并刷新时间戳
fn update_item_content(
    clipboard_id: Option<i64>,
    favorite_id: Option<&str>,
    new_content: &str,
) -> Result<(), String> {
    use crate::services::database::connection::with_connection;
    use rusqlite::params;

    with_connection(|conn| {
        let now = chrono::Local::now().timestamp();

        if let Some(id) = clipboard_id {
            conn.execute(
                "UPDATE clipboard SET content = ?, updated_at = ?, created_at = ? WHERE id = ?",
                params![new_content, now, now, id],
            )?;
        } else if let Some(id) = favorite_id {
            conn.execute(
                "UPDATE favorites SET content = ?, updated_at = ? WHERE id = ?",
                params![new_content, now, id],
            )?;
        }
        Ok(())
    })
}
