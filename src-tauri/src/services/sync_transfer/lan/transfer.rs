use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferResult {
    pub saved: bool,
    pub path: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTransferProgress {
    pub transfer_id: String,
    pub device_id: String,
    pub file_path: String,
    pub file_name: String,
    pub sent_bytes: u64,
    pub total_bytes: u64,
    pub status: String,
}

pub type FileTransferProgressCallback = Arc<dyn Fn(FileTransferProgress) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct FileTransferProgressReporter {
    transfer_id: String,
    device_id: String,
    file_path: String,
    file_name: String,
    total_bytes: u64,
    callback: FileTransferProgressCallback,
}

impl FileTransferProgressReporter {
    pub fn new(
        transfer_id: String,
        device_id: String,
        file_path: String,
        file_name: String,
        total_bytes: u64,
        callback: FileTransferProgressCallback,
    ) -> Self {
        Self {
            transfer_id,
            device_id,
            file_path,
            file_name,
            total_bytes,
            callback,
        }
    }

    pub fn emit(&self, status: &str, sent_bytes: u64) {
        (self.callback)(FileTransferProgress {
            transfer_id: self.transfer_id.clone(),
            device_id: self.device_id.clone(),
            file_path: self.file_path.clone(),
            file_name: self.file_name.clone(),
            sent_bytes,
            total_bytes: self.total_bytes,
            status: status.to_string(),
        });
    }
}

pub async fn send_file_to_peer(
    device_id: &str,
    file_path: &str,
) -> Result<FileTransferResult, String> {
    send_file_to_peer_with_progress(device_id, file_path, None, None).await
}

pub async fn send_file_to_peer_with_progress(
    device_id: &str,
    file_path: &str,
    transfer_id: Option<String>,
    progress: Option<FileTransferProgressCallback>,
) -> Result<FileTransferResult, String> {
    let peer = super::peer_store::list_peers()
        .into_iter()
        .find(|peer| peer.device_id == device_id)
        .ok_or_else(|| "未找到已配对设备".to_string())?;
    let (file_name, path, size) = super::files::outgoing_file_info(file_path)?;
    let reporter = progress.map(|callback| {
        FileTransferProgressReporter::new(
            transfer_id.unwrap_or_else(|| format!("{}:{}", device_id, file_path)),
            device_id.to_string(),
            path.to_string_lossy().to_string(),
            file_name.clone(),
            size,
            callback,
        )
    });
    super::http_client::send_peer_file_stream(&peer, &file_name, path, size, reporter).await
}
