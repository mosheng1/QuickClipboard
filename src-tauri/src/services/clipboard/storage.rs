use super::processor::ProcessedContent;
use crate::services::database::clipboard::limit_clipboard_history;
use crate::services::database::connection::with_connection;
use crate::services::database::ClipboardDataSeed;
use crate::services::settings::get_settings;
use chrono;
use rusqlite::params;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
struct DuplicateClipboardItem {
    id: i64,
    is_pinned: i64,
}

// 计算文本字符数
fn calculate_char_count(content: &str, content_type: &str) -> Option<i64> {
    if content_type.contains("text") || content_type.contains("rich_text") {
        let count = content.chars().count() as i64;
        if count > 0 {
            Some(count)
        } else {
            None
        }
    } else {
        None
    }
}

pub fn store_clipboard_item(content: ProcessedContent) -> Result<i64, String> {
    let settings = get_settings();

    if !settings.save_images && is_image_type(&content.content_type) {
        return Err("已禁止保存图片".to_string());
    }

    let result = with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        let now = chrono::Local::now().timestamp();

        if let Some(duplicate) = find_duplicate_item(&content, &tx)? {
            let clipboard_id = refresh_duplicate_item(&content, &tx, duplicate, now)?;
            tx.commit()?;
            return Ok(clipboard_id);
        }

        let new_order = next_item_order(&tx, 0, None)?;
        let char_count = calculate_char_count(&content.content, &content.content_type);
        let uuid = Uuid::new_v4().to_string();

        tx.execute(
            "INSERT INTO clipboard (content, html_content, content_type, image_id, item_order, source_app, source_icon_hash, char_count, uuid, source_device_id, is_remote, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &content.content,
                content.html_content.as_deref(),
                &content.content_type,
                content.image_id.as_deref(),
                new_order,
                content.source_app.as_deref(),
                content.source_icon_hash.as_deref(),
                char_count,
                uuid,
                Option::<&str>::None,
                0,
                now,
                now
            ],
        )?;

        let clipboard_id = tx.last_insert_rowid();

        if !content.raw_formats.is_empty() {
            let target_id = clipboard_id.to_string();
            save_clipboard_data_items_with_conn(
                &tx,
                "clipboard",
                &target_id,
                &content.raw_formats,
            )?;
        }

        tx.commit()?;
        Ok(clipboard_id)
    });

    match result {
        Ok(id) => {
            let _ = limit_clipboard_history(settings.history_limit);
            Ok(id)
        }
        Err(e) => Err(e),
    }
}

// 智能去重
fn find_duplicate_item(
    content: &ProcessedContent,
    conn: &rusqlite::Connection,
) -> Result<Option<DuplicateClipboardItem>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, content, content_type, is_pinned
         FROM clipboard 
         ORDER BY updated_at DESC, id DESC
         LIMIT 100",
    )?;

    let recent_items = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,    // id
            row.get::<_, String>(1)?, // content
            row.get::<_, String>(2)?, // content_type
            row.get::<_, i64>(3)?,    // is_pinned
        ))
    })?;

    for item in recent_items {
        let (db_id, db_content, db_type, is_pinned) = item?;

        let is_text_same = if is_text_type(&content.content_type) && is_text_type(&db_type) {
            content.content == db_content
        } else if is_file_type(&content.content_type) && is_file_type(&db_type) {
            compare_file_contents(&content.content, &db_content)
        } else {
            false
        };

        if !is_text_same {
            continue;
        }

        return Ok(Some(DuplicateClipboardItem {
            id: db_id,
            is_pinned,
        }));
    }

    Ok(None)
}

fn refresh_duplicate_item(
    content: &ProcessedContent,
    conn: &rusqlite::Connection,
    duplicate: DuplicateClipboardItem,
    now: i64,
) -> Result<i64, rusqlite::Error> {
    let new_order = next_item_order(conn, duplicate.is_pinned, Some(duplicate.id))?;
    let char_count = calculate_char_count(&content.content, &content.content_type);

    let rows = conn.execute(
        "UPDATE clipboard
         SET content = ?1,
             html_content = ?2,
             content_type = ?3,
             image_id = ?4,
             item_order = ?5,
             source_app = ?6,
             source_icon_hash = ?7,
             char_count = ?8,
             updated_at = ?9
         WHERE id = ?10",
        params![
            &content.content,
            content.html_content.as_deref(),
            &content.content_type,
            content.image_id.as_deref(),
            new_order,
            content.source_app.as_deref(),
            content.source_icon_hash.as_deref(),
            char_count,
            now,
            duplicate.id,
        ],
    )?;

    if rows == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    let target_id = duplicate.id.to_string();
    conn.execute(
        "DELETE FROM clipboard_data WHERE target_kind = 'clipboard' AND target_id = ?1",
        params![target_id],
    )?;

    if !content.raw_formats.is_empty() {
        save_clipboard_data_items_with_conn(conn, "clipboard", &target_id, &content.raw_formats)?;
    }

    Ok(duplicate.id)
}

fn next_item_order(
    conn: &rusqlite::Connection,
    is_pinned: i64,
    exclude_id: Option<i64>,
) -> Result<i64, rusqlite::Error> {
    let max_order: i64 = if let Some(exclude_id) = exclude_id {
        conn.query_row(
            "SELECT COALESCE(MAX(item_order), 0) FROM clipboard WHERE is_pinned = ?1 AND id <> ?2",
            params![is_pinned, exclude_id],
            |row| row.get(0),
        )?
    } else {
        conn.query_row(
            "SELECT COALESCE(MAX(item_order), 0) FROM clipboard WHERE is_pinned = ?1",
            params![is_pinned],
            |row| row.get(0),
        )?
    };

    Ok(max_order + 1)
}

fn save_clipboard_data_items_with_conn(
    conn: &rusqlite::Connection,
    target_kind: &str,
    target_id: &str,
    items: &[ClipboardDataSeed],
) -> Result<(), rusqlite::Error> {
    if items.is_empty() {
        return Ok(());
    }

    let now = chrono::Local::now().timestamp();
    for item in items {
        conn.execute(
            "INSERT INTO clipboard_data (
                target_kind, target_id, format_name, raw_data,
                is_primary, format_order, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(target_kind, target_id, format_name)
             DO UPDATE SET
                raw_data = excluded.raw_data,
                is_primary = excluded.is_primary,
                format_order = excluded.format_order,
                updated_at = excluded.updated_at",
            params![
                target_kind,
                target_id,
                item.format_name,
                item.raw_data,
                if item.is_primary { 1 } else { 0 },
                item.format_order,
                now,
                now,
            ],
        )?;
    }

    Ok(())
}

fn is_text_type(content_type: &str) -> bool {
    content_type.starts_with("text")
        || content_type.contains("rich_text")
        || content_type.contains("link")
}

fn is_file_type(content_type: &str) -> bool {
    content_type.contains("image") || content_type.contains("file")
}

fn is_image_type(content_type: &str) -> bool {
    content_type.contains("image")
}

// 比较文件内容
fn compare_file_contents(content1: &str, content2: &str) -> bool {
    if !content1.starts_with("files:") || !content2.starts_with("files:") {
        return content1 == content2;
    }

    let Ok(json1) = serde_json::from_str::<Value>(&content1[6..]) else {
        return false;
    };
    let Ok(json2) = serde_json::from_str::<Value>(&content2[6..]) else {
        return false;
    };

    extract_file_paths(&json1) == extract_file_paths(&json2)
}

// 从 JSON 提取并排序文件路径
fn extract_file_paths(json: &Value) -> Vec<String> {
    let mut paths: Vec<String> = json["files"]
        .as_array()
        .into_iter()
        .flat_map(|files| files.iter())
        .filter_map(|file| file["path"].as_str().map(String::from))
        .collect();

    paths.sort();
    paths
}
