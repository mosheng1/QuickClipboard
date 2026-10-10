use super::connection::with_connection;
use super::models::GroupInfo;
use crate::services::webdav_sync::types::CloudGroup;
use chrono;
use rusqlite::{params, OptionalExtension};

const DEFAULT_GROUP_COLOR: &str = "#dc2626";
const DEFAULT_GROUP_ICON: &str = "ti ti-folder";

// 获取所有分组
pub fn webdav_list_groups(device_id: &str) -> Result<Vec<CloudGroup>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT g.name, g.icon, g.color, g.order_index, COALESCE(g.source_device_id, ''),
                    g.created_at, g.updated_at
             FROM groups g
             ORDER BY g.order_index, g.name",
        )?;
        let rows = stmt.query_map([], |row| {
            let source_device_id = row.get::<_, String>(4)?.trim().to_string();
            let icon = row.get::<_, String>(1)?;
            Ok(CloudGroup {
                name: row.get(0)?,
                icon: normalize_group_icon(&icon),
                color: normalize_group_color(&row.get::<_, String>(2)?),
                order: row.get(3)?,
                source_device_id: if source_device_id.is_empty() {
                    device_id.to_string()
                } else {
                    source_device_id
                },
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    })
}

pub fn lan_save_groups(groups: &[CloudGroup]) -> Result<Vec<CloudGroup>, String> {
    save_groups(groups, false)
}

pub fn webdav_repair_groups(groups: &[CloudGroup]) -> Result<Vec<CloudGroup>, String> {
    save_groups(groups, true)
}

fn save_groups(groups: &[CloudGroup], ignore_tombstones: bool) -> Result<Vec<CloudGroup>, String> {
    if groups.is_empty() {
        return Ok(Vec::new());
    }

    with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        let mut changed = Vec::new();

        for group in groups {
            let tombstone_deleted_at = super::tombstones::sync_tombstone_deleted_at_in_conn(
                &tx,
                super::tombstones::COLLECTION_GROUPS,
                &group.name,
            )?;
            if !ignore_tombstones
                && tombstone_deleted_at
                    .map(|value| value >= group.updated_at)
                    .unwrap_or(false)
            {
                continue;
            }
            let restored_updated_at = if ignore_tombstones {
                super::tombstones::restored_record_updated_at(
                    group.updated_at,
                    tombstone_deleted_at,
                )
            } else {
                group.updated_at
            };
            let incoming_icon = normalize_group_icon(&group.icon);

            let existing = tx
                .query_row(
                    "SELECT icon, color, order_index, COALESCE(source_device_id, ''), created_at, updated_at
                     FROM groups WHERE name = ?1 LIMIT 1",
                    params![group.name],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i32>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, i64>(5)?,
                        ))
                    },
                )
                .optional()?;

            if let Some((icon, color, order, source_device_id, created_at, updated_at)) = existing {
                let existing_icon = normalize_group_icon(&icon);
                let incoming_icon = merge_group_icon(&icon, &group.icon);
                let existing_color = normalize_group_color(&color);
                let incoming_color = normalize_incoming_group_color(&group.color)
                    .unwrap_or_else(|| existing_color.clone());
                let should_repair_existing_icon = icon != existing_icon;
                let should_repair_existing_color = !is_canonical_group_color(&color);
                let repair_icon = existing_icon.clone();
                let repair_color = if is_empty_or_transparent_group_color(&color)
                    && !is_empty_or_transparent_group_color(&group.color)
                {
                    incoming_color.clone()
                } else {
                    existing_color.clone()
                };
                let same = existing_icon == incoming_icon
                    && existing_color == incoming_color
                    && order == group.order
                    && source_device_id == group.source_device_id
                    && created_at == group.created_at
                    && updated_at == restored_updated_at;

                if updated_at >= restored_updated_at {
                    if should_repair_existing_icon || should_repair_existing_color {
                        tx.execute(
                            "UPDATE groups SET icon = ?1, color = ?2 WHERE name = ?3",
                            params![repair_icon, repair_color, group.name],
                        )?;
                        let mut changed_group = group.clone();
                        changed_group.icon = repair_icon;
                        changed_group.color = repair_color;
                        changed_group.updated_at = updated_at;
                        changed.push(changed_group);
                    }
                    if tombstone_deleted_at
                        .map(|deleted_at| deleted_at < updated_at)
                        .unwrap_or(false)
                    {
                        super::tombstones::delete_sync_tombstone_in_conn(
                            &tx,
                            super::tombstones::COLLECTION_GROUPS,
                            &group.name,
                        )?;
                    }
                    continue;
                }

                if same {
                    if tombstone_deleted_at
                        .map(|deleted_at| deleted_at < updated_at)
                        .unwrap_or(false)
                    {
                        super::tombstones::delete_sync_tombstone_in_conn(
                            &tx,
                            super::tombstones::COLLECTION_GROUPS,
                            &group.name,
                        )?;
                    }
                    continue;
                }

                tx.execute(
                    "UPDATE groups SET
                        icon = ?1,
                        color = ?2,
                        order_index = ?3,
                        source_device_id = ?4,
                        created_at = ?5,
                        updated_at = ?6
                     WHERE name = ?7",
                    params![
                        incoming_icon,
                        incoming_color,
                        group.order,
                        group.source_device_id,
                        group.created_at,
                        restored_updated_at,
                        group.name,
                    ],
                )?;
                if tombstone_deleted_at
                    .map(|deleted_at| deleted_at < restored_updated_at)
                    .unwrap_or(false)
                {
                    super::tombstones::delete_sync_tombstone_in_conn(
                        &tx,
                        super::tombstones::COLLECTION_GROUPS,
                        &group.name,
                    )?;
                }
                let mut changed_group = group.clone();
                changed_group.icon = incoming_icon;
                changed_group.color = incoming_color;
                changed_group.updated_at = restored_updated_at;
                changed.push(changed_group);
                continue;
            }

            let incoming_color = normalize_incoming_group_color(&group.color)
                .unwrap_or_else(|| DEFAULT_GROUP_COLOR.to_string());
            tx.execute(
                "INSERT INTO groups (name, icon, color, order_index, source_device_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    group.name,
                    incoming_icon,
                    incoming_color,
                    group.order,
                    group.source_device_id,
                    group.created_at,
                    restored_updated_at,
                ],
            )?;
            if tombstone_deleted_at
                .map(|deleted_at| deleted_at < restored_updated_at)
                .unwrap_or(false)
            {
                super::tombstones::delete_sync_tombstone_in_conn(
                    &tx,
                    super::tombstones::COLLECTION_GROUPS,
                    &group.name,
                )?;
            }
            let mut changed_group = group.clone();
            changed_group.icon = incoming_icon;
            changed_group.color = incoming_color;
            changed_group.updated_at = restored_updated_at;
            changed.push(changed_group);
        }

        tx.commit()?;
        Ok(changed)
    })
}

// 获取所有分组
pub fn get_all_groups() -> Result<Vec<GroupInfo>, String> {
    with_connection(|conn| {
        let mut groups = Vec::new();

        let mut stmt = conn.prepare(
            "SELECT name, icon, color, order_index FROM groups ORDER BY order_index, name",
        )?;
        let group_rows: Vec<(String, String, String, i32)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        drop(stmt);

        for (name, icon, color, order) in group_rows {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM favorites WHERE group_name = ?1",
                params![&name],
                |row| row.get(0),
            )?;

            groups.push(GroupInfo {
                name,
                icon: normalize_group_icon(&icon),
                color: normalize_group_color(&color),
                order,
                item_count: count,
            });
        }

        Ok(groups)
    })
}

// 添加分组
pub fn add_group(name: String, icon: String, color: String) -> Result<GroupInfo, String> {
    with_connection(|conn| {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM groups WHERE name = ?1",
            params![&name],
            |row| row.get(0),
        )?;

        if exists > 0 {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "分组 '{}' 已存在",
                name
            )));
        }

        let max_order: Option<i32> = conn
            .query_row("SELECT MAX(order_index) FROM groups", [], |row| row.get(0))
            .ok()
            .flatten();

        let new_order = max_order.unwrap_or(0) + 1;
        let now = chrono::Local::now().timestamp();
        let icon = normalize_group_icon(&icon);
        let color = normalize_group_color(&color);

        conn.execute(
            "INSERT INTO groups (name, icon, color, order_index, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![&name, &icon, &color, new_order, now, now],
        )?;

        Ok(GroupInfo {
            name,
            icon,
            color,
            order: new_order,
            item_count: 0,
        })
    })
}

// 更新分组
pub fn update_group(
    old_name: String,
    new_name: String,
    new_icon: String,
    new_color: String,
) -> Result<GroupInfo, String> {
    with_connection(|conn| {
        if old_name != new_name {
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM groups WHERE name = ?1",
                params![&new_name],
                |row| row.get(0),
            )?;

            if exists > 0 {
                return Err(rusqlite::Error::InvalidParameterName(format!(
                    "分组 '{}' 已存在",
                    new_name
                )));
            }
        }

        let now = chrono::Local::now().timestamp();
        let new_icon = normalize_group_icon(&new_icon);
        let new_color = normalize_group_color(&new_color);
        let tx = conn.unchecked_transaction()?;

        tx.execute(
            "UPDATE groups SET name = ?1, icon = ?2, color = ?3, updated_at = ?4 WHERE name = ?5",
            params![&new_name, &new_icon, &new_color, now, &old_name],
        )?;

        if old_name != new_name {
            tx.execute(
                "UPDATE favorites SET group_name = ?1 WHERE group_name = ?2",
                params![&new_name, &old_name],
            )?;
        }

        tx.commit()?;

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM favorites WHERE group_name = ?1",
            params![&new_name],
            |row| row.get(0),
        )?;

        let (order, color): (i32, String) = conn.query_row(
            "SELECT order_index, color FROM groups WHERE name = ?1",
            params![&new_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        Ok(GroupInfo {
            name: new_name,
            icon: new_icon,
            color: normalize_group_color(&color),
            order,
            item_count: count,
        })
    })
}

// 删除分组
pub fn delete_group(name: String) -> Result<(), String> {
    with_connection(|conn| {
        if name == "全部" {
            return Err(rusqlite::Error::InvalidParameterName(
                "不能删除'全部'分组".to_string(),
            ));
        }

        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM groups WHERE name = ?1",
            params![&name],
            |row| row.get(0),
        )?;

        let tx = conn.unchecked_transaction()?;
        if exists > 0 {
            super::tombstones::record_sync_tombstone_in_conn(
                &tx,
                super::tombstones::COLLECTION_GROUPS,
                &name,
                &crate::services::sync_transfer::device_id(),
                chrono::Local::now().timestamp(),
            )?;
        }

        tx.execute(
            "UPDATE favorites SET group_name = '全部' WHERE group_name = ?1",
            params![&name],
        )?;

        tx.execute("DELETE FROM groups WHERE name = ?1", params![&name])?;

        tx.commit()?;
        Ok(())
    })
}

fn normalize_group_color(raw: &str) -> String {
    normalize_incoming_group_color(raw).unwrap_or_else(|| DEFAULT_GROUP_COLOR.to_string())
}

fn normalize_group_icon(raw: &str) -> String {
    let icon = raw.trim();
    if icon.is_empty() {
        DEFAULT_GROUP_ICON.to_string()
    } else {
        icon.to_string()
    }
}

fn merge_group_icon(existing: &str, incoming: &str) -> String {
    if incoming.trim().is_empty() && !existing.trim().is_empty() {
        normalize_group_icon(existing)
    } else {
        normalize_group_icon(incoming)
    }
}

fn normalize_incoming_group_color(raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() || text == "0" {
        return None;
    }

    if let Some(hex) = text.strip_prefix('#') {
        return normalize_hex_group_color(hex);
    }

    parse_numeric_group_color(text).map(rgb_to_hex)
}

fn normalize_hex_group_color(hex: &str) -> Option<String> {
    if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }

    match hex.len() {
        3 => {
            let mut rgb = String::with_capacity(6);
            for ch in hex.chars() {
                rgb.push(ch);
                rgb.push(ch);
            }
            Some(format!("#{}", rgb.to_ascii_lowercase()))
        }
        6 => Some(format!("#{}", hex.to_ascii_lowercase())),
        8 => {
            let alpha = u8::from_str_radix(&hex[0..2], 16).ok()?;
            if alpha == 0 {
                return None;
            }
            Some(format!("#{}", hex[2..8].to_ascii_lowercase()))
        }
        _ => None,
    }
}

fn parse_numeric_group_color(text: &str) -> Option<u32> {
    let value = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()?
    } else {
        text.parse::<i64>().ok()?
    };

    let unsigned = value as u32;
    if unsigned == 0 {
        return None;
    }

    if unsigned <= 0x00ff_ffff {
        Some(unsigned)
    } else {
        let alpha = (unsigned >> 24) & 0xff;
        if alpha == 0 {
            None
        } else {
            Some(unsigned & 0x00ff_ffff)
        }
    }
}

fn rgb_to_hex(rgb: u32) -> String {
    format!("#{:06x}", rgb & 0x00ff_ffff)
}

fn is_canonical_group_color(raw: &str) -> bool {
    let text = raw.trim();
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_empty_or_transparent_group_color(raw: &str) -> bool {
    let text = raw.trim();
    if text.is_empty() || text == "0" {
        return true;
    }

    if let Some(hex) = text.strip_prefix('#') {
        if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return true;
        }
        return matches!(hex.len(), 8) && u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) == 0;
    }

    parse_numeric_group_color(text).is_none()
}

#[cfg(test)]
mod tests {
    use super::{merge_group_icon, normalize_group_color, normalize_group_icon};

    #[test]
    fn normalizes_group_color_variants() {
        assert_eq!(normalize_group_color("#DC2626"), "#dc2626");
        assert_eq!(normalize_group_color("#f00"), "#ff0000");
        assert_eq!(normalize_group_color("#ff3b82f6"), "#3b82f6");
        assert_eq!(normalize_group_color("4282090230"), "#3b82f6");
        assert_eq!(normalize_group_color("-2349530"), "#dc2626");
        assert_eq!(normalize_group_color("0"), "#dc2626");
        assert_eq!(normalize_group_color("#003b82f6"), "#dc2626");
    }

    #[test]
    fn normalizes_empty_group_icon() {
        assert_eq!(normalize_group_icon(""), "ti ti-folder");
        assert_eq!(normalize_group_icon("   "), "ti ti-folder");
        assert_eq!(normalize_group_icon(" ti ti-star "), "ti ti-star");
    }

    #[test]
    fn keeps_existing_group_icon_when_incoming_empty() {
        assert_eq!(merge_group_icon("ti ti-star", ""), "ti ti-star");
        assert_eq!(merge_group_icon("", ""), "ti ti-folder");
        assert_eq!(merge_group_icon("ti ti-star", "ti ti-tag"), "ti ti-tag");
    }
}

// 更新分组排序
pub fn reorder_groups(group_orders: Vec<(String, i32)>) -> Result<(), String> {
    with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        let now = chrono::Local::now().timestamp();

        for (name, order) in group_orders {
            tx.execute(
                "UPDATE groups SET order_index = ?1, updated_at = ?2 WHERE name = ?3",
                params![order, now, &name],
            )?;
        }

        tx.commit()?;
        Ok(())
    })
}
