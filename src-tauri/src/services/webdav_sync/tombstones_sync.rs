use std::collections::HashMap;

use super::types::{SyncReport, TombstoneList};
use super::webdav_client::WebdavClient;

const TOMBSTONES_PATH: &str = "tombstones/tombstones.json";

pub struct WebdavTombstoneUploadResult {
    pub states: HashMap<String, i64>,
    pub applied: crate::services::database::SyncTombstoneApplyReport,
}

pub async fn upload_tombstones(
    client: &WebdavClient,
) -> Result<WebdavTombstoneUploadResult, String> {
    let mut remote = load_remote_tombstones(client).await?;
    let remote_tombstones = remote.values().cloned().collect::<Vec<_>>();
    let _ = crate::services::database::upsert_sync_tombstones(&remote_tombstones)?;
    let applied = crate::services::database::apply_sync_tombstones(&remote_tombstones)?;

    let mut changed = Vec::new();
    let local = crate::services::database::list_sync_tombstones_since(None)?;
    for tombstone in local {
        let key = crate::services::database::tombstone_state_key(
            &tombstone.collection,
            &tombstone.item_id,
        );
        let needs_upload = remote
            .get(&key)
            .map(|existing| existing.deleted_at < tombstone.deleted_at)
            .unwrap_or(true);
        if needs_upload {
            remote.insert(key, tombstone.clone());
            changed.push(tombstone);
        }
    }

    if !changed.is_empty() {
        save_remote_tombstones(client, &remote).await?;
    }
    let states = remote
        .iter()
        .map(|(key, tombstone)| (key.clone(), tombstone.deleted_at))
        .collect();
    Ok(WebdavTombstoneUploadResult { states, applied })
}

pub async fn download_tombstones(client: &WebdavClient) -> Result<SyncReport, String> {
    let remote = load_remote_tombstones(client).await?;
    if remote.is_empty() {
        return Ok(SyncReport::default());
    }

    let tombstones = remote.into_values().collect::<Vec<_>>();
    let _ = crate::services::database::upsert_sync_tombstones(&tombstones)?;
    let applied = crate::services::database::apply_sync_tombstones(&tombstones)?;
    let mut report = SyncReport::default();
    report.pulled_clipboard = applied.history;
    report.pulled_favorites = applied.favorites;
    report.pulled_groups = applied.groups;
    report.pulled = applied.total();
    Ok(report)
}

pub async fn remote_tombstone_states(
    client: &WebdavClient,
) -> Result<HashMap<String, i64>, String> {
    Ok(load_remote_tombstones(client)
        .await?
        .into_iter()
        .map(|(key, tombstone)| (key, tombstone.deleted_at))
        .collect())
}

async fn load_remote_tombstones(
    client: &WebdavClient,
) -> Result<HashMap<String, crate::services::database::SyncTombstone>, String> {
    let remote = client.get_json::<TombstoneList>(TOMBSTONES_PATH).await?;
    if remote.is_some() {
        client.mark_dir_ensured("");
        client.mark_dir_ensured("tombstones");
    }
    let Some(remote) = remote else {
        return Ok(HashMap::new());
    };
    Ok(remote
        .tombstones
        .into_iter()
        .filter(|tombstone| {
            !tombstone.collection.trim().is_empty() && !tombstone.item_id.trim().is_empty()
        })
        .map(|tombstone| {
            (
                crate::services::database::tombstone_state_key(
                    &tombstone.collection,
                    &tombstone.item_id,
                ),
                tombstone,
            )
        })
        .collect())
}

async fn save_remote_tombstones(
    client: &WebdavClient,
    tombstones: &HashMap<String, crate::services::database::SyncTombstone>,
) -> Result<(), String> {
    let mut tombstones = tombstones.values().cloned().collect::<Vec<_>>();
    tombstones.sort_by(|a, b| {
        a.collection
            .cmp(&b.collection)
            .then_with(|| a.item_id.cmp(&b.item_id))
            .then_with(|| a.deleted_at.cmp(&b.deleted_at))
    });
    client.ensure_tombstones_dir().await?;
    client
        .put_json(TOMBSTONES_PATH, &TombstoneList { tombstones })
        .await
}
