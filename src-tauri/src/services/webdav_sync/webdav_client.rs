use std::path::Path;
use std::sync::Arc;

use futures_util::TryStreamExt;
use parking_lot::Mutex;
use reqwest::{Body, Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashSet;
use tokio::io::DuplexStream;
use tokio_util::io::ReaderStream;

use super::crypto::{self, WebdavCryptoContext};
use super::types::{SyncCollection, WebdavConfig};

const WEBDAV_NETWORK_ERROR: &str = "无法连接 WebDAV 服务，请检查地址、网络或服务器状态";

#[derive(Clone)]
pub struct WebdavClient {
    client: Client,
    config: WebdavConfig,
    base_url: String,
    crypto: Arc<Mutex<Option<EncryptionState>>>,
    ensured_dirs: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone)]
struct EncryptionState {
    context: WebdavCryptoContext,
    password: String,
}

impl WebdavClient {
    pub fn new(config: WebdavConfig) -> Result<Self, String> {
        let url = config.url.trim().trim_end_matches('/').to_string();
        if url.is_empty() {
            return Err("WebDAV 地址不能为空".to_string());
        }
        let root = normalize_path(&config.root_path);
        let base_url = if root.is_empty() {
            url.clone()
        } else {
            format!("{}/{}", url, root)
        };
        Ok(Self {
            client: Client::new(),
            config,
            base_url,
            crypto: Arc::new(Mutex::new(None)),
            ensured_dirs: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub async fn enable_encryption(&mut self, password: &str) -> Result<(), String> {
        let scope = self.encryption_scope();
        let config = match crypto::cached_config(&scope) {
            Some(config) => config,
            None => self.load_or_create_encryption_config(&scope).await?,
        };
        self.set_encryption_state(password, &config)?;
        Ok(())
    }

    async fn refresh_encryption_config(&self) -> Result<(), String> {
        let password = self
            .encryption_password()
            .ok_or_else(|| "WebDAV 云端加密未启用".to_string())?;
        let scope = self.encryption_scope();
        let config = self.load_or_create_encryption_config(&scope).await?;
        self.set_encryption_state(&password, &config)?;
        Ok(())
    }

    fn set_encryption_state(&self, password: &str, config: &crypto::WebdavE2eeConfig) -> Result<(), String> {
        let context = crypto::context_for_config(
            &self.encryption_scope(),
            config,
            password,
        )?;
        *self.crypto.lock() = Some(EncryptionState {
            context,
            password: password.to_string(),
        });
        Ok(())
    }

    fn crypto_context(&self) -> Option<WebdavCryptoContext> {
        self.crypto.lock().as_ref().map(|state| state.context.clone())
    }

    fn encryption_password(&self) -> Option<String> {
        self.crypto.lock().as_ref().map(|state| state.password.clone())
    }

    async fn load_or_create_encryption_config(&self, scope: &str) -> Result<crypto::WebdavE2eeConfig, String> {
        let config = match self.get_plain_json::<crypto::WebdavE2eeConfig>(crypto::CONFIG_PATH).await {
            Ok(Some(config)) => config,
            Ok(None) => self.create_or_load_encryption_config().await?,
            Err(error) if error.contains("409") => self.create_or_load_encryption_config().await?,
            Err(error) => return Err(error),
        };
        crypto::cache_config(scope, &config);
        Ok(config)
    }

    async fn create_or_load_encryption_config(&self) -> Result<crypto::WebdavE2eeConfig, String> {
        self.mkcol("").await?;
        match self.get_plain_json::<crypto::WebdavE2eeConfig>(crypto::CONFIG_PATH).await? {
            Some(config) => Ok(config),
            None => {
                let config = crypto::create_config();
                self.put_plain_json(crypto::CONFIG_PATH, &config).await?;
                Ok(config)
            }
        }
    }

    pub async fn test_connection(&self) -> Result<(), String> {
        self.mkcol("").await?;
        self.mkcol("history").await?;
        self.mkcol_with_parent_retry("history", "history/chunks").await?;
        self.mkcol("favorites").await?;
        self.mkcol_with_parent_retry("favorites", "favorites/chunks").await?;
        self.mkcol("groups").await?;
        self.mkcol("files").await?;
        self.mkcol("tombstones").await?;
        Ok(())
    }

    pub async fn ensure_collection_dirs(&self, collection: SyncCollection) -> Result<(), String> {
        self.ensure_dir_cached("").await?;
        self.ensure_dir_cached(collection.dir()).await?;
        self.ensure_dir_cached(&format!("{}/chunks", collection.dir())).await
    }

    pub async fn ensure_groups_dir(&self) -> Result<(), String> {
        self.ensure_dir_cached("").await?;
        self.ensure_dir_cached("groups").await
    }

    pub async fn ensure_files_dir(&self) -> Result<(), String> {
        self.ensure_dir_cached("").await?;
        self.ensure_dir_cached("files").await
    }

    pub async fn ensure_cloud_files_dir(&self) -> Result<(), String> {
        if let Err(error) = self.mkcol("cloud_files").await {
            if !is_webdav_conflict(&error) {
                return Err(error);
            }
            self.mkcol("").await?;
            self.mkcol("cloud_files").await?;
        }
        if let Err(error) = self.mkcol("cloud_files/objects").await {
            if !is_webdav_conflict(&error) {
                return Err(error);
            }
            self.mkcol("cloud_files").await?;
            self.mkcol("cloud_files/objects").await?;
        }
        Ok(())
    }

    pub async fn ensure_tombstones_dir(&self) -> Result<(), String> {
        self.ensure_dir_cached("").await?;
        self.ensure_dir_cached("tombstones").await
    }

    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, String> {
        let Some(bytes) = self.get_bytes(path).await? else {
            return Ok(None);
        };
        let value = serde_json::from_slice(&bytes).map_err(|e| format!("解析 WebDAV JSON 失败: {}", e))?;
        Ok(Some(value))
    }

    pub async fn put_json<T: Serialize + ?Sized>(&self, path: &str, value: &T) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        self.put_bytes(path, body).await
    }

    pub async fn get_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(bytes) = self.get_raw_bytes(path).await? else {
            return Ok(None);
        };
        let path = normalize_path(path);
        match self.crypto_context() {
            Some(crypto) => match crypto.decrypt_bytes(&path, &bytes) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if should_refresh_encryption_config(&error) => {
                    self.refresh_encryption_config().await?;
                    let crypto = self.crypto_context().ok_or_else(|| "WebDAV 云端加密未启用".to_string())?;
                    Ok(Some(crypto.decrypt_bytes(&path, &bytes)?))
                }
                Err(error) => Err(error),
            },
            None => Ok(Some(bytes)),
        }
    }

    pub async fn put_bytes(&self, path: &str, bytes: Vec<u8>) -> Result<(), String> {
        let bytes = match self.crypto_context() {
            Some(crypto) => crypto.encrypt_bytes(&normalize_path(path), &bytes)?,
            None => bytes,
        };
        self.put_raw_bytes(path, bytes).await
    }

    pub async fn upload_encrypted_file_with_progress(
        &self,
        path: &str,
        source: &Path,
        plain_size: u64,
        chunk_size: usize,
        progress: Option<Arc<dyn Fn(u64) + Send + Sync + 'static>>,
    ) -> Result<String, String> {
        let crypto = self.crypto_context().ok_or_else(|| "WebDAV 云端加密未启用".to_string())?;
        let remote_path = normalize_path(path);
        let encrypted_size = crypto.encrypted_file_size(plain_size, chunk_size)?;
        let source = source.to_path_buf();
        let (writer, reader) = tokio::io::duplex(chunk_size.min(1024 * 1024).max(64 * 1024));
        let encrypt_task = tokio::spawn(async move {
            let file = tokio::fs::File::open(&source)
                .await
                .map_err(|e| format!("打开待上传文件失败: {}", e))?;
            crypto
                .write_encrypted_file(&remote_path, file, writer, plain_size, chunk_size, progress)
                .await
        });

        let upload_result = self.put_raw_stream(path, reader, encrypted_size).await;
        if let Err(error) = upload_result {
            encrypt_task.abort();
            let _ = encrypt_task.await;
            return Err(error);
        }

        let encrypt_result = encrypt_task
            .await
            .map_err(|e| format!("云端文件加密任务失败: {}", e))?;
        match encrypt_result {
            Err(error) => Err(error),
            Ok(sha256) => Ok(sha256),
        }
    }

    pub(crate) async fn get_plain_json<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, String> {
        let Some(bytes) = self.get_raw_bytes(path).await? else {
            return Ok(None);
        };
        let value = serde_json::from_slice(&bytes).map_err(|e| format!("解析 WebDAV JSON 失败: {}", e))?;
        Ok(Some(value))
    }

    pub(crate) async fn put_plain_json<T: Serialize + ?Sized>(&self, path: &str, value: &T) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        self.put_raw_bytes(path, body).await
    }

    async fn get_raw_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let resp = self.request(Method::GET, path).send().await.map_err(map_reqwest_error)?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format_webdav_status_error("读取 WebDAV 文件失败", resp.status()));
        }
        Ok(Some(resp.bytes().await.map_err(map_reqwest_error)?.to_vec()))
    }

    async fn put_raw_bytes(&self, path: &str, bytes: Vec<u8>) -> Result<(), String> {
        let resp = self
            .request(Method::PUT, path)
            .body(bytes)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format_webdav_status_error("写入 WebDAV 文件失败", resp.status()))
        }
    }

    async fn put_raw_stream(&self, path: &str, reader: DuplexStream, content_length: u64) -> Result<(), String> {
        let stream = ReaderStream::new(reader);
        let resp = self
            .request(Method::PUT, path)
            .header(reqwest::header::CONTENT_LENGTH, content_length)
            .body(Body::wrap_stream(stream))
            .send()
            .await
            .map_err(map_reqwest_error)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format_webdav_status_error("写入 WebDAV 文件失败", resp.status()))
        }
    }

    pub async fn download_encrypted_file(&self, path: &str, destination: &Path) -> Result<String, String> {
        let crypto = self.crypto_context().ok_or_else(|| "WebDAV 云端加密未启用".to_string())?;
        match self.download_encrypted_file_with_context(path, destination, crypto).await {
            Ok(sha256) => Ok(sha256),
            Err(error) if should_refresh_encryption_config(&error) => {
                self.refresh_encryption_config().await?;
                let crypto = self.crypto_context().ok_or_else(|| "WebDAV 云端加密未启用".to_string())?;
                self.download_encrypted_file_with_context(path, destination, crypto).await
            }
            Err(error) => Err(error),
        }
    }

    async fn download_encrypted_file_with_context(
        &self,
        path: &str,
        destination: &Path,
        crypto: WebdavCryptoContext,
    ) -> Result<String, String> {
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("创建下载目录失败: {}", e))?;
        }

        let resp = self.request(Method::GET, path).send().await.map_err(map_reqwest_error)?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Err("云端文件不存在".to_string());
        }
        if !resp.status().is_success() {
            return Err(format_webdav_status_error("读取 WebDAV 文件失败", resp.status()));
        }

        let stream = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
        let reader = tokio_util::io::StreamReader::new(stream);
        let file = tokio::fs::File::create(destination)
            .await
            .map_err(|e| format!("创建本地下载文件失败: {}", e))?;
        crypto
            .read_encrypted_file(&normalize_path(path), reader, file)
            .await
    }

    pub async fn delete_path(&self, path: &str) -> Result<(), String> {
        let resp = self.request(Method::DELETE, path).send().await.map_err(map_reqwest_error)?;
        if resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(format_webdav_status_error("删除 WebDAV 文件失败", resp.status()))
        }
    }

    pub async fn mkcol(&self, path: &str) -> Result<(), String> {
        let method = Method::from_bytes(b"MKCOL").map_err(|e| e.to_string())?;
        let resp = self.request(method, path).send().await.map_err(map_reqwest_error)?;
        let status = resp.status();
        if status.is_success()
            || status == StatusCode::METHOD_NOT_ALLOWED
            || status.as_u16() == 405
        {
            Ok(())
        } else if status == StatusCode::CONFLICT && self.collection_exists(path).await? {
            Ok(())
        } else {
            Err(format_webdav_status_error("创建 WebDAV 目录失败", status))
        }
    }

    async fn collection_exists(&self, path: &str) -> Result<bool, String> {
        let method = Method::from_bytes(b"PROPFIND").map_err(|e| e.to_string())?;
        let resp = self
            .request(method, path)
            .header("Depth", "0")
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = resp.status();
        if status == StatusCode::NOT_FOUND || status == StatusCode::CONFLICT {
            return Ok(false);
        }
        if status == StatusCode::UNAUTHORIZED
            || status == StatusCode::FORBIDDEN
            || status.as_u16() == 507
            || status.is_server_error()
        {
            return Err(format_webdav_status_error("创建 WebDAV 目录失败", status));
        }
        Ok(true)
    }

    async fn mkcol_with_parent_retry(&self, parent: &str, path: &str) -> Result<(), String> {
        if let Err(error) = self.mkcol(path).await {
            if !is_webdav_conflict(&error) {
                return Err(error);
            }
            self.mkcol(parent).await?;
            self.mkcol(path).await?;
        }
        Ok(())
    }

    pub fn mark_dir_ensured(&self, path: &str) {
        self.ensured_dirs.lock().insert(normalize_path(path));
    }

    async fn ensure_dir_cached(&self, path: &str) -> Result<(), String> {
        let path = normalize_path(path);
        if self.ensured_dirs.lock().contains(&path) {
            return Ok(());
        }
        self.mkcol(&path).await?;
        self.ensured_dirs.lock().insert(path);
        Ok(())
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let url = self.url_for(path);
        let builder = self.client.request(method, url);
        if self.config.username.trim().is_empty() {
            builder
        } else {
            builder.basic_auth(self.config.username.trim().to_string(), Some(self.config.password.clone()))
        }
    }

    fn url_for(&self, path: &str) -> String {
        let path = normalize_path(path);
        if path.is_empty() {
            self.base_url.clone()
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }

    fn encryption_scope(&self) -> String {
        format!("{}\n{}", self.base_url, self.config.username.trim())
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_webdav_conflict(error: &str) -> bool {
    error.contains("409") || error.contains("Conflict")
}

fn should_refresh_encryption_config(error: &str) -> bool {
    error.contains("解密失败")
        || error.contains("加密格式不兼容")
        || error.contains("不是 QuickClipboard 加密格式")
}

fn map_reqwest_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        return "WebDAV 请求超时".to_string();
    }
    if error.is_connect() || error.is_request() {
        return WEBDAV_NETWORK_ERROR.to_string();
    }
    format!("WebDAV 请求失败: {}", error)
}

fn format_webdav_status_error(action: &str, status: StatusCode) -> String {
    let hint = match status {
        StatusCode::UNAUTHORIZED => "账号或密码不正确",
        StatusCode::FORBIDDEN => "当前账号没有访问权限",
        StatusCode::NOT_FOUND => "路径不存在",
        StatusCode::CONFLICT => "目标目录不存在或路径冲突",
        _ if status.as_u16() == 507 => "存储空间不足",
        _ if status.is_server_error() => "服务器返回异常",
        _ => "请求被服务器拒绝",
    };
    format!("{}: {} ({})", action, hint, status.as_u16())
}
