use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::webdav_client::WebdavClient;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloudFileIndex {
    #[serde(default)]
    pub files: HashMap<String, CloudFileManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFileManifest {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub source_device_id: String,
    pub source_device_name: String,
    pub uploaded_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFileUploadResult {
    pub manifest: CloudFileManifest,
}

pub struct CloudFileUploadRequest {
    pub path: String,
    pub transfer_id: Option<String>,
    pub progress: Option<CloudFileUploadProgressCallback>,
}

pub struct CloudFileUploadBatchItem {
    pub path: String,
    pub result: Result<CloudFileUploadResult, String>,
    pub uploaded: bool,
}

#[derive(Clone)]
pub struct CloudFileUploadProgress {
    pub transfer_id: String,
    pub file_path: String,
    pub sent_bytes: u64,
    pub total_bytes: u64,
    pub status: String,
}

pub type CloudFileUploadProgressCallback =
    Arc<dyn Fn(CloudFileUploadProgress) + Send + Sync + 'static>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFileListItem {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub source_device_id: String,
    pub source_device_name: String,
    pub uploaded_at: i64,
    pub local_status: String,
    pub local_path: Option<String>,
    pub downloaded_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFileDownloadResult {
    pub file: CloudFileListItem,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CloudFileDownloadIndex {
    #[serde(default)]
    files: HashMap<String, CloudFileDownloadRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudFileDownloadRecord {
    path: String,
    downloaded_at: i64,
    sha256: String,
}

const INDEX_PATH: &str = "cloud_files/index.json";
const CLOUD_FILE_STREAM_CHUNK_SIZE: usize = 4 * 1024 * 1024;
const DOWNLOADS_DIR: &str = "cloud_file_downloads";
const DOWNLOAD_FILES_DIR: &str = "files";
const DOWNLOAD_INDEX_NAME: &str = "index.json";
static CLOUD_FILES_DIR_READY: AtomicBool = AtomicBool::new(false);

pub async fn upload_files_with_progress(
    client: &WebdavClient,
    requests: Vec<CloudFileUploadRequest>,
) -> Result<Vec<CloudFileUploadBatchItem>, String> {
    let pending = requests
        .into_iter()
        .map(prepare_upload_request)
        .collect::<Vec<_>>();
    let mut index = load_index(client).await?;
    let mut results = Vec::with_capacity(pending.len());
    let mut uploaded_objects = Vec::<String>::new();
    let mut new_result_indices = Vec::<usize>::new();
    let mut index_changed = false;
    let mut dirs_ready = false;

    for item in pending {
        let upload = match item {
            Ok(upload) => upload,
            Err((path, error)) => {
                results.push(CloudFileUploadBatchItem {
                    path,
                    result: Err(error),
                    uploaded: false,
                });
                continue;
            }
        };

        let mut precomputed_sha256 = None;
        if index.files.values().any(|file| file.size == upload.size) {
            let sha256 = match sha256_file(&upload.source) {
                Ok(sha256) => sha256,
                Err(error) => {
                    results.push(CloudFileUploadBatchItem {
                        path: upload.path,
                        result: Err(error),
                        uploaded: false,
                    });
                    continue;
                }
            };
            if let Some(existing) = index
                .files
                .values()
                .find(|file| file.sha256 == sha256 && file.size == upload.size)
                .cloned()
            {
                results.push(CloudFileUploadBatchItem {
                    path: upload.path,
                    result: Ok(CloudFileUploadResult { manifest: existing }),
                    uploaded: false,
                });
                continue;
            }
            precomputed_sha256 = Some(sha256);
        }

        if !dirs_ready {
            ensure_cloud_files_dir_once(client).await?;
            dirs_ready = true;
        }

        let id = Uuid::new_v4().to_string();
        let object_path = cloud_file_object_path(&id);
        let sha256 = match upload_cloud_file_object(client, &object_path, &upload).await {
            Ok(sha256) => sha256,
            Err(error) => {
                let _ = client.delete_path(&object_path).await;
                results.push(CloudFileUploadBatchItem {
                    path: upload.path,
                    result: Err(error),
                    uploaded: false,
                });
                continue;
            }
        };
        let sha256 = match precomputed_sha256 {
            Some(precomputed) if precomputed != sha256 => sha256,
            Some(precomputed) => precomputed,
            None => sha256,
        };

        let manifest = CloudFileManifest {
            id: id.clone(),
            name: upload.name,
            size: upload.size,
            sha256,
            source_device_id: crate::services::sync_transfer::device_id(),
            source_device_name: crate::services::sync_transfer::lan::runtime::device_name(),
            uploaded_at: chrono::Utc::now().timestamp_millis(),
        };

        index.files.insert(id, manifest.clone());
        uploaded_objects.push(object_path);
        index_changed = true;
        let result_index = results.len();
        new_result_indices.push(result_index);
        results.push(CloudFileUploadBatchItem {
            path: upload.path,
            result: Ok(CloudFileUploadResult { manifest }),
            uploaded: true,
        });
    }

    if index_changed {
        if let Err(error) = save_index(client, &index).await {
            for object_path in uploaded_objects {
                let _ = client.delete_path(&object_path).await;
            }
            for result_index in new_result_indices {
                if let Some(item) = results.get_mut(result_index) {
                    item.result = Err(error.clone());
                    item.uploaded = false;
                }
            }
        }
    }

    Ok(results)
}

pub async fn list_files(client: &WebdavClient) -> Result<Vec<CloudFileListItem>, String> {
    let download_index = load_download_index()?;
    let mut files = load_index(client)
        .await?
        .files
        .into_values()
        .map(|file| to_list_item(file, &download_index))
        .collect::<Vec<_>>();
    files.sort_by(|a, b| b.uploaded_at.cmp(&a.uploaded_at).then(a.name.cmp(&b.name)));
    Ok(files)
}

pub async fn download_file(
    client: &WebdavClient,
    file_id: &str,
) -> Result<CloudFileDownloadResult, String> {
    validate_file_id(file_id)?;
    let index = load_index(client).await?;
    let manifest = index
        .files
        .get(file_id)
        .cloned()
        .ok_or_else(|| "云端文件不存在".to_string())?;

    let mut download_index = load_download_index()?;
    let target = local_download_path(&manifest, &download_index)?;
    if target.exists() && sha256_file(&target)? == manifest.sha256 {
        download_index.files.insert(
            manifest.id.clone(),
            CloudFileDownloadRecord {
                path: target.to_string_lossy().to_string(),
                downloaded_at: chrono::Utc::now().timestamp_millis(),
                sha256: manifest.sha256.clone(),
            },
        );
        save_download_index(&download_index)?;
        return Ok(CloudFileDownloadResult {
            file: to_list_item(manifest, &download_index),
        });
    }

    let temp = target.with_extension(format!(
        "{}.qcpart",
        target
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("tmp")
    ));
    if temp.exists() {
        let _ = std::fs::remove_file(&temp);
    }

    let download_result = client
        .download_encrypted_file(&cloud_file_object_path(&manifest.id), &temp)
        .await
        .and_then(|sha256| {
            if sha256 == manifest.sha256 {
                Ok(())
            } else {
                Err("下载文件校验失败".to_string())
            }
        });
    if let Err(error) = download_result {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }

    if target.exists() {
        std::fs::remove_file(&target).map_err(|e| format!("替换本地下载文件失败: {}", e))?;
    }
    std::fs::rename(&temp, &target).map_err(|e| format!("保存本地下载文件失败: {}", e))?;

    download_index.files.insert(
        manifest.id.clone(),
        CloudFileDownloadRecord {
            path: target.to_string_lossy().to_string(),
            downloaded_at: chrono::Utc::now().timestamp_millis(),
            sha256: manifest.sha256.clone(),
        },
    );
    save_download_index(&download_index)?;

    Ok(CloudFileDownloadResult {
        file: to_list_item(manifest, &download_index),
    })
}

pub async fn delete_file(client: &WebdavClient, file_id: &str) -> Result<(), String> {
    validate_file_id(file_id)?;
    let mut index = load_index(client).await?;
    let manifest = index
        .files
        .remove(file_id)
        .ok_or_else(|| "云端文件不存在".to_string())?;

    client
        .delete_path(&cloud_file_object_path(&manifest.id))
        .await?;
    save_index(client, &index).await?;

    let mut download_index = load_download_index()?;
    if download_index.files.remove(&manifest.id).is_some() {
        save_download_index(&download_index)?;
    }
    Ok(())
}

pub async fn load_index(client: &WebdavClient) -> Result<CloudFileIndex, String> {
    match client.get_json(INDEX_PATH).await? {
        Some(index) => Ok(index),
        None => {
            reset_cloud_files_dir_ready();
            Ok(CloudFileIndex::default())
        }
    }
}

async fn save_index(client: &WebdavClient, index: &CloudFileIndex) -> Result<(), String> {
    client.put_json(INDEX_PATH, index).await
}

async fn ensure_cloud_files_dir_once(client: &WebdavClient) -> Result<(), String> {
    if !CLOUD_FILES_DIR_READY.load(Ordering::Acquire) {
        client.ensure_cloud_files_dir().await?;
        CLOUD_FILES_DIR_READY.store(true, Ordering::Release);
    }
    Ok(())
}

fn cloud_file_object_path(file_id: &str) -> String {
    format!("cloud_files/objects/{}.qcf", file_id)
}

async fn upload_cloud_file_object(
    client: &WebdavClient,
    object_path: &str,
    upload: &PreparedCloudFileUpload,
) -> Result<String, String> {
    let result = client
        .upload_encrypted_file_with_progress(
            object_path,
            &upload.source,
            upload.size,
            CLOUD_FILE_STREAM_CHUNK_SIZE,
            upload_progress_callback(upload),
        )
        .await;
    match result {
        Ok(sha256) => Ok(sha256),
        Err(error) if is_webdav_conflict_error(&error) => {
            reset_cloud_files_dir_ready();
            ensure_cloud_files_dir_once(client).await?;
            client
                .upload_encrypted_file_with_progress(
                    object_path,
                    &upload.source,
                    upload.size,
                    CLOUD_FILE_STREAM_CHUNK_SIZE,
                    upload_progress_callback(upload),
                )
                .await
        }
        Err(error) => Err(error),
    }
}

fn reset_cloud_files_dir_ready() {
    CLOUD_FILES_DIR_READY.store(false, Ordering::Release);
}

fn is_webdav_conflict_error(error: &str) -> bool {
    error.contains("409") || error.contains("Conflict")
}

struct PreparedCloudFileUpload {
    path: String,
    source: PathBuf,
    name: String,
    size: u64,
    transfer_id: Option<String>,
    progress: Option<CloudFileUploadProgressCallback>,
}

fn prepare_upload_request(
    request: CloudFileUploadRequest,
) -> Result<PreparedCloudFileUpload, (String, String)> {
    let path = request.path;
    let source = PathBuf::from(&path);
    let metadata = std::fs::metadata(&source)
        .map_err(|e| (path.clone(), format!("读取待上传文件信息失败: {}", e)))?;
    if !metadata.is_file() {
        return Err((path, "只能上传普通文件".to_string()));
    }

    let name = source
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| (path.clone(), "文件名无效".to_string()))?
        .to_string();
    let size = metadata.len();
    Ok(PreparedCloudFileUpload {
        path,
        source,
        name,
        size,
        transfer_id: request.transfer_id,
        progress: request.progress,
    })
}

fn upload_progress_callback(
    upload: &PreparedCloudFileUpload,
) -> Option<Arc<dyn Fn(u64) + Send + Sync + 'static>> {
    upload.progress.clone().map(|callback| {
        let transfer_id = upload
            .transfer_id
            .clone()
            .unwrap_or_else(|| format!("cloud:{}", upload.path));
        let file_path = upload.path.clone();
        let total_bytes = upload.size;
        Arc::new(move |sent_bytes| {
            callback(CloudFileUploadProgress {
                transfer_id: transfer_id.clone(),
                file_path: file_path.clone(),
                sent_bytes,
                total_bytes,
                status: "uploading".to_string(),
            });
        }) as Arc<dyn Fn(u64) + Send + Sync + 'static>
    })
}

fn to_list_item(
    file: CloudFileManifest,
    download_index: &CloudFileDownloadIndex,
) -> CloudFileListItem {
    let record = download_index.files.get(&file.id);
    let (local_status, local_path, downloaded_at) = match record {
        Some(record) if PathBuf::from(&record.path).exists() => (
            "downloaded".to_string(),
            Some(record.path.clone()),
            record.downloaded_at,
        ),
        Some(record) => (
            "missing".to_string(),
            Some(record.path.clone()),
            record.downloaded_at,
        ),
        None => ("notDownloaded".to_string(), None, 0),
    };

    CloudFileListItem {
        id: file.id,
        name: file.name,
        size: file.size,
        sha256: file.sha256,
        source_device_id: file.source_device_id,
        source_device_name: file.source_device_name,
        uploaded_at: file.uploaded_at,
        local_status,
        local_path,
        downloaded_at,
    }
}

fn load_download_index() -> Result<CloudFileDownloadIndex, String> {
    let path = download_index_path()?;
    if !path.exists() {
        return Ok(CloudFileDownloadIndex::default());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("读取云端下载状态失败: {}", e))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("解析云端下载状态失败: {}", e))
}

fn save_download_index(index: &CloudFileDownloadIndex) -> Result<(), String> {
    let path = download_index_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建云端下载状态目录失败: {}", e))?;
    }
    let bytes =
        serde_json::to_vec_pretty(index).map_err(|e| format!("序列化云端下载状态失败: {}", e))?;
    std::fs::write(&path, bytes).map_err(|e| format!("保存云端下载状态失败: {}", e))
}

fn download_index_path() -> Result<PathBuf, String> {
    Ok(downloads_dir()?.join(DOWNLOAD_INDEX_NAME))
}

fn local_download_path(
    manifest: &CloudFileManifest,
    download_index: &CloudFileDownloadIndex,
) -> Result<PathBuf, String> {
    validate_file_id(&manifest.id)?;
    if let Some(record) = download_index.files.get(&manifest.id) {
        let path = PathBuf::from(&record.path);
        if path.exists() {
            return Ok(path);
        }
    }
    let dir = download_files_dir()?;
    let file_name = sanitize_file_name(&manifest.name);
    Ok(unique_download_path(
        &dir,
        &file_name,
        &manifest.id,
        download_index,
    ))
}

fn downloads_dir() -> Result<PathBuf, String> {
    Ok(crate::services::get_data_directory()?.join(DOWNLOADS_DIR))
}

fn download_files_dir() -> Result<PathBuf, String> {
    Ok(downloads_dir()?.join(DOWNLOAD_FILES_DIR))
}

fn unique_download_path(
    dir: &Path,
    file_name: &str,
    file_id: &str,
    download_index: &CloudFileDownloadIndex,
) -> PathBuf {
    let (stem, extension) = split_file_name(file_name);
    let mut suffix = 0u32;
    loop {
        let candidate_name = if suffix == 0 {
            file_name.to_string()
        } else if let Some(extension) = extension.as_deref() {
            format!("{} ({}).{}", stem, suffix + 1, extension)
        } else {
            format!("{} ({})", stem, suffix + 1)
        };
        let candidate = dir.join(candidate_name);
        if !candidate.exists() && !download_path_used_by_other(file_id, &candidate, download_index)
        {
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }
}

fn download_path_used_by_other(
    file_id: &str,
    candidate: &Path,
    download_index: &CloudFileDownloadIndex,
) -> bool {
    download_index
        .files
        .iter()
        .any(|(id, record)| id != file_id && PathBuf::from(&record.path) == candidate)
}

fn split_file_name(file_name: &str) -> (String, Option<String>) {
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("file")
        .to_string();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    (stem, extension)
}

fn validate_file_id(file_id: &str) -> Result<(), String> {
    let valid = !file_id.trim().is_empty()
        && file_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    if valid {
        Ok(())
    } else {
        Err("云端文件 ID 无效".to_string())
    }
}

fn sanitize_file_name(name: &str) -> String {
    let mut out = name
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>()
        .trim_matches(|ch| ch == ' ' || ch == '.')
        .to_string();
    if out.is_empty() {
        out = "file".to_string();
    }
    let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        out.insert(0, '_');
    }
    out
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("打开待上传文件失败: {}", e))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| format!("计算文件校验值失败: {}", e))?;
    Ok(hex::encode(hasher.finalize()))
}
