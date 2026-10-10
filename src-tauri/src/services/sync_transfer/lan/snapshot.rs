use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::services::webdav_sync::types::{CloudGroup, CloudRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanSyncSnapshot {
    pub device_id: String,
    pub history_states: HashMap<String, i64>,
    pub favorite_states: HashMap<String, i64>,
    pub groups: Vec<CloudGroup>,
    #[serde(default)]
    pub tombstone_states: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanRecordBatch {
    pub collection: String,
    pub records: Vec<CloudRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanGroupBatch {
    pub groups: Vec<CloudGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanTombstoneBatch {
    pub tombstones: Vec<crate::services::database::SyncTombstone>,
}

pub fn snapshot() -> Result<LanSyncSnapshot, String> {
    let device_id = super::runtime::device_id();
    Ok(LanSyncSnapshot {
        device_id: device_id.clone(),
        history_states: crate::services::database::webdav_history_record_states()?,
        favorite_states: crate::services::database::webdav_favorite_record_states()?,
        groups: crate::services::database::webdav_list_groups(&device_id)?,
        tombstone_states: crate::services::database::sync_tombstone_states()?,
    })
}

pub fn list_history_records_since(since_updated_at: Option<i64>) -> Result<LanRecordBatch, String> {
    let device_id = super::runtime::device_id();
    let mut records = crate::services::database::webdav_list_history_records(&device_id)?;
    if let Some(since_updated_at) = since_updated_at {
        records.retain(|record| record.updated_at > since_updated_at);
    }
    let records = crate::services::database::filter_records_not_deleted(
        crate::services::database::COLLECTION_HISTORY,
        &records,
    )?;
    Ok(LanRecordBatch {
        collection: "history".to_string(),
        records,
    })
}

pub fn list_favorite_records_since(
    since_updated_at: Option<i64>,
) -> Result<LanRecordBatch, String> {
    let device_id = super::runtime::device_id();
    let mut records = crate::services::database::webdav_list_favorite_records(&device_id)?;
    if let Some(since_updated_at) = since_updated_at {
        records.retain(|record| record.updated_at > since_updated_at);
    }
    let records = crate::services::database::filter_records_not_deleted(
        crate::services::database::COLLECTION_FAVORITES,
        &records,
    )?;
    Ok(LanRecordBatch {
        collection: "favorites".to_string(),
        records,
    })
}

pub fn list_groups() -> Result<LanGroupBatch, String> {
    let device_id = super::runtime::device_id();
    let groups = crate::services::database::webdav_list_groups(&device_id)?;
    Ok(LanGroupBatch {
        groups: crate::services::database::filter_groups_not_deleted(&groups)?,
    })
}

pub fn list_tombstones_since(since_deleted_at: Option<i64>) -> Result<LanTombstoneBatch, String> {
    Ok(LanTombstoneBatch {
        tombstones: crate::services::database::list_sync_tombstones_since(since_deleted_at)?,
    })
}
