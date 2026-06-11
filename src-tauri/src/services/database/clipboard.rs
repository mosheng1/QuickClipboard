use super::connection::{with_connection, MAX_CONTENT_LENGTH};
use super::models::{
    ClipboardDataItem, ClipboardDataSeed, ClipboardItem, PaginatedResult, QueryParams,
};
use crate::services::webdav_sync::types::{CloudRecord, CloudRecordMeta};
use crate::utils::{truncate_around_keyword, truncate_html, truncate_string};
use chrono;
use rusqlite::{params, OptionalExtension};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const SEARCH_CONTENT_PREFIX_LENGTH: i64 = 4096;

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

fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '%' => escaped.push_str("\\%"),
            '_' => escaped.push_str("\\_"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub fn save_clipboard_data_items(
    target_kind: &str,
    target_id: &str,
    items: &[ClipboardDataSeed],
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }

    with_connection(|conn| {
        let now = chrono::Local::now().timestamp();
        let tx = conn.unchecked_transaction()?;

        for item in items {
            tx.execute(
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

        tx.commit()?;
        Ok(())
    })
}

fn get_clipboard_data_items_by_target(
    target_kind: &str,
    target_id: &str,
) -> Result<Vec<ClipboardDataItem>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, target_kind, target_id, format_name, raw_data, is_primary, format_order, created_at, updated_at
             FROM clipboard_data
             WHERE target_kind = ?1 AND target_id = ?2
             ORDER BY format_order ASC, id ASC",
        )?;

        let items = stmt
            .query_map(params![target_kind, target_id], |row| {
                Ok(ClipboardDataItem {
                    id: row.get(0)?,
                    target_kind: row.get(1)?,
                    target_id: row.get(2)?,
                    format_name: row.get(3)?,
                    raw_data: row.get(4)?,
                    is_primary: row.get::<_, i64>(5)? != 0,
                    format_order: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    })
}

pub fn get_clipboard_data_items(
    target_kind: &str,
    target_id: &str,
) -> Result<Vec<ClipboardDataItem>, String> {
    get_clipboard_data_items_by_target(target_kind, target_id)
}

pub fn delete_clipboard_data_items(target_kind: &str, target_id: &str) -> Result<(), String> {
    with_connection(|conn| {
        conn.execute(
            "DELETE FROM clipboard_data WHERE target_kind = ?1 AND target_id = ?2",
            params![target_kind, target_id],
        )?;
        Ok(())
    })
}

pub fn delete_clipboard_data_items_by_kind(target_kind: &str) -> Result<(), String> {
    with_connection(|conn| {
        conn.execute(
            "DELETE FROM clipboard_data WHERE target_kind = ?1",
            params![target_kind],
        )?;
        Ok(())
    })
}

// 异步更新缺失的字符数
pub fn update_missing_char_counts(items: Vec<(i64, String, String)>) {
    if items.is_empty() {
        return;
    }

    std::thread::spawn(move || {
        let _ = with_connection(|conn| {
            for (id, content, content_type) in items {
                if let Some(char_count) = calculate_char_count(&content, &content_type) {
                    conn.execute(
                        "UPDATE clipboard SET char_count = ?1 WHERE id = ?2",
                        params![char_count, id],
                    )?;
                }
            }
            Ok(())
        });
    });
}

// 按逗号拆分图片ID
fn split_image_ids(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .map(|x| x.to_string())
        .collect()
}

// 检查图片ID是否仍被 clipboard 或 favorites 引用
fn is_image_id_referenced(
    conn: &rusqlite::Connection,
    image_id: &str,
) -> Result<bool, rusqlite::Error> {
    let exact = image_id;
    let p1 = format!("{},%", image_id);
    let p2 = format!("%,{},%", image_id);
    let p3 = format!("%,{}", image_id);

    let q = |table: &str| -> Result<bool, rusqlite::Error> {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE image_id = ?1 OR image_id LIKE ?2 OR image_id LIKE ?3 OR image_id LIKE ?4)",
            table
        );
        let exists: i64 = conn.query_row(&sql, params![exact, p1, p2, p3], |row| row.get(0))?;
        Ok(exists != 0)
    };

    Ok(q("clipboard")? || q("favorites")?)
}

// 删除图片文件
fn delete_image_files(image_ids: Vec<String>) -> Result<(), String> {
    if image_ids.is_empty() {
        return Ok(());
    }
    let data_dir = crate::services::get_data_directory()?;
    let images_dir = data_dir.join("clipboard_images");
    for iid in image_ids {
        let p = images_dir.join(format!("{}.png", iid));
        if p.exists() {
            let _ = std::fs::remove_file(&p);
        }
    }
    Ok(())
}

// 分页查询剪贴板历史
pub fn query_clipboard_items(
    params: QueryParams,
) -> Result<PaginatedResult<ClipboardItem>, String> {
    let search_keyword = params.search.clone();

    with_connection(|conn| {
        let mut where_clauses = vec![];
        let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(ref search) = search_keyword {
            if !search.trim().is_empty() {
                where_clauses.push("substr(content, 1, ?) LIKE ? ESCAPE '\\'");
                query_params.push(Box::new(SEARCH_CONTENT_PREFIX_LENGTH));
                let search_pattern = format!("%{}%", escape_like_pattern(search));
                query_params.push(Box::new(search_pattern));
            }
        }

        if let Some(ref content_type) = params.content_type {
            if content_type != "all" {
                where_clauses.push("content_type LIKE ?");
                let pattern = format!("%{}%", content_type);
                query_params.push(Box::new(pattern));
            }
        }

        let where_clause = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let query_sql = format!(
            "WITH filtered AS (
                SELECT id, is_pinned, item_order, updated_at
                FROM clipboard
                {}
             ),
             total AS (
                SELECT COUNT(*) AS total_count FROM filtered
             ),
             page_ids AS (
                SELECT id
                FROM filtered
                ORDER BY is_pinned DESC, item_order DESC, updated_at DESC
                LIMIT ? OFFSET ?
             )
             SELECT total.total_count, c.id, c.uuid, c.source_device_id, c.is_remote, c.content,
                    c.html_content, c.content_type, c.image_id, c.item_order, c.is_pinned,
                    c.paste_count, c.source_app, c.source_icon_hash, c.created_at, c.updated_at,
                    c.char_count
             FROM total
             LEFT JOIN page_ids ON 1 = 1
             LEFT JOIN clipboard c ON c.id = page_ids.id
             ORDER BY c.is_pinned DESC, c.item_order DESC, c.updated_at DESC",
            where_clause
        );

        query_params.push(Box::new(params.limit));
        query_params.push(Box::new(params.offset));

        let mut stmt = conn.prepare(&query_sql)?;

        let mut items_to_update: Vec<(i64, String, String)> = vec![];

        let items = stmt
            .query_map(
                rusqlite::params_from_iter(query_params.iter().map(|p| p.as_ref())),
                |row| {
                    let total_count: i64 = row.get(0)?;
                    let id: Option<i64> = row.get(1)?;
                    let Some(id) = id else {
                        return Ok((None, total_count));
                    };
                    let uuid: Option<String> = row.get(2)?;
                    let source_device_id: Option<String> = row.get(3)?;
                    let is_remote: i64 = row.get(4)?;
                    let content: String = row.get(5)?;
                    let html_content: Option<String> = row.get(6)?;
                    let content_type: String = row.get(7)?;
                    let char_count: Option<i64> = row.get(16)?;

                    let (truncated_content, truncated_html) = if content_type == "text"
                        || content_type == "rich_text"
                        || content_type == "link"
                    {
                        let truncated_content = if content.len() > MAX_CONTENT_LENGTH {
                            if let Some(ref keyword) = search_keyword {
                                if !keyword.trim().is_empty() {
                                    truncate_around_keyword(
                                        content.clone(),
                                        keyword,
                                        MAX_CONTENT_LENGTH,
                                    )
                                } else {
                                    truncate_string(content.clone(), MAX_CONTENT_LENGTH)
                                }
                            } else {
                                truncate_string(content.clone(), MAX_CONTENT_LENGTH)
                            }
                        } else {
                            content.clone()
                        };

                        let truncated_html = html_content.map(|h| {
                            if h.len() > MAX_CONTENT_LENGTH {
                                truncate_html(h, MAX_CONTENT_LENGTH)
                            } else {
                                h
                            }
                        });

                        (truncated_content, truncated_html)
                    } else {
                        (content.clone(), html_content)
                    };

                    let needs_char_count =
                        content_type.contains("text") || content_type.contains("rich_text");
                    let final_char_count =
                        if char_count.is_none() && needs_char_count && !content.is_empty() {
                            Some(content.chars().count() as i64)
                        } else {
                            char_count
                        };

                    Ok((
                        Some((
                            ClipboardItem {
                                id,
                                uuid,
                                source_device_id,
                                is_remote: is_remote != 0,
                                content: truncated_content,
                                html_content: truncated_html,
                                content_type: content_type.clone(),
                                image_id: row.get(8)?,
                                item_order: row.get(9)?,
                                is_pinned: row.get::<_, i64>(10)? != 0,
                                paste_count: row.get(11)?,
                                source_app: row.get(12)?,
                                source_icon_hash: row.get(13)?,
                                char_count: final_char_count,
                                created_at: row.get(14)?,
                                updated_at: row.get(15)?,
                            },
                            char_count.is_none() && needs_char_count,
                            id,
                            content,
                            content_type,
                        )),
                        total_count,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        let mut result_items = vec![];
        let mut total_count = 0;
        for (item, row_total_count) in items {
            total_count = row_total_count;
            if let Some((item, needs_update, id, content, content_type)) = item {
                if needs_update {
                    items_to_update.push((id, content, content_type));
                }
                result_items.push(item);
            }
        }

        if !items_to_update.is_empty() {
            update_missing_char_counts(items_to_update);
        }

        Ok(PaginatedResult::new(
            total_count,
            result_items,
            params.offset,
            params.limit,
        ))
    })
}

pub fn webdav_list_history_records(device_id: &str) -> Result<Vec<CloudRecord>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, uuid, source_device_id, is_remote, content, html_content, content_type,
                    image_id, item_order, is_pinned, paste_count, source_app, source_icon_hash,
                    char_count, created_at, updated_at
             FROM clipboard
             ORDER BY item_order DESC, updated_at DESC, id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let uuid_opt: Option<String> = row.get(1)?;
            let uuid = uuid_opt
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| id.to_string());
            let source_device_id = row
                .get::<_, Option<String>>(2)?
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| device_id.to_string());

            Ok(CloudRecord {
                uuid,
                source_device_id,
                is_remote: row.get::<_, i64>(3)? != 0,
                content: row.get(4)?,
                html_content: row.get(5)?,
                content_type: row.get(6)?,
                image_id: row.get(7)?,
                item_order: row.get(8)?,
                paste_count: row.get(10)?,
                source_app: row.get(11)?,
                source_icon_hash: row.get(12)?,
                char_count: row.get(13)?,
                title: String::new(),
                group_name: "全部".to_string(),
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })?;

        Ok(rows.filter_map(|row| row.ok()).collect())
    })
}

pub fn webdav_list_history_record_metas() -> Result<Vec<CloudRecordMeta>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, uuid, updated_at, image_id
             FROM clipboard
             ORDER BY item_order DESC, updated_at DESC, id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let uuid_opt: Option<String> = row.get(1)?;
            let uuid = uuid_opt
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| id.to_string());
            Ok(CloudRecordMeta {
                uuid,
                updated_at: row.get(2)?,
                image_id: row.get(3)?,
            })
        })?;

        Ok(rows.filter_map(|row| row.ok()).collect())
    })
}

pub fn webdav_get_history_record_by_uuid(
    uuid: &str,
    device_id: &str,
) -> Result<Option<CloudRecord>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, uuid, source_device_id, is_remote, content, html_content, content_type,
                    image_id, item_order, is_pinned, paste_count, source_app, source_icon_hash,
                    char_count, created_at, updated_at
             FROM clipboard
             WHERE uuid = ?1 OR ((uuid IS NULL OR uuid = '') AND id = ?2)
             LIMIT 1",
        )?;
        let id = uuid.parse::<i64>().ok();
        let record = stmt
            .query_row(params![uuid, id], |row| {
                let id: i64 = row.get(0)?;
                let uuid_opt: Option<String> = row.get(1)?;
                let uuid = uuid_opt
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| id.to_string());
                let source_device_id = row
                    .get::<_, Option<String>>(2)?
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| device_id.to_string());

                Ok(CloudRecord {
                    uuid,
                    source_device_id,
                    is_remote: row.get::<_, i64>(3)? != 0,
                    content: row.get(4)?,
                    html_content: row.get(5)?,
                    content_type: row.get(6)?,
                    image_id: row.get(7)?,
                    item_order: row.get(8)?,
                    paste_count: row.get(10)?,
                    source_app: row.get(11)?,
                    source_icon_hash: row.get(12)?,
                    char_count: row.get(13)?,
                    title: String::new(),
                    group_name: "全部".to_string(),
                    created_at: row.get(14)?,
                    updated_at: row.get(15)?,
                })
            })
            .optional()?;

        Ok(record)
    })
}

pub fn webdav_list_own_history_records(device_id: &str) -> Result<Vec<CloudRecord>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, uuid, source_device_id, is_remote, content, html_content, content_type,
                    image_id, item_order, is_pinned, paste_count, source_app, source_icon_hash,
                    char_count, created_at, updated_at
             FROM clipboard
             WHERE source_device_id IS NULL OR source_device_id = '' OR source_device_id = ?1
             ORDER BY item_order DESC, updated_at DESC, id DESC",
        )?;

        let rows = stmt.query_map(params![device_id], |row| {
            let id: i64 = row.get(0)?;
            let uuid_opt: Option<String> = row.get(1)?;
            let uuid = uuid_opt
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| id.to_string());

            Ok(CloudRecord {
                uuid,
                source_device_id: device_id.to_string(),
                is_remote: row.get::<_, i64>(3)? != 0,
                content: row.get(4)?,
                html_content: row.get(5)?,
                content_type: row.get(6)?,
                image_id: row.get(7)?,
                item_order: row.get(8)?,
                paste_count: row.get(10)?,
                source_app: row.get(11)?,
                source_icon_hash: row.get(12)?,
                char_count: row.get(13)?,
                title: String::new(),
                group_name: "全部".to_string(),
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })?;

        Ok(rows.filter_map(|row| row.ok()).collect())
    })
}

pub fn webdav_history_record_states() -> Result<HashMap<String, i64>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT uuid, updated_at FROM clipboard WHERE uuid IS NOT NULL AND uuid != ''",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut states = HashMap::new();
        for row in rows {
            let (uuid, updated_at) = row?;
            states.insert(uuid, updated_at);
        }
        Ok(states)
    })
}

pub fn lan_upsert_history_records(records: &[CloudRecord]) -> Result<Vec<CloudRecord>, String> {
    upsert_history_records(records, false)
}

pub fn webdav_repair_history_records(records: &[CloudRecord]) -> Result<Vec<CloudRecord>, String> {
    upsert_history_records(records, true)
}

fn upsert_history_records(
    records: &[CloudRecord],
    ignore_tombstones: bool,
) -> Result<Vec<CloudRecord>, String> {
    if records.is_empty() {
        return Ok(Vec::new());
    }

    with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        let mut changed = Vec::new();

        for record in records {
            if record.uuid.trim().is_empty() {
                continue;
            }
            let tombstone_deleted_at = super::tombstones::sync_tombstone_deleted_at_in_conn(
                &tx,
                super::tombstones::COLLECTION_HISTORY,
                &record.uuid,
            )?;
            if !ignore_tombstones
                && tombstone_deleted_at
                    .map(|value| value >= record.updated_at)
                    .unwrap_or(false)
            {
                continue;
            }
            let restored_updated_at = if ignore_tombstones {
                super::tombstones::restored_record_updated_at(
                    record.updated_at,
                    tombstone_deleted_at,
                )
            } else {
                record.updated_at
            };

            let existing = tx
                .query_row(
                    "SELECT COALESCE(source_device_id, ''), updated_at, content, html_content, content_type,
                            image_id, item_order, paste_count, source_app, source_icon_hash, char_count, created_at
                     FROM clipboard WHERE uuid = ?1 LIMIT 1",
                    params![record.uuid],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, i64>(6)?,
                            row.get::<_, i64>(7)?,
                            row.get::<_, Option<String>>(8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, Option<i64>>(10)?,
                            row.get::<_, i64>(11)?,
                        ))
                    },
                )
                .optional()?;

            if let Some((
                source_device_id,
                updated_at,
                content,
                html_content,
                content_type,
                image_id,
                item_order,
                paste_count,
                source_app,
                source_icon_hash,
                char_count,
                created_at,
            )) = existing
            {
                let same = source_device_id == record.source_device_id
                    && updated_at == restored_updated_at
                    && content == record.content
                    && html_content == record.html_content
                    && content_type == record.content_type
                    && image_id == record.image_id
                    && item_order == record.item_order
                    && paste_count == record.paste_count
                    && source_app == record.source_app
                    && source_icon_hash == record.source_icon_hash
                    && char_count == record.char_count
                    && created_at == record.created_at;

                if updated_at >= restored_updated_at || same {
                    if tombstone_deleted_at
                        .map(|deleted_at| deleted_at < updated_at)
                        .unwrap_or(false)
                    {
                        super::tombstones::delete_sync_tombstone_in_conn(
                            &tx,
                            super::tombstones::COLLECTION_HISTORY,
                            &record.uuid,
                        )?;
                    }
                    continue;
                }

                tx.execute(
                    "UPDATE clipboard SET
                        source_device_id = ?1,
                        is_remote = 1,
                        content = ?2,
                        html_content = ?3,
                        content_type = ?4,
                        image_id = ?5,
                        item_order = ?6,
                        paste_count = ?7,
                        source_app = ?8,
                        source_icon_hash = ?9,
                        char_count = ?10,
                        created_at = ?11,
                        updated_at = ?12
                     WHERE uuid = ?13",
                    params![
                        record.source_device_id,
                        record.content,
                        record.html_content,
                        record.content_type,
                        record.image_id,
                        record.item_order,
                        record.paste_count,
                        record.source_app,
                        record.source_icon_hash,
                        record.char_count,
                        record.created_at,
                        restored_updated_at,
                        record.uuid,
                    ],
                )?;
                if tombstone_deleted_at
                    .map(|deleted_at| deleted_at < restored_updated_at)
                    .unwrap_or(false)
                {
                    super::tombstones::delete_sync_tombstone_in_conn(
                        &tx,
                        super::tombstones::COLLECTION_HISTORY,
                        &record.uuid,
                    )?;
                }
                let mut changed_record = record.clone();
                changed_record.updated_at = restored_updated_at;
                changed.push(changed_record);
                continue;
            }

            tx.execute(
                "INSERT INTO clipboard (
                    uuid, source_device_id, is_remote, content, html_content, content_type,
                    image_id, item_order, is_pinned, paste_count, source_app, source_icon_hash,
                    char_count, created_at, updated_at
                 ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    record.uuid,
                    record.source_device_id,
                    record.content,
                    record.html_content,
                    record.content_type,
                    record.image_id,
                    record.item_order,
                    record.paste_count,
                    record.source_app,
                    record.source_icon_hash,
                    record.char_count,
                    record.created_at,
                    restored_updated_at,
                ],
            )?;
            if tombstone_deleted_at
                .map(|deleted_at| deleted_at < restored_updated_at)
                .unwrap_or(false)
            {
                super::tombstones::delete_sync_tombstone_in_conn(
                    &tx,
                    super::tombstones::COLLECTION_HISTORY,
                    &record.uuid,
                )?;
            }
            let mut changed_record = record.clone();
            changed_record.updated_at = restored_updated_at;
            changed.push(changed_record);
        }

        tx.commit()?;
        Ok(changed)
    })
}

// 获取剪贴板总数
pub fn get_clipboard_count() -> Result<i64, String> {
    with_connection(|conn| conn.query_row("SELECT COUNT(*) FROM clipboard", [], |row| row.get(0)))
}

pub fn get_clipboard_item_position(id: i64) -> Result<Option<i64>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id FROM clipboard ORDER BY is_pinned DESC, item_order DESC, updated_at DESC",
        )?;

        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids
            .iter()
            .position(|item_id| *item_id == id)
            .map(|index| index as i64))
    })
}

// 根据ID获取剪贴板项（完整内容，不截断）
pub fn get_clipboard_item_by_id(id: i64) -> Result<Option<ClipboardItem>, String> {
    get_clipboard_item_by_id_with_limit(id, None)
}

pub fn ensure_clipboard_item_uuid(id: i64) -> Result<String, String> {
    let maybe_uuid: Option<String> = with_connection(|conn| {
        let existing: Option<Option<String>> = conn
            .query_row(
                "SELECT uuid FROM clipboard WHERE id = ?1 LIMIT 1",
                params![id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;

        let existing = existing.flatten();

        if let Some(uuid) = existing.clone().filter(|u| !u.trim().is_empty()) {
            return Ok(Some(uuid));
        }

        let new_uuid = Uuid::new_v4().to_string();
        conn.execute(
            "UPDATE clipboard SET uuid = ?1 WHERE id = ?2 AND (uuid IS NULL OR uuid = '')",
            params![new_uuid, id],
        )?;

        let uuid: Option<Option<String>> = conn
            .query_row(
                "SELECT uuid FROM clipboard WHERE id = ?1 LIMIT 1",
                params![id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;

        Ok(uuid.flatten())
    })?;

    maybe_uuid
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| "生成 uuid 失败".to_string())
}

pub fn get_clipboard_item_id_by_uuid(uuid: &str) -> Result<Option<i64>, String> {
    with_connection(|conn| {
        conn.query_row(
            "SELECT id FROM clipboard WHERE uuid = ?1 LIMIT 1",
            params![uuid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.into())
    })
}

// 根据ID获取剪贴板项（指定截断长度）
pub fn get_clipboard_item_by_id_with_limit(
    id: i64,
    max_content_length: Option<usize>,
) -> Result<Option<ClipboardItem>, String> {
    with_connection(|conn| {
        conn.query_row(
            "SELECT id, uuid, source_device_id, is_remote, content, html_content, content_type, image_id, item_order, is_pinned, paste_count, source_app, source_icon_hash, created_at, updated_at, char_count 
             FROM clipboard WHERE id = ?",
            params![id],
            |row| {
                let uuid: Option<String> = row.get(1)?;
                let source_device_id: Option<String> = row.get(2)?;
                let is_remote: i64 = row.get(3)?;
                let content: String = row.get(4)?;
                let html_content: Option<String> = row.get(5)?;
                let content_type: String = row.get(6)?;
                let char_count: Option<i64> = row.get(15)?;
                let final_content = if let Some(max_len) = max_content_length {
                    let is_text_type = content_type == "text" || content_type == "rich_text" || content_type == "link";
                    if is_text_type && content.len() > max_len {
                        truncate_string(content.clone(), max_len)
                    } else {
                        content.clone()
                    }
                } else {
                    content.clone()
                };

                // 计算字符数
                let final_char_count = if char_count.is_none() && (content_type.contains("text") || content_type.contains("rich_text")) && !content.is_empty() {
                    Some(content.chars().count() as i64)
                } else {
                    char_count
                };

                Ok(ClipboardItem {
                    id: row.get(0)?,
                    uuid,
                    source_device_id,
                    is_remote: is_remote != 0,
                    content: final_content,
                    html_content,
                    content_type,
                    image_id: row.get(7)?,
                    item_order: row.get(8)?,
                    is_pinned: row.get::<_, i64>(9)? != 0,
                    paste_count: row.get(10)?,
                    source_app: row.get(11)?,
                    source_icon_hash: row.get(12)?,
                    char_count: final_char_count,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            }
        )
        .optional()
        .map_err(|e| e.into())
    })
}

pub fn increment_paste_count(id: i64) -> Result<(), String> {
    with_connection(|conn| {
        conn.execute(
            "UPDATE clipboard SET paste_count = paste_count + 1 WHERE id = ?",
            params![id],
        )?;
        Ok(())
    })
}

pub fn increment_paste_counts(ids: &[i64]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE clipboard SET paste_count = paste_count + 1 WHERE id = ?",
                params![id],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
}

// 限制剪贴板历史数量（删除超出限制的旧记录）
pub fn limit_clipboard_history(max_count: u64) -> Result<(), String> {
    if max_count >= 999999 {
        return Ok(());
    }

    let (images_to_delete, deleted_ids): (Vec<String>, Vec<i64>) = with_connection(|conn| {
        let sql_ids = "SELECT image_id FROM clipboard WHERE id NOT IN (SELECT id FROM clipboard ORDER BY is_pinned DESC, item_order DESC, updated_at DESC LIMIT ?1) AND image_id IS NOT NULL AND image_id <> ''";
        let mut stmt = conn.prepare(sql_ids)?;
        let ids_iter = stmt.query_map(params![max_count], |row| row.get::<_, String>(0))?;
        let mut set: HashSet<String> = HashSet::new();
        for r in ids_iter {
            if let Ok(s) = r {
                for iid in split_image_ids(&s) {
                    set.insert(iid);
                }
            }
        }
        drop(stmt);

        let mut delete_ids_stmt = conn.prepare(
            "SELECT id FROM clipboard WHERE id NOT IN (
                SELECT id FROM clipboard ORDER BY is_pinned DESC, item_order DESC, updated_at DESC LIMIT ?1
            )",
        )?;
        let deleted_ids = delete_ids_stmt
            .query_map(params![max_count], |row| row.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        conn.execute(
            "DELETE FROM clipboard WHERE id NOT IN (
                SELECT id FROM clipboard ORDER BY is_pinned DESC, item_order DESC, updated_at DESC LIMIT ?1
            )",
            params![max_count],
        )?;

        let mut to_delete = Vec::new();
        for iid in set.into_iter() {
            if !is_image_id_referenced(conn, &iid)? {
                to_delete.push(iid);
            }
        }
        Ok((to_delete, deleted_ids))
    })?;

    for id in deleted_ids {
        let _ = delete_clipboard_data_items("clipboard", &id.to_string());
    }

    delete_image_files(images_to_delete)
}

// 删除单个剪贴板项
pub fn delete_clipboard_item(id: i64) -> Result<(), String> {
    let images_to_delete: Vec<String> = with_connection(|conn| {
        let item: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT image_id, uuid FROM clipboard WHERE id = ?",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .optional()?;
        let Some((image_ids, uuid)) = item else {
            return Ok(Vec::new());
        };
        let deleted_at = chrono::Local::now().timestamp();
        let tombstone_id = uuid
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| id.to_string());
        super::tombstones::record_sync_tombstone_in_conn(
            conn,
            super::tombstones::COLLECTION_HISTORY,
            &tombstone_id,
            &crate::services::sync_transfer::device_id(),
            deleted_at,
        )?;

        conn.execute("DELETE FROM clipboard WHERE id = ?1", params![id])?;

        let mut to_delete = Vec::new();
        if let Some(ids) = image_ids {
            for iid in split_image_ids(&ids) {
                if !is_image_id_referenced(conn, &iid)? {
                    to_delete.push(iid);
                }
            }
        }
        Ok(to_delete)
    })?;

    let _ = delete_clipboard_data_items("clipboard", &id.to_string());
    delete_image_files(images_to_delete)
}

pub fn delete_clipboard_items(ids: &[i64]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let unique_ids: Vec<i64> = ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let images_to_delete: Vec<String> = with_connection(|conn| {
        let mut image_id_set: HashSet<String> = HashSet::new();
        let mut tombstone_ids = Vec::new();
        for id in &unique_ids {
            let item: Option<(Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT image_id, uuid FROM clipboard WHERE id = ?",
                    params![id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                        ))
                    },
                )
                .optional()?;

            if let Some((image_ids, uuid)) = item {
                if let Some(image_ids) = image_ids {
                    for image_id in split_image_ids(&image_ids) {
                        image_id_set.insert(image_id);
                    }
                }
                tombstone_ids.push(
                    uuid.filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| id.to_string()),
                );
            }
        }

        let tx = conn.unchecked_transaction()?;
        let deleted_at = chrono::Local::now().timestamp();
        let local_device_id = crate::services::sync_transfer::device_id();
        for uuid in &tombstone_ids {
            super::tombstones::record_sync_tombstone_in_conn(
                &tx,
                super::tombstones::COLLECTION_HISTORY,
                uuid,
                &local_device_id,
                deleted_at,
            )?;
        }
        for id in &unique_ids {
            tx.execute("DELETE FROM clipboard WHERE id = ?1", params![id])?;
        }
        tx.commit()?;

        let mut to_delete = Vec::new();
        for image_id in image_id_set {
            if !is_image_id_referenced(conn, &image_id)? {
                to_delete.push(image_id);
            }
        }

        Ok(to_delete)
    })?;

    for id in &unique_ids {
        let _ = delete_clipboard_data_items("clipboard", &id.to_string());
    }
    delete_image_files(images_to_delete)
}

// 清空所有剪贴板历史
pub fn clear_clipboard_history() -> Result<(), String> {
    let images_to_delete: Vec<String> = with_connection(|conn| {
        let mut stmt = conn.prepare("SELECT id, image_id, uuid FROM clipboard")?;
        let ids_iter = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut set: HashSet<String> = HashSet::new();
        let mut tombstone_ids = Vec::new();
        for r in ids_iter {
            if let Ok((id, image_ids, uuid)) = r {
                if let Some(image_ids) = image_ids {
                    for iid in split_image_ids(&image_ids) {
                        set.insert(iid);
                    }
                }
                tombstone_ids.push(
                    uuid.filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| id.to_string()),
                );
            }
        }
        drop(stmt);

        let tx = conn.unchecked_transaction()?;
        let deleted_at = chrono::Local::now().timestamp();
        let local_device_id = crate::services::sync_transfer::device_id();
        for uuid in tombstone_ids {
            super::tombstones::record_sync_tombstone_in_conn(
                &tx,
                super::tombstones::COLLECTION_HISTORY,
                &uuid,
                &local_device_id,
                deleted_at,
            )?;
        }
        tx.execute("DELETE FROM clipboard", [])?;
        tx.commit()?;

        let mut to_delete = Vec::new();
        for iid in set.into_iter() {
            if !is_image_id_referenced(conn, &iid)? {
                to_delete.push(iid);
            }
        }
        Ok(to_delete)
    })?;

    let _ = delete_clipboard_data_items_by_kind("clipboard");
    delete_image_files(images_to_delete)
}

// 排序逻辑
fn reorder_items(
    conn: &rusqlite::Connection,
    from_idx: usize,
    to_idx: usize,
    items: &[(i64, i64)],
) -> Result<(), rusqlite::Error> {
    if from_idx == to_idx {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    let now = chrono::Local::now().timestamp();
    let moved_id = items[from_idx].0;
    let target_order = items[to_idx].1;

    if from_idx < to_idx {
        for i in (from_idx + 1)..=to_idx {
            tx.execute(
                "UPDATE clipboard SET item_order = item_order + 1, updated_at = ?1 WHERE id = ?2",
                params![now, items[i].0],
            )?;
        }
    } else {
        for i in to_idx..from_idx {
            tx.execute(
                "UPDATE clipboard SET item_order = item_order - 1, updated_at = ?1 WHERE id = ?2",
                params![now, items[i].0],
            )?;
        }
    }
    tx.execute(
        "UPDATE clipboard SET item_order = ?1, updated_at = ?2 WHERE id = ?3",
        params![target_order, now, moved_id],
    )?;
    tx.commit()
}

// 移动剪贴板项到顶部（非置顶区的顶部）
pub fn move_clipboard_item_to_top(id: i64) -> Result<(), String> {
    with_connection(|conn| {
        let now = chrono::Local::now().timestamp();
        let max_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(item_order), 0) FROM clipboard WHERE is_pinned = 0",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        conn.execute(
            "UPDATE clipboard SET item_order = ?1, updated_at = ?2 WHERE id = ?3 AND is_pinned = 0",
            params![max_order + 1, now, id],
        )?;
        Ok(())
    })
}

// 移动剪贴板项
pub fn move_clipboard_item_by_id(from_id: i64, to_id: i64) -> Result<(), String> {
    if from_id == to_id {
        return Ok(());
    }

    with_connection(|conn| {
        let from_pinned: i64 = conn.query_row(
            "SELECT is_pinned FROM clipboard WHERE id = ?",
            params![from_id],
            |row| row.get(0),
        )?;
        let to_pinned: i64 = conn.query_row(
            "SELECT is_pinned FROM clipboard WHERE id = ?",
            params![to_id],
            |row| row.get(0),
        )?;

        if from_pinned != to_pinned {
            return Ok(());
        }

        let items: Vec<(i64, i64)> = conn.prepare("SELECT id, item_order FROM clipboard ORDER BY is_pinned DESC, item_order DESC, updated_at DESC")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let from_idx = items
            .iter()
            .position(|(id, _)| *id == from_id)
            .ok_or_else(|| {
                rusqlite::Error::InvalidParameterName(format!("ID {} 不存在", from_id))
            })?;
        let to_idx = items
            .iter()
            .position(|(id, _)| *id == to_id)
            .ok_or_else(|| rusqlite::Error::InvalidParameterName(format!("ID {} 不存在", to_id)))?;

        reorder_items(conn, from_idx, to_idx, &items)
    })
}

// 更新剪贴板项的内容
pub fn update_clipboard_item(
    id: i64,
    content: String,
    html_content: Option<String>,
) -> Result<(), String> {
    let should_clear_raw_formats = with_connection(|conn| {
        let (old_content, old_html_content): (String, Option<String>) = conn.query_row(
            "SELECT content, html_content FROM clipboard WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let content_changed = old_content != content;
        let html_changed = html_content
            .as_ref()
            .map(|new_html| old_html_content.as_deref() != Some(new_html.as_str()))
            .unwrap_or(false);

        let now = chrono::Local::now().timestamp();
        let rows = if let Some(ref html_content) = html_content {
            conn.execute(
                "UPDATE clipboard SET content = ?1, html_content = ?2, updated_at = ?3 WHERE id = ?4",
                params![&content, html_content, now, id],
            )?
        } else {
            conn.execute(
                "UPDATE clipboard SET content = ?1, updated_at = ?2 WHERE id = ?3",
                params![&content, now, id],
            )?
        };
        if rows == 0 {
            Err(rusqlite::Error::QueryReturnedNoRows)
        } else {
            Ok(content_changed || html_changed)
        }
    }).map_err(|e| if e.contains("QueryReturnedNoRows") {
        format!("剪贴板项不存在: {}", id)
    } else { e })?;

    if should_clear_raw_formats {
        delete_clipboard_data_items("clipboard", &id.to_string())?;
    }

    Ok(())
}

// 切换剪贴板项的置顶状态（置顶时放到置顶区第一位，取消置顶时移到非置顶区第一位）
pub fn toggle_pin_clipboard_item(id: i64) -> Result<bool, String> {
    with_connection(|conn| {
        let current_pinned: i64 = conn.query_row(
            "SELECT is_pinned FROM clipboard WHERE id = ?",
            params![id],
            |row| row.get(0),
        )?;

        let now = chrono::Local::now().timestamp();
        if current_pinned == 0 {
            let max_pinned_order: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(item_order), 0) FROM clipboard WHERE is_pinned = 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            conn.execute("UPDATE clipboard SET is_pinned = 1, item_order = ?1, updated_at = ?2 WHERE id = ?3", params![max_pinned_order + 1, now, id])?;
            Ok(true)
        } else {
            let max_order: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(item_order), 0) FROM clipboard WHERE is_pinned = 0",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            conn.execute("UPDATE clipboard SET is_pinned = 0, item_order = ?1, updated_at = ?2 WHERE id = ?3", params![max_order + 1, now, id])?;
            Ok(false)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::connection::{close_database, init_database};
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static TEST_DB_LOCK: Mutex<()> = Mutex::new(());

    struct TestDatabase {
        _guard: MutexGuard<'static, ()>,
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            close_database();
        }
    }

    fn setup_test_database() -> TestDatabase {
        let guard = TEST_DB_LOCK.lock().expect("test database lock poisoned");
        close_database();
        init_database(":memory:").expect("init test database");
        TestDatabase { _guard: guard }
    }

    fn seed_text_items(contents: Vec<String>) {
        with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;
            for (index, content) in contents.iter().enumerate() {
                let now = 1_700_000_000 + index as i64;
                tx.execute(
                    "INSERT INTO clipboard (
                        content, content_type, item_order, is_pinned, paste_count,
                        char_count, created_at, updated_at
                     ) VALUES (?1, 'text', ?2, 0, 0, ?3, ?4, ?5)",
                    rusqlite::params![
                        content,
                        index as i64,
                        content.chars().count() as i64,
                        now,
                        now,
                    ],
                )?;
            }
            tx.commit()
        })
        .expect("seed clipboard items");
    }

    fn search_clipboard(search: &str, offset: i64, limit: i64) -> PaginatedResult<ClipboardItem> {
        query_clipboard_items(QueryParams {
            offset,
            limit,
            search: Some(search.to_string()),
            content_type: None,
        })
        .expect("query clipboard items")
    }

    #[test]
    fn search_large_history_returns_paginated_total() {
        let _db = setup_test_database();
        let long_tail = "x".repeat(SEARCH_CONTENT_PREFIX_LENGTH as usize + 512);
        let contents = (0..3_000)
            .map(|index| {
                if index % 100 == 0 {
                    format!("needle item {} {}", index, long_tail)
                } else {
                    format!("regular item {} {}", index, long_tail)
                }
            })
            .collect();
        seed_text_items(contents);

        let result = search_clipboard("needle", 3, 7);

        assert_eq!(result.total_count, 30);
        assert_eq!(result.items.len(), 7);
        assert!(result.has_more);
        assert!(result
            .items
            .iter()
            .all(|item| item.content.contains("needle")));
    }

    #[test]
    fn search_like_metacharacters_match_literally() {
        let _db = setup_test_database();
        seed_text_items(vec![
            "literal %_ marker".to_string(),
            "literal ax marker".to_string(),
            "literal percent underscore marker".to_string(),
        ]);

        let result = search_clipboard("%_", 0, 10);

        assert_eq!(result.total_count, 1);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].content, "literal %_ marker");
    }

    #[test]
    fn empty_result_then_broader_query_repopulates() {
        let _db = setup_test_database();
        seed_text_items(vec![
            "alpha first".to_string(),
            "beta second".to_string(),
            "alpha third".to_string(),
        ]);

        let empty = search_clipboard("does-not-exist", 0, 10);
        assert_eq!(empty.total_count, 0);
        assert!(empty.items.is_empty());

        let broader = search_clipboard("alpha", 0, 10);
        assert_eq!(broader.total_count, 2);
        assert_eq!(broader.items.len(), 2);
        assert!(broader
            .items
            .iter()
            .all(|item| item.content.contains("alpha")));
    }
}
