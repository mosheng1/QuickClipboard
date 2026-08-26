use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use super::pairing::PairingConfirmResponse;

const HEADER_LIMIT: usize = 64 * 1024;
const HELLO_PATH: &str = "/qc-sync/hello";
const PAIRING_CONFIRM_PATH: &str = "/qc-sync/pairing/confirm";
const STATUS_PATH: &str = "/qc-sync/status";
const SNAPSHOT_PATH: &str = "/qc-sync/snapshot";
const HISTORY_RECORDS_PATH: &str = "/qc-sync/records/history";
const FAVORITE_RECORDS_PATH: &str = "/qc-sync/records/favorites";
const GROUPS_PATH: &str = "/qc-sync/groups";
const TOMBSTONES_PATH: &str = "/qc-sync/tombstones";
const FILES_PREFIX: &str = "/qc-sync/files/";
const TRANSFER_FILES_PREFIX: &str = "/qc-transfer/files/";
const MAX_REQUEST_BODY_SIZE: usize = super::files::MAX_DIRECT_TRANSFER_FILE_SIZE as usize;
const FILE_TRANSFER_BUFFER_SIZE: usize = 1024 * 1024;

static SERVER: Lazy<tokio::sync::Mutex<Option<ServerState>>> =
    Lazy::new(|| tokio::sync::Mutex::new(None));

struct ServerState {
    port: u16,
    task: tokio::task::JoinHandle<()>,
}

struct SavedTransferFile {
    path: std::path::PathBuf,
    size: u64,
    sha256: String,
}

#[derive(Clone)]
struct ReceiveTransferProgressReporter {
    app: AppHandle,
    transfer_id: String,
    file_name: String,
    total_bytes: u64,
    source_device_id: String,
    source_device_name: String,
}

impl ReceiveTransferProgressReporter {
    fn emit(&self, status: &str, received_bytes: u64) {
        crate::windows::receive_box::emit_lan_file_progress(
            &self.app,
            crate::windows::receive_box::ReceiveBoxLanFileProgress {
                transfer_id: self.transfer_id.clone(),
                name: self.file_name.clone(),
                received_bytes: received_bytes.min(self.total_bytes),
                total_bytes: self.total_bytes,
                source_device_id: self.source_device_id.clone(),
                source_device_name: self.source_device_name.clone(),
                status: status.to_string(),
            },
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanHttpServerConfig {
    pub port: u16,
}

impl Default for LanHttpServerConfig {
    fn default() -> Self {
        Self {
            port: super::DEFAULT_HTTP_PORT,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpPairingConfirmRequest {
    pub device_id: String,
    pub device_name: String,
    pub base_url: String,
    pub pairing_code: String,
}

pub fn confirm_pairing(input: HttpPairingConfirmRequest) -> Result<PairingConfirmResponse, String> {
    let peer_token = super::runtime::confirm_pairing(
        input.device_id,
        input.device_name,
        input.base_url,
        input.pairing_code,
    )?;
    Ok(PairingConfirmResponse {
        peer_token,
        expires_at_ms: None,
    })
}

pub fn verify_authorization(device_id: &str, authorization: &str) -> bool {
    let Some(token) = authorization.trim().strip_prefix("Bearer ") else {
        return false;
    };
    super::runtime::verify_peer_token(device_id, token)
}

pub fn is_running() -> bool {
    if let Ok(state) = SERVER.try_lock() {
        return state
            .as_ref()
            .map(|server| !server.task.is_finished())
            .unwrap_or(false);
    }
    false
}

pub fn running_port() -> Option<u16> {
    SERVER.try_lock().ok().and_then(|state| {
        state
            .as_ref()
            .filter(|server| !server.task.is_finished())
            .map(|server| server.port)
    })
}

pub async fn start(app: AppHandle, config: LanHttpServerConfig) -> Result<u16, String> {
    let mut state = SERVER.lock().await;
    if let Some(server) = state.as_ref() {
        if server.port == config.port && !server.task.is_finished() {
            return Ok(config.port);
        }
    }

    if let Some(server) = state.take() {
        server.task.abort();
    }

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port))
        .await
        .map_err(|e| format!("局域网 HTTP 服务启动失败: {}", e))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    super::discovery::start_responder(port).await?;
    let task = tokio::spawn(async move {
        loop {
            let Ok((stream, remote_addr)) = listener.accept().await else {
                break;
            };
            let app = app.clone();
            tokio::spawn(async move {
                let _ = handle_client(stream, remote_addr, app).await;
            });
        }
    });
    *state = Some(ServerState { port, task });
    Ok(port)
}

pub async fn stop() {
    let mut state = SERVER.lock().await;
    if let Some(server) = state.take() {
        server.task.abort();
    }
    super::discovery::stop_responder().await;
}

async fn handle_client(
    mut stream: tokio::net::TcpStream,
    remote_addr: std::net::SocketAddr,
    app: AppHandle,
) -> Result<(), String> {
    let mut request = read_request(&mut stream).await?;
    let response = if request.method == "PUT" && request.path.starts_with(TRANSFER_FILES_PREFIX) {
        receive_transfer_file_stream(&request, &mut stream, &app).await
    } else {
        read_request_body(&mut request, &mut stream, MAX_REQUEST_BODY_SIZE).await?;
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", HELLO_PATH) => json_response(
                200,
                serde_json::json!({
                    "device_id": super::runtime::device_id(),
                    "device_name": super::runtime::device_name(),
                    "protocol": "quickclipboard-sync-transfer-lan-http",
                    "version": 1,
                }),
            ),
            ("POST", PAIRING_CONFIRM_PATH) => handle_pairing_confirm(&request, remote_addr, &app),
            ("GET", STATUS_PATH) => authorized_json(&request, || Ok(super::runtime::status())),
            ("GET", SNAPSHOT_PATH) => authorized_json(&request, super::snapshot::snapshot),
            ("GET", HISTORY_RECORDS_PATH) => authorized_json(&request, || {
                super::snapshot::list_history_records_since(query_i64(&request, "since"))
            }),
            ("POST", HISTORY_RECORDS_PATH) => {
                authorized_receive_json(&request, || save_history_records(&request, &app))
            }
            ("GET", FAVORITE_RECORDS_PATH) => authorized_json(&request, || {
                super::snapshot::list_favorite_records_since(query_i64(&request, "since"))
            }),
            ("POST", FAVORITE_RECORDS_PATH) => {
                authorized_receive_json(&request, || save_favorite_records(&request, &app))
            }
            ("GET", GROUPS_PATH) => authorized_json(&request, super::snapshot::list_groups),
            ("POST", GROUPS_PATH) => {
                authorized_receive_json(&request, || save_groups(&request, &app))
            }
            ("GET", TOMBSTONES_PATH) => authorized_json(&request, || {
                super::snapshot::list_tombstones_since(query_i64(&request, "since"))
            }),
            ("POST", TOMBSTONES_PATH) => {
                authorized_receive_json(&request, || save_tombstones(&request, &app))
            }
            ("GET", path) if path.starts_with(FILES_PREFIX) => {
                authorized_bytes(&request, || read_file(path))
            }
            ("PUT", path) if path.starts_with(FILES_PREFIX) => {
                authorized_receive_json(&request, || save_file(path, &request.body))
            }
            _ => json_response(404, serde_json::json!({ "message": "未找到接口" })),
        }
    };
    write_response(&mut stream, response).await
}

fn handle_pairing_confirm(
    request: &HttpRequest,
    remote_addr: std::net::SocketAddr,
    app: &AppHandle,
) -> HttpResponse {
    let input = serde_json::from_slice::<HttpPairingConfirmRequest>(&request.body)
        .map_err(|e| format!("解析配对请求失败: {}", e));
    match input
        .map(|input| normalize_pairing_base_url(input, remote_addr))
        .and_then(confirm_pairing)
    {
        Ok(output) => {
            let _ = app.emit("sync-transfer-lan-peers-changed", serde_json::json!({}));
            json_response(200, output)
        }
        Err(message) => json_response(400, serde_json::json!({ "message": message })),
    }
}

fn authorized_json<T, F>(request: &HttpRequest, action: F) -> HttpResponse
where
    T: Serialize,
    F: FnOnce() -> Result<T, String>,
{
    if !is_authorized_request(request) {
        return json_response(
            403,
            serde_json::json!({ "message": "未授权的局域网同步请求" }),
        );
    }
    match action() {
        Ok(value) => json_response(200, value),
        Err(message) => json_response(500, serde_json::json!({ "message": message })),
    }
}

fn authorized_receive_json<T, F>(request: &HttpRequest, action: F) -> HttpResponse
where
    T: Serialize,
    F: FnOnce() -> Result<T, String>,
{
    if !super::auto_sync::can_receive() {
        return json_response(403, serde_json::json!({ "message": "局域网接收已关闭" }));
    }
    authorized_json(request, action)
}

fn authorized_bytes<F>(request: &HttpRequest, action: F) -> HttpResponse
where
    F: FnOnce() -> Result<Option<Vec<u8>>, String>,
{
    if !is_authorized_request(request) {
        return json_response(
            403,
            serde_json::json!({ "message": "未授权的局域网同步请求" }),
        );
    }
    match action() {
        Ok(Some(bytes)) => bytes_response(200, bytes),
        Ok(None) => json_response(404, serde_json::json!({ "message": "文件不存在" })),
        Err(message) => json_response(500, serde_json::json!({ "message": message })),
    }
}

fn is_authorized_request(request: &HttpRequest) -> bool {
    request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .and_then(|(_, value)| {
            request
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("x-device-id"))
                .map(|(_, device_id)| verify_authorization(device_id, value))
        })
        .unwrap_or(false)
}

fn query_i64(request: &HttpRequest, name: &str) -> Option<i64> {
    request
        .query
        .iter()
        .find(|(key, _)| key == name)
        .and_then(|(_, value)| value.parse::<i64>().ok())
}

fn save_history_records(
    request: &HttpRequest,
    app: &AppHandle,
) -> Result<super::LanRecordBatch, String> {
    let batch = serde_json::from_slice::<super::LanRecordBatch>(&request.body)
        .map_err(|e| format!("解析局域网历史数据失败: {}", e))?;
    let records = crate::services::database::filter_records_not_deleted(
        crate::services::database::COLLECTION_HISTORY,
        &batch.records,
    )?;
    let changed = crate::services::database::lan_upsert_history_records(&records)?;
    if !changed.is_empty() {
        crate::windows::main_window::mark_clipboard_refresh_pending();
        emit_refresh_if_visible(app);
        crate::services::sync_transfer::lan_notify_local_change(app.clone(), "relay");
    }
    Ok(super::LanRecordBatch {
        collection: "history".to_string(),
        records: changed,
    })
}

fn save_favorite_records(
    request: &HttpRequest,
    app: &AppHandle,
) -> Result<super::LanRecordBatch, String> {
    let batch = serde_json::from_slice::<super::LanRecordBatch>(&request.body)
        .map_err(|e| format!("解析局域网收藏数据失败: {}", e))?;
    let records = crate::services::database::filter_records_not_deleted(
        crate::services::database::COLLECTION_FAVORITES,
        &batch.records,
    )?;
    let changed = crate::services::database::lan_upsert_favorite_records(&records)?;
    if !changed.is_empty() {
        crate::windows::main_window::mark_favorites_refresh_pending();
        emit_refresh_if_visible(app);
        crate::services::sync_transfer::lan_notify_local_change(app.clone(), "relay");
    }
    Ok(super::LanRecordBatch {
        collection: "favorites".to_string(),
        records: changed,
    })
}

fn save_groups(request: &HttpRequest, app: &AppHandle) -> Result<super::LanGroupBatch, String> {
    let batch = serde_json::from_slice::<super::LanGroupBatch>(&request.body)
        .map_err(|e| format!("解析局域网分组数据失败: {}", e))?;
    let groups = crate::services::database::filter_groups_not_deleted(&batch.groups)?;
    let changed = crate::services::database::lan_save_groups(&groups)?;
    if !changed.is_empty() {
        crate::windows::main_window::mark_groups_refresh_pending();
        crate::windows::main_window::mark_favorites_refresh_pending();
        emit_refresh_if_visible(app);
        crate::services::sync_transfer::lan_notify_local_change(app.clone(), "relay");
    }
    Ok(super::LanGroupBatch { groups: changed })
}

fn save_tombstones(
    request: &HttpRequest,
    app: &AppHandle,
) -> Result<super::LanTombstoneBatch, String> {
    let batch = serde_json::from_slice::<super::LanTombstoneBatch>(&request.body)
        .map_err(|e| format!("解析局域网删除记录失败: {}", e))?;
    let changed = crate::services::database::upsert_sync_tombstones(&batch.tombstones)?;
    let report = crate::services::database::apply_sync_tombstones(&batch.tombstones)?;
    mark_tombstone_refresh(&report, app);
    if !changed.is_empty() || report.total() > 0 {
        crate::services::sync_transfer::lan_notify_local_change(app.clone(), "relay");
    }
    Ok(super::LanTombstoneBatch {
        tombstones: changed,
    })
}

fn mark_tombstone_refresh(
    report: &crate::services::database::SyncTombstoneApplyReport,
    app: &AppHandle,
) {
    if report.history > 0 {
        crate::windows::main_window::mark_clipboard_refresh_pending();
    }
    if report.favorites > 0 {
        crate::windows::main_window::mark_favorites_refresh_pending();
    }
    if report.groups > 0 {
        crate::windows::main_window::mark_groups_refresh_pending();
        crate::windows::main_window::mark_favorites_refresh_pending();
    }
    if report.total() > 0 {
        emit_refresh_if_visible(app);
    }
}

fn read_file(path: &str) -> Result<Option<Vec<u8>>, String> {
    let image_id = super::files::image_id_from_file_path(path)?;
    super::files::read_image_file(&image_id)
}

fn save_file(path: &str, bytes: &[u8]) -> Result<serde_json::Value, String> {
    let image_id = super::files::image_id_from_file_path(path)?;
    super::files::save_image_file(&image_id, bytes)?;
    Ok(serde_json::json!({ "saved": true }))
}

async fn receive_transfer_file_stream(
    request: &HttpRequest,
    stream: &mut tokio::net::TcpStream,
    app: &AppHandle,
) -> HttpResponse {
    if !super::auto_sync::can_receive() {
        return json_response(403, serde_json::json!({ "message": "局域网接收已关闭" }));
    }
    if !is_authorized_request(request) {
        return json_response(
            403,
            serde_json::json!({ "message": "未授权的局域网同步请求" }),
        );
    }

    let file_name = match super::files::file_name_from_transfer_path(&request.path) {
        Ok(value) => value,
        Err(message) => return json_response(500, serde_json::json!({ "message": message })),
    };
    let source_device_id = request_header(request, "x-device-id").unwrap_or_default();
    let source_device_name = source_device_name(&source_device_id);
    let reporter = ReceiveTransferProgressReporter {
        app: app.clone(),
        transfer_id: format!("lan-receive:{}", Uuid::new_v4()),
        file_name: file_name.clone(),
        total_bytes: request.content_length as u64,
        source_device_id: source_device_id.clone(),
        source_device_name: source_device_name.clone(),
    };
    reporter.emit("receiving", 0);

    match save_transfer_file_stream(
        request,
        stream,
        file_name,
        &source_device_id,
        &source_device_name,
        &reporter,
    )
    .await
    {
        Ok(saved) => {
            reporter.emit("done", saved.size);
            crate::windows::receive_box::emit_lan_files_changed(app);
            json_response(
                200,
                serde_json::json!({
                    "saved": true,
                    "path": saved.path.to_string_lossy().to_string(),
                    "size": saved.size,
                    "sha256": saved.sha256,
                }),
            )
        }
        Err(message) => {
            reporter.emit("failed", 0);
            json_response(500, serde_json::json!({ "message": message }))
        }
    }
}

async fn save_transfer_file_stream(
    request: &HttpRequest,
    stream: &mut tokio::net::TcpStream,
    file_name: String,
    source_device_id: &str,
    source_device_name: &str,
    reporter: &ReceiveTransferProgressReporter,
) -> Result<SavedTransferFile, String> {
    let reservation = super::files::prepare_received_file(&file_name)?;
    let result = async {
        let mut file = tokio::fs::File::create(&reservation.temp_path)
            .await
            .map_err(|e| format!("创建接收文件失败: {}", e))?;
        let mut hasher = Sha256::new();
        let mut written = 0usize;
        if !request.body.is_empty() {
            file.write_all(&request.body)
                .await
                .map_err(|e| format!("保存接收文件失败: {}", e))?;
            hasher.update(&request.body);
            written = request.body.len();
            reporter.emit("receiving", written as u64);
        }
        let remaining = request
            .content_length
            .checked_sub(written)
            .ok_or_else(|| "请求体长度异常".to_string())?;
        if remaining > 0 {
            copy_exact_to_file(
                stream,
                &mut file,
                &mut hasher,
                remaining,
                written as u64,
                reporter,
            )
            .await?;
        }
        file.flush()
            .await
            .map_err(|e| format!("保存接收文件失败: {}", e))?;
        drop(file);
        let saved_len = tokio::fs::metadata(&reservation.temp_path)
            .await
            .map_err(|e| format!("校验接收文件失败: {}", e))?
            .len();
        if saved_len != request.content_length as u64 {
            return Err(format!(
                "局域网文件接收不完整: 期望 {} 字节，实际 {} 字节",
                request.content_length, saved_len,
            ));
        }
        let saved_path = tokio::task::spawn_blocking({
            let reservation = reservation.clone();
            move || super::files::commit_received_file(&reservation)
        })
        .await
        .map_err(|e| format!("完成接收文件保存失败: {}", e))??;
        let sha256 = hex::encode(hasher.finalize());
        if let Err(message) = super::files::record_received_file(
            &saved_path,
            saved_len,
            &sha256,
            source_device_id,
            source_device_name,
        ) {
            eprintln!("[局域网文件接收] 写入接收文件索引失败: {}", message);
        }
        Ok(SavedTransferFile {
            path: saved_path,
            size: saved_len,
            sha256,
        })
    }
    .await;

    match result {
        Ok(saved) => Ok(saved),
        Err(message) => {
            super::files::discard_received_file(&reservation);
            Err(message)
        }
    }
}

fn request_header(request: &HttpRequest, name: &str) -> Option<String> {
    request
        .headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn source_device_name(device_id: &str) -> String {
    if device_id.trim().is_empty() {
        return String::new();
    }
    super::peer_store::list_peers()
        .into_iter()
        .find(|peer| peer.device_id == device_id)
        .map(|peer| peer.device_name)
        .unwrap_or_default()
}

async fn copy_exact_to_file(
    stream: &mut tokio::net::TcpStream,
    file: &mut tokio::fs::File,
    hasher: &mut Sha256,
    mut remaining: usize,
    mut received: u64,
    reporter: &ReceiveTransferProgressReporter,
) -> Result<(), String> {
    let mut buffer = vec![0u8; FILE_TRANSFER_BUFFER_SIZE];
    while remaining > 0 {
        let read_len = remaining.min(buffer.len());
        let read = stream
            .read(&mut buffer[..read_len])
            .await
            .map_err(|e| format!("读取局域网传输内容失败: {}", e))?;
        if read == 0 {
            return Err("局域网文件传输连接提前关闭".to_string());
        }
        file.write_all(&buffer[..read])
            .await
            .map_err(|e| format!("保存接收文件失败: {}", e))?;
        hasher.update(&buffer[..read]);
        remaining -= read;
        received = received.saturating_add(read as u64);
        reporter.emit("receiving", received);
    }
    Ok(())
}

fn emit_refresh_if_visible(app: &AppHandle) {
    if crate::windows::main_window::is_main_window_visible_for_updates() {
        let _ = crate::commands::window::emit_main_window_refresh_needed_event(app);
    }
}

fn normalize_pairing_base_url(
    mut input: HttpPairingConfirmRequest,
    remote_addr: std::net::SocketAddr,
) -> HttpPairingConfirmRequest {
    let base_url = input.base_url.trim();
    if base_url.is_empty()
        || base_url.contains("127.0.0.1")
        || base_url.contains("localhost")
        || base_url.contains("[::1]")
    {
        let port = pairing_base_url_port(base_url)
            .unwrap_or_else(|| running_port().unwrap_or(super::DEFAULT_HTTP_PORT));
        input.base_url = format!("http://{}:{}", remote_addr.ip(), port);
    }
    input
}

fn pairing_base_url_port(base_url: &str) -> Option<u16> {
    let trimmed = base_url.trim().trim_end_matches('/');
    let (_, port) = trimmed.rsplit_once(':')?;
    port.parse::<u16>().ok()
}

struct HttpRequest {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    content_length: usize,
}

struct HttpResponse {
    status_code: u16,
    body: Vec<u8>,
    content_type: &'static str,
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = vec![0u8; 2048];
    let header_end = loop {
        if buffer.len() > HEADER_LIMIT {
            return Err("请求头过大".to_string());
        }
        let read = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("连接已关闭".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(pos) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let header_text = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().ok_or_else(|| "请求格式错误".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    if method.is_empty() || target.is_empty() {
        return Err("请求格式错误".to_string());
    }

    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, ':');
        let name = parts.next().unwrap_or("").trim().to_string();
        let value = parts.next().unwrap_or("").trim().to_string();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value
                .parse::<usize>()
                .map_err(|_| "无效的 Content-Length".to_string())?;
        }
        headers.push((name, value));
    }

    let mut body = buffer[header_end..].to_vec();
    if body.len() > content_length {
        body.truncate(content_length);
    }

    let path = target.split('?').next().unwrap_or("/").to_string();
    let query = target
        .split_once('?')
        .map(|(_, query)| {
            query
                .split('&')
                .filter_map(|part| {
                    let (key, value) = part.split_once('=').unwrap_or((part, ""));
                    if key.is_empty() {
                        return None;
                    }
                    Some((key.to_string(), value.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(HttpRequest {
        method,
        path,
        query,
        headers,
        body,
        content_length,
    })
}

async fn read_request_body(
    request: &mut HttpRequest,
    stream: &mut tokio::net::TcpStream,
    max_size: usize,
) -> Result<(), String> {
    if request.content_length > max_size || request.body.len() > max_size {
        return Err("请求体超过 512MB，第一版直传暂不支持超大文件".to_string());
    }
    if request.content_length > request.body.len() {
        let remaining = request.content_length - request.body.len();
        let mut extra = vec![0u8; remaining];
        stream
            .read_exact(&mut extra)
            .await
            .map_err(|e| e.to_string())?;
        request.body.extend_from_slice(&extra);
    }
    if request.body.len() > request.content_length {
        request.body.truncate(request.content_length);
    }
    Ok(())
}

fn json_response<T: Serialize>(status_code: u16, value: T) -> HttpResponse {
    let body = serde_json::to_vec(&value)
        .unwrap_or_else(|_| "{\"message\":\"序列化响应失败\"}".as_bytes().to_vec());
    HttpResponse {
        status_code,
        body,
        content_type: "application/json; charset=utf-8",
    }
}

fn bytes_response(status_code: u16, body: Vec<u8>) -> HttpResponse {
    HttpResponse {
        status_code,
        body,
        content_type: "image/png",
    }
}

async fn write_response(
    stream: &mut tokio::net::TcpStream,
    response: HttpResponse,
) -> Result<(), String> {
    let status_text = match response.status_code {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status_code,
        status_text,
        response.content_type,
        response.body.len()
    );
    stream
        .write_all(header.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    stream
        .write_all(&response.body)
        .await
        .map_err(|e| e.to_string())?;
    stream.flush().await.map_err(|e| e.to_string())
}
