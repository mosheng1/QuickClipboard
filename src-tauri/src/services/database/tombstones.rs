use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};

use super::connection::with_connection;
use crate::services::webdav_sync::types::{CloudGroup, CloudRecord, CloudRecordMeta};

pub const COLLECTION_HISTORY: &str = "history";
pub const COLLECTION_FAVORITES: &str = "favorites";
pub const COLLECTION_GROUPS: &str = "groups";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncTombstone {
    pub collection: String,
    pub item_id: String,
    pub source_device_id: String,
    pub deleted_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct SyncTombstoneApplyReport {
    pub history: u32,
    pub favorites: u32,
    pub groups: u32,
}

impl SyncTombstoneApplyReport {
    pub fn total(&self) -> u32 {
        self.history + self.favorites + self.groups
    }
}

pub(crate) fn record_sync_tombstone_in_conn(
    conn: &rusqlite::Connection,
    collection: &str,
    item_id: &str,
    source_device_id: &str,
    deleted_at: i64,
) -> Result<(), rusqlite::Error> {
    if collection.trim().is_empty() || item_id.trim().is_empty() {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO sync_tombstones (collection, item_id, source_device_id, deleted_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(collection, item_id) DO UPDATE SET
            source_device_id = excluded.source_device_id,
            deleted_at = excluded.deleted_at,
            created_at = excluded.created_at
         WHERE excluded.deleted_at >= sync_tombstones.deleted_at",
        params![collection, item_id, source_device_id, deleted_at],
    )?;
    Ok(())
}

pub(crate) fn sync_tombstone_deleted_at_in_conn(
    conn: &rusqlite::Connection,
    collection: &str,
    item_id: &str,
) -> Result<Option<i64>, rusqlite::Error> {
    conn.query_row(
        "SELECT deleted_at FROM sync_tombstones WHERE collection = ?1 AND item_id = ?2",
        params![collection, item_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
}

pub(crate) fn delete_sync_tombstone_in_conn(
    conn: &rusqlite::Connection,
    collection: &str,
    item_id: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM sync_tombstones WHERE collection = ?1 AND item_id = ?2",
        params![collection, item_id],
    )?;
    Ok(())
}

pub(crate) fn restored_record_updated_at(
    updated_at: i64,
    tombstone_deleted_at: Option<i64>,
) -> i64 {
    let now = chrono::Utc::now().timestamp();
    match tombstone_deleted_at {
        Some(deleted_at) => updated_at.max(now).max(deleted_at.saturating_add(1)),
        None => updated_at.max(now),
    }
}

pub fn list_sync_tombstones_since(
    since_deleted_at: Option<i64>,
) -> Result<Vec<SyncTombstone>, String> {
    with_connection(|conn| {
        let mut tombstones = Vec::new();
        if let Some(since_deleted_at) = since_deleted_at {
            let mut stmt = conn.prepare(
                "SELECT collection, item_id, source_device_id, deleted_at, created_at
                 FROM sync_tombstones
                 WHERE deleted_at > ?1
                 ORDER BY deleted_at ASC",
            )?;
            let rows = stmt.query_map(params![since_deleted_at], sync_tombstone_from_row)?;
            for row in rows {
                tombstones.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT collection, item_id, source_device_id, deleted_at, created_at
                 FROM sync_tombstones
                 ORDER BY deleted_at ASC",
            )?;
            let rows = stmt.query_map([], sync_tombstone_from_row)?;
            for row in rows {
                tombstones.push(row?);
            }
        }
        Ok(tombstones)
    })
}

pub fn sync_tombstone_states() -> Result<HashMap<String, i64>, String> {
    with_connection(|conn| {
        let mut stmt =
            conn.prepare("SELECT collection, item_id, deleted_at FROM sync_tombstones")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut states = HashMap::new();
        for row in rows {
            let (collection, item_id, deleted_at) = row?;
            states.insert(tombstone_state_key(&collection, &item_id), deleted_at);
        }
        Ok(states)
    })
}

pub fn upsert_sync_tombstones(tombstones: &[SyncTombstone]) -> Result<Vec<SyncTombstone>, String> {
    if tombstones.is_empty() {
        return Ok(Vec::new());
    }

    with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        let mut changed = Vec::new();
        for tombstone in tombstones {
            if tombstone.collection.trim().is_empty() || tombstone.item_id.trim().is_empty() {
                continue;
            }

            let existing_deleted_at = tx
                .query_row(
                    "SELECT deleted_at FROM sync_tombstones WHERE collection = ?1 AND item_id = ?2",
                    params![tombstone.collection, tombstone.item_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if existing_deleted_at
                .map(|value| value >= tombstone.deleted_at)
                .unwrap_or(false)
            {
                continue;
            }

            tx.execute(
                "INSERT INTO sync_tombstones (collection, item_id, source_device_id, deleted_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(collection, item_id) DO UPDATE SET
                    source_device_id = excluded.source_device_id,
                    deleted_at = excluded.deleted_at,
                    created_at = excluded.created_at",
                params![
                    tombstone.collection,
                    tombstone.item_id,
                    tombstone.source_device_id,
                    tombstone.deleted_at,
                    tombstone.created_at,
                ],
            )?;
            changed.push(tombstone.clone());
        }
        tx.commit()?;
        Ok(changed)
    })
}

pub fn apply_sync_tombstones(
    tombstones: &[SyncTombstone],
) -> Result<SyncTombstoneApplyReport, String> {
    if tombstones.is_empty() {
        return Ok(SyncTombstoneApplyReport::default());
    }

    let (report, images_to_delete) = with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        let mut report = SyncTombstoneApplyReport::default();
        let mut image_ids = Vec::new();

        for tombstone in tombstones {
            match tombstone.collection.as_str() {
                COLLECTION_HISTORY => {
                    if delete_history_by_tombstone(&tx, tombstone, &mut image_ids)? {
                        report.history += 1;
                    }
                }
                COLLECTION_FAVORITES => {
                    if delete_favorite_by_tombstone(&tx, tombstone, &mut image_ids)? {
                        report.favorites += 1;
                    }
                }
                COLLECTION_GROUPS => {
                    if delete_group_by_tombstone(&tx, tombstone)? {
                        report.groups += 1;
                    }
                }
                _ => {}
            }
        }

        tx.commit()?;
        Ok((report, image_ids))
    })?;

    delete_unreferenced_image_files(images_to_delete)?;
    Ok(report)
}

pub fn filter_records_not_deleted(
    collection: &str,
    records: &[CloudRecord],
) -> Result<Vec<CloudRecord>, String> {
    let states = sync_tombstone_states()?;
    Ok(records
        .iter()
        .filter(|record| {
            states
                .get(&tombstone_state_key(collection, &record.uuid))
                .map(|deleted_at| *deleted_at < record.updated_at)
                .unwrap_or(true)
        })
        .cloned()
        .collect())
}

pub fn filter_groups_not_deleted(groups: &[CloudGroup]) -> Result<Vec<CloudGroup>, String> {
    let states = sync_tombstone_states()?;
    Ok(groups
        .iter()
        .filter(|group| {
            states
                .get(&tombstone_state_key(COLLECTION_GROUPS, &group.name))
                .map(|deleted_at| *deleted_at < group.updated_at)
                .unwrap_or(true)
        })
        .cloned()
        .collect())
}

pub fn filter_record_metas_not_deleted_by_states(
    collection: &str,
    records: Vec<CloudRecordMeta>,
    tombstone_states: &HashMap<String, i64>,
) -> Vec<CloudRecordMeta> {
    records
        .into_iter()
        .filter(|record| {
            tombstone_states
                .get(&tombstone_state_key(collection, &record.uuid))
                .map(|deleted_at| *deleted_at < record.updated_at)
                .unwrap_or(true)
        })
        .collect()
}

pub fn filter_groups_not_deleted_by_states(
    groups: Vec<CloudGroup>,
    tombstone_states: &HashMap<String, i64>,
) -> Vec<CloudGroup> {
    groups
        .into_iter()
        .filter(|group| {
            tombstone_states
                .get(&tombstone_state_key(COLLECTION_GROUPS, &group.name))
                .map(|deleted_at| *deleted_at < group.updated_at)
                .unwrap_or(true)
        })
        .collect()
}

pub fn tombstones_newer_than_remote(
    tombstones: Vec<SyncTombstone>,
    remote_states: &HashMap<String, i64>,
) -> Vec<SyncTombstone> {
    tombstones
        .into_iter()
        .filter(|tombstone| {
            remote_states
                .get(&tombstone_state_key(
                    &tombstone.collection,
                    &tombstone.item_id,
                ))
                .map(|remote_deleted_at| *remote_deleted_at < tombstone.deleted_at)
                .unwrap_or(true)
        })
        .collect()
}

pub fn tombstone_state_key(collection: &str, item_id: &str) -> String {
    format!("{}:{}", collection, item_id)
}

fn sync_tombstone_from_row(row: &rusqlite::Row<'_>) -> Result<SyncTombstone, rusqlite::Error> {
    Ok(SyncTombstone {
        collection: row.get(0)?,
        item_id: row.get(1)?,
        source_device_id: row.get(2)?,
        deleted_at: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn delete_history_by_tombstone(
    conn: &rusqlite::Connection,
    tombstone: &SyncTombstone,
    image_ids: &mut Vec<String>,
) -> Result<bool, rusqlite::Error> {
    let existing = conn
        .query_row(
            "SELECT id, image_id, updated_at FROM clipboard
             WHERE uuid = ?1 OR (uuid IS NULL OR uuid = '') AND CAST(id AS TEXT) = ?1
             LIMIT 1",
            params![tombstone.item_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;

    let Some((id, image_id, updated_at)) = existing else {
        return Ok(false);
    };
    if updated_at > tombstone.deleted_at {
        return Ok(false);
    }

    conn.execute("DELETE FROM clipboard WHERE id = ?1", params![id])?;
    conn.execute(
        "DELETE FROM clipboard_data WHERE target_kind = 'clipboard' AND target_id = ?1",
        params![id.to_string()],
    )?;
    push_split_image_ids(image_ids, image_id);
    Ok(true)
}

fn delete_favorite_by_tombstone(
    conn: &rusqlite::Connection,
    tombstone: &SyncTombstone,
    image_ids: &mut Vec<String>,
) -> Result<bool, rusqlite::Error> {
    let existing = conn
        .query_row(
            "SELECT image_id, updated_at FROM favorites WHERE id = ?1 LIMIT 1",
            params![tombstone.item_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;

    let Some((image_id, updated_at)) = existing else {
        return Ok(false);
    };
    if updated_at > tombstone.deleted_at {
        return Ok(false);
    }

    conn.execute(
        "DELETE FROM favorites WHERE id = ?1",
        params![tombstone.item_id],
    )?;
    conn.execute(
        "DELETE FROM clipboard_data WHERE target_kind = 'favorite' AND target_id = ?1",
        params![tombstone.item_id],
    )?;
    push_split_image_ids(image_ids, image_id);
    Ok(true)
}

fn delete_group_by_tombstone(
    conn: &rusqlite::Connection,
    tombstone: &SyncTombstone,
) -> Result<bool, rusqlite::Error> {
    if tombstone.item_id == "全部" {
        return Ok(false);
    }

    let updated_at = conn
        .query_row(
            "SELECT updated_at FROM groups WHERE name = ?1 LIMIT 1",
            params![tombstone.item_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let Some(updated_at) = updated_at else {
        return Ok(false);
    };
    if updated_at > tombstone.deleted_at {
        return Ok(false);
    }

    conn.execute(
        "UPDATE favorites SET group_name = '全部' WHERE group_name = ?1",
        params![tombstone.item_id],
    )?;
    conn.execute(
        "DELETE FROM groups WHERE name = ?1",
        params![tombstone.item_id],
    )?;
    Ok(true)
}

fn push_split_image_ids(output: &mut Vec<String>, image_ids: Option<String>) {
    if let Some(image_ids) = image_ids {
        output.extend(
            image_ids
                .split(',')
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string()),
        );
    }
}

fn delete_unreferenced_image_files(image_ids: Vec<String>) -> Result<(), String> {
    if image_ids.is_empty() {
        return Ok(());
    }

    let data_dir = crate::services::get_data_directory()?;
    let images_dir = data_dir.join("clipboard_images");
    with_connection(|conn| {
        for image_id in image_ids {
            if is_image_id_referenced(conn, &image_id)? {
                continue;
            }
            let path = images_dir.join(format!("{}.png", image_id));
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(())
    })
}

fn is_image_id_referenced(
    conn: &rusqlite::Connection,
    image_id: &str,
) -> Result<bool, rusqlite::Error> {
    let exact = image_id;
    let prefix = format!("{},%", image_id);
    let middle = format!("%,{},%", image_id);
    let suffix = format!("%,{}", image_id);

    let query = |table: &str| -> Result<bool, rusqlite::Error> {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE image_id = ?1 OR image_id LIKE ?2 OR image_id LIKE ?3 OR image_id LIKE ?4)",
            table
        );
        let exists: i64 = conn.query_row(&sql, params![exact, prefix, middle, suffix], |row| {
            row.get(0)
        })?;
        Ok(exists != 0)
    };

    Ok(query("clipboard")? || query("favorites")?)
}
