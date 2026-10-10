use std::collections::{BTreeMap, HashMap, HashSet};

use super::chunk_manager::{load_chunk, save_chunk};
use super::index_manager::{load_index, save_index};
use super::types::{
    CloudRecord, CloudRecordMeta, ImageFileIndex, ImageFileIndexEntry, RecordChunk, SyncCollection,
    SyncIndexEntry, SyncReport, CHUNK_RECORD_LIMIT,
};
use super::webdav_client::WebdavClient;

pub async fn upload_all(client: &WebdavClient, device_id: &str) -> Result<SyncReport, String> {
    upload_parts(client, device_id, true, true, true, true).await
}

pub async fn upload_parts(
    client: &WebdavClient,
    device_id: &str,
    upload_clipboard: bool,
    upload_favorites: bool,
    upload_groups: bool,
    upload_tombstones: bool,
) -> Result<SyncReport, String> {
    let mut report = SyncReport::default();
    let settings = crate::services::get_settings();
    let mut uploaded_records = Vec::new();

    let tombstone_states = if upload_tombstones {
        match super::tombstones_sync::upload_tombstones(client).await {
            Ok(result) => {
                report.pulled_clipboard += result.applied.history;
                report.pulled_favorites += result.applied.favorites;
                report.pulled_groups += result.applied.groups;
                report.pulled += result.applied.total();
                result.states
            }
            Err(e) => {
                report.errors.push(format!("删除记录推送失败: {}", e));
                crate::services::database::sync_tombstone_states().unwrap_or_default()
            }
        }
    } else {
        crate::services::database::sync_tombstone_states().unwrap_or_default()
    };

    let history_records = if settings.webdav_sync_clipboard && upload_clipboard {
        let index = load_index(client, SyncCollection::History).await?;
        let metas = crate::services::database::webdav_list_history_record_metas()?;
        let metas = crate::services::database::filter_record_metas_not_deleted_by_states(
            crate::services::database::COLLECTION_HISTORY,
            metas,
            &tombstone_states,
        );
        let metas = metas_newer_than_index(metas, &index.entries);
        let records = load_history_records(&metas, device_id)?;
        Some((index, records))
    } else {
        None
    };
    let favorite_records = if settings.webdav_sync_favorites && upload_favorites {
        let index = load_index(client, SyncCollection::Favorites).await?;
        let metas = crate::services::database::webdav_list_favorite_record_metas()?;
        let metas = crate::services::database::filter_record_metas_not_deleted_by_states(
            crate::services::database::COLLECTION_FAVORITES,
            metas,
            &tombstone_states,
        );
        let metas = metas_newer_than_index(metas, &index.entries);
        let records = load_favorite_records(&metas, device_id)?;
        Some((index, records))
    } else {
        None
    };

    if let Some((index, history_records)) = history_records {
        match upload_collection_incremental(
            client,
            SyncCollection::History,
            index,
            history_records.clone(),
            device_id,
        )
        .await
        {
            Ok(records) => {
                let count = records.len() as u32;
                report.pushed += count;
                report.pushed_clipboard = count;
                report
                    .pushed_items
                    .extend(records.iter().map(|record| record.report_item("clipboard")));
                uploaded_records.extend(records);
            }
            Err(e) => report.errors.push(format!("剪贴板历史推送失败: {}", e)),
        }
    }

    if settings.webdav_sync_favorites && (upload_favorites || upload_groups) {
        if let Some((index, favorite_records)) = favorite_records {
            match upload_collection_incremental(
                client,
                SyncCollection::Favorites,
                index,
                favorite_records.clone(),
                device_id,
            )
            .await
            {
                Ok(records) => {
                    let count = records.len() as u32;
                    report.pushed += count;
                    report.pushed_favorites = count;
                    report
                        .pushed_items
                        .extend(records.iter().map(|record| record.report_item("favorites")));
                    uploaded_records.extend(records);
                }
                Err(e) => report.errors.push(format!("收藏推送失败: {}", e)),
            }
        }

        if upload_groups {
            match super::groups_sync::upload_groups_with_tombstones(
                client,
                device_id,
                &tombstone_states,
            )
            .await
            {
                Ok(groups) => {
                    let count = groups.len() as u32;
                    report.pushed += count;
                    report.pushed_groups = count;
                    report.pushed_items.extend(groups.into_iter().map(|group| {
                        super::types::SyncReportItem {
                            category: "groups".to_string(),
                            id: group.name.clone(),
                            summary: group.name,
                            source_device_id: group.source_device_id,
                            updated_at: group.updated_at,
                        }
                    }));
                }
                Err(e) => report.errors.push(format!("分组推送失败: {}", e)),
            }
        }
    }

    if settings.webdav_sync_images {
        upload_images(client, &uploaded_records)
            .await
            .map_err(|e| format!("上传图片失败: {}", e))?;
    }

    Ok(report)
}

async fn upload_collection_incremental(
    client: &WebdavClient,
    collection: SyncCollection,
    mut index: super::types::SyncIndex,
    records: Vec<CloudRecord>,
    device_id: &str,
) -> Result<Vec<CloudRecord>, String> {
    let mut changed = Vec::new();
    let mut existing_by_chunk: HashMap<u32, Vec<CloudRecord>> = HashMap::new();
    let mut new_records = Vec::new();

    for record in records {
        let needs_upload = match index.entries.get(&record.uuid) {
            Some(entry) => entry.updated_at < record.updated_at,
            None => true,
        };

        if !needs_upload {
            continue;
        }

        if let Some(entry) = index.entries.get(&record.uuid) {
            existing_by_chunk
                .entry(entry.chunk)
                .or_default()
                .push(record.clone());
        } else {
            new_records.push(record.clone());
        }
        changed.push(record);
    }

    let mut new_records_by_chunk = Vec::<(u32, Vec<CloudRecord>)>::new();
    let mut chunk_counts = chunk_record_counts(&index.entries);
    let fillable_chunk_ids = chunk_counts
        .iter()
        .filter_map(|(chunk_id, count)| {
            if *count < CHUNK_RECORD_LIMIT {
                Some(*chunk_id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let mut fillable_index = 0usize;
    let next_available_chunk = chunk_counts
        .keys()
        .next_back()
        .copied()
        .map(|chunk_id| chunk_id.saturating_add(1))
        .unwrap_or(0);
    let mut current_chunk_id = index.next_chunk.max(next_available_chunk);
    let mut current_chunk_records = Vec::new();

    for record in new_records {
        let fillable_chunk_id = loop {
            let Some(chunk_id) = fillable_chunk_ids.get(fillable_index).copied() else {
                break None;
            };
            let count = chunk_counts.get(&chunk_id).copied().unwrap_or(0);
            if count < CHUNK_RECORD_LIMIT {
                break Some(chunk_id);
            }
            fillable_index += 1;
        };

        if let Some(chunk_id) = fillable_chunk_id {
            existing_by_chunk.entry(chunk_id).or_default().push(record);
            *chunk_counts.entry(chunk_id).or_default() += 1;
            continue;
        }

        current_chunk_records.push(record);
        if current_chunk_records.len() >= CHUNK_RECORD_LIMIT {
            let count = current_chunk_records.len();
            new_records_by_chunk.push((current_chunk_id, current_chunk_records));
            chunk_counts.insert(current_chunk_id, count);
            current_chunk_id = current_chunk_id.saturating_add(1);
            current_chunk_records = Vec::new();
        }
    }
    if !current_chunk_records.is_empty() {
        let count = current_chunk_records.len();
        new_records_by_chunk.push((current_chunk_id, current_chunk_records));
        chunk_counts.insert(current_chunk_id, count);
        current_chunk_id = current_chunk_id.saturating_add(1);
    }

    if !existing_by_chunk.is_empty() || !new_records_by_chunk.is_empty() {
        client.ensure_collection_dirs(collection).await?;
    }

    for (chunk_id, records) in existing_by_chunk {
        let mut chunk = load_chunk(client, collection, chunk_id).await?;
        for record in records {
            chunk.records.insert(record.uuid.clone(), record.clone());
            index.entries.insert(
                record.uuid.clone(),
                SyncIndexEntry {
                    chunk: chunk_id,
                    updated_at: record.updated_at,
                    source_device_id: device_id.to_string(),
                },
            );
        }
        save_chunk(client, collection, chunk_id, &chunk).await?;
    }

    for (chunk_id, records) in new_records_by_chunk {
        let mut chunk = RecordChunk::default();
        for record in records {
            chunk.records.insert(record.uuid.clone(), record.clone());
            index.entries.insert(
                record.uuid.clone(),
                SyncIndexEntry {
                    chunk: chunk_id,
                    updated_at: record.updated_at,
                    source_device_id: device_id.to_string(),
                },
            );
        }
        save_chunk(client, collection, chunk_id, &chunk).await?;
    }
    index.next_chunk = current_chunk_id;

    if !changed.is_empty() {
        save_index(client, collection, &index).await?;
    }

    Ok(changed)
}

fn metas_newer_than_index(
    metas: Vec<CloudRecordMeta>,
    entries: &HashMap<String, SyncIndexEntry>,
) -> Vec<CloudRecordMeta> {
    metas
        .into_iter()
        .filter(|meta| {
            entries
                .get(&meta.uuid)
                .map(|entry| entry.updated_at < meta.updated_at)
                .unwrap_or(true)
        })
        .collect()
}

fn load_history_records(
    metas: &[CloudRecordMeta],
    device_id: &str,
) -> Result<Vec<CloudRecord>, String> {
    let mut records = Vec::with_capacity(metas.len());
    for meta in metas {
        if let Some(record) =
            crate::services::database::webdav_get_history_record_by_uuid(&meta.uuid, device_id)?
        {
            records.push(record);
        }
    }
    Ok(records)
}

fn load_favorite_records(
    metas: &[CloudRecordMeta],
    device_id: &str,
) -> Result<Vec<CloudRecord>, String> {
    let mut records = Vec::with_capacity(metas.len());
    for meta in metas {
        if let Some(record) =
            crate::services::database::webdav_get_favorite_record_by_uuid(&meta.uuid, device_id)?
        {
            records.push(record);
        }
    }
    Ok(records)
}

fn chunk_record_counts(index_entries: &HashMap<String, SyncIndexEntry>) -> BTreeMap<u32, usize> {
    let mut counts = BTreeMap::new();
    for entry in index_entries.values() {
        *counts.entry(entry.chunk).or_default() += 1;
    }
    counts
}

async fn upload_images(client: &WebdavClient, records: &[CloudRecord]) -> Result<(), String> {
    let mut image_ids = HashSet::new();
    for record in records {
        collect_image_ids(&mut image_ids, record.image_id.as_deref());
    }

    if image_ids.is_empty() {
        return Ok(());
    }

    let mut index = load_image_file_index(client).await?;
    let mut changed = false;

    let data_dir = crate::services::get_data_directory()?;
    let images_dir = data_dir.join("clipboard_images");
    for image_id in image_ids {
        if index.images.contains_key(&image_id) {
            continue;
        }
        let path = images_dir.join(format!("{}.png", image_id));
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if !changed {
            client.ensure_files_dir().await?;
        }
        client
            .put_bytes(&format!("files/{}.png", image_id), bytes)
            .await?;
        index.images.insert(
            image_id,
            ImageFileIndexEntry {
                uploaded_at: chrono::Utc::now().timestamp(),
            },
        );
        changed = true;
    }

    if changed {
        save_image_file_index(client, &index).await?;
    }

    Ok(())
}

async fn load_image_file_index(client: &WebdavClient) -> Result<ImageFileIndex, String> {
    let index = client.get_json("files/index.json").await?;
    if index.is_some() {
        client.mark_dir_ensured("");
        client.mark_dir_ensured("files");
    }
    Ok(index.unwrap_or_default())
}

async fn save_image_file_index(
    client: &WebdavClient,
    index: &ImageFileIndex,
) -> Result<(), String> {
    client.put_json("files/index.json", index).await
}

fn collect_image_ids(out: &mut HashSet<String>, raw: Option<&str>) {
    let Some(raw) = raw else {
        return;
    };
    for item in raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        out.insert(item.to_string());
    }
}
