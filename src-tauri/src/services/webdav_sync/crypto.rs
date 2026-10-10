use std::collections::HashMap;
use std::sync::Arc;

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose, Engine as _};
use chacha20poly1305::aead::{Aead, AeadInPlace, Payload};
use chacha20poly1305::{KeyInit, Tag, XChaCha20Poly1305, XNonce};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroize;

pub const CONFIG_PATH: &str = ".qc-e2ee.json";

const CONFIG_FORMAT: &str = "qc-e2ee-config-v1";
const DATA_FORMAT: &str = "qc-e2ee-data-v1";
const FILE_MAGIC: &[u8; 8] = b"QCFE2EE1";
const FILE_FRAME_AAD_PREFIX: &[u8] = b"qc-e2ee-file-frame-v1";
const CIPHER_NAME: &str = "xchacha20poly1305";
const KDF_NAME: &str = "argon2id";
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const TAG_LEN: u64 = 16;
const FILE_HEADER_LEN: u64 = 24;
const FILE_FRAME_HEADER_LEN: u64 = 28;
const MAX_FILE_CHUNK_SIZE: usize = 64 * 1024 * 1024;
const DEFAULT_MEMORY_KIB: u32 = 64 * 1024;
const DEFAULT_ITERATIONS: u32 = 3;
const DEFAULT_PARALLELISM: u32 = 1;

static MASTER_KEY_CACHE: Lazy<Mutex<HashMap<String, Arc<MasterKey>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static CONFIG_CACHE: Lazy<Mutex<HashMap<String, WebdavE2eeConfig>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebdavE2eeConfig {
    pub format: String,
    pub kdf: KdfConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfConfig {
    pub name: String,
    pub salt: String,
    #[serde(rename = "memoryKiB")]
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

#[derive(Clone)]
pub struct WebdavCryptoContext {
    key: Arc<MasterKey>,
}

struct MasterKey {
    bytes: [u8; KEY_LEN],
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

#[derive(Serialize, Deserialize)]
struct DataEnvelope {
    format: String,
    cipher: CipherEnvelope,
    payload: String,
}

#[derive(Serialize, Deserialize)]
struct CipherEnvelope {
    name: String,
    nonce: String,
}

pub fn create_config() -> WebdavE2eeConfig {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    WebdavE2eeConfig {
        format: CONFIG_FORMAT.to_string(),
        kdf: KdfConfig {
            name: KDF_NAME.to_string(),
            salt: general_purpose::STANDARD.encode(salt),
            memory_kib: DEFAULT_MEMORY_KIB,
            iterations: DEFAULT_ITERATIONS,
            parallelism: DEFAULT_PARALLELISM,
        },
    }
}

pub fn context_for_config(
    scope: &str,
    config: &WebdavE2eeConfig,
    password: &str,
) -> Result<WebdavCryptoContext, String> {
    validate_config(config)?;
    if password.is_empty() {
        return Err("请先设置 WebDAV 云端加密密码".to_string());
    }

    let cache_key = cache_key(scope, config, password);
    if let Some(key) = MASTER_KEY_CACHE.lock().get(&cache_key).cloned() {
        return Ok(WebdavCryptoContext { key });
    }

    let key = Arc::new(MasterKey {
        bytes: derive_master_key(config, password)?,
    });
    MASTER_KEY_CACHE.lock().insert(cache_key, key.clone());
    Ok(WebdavCryptoContext { key })
}

pub fn cached_config(scope: &str) -> Option<WebdavE2eeConfig> {
    CONFIG_CACHE.lock().get(scope).cloned()
}

pub fn cache_config(scope: &str, config: &WebdavE2eeConfig) {
    CONFIG_CACHE
        .lock()
        .insert(scope.to_string(), config.clone());
}

pub fn clear_cached_keys() {
    MASTER_KEY_CACHE.lock().clear();
    CONFIG_CACHE.lock().clear();
}

impl WebdavCryptoContext {
    pub fn encrypted_file_size(&self, plain_size: u64, chunk_size: usize) -> Result<u64, String> {
        validate_file_chunk_size(chunk_size)?;
        let chunk_size = chunk_size as u64;
        let frames = if plain_size == 0 {
            0
        } else {
            plain_size
                .checked_add(chunk_size - 1)
                .ok_or_else(|| "云端文件过大".to_string())?
                / chunk_size
        };
        FILE_HEADER_LEN
            .checked_add(plain_size)
            .and_then(|value| {
                value.checked_add(frames.checked_mul(FILE_FRAME_HEADER_LEN + TAG_LEN)?)
            })
            .ok_or_else(|| "云端加密文件大小溢出".to_string())
    }

    pub async fn write_encrypted_file<R, W>(
        &self,
        path: &str,
        mut reader: R,
        mut writer: W,
        plain_size: u64,
        chunk_size: usize,
        progress: Option<Arc<dyn Fn(u64) + Send + Sync + 'static>>,
    ) -> Result<String, String>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        validate_file_chunk_size(chunk_size)?;
        if let Some(callback) = progress.as_ref() {
            callback(0);
        }

        writer
            .write_all(FILE_MAGIC)
            .await
            .map_err(|e| format!("写入云端加密文件头失败: {}", e))?;
        writer
            .write_all(&(chunk_size as u64).to_le_bytes())
            .await
            .map_err(|e| format!("写入云端加密文件头失败: {}", e))?;
        writer
            .write_all(&plain_size.to_le_bytes())
            .await
            .map_err(|e| format!("写入云端加密文件头失败: {}", e))?;

        let cipher = XChaCha20Poly1305::new_from_slice(&self.key.bytes)
            .map_err(|e| format!("初始化 WebDAV 加密器失败: {}", e))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; chunk_size];
        let mut sent_bytes = 0u64;
        let mut frame_index = 0u64;

        loop {
            let read = read_file_chunk(&mut reader, &mut buffer).await?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            let plain_len = u32::try_from(read).map_err(|_| "云端文件分片过大".to_string())?;
            let mut nonce = [0u8; NONCE_LEN];
            OsRng.fill_bytes(&mut nonce);
            let tag = cipher
                .encrypt_in_place_detached(
                    XNonce::from_slice(&nonce),
                    &file_frame_aad(path, frame_index, plain_len),
                    &mut buffer[..read],
                )
                .map_err(|_| "加密云端文件分片失败".to_string())?;

            writer
                .write_all(&plain_len.to_le_bytes())
                .await
                .map_err(|e| format!("写入云端加密文件分片失败: {}", e))?;
            writer
                .write_all(&nonce)
                .await
                .map_err(|e| format!("写入云端加密文件分片失败: {}", e))?;
            writer
                .write_all(&buffer[..read])
                .await
                .map_err(|e| format!("写入云端加密文件分片失败: {}", e))?;
            writer
                .write_all(&tag)
                .await
                .map_err(|e| format!("写入云端加密文件分片失败: {}", e))?;

            sent_bytes = sent_bytes.saturating_add(read as u64);
            if sent_bytes > plain_size {
                return Err("待上传文件大小发生变化".to_string());
            }
            if let Some(callback) = progress.as_ref() {
                callback(sent_bytes);
            }
            frame_index = frame_index
                .checked_add(1)
                .ok_or_else(|| "云端文件分片数量过多".to_string())?;
        }

        if sent_bytes != plain_size {
            return Err("待上传文件大小发生变化".to_string());
        }
        writer
            .shutdown()
            .await
            .map_err(|e| format!("结束云端加密上传流失败: {}", e))?;
        Ok(hex::encode(hasher.finalize()))
    }

    pub async fn read_encrypted_file<R, W>(
        &self,
        path: &str,
        mut reader: R,
        mut writer: W,
    ) -> Result<String, String>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut magic = [0u8; FILE_MAGIC.len()];
        reader
            .read_exact(&mut magic)
            .await
            .map_err(|e| format!("读取云端加密文件头失败: {}", e))?;
        if &magic != FILE_MAGIC {
            return Err("云端文件加密格式不兼容".to_string());
        }

        let mut u64_bytes = [0u8; 8];
        reader
            .read_exact(&mut u64_bytes)
            .await
            .map_err(|e| format!("读取云端加密文件头失败: {}", e))?;
        let chunk_size = u64::from_le_bytes(u64_bytes);
        let chunk_size_usize =
            usize::try_from(chunk_size).map_err(|_| "云端文件分片大小无效".to_string())?;
        validate_file_chunk_size(chunk_size_usize)?;

        reader
            .read_exact(&mut u64_bytes)
            .await
            .map_err(|e| format!("读取云端加密文件头失败: {}", e))?;
        let plain_size = u64::from_le_bytes(u64_bytes);

        let cipher = XChaCha20Poly1305::new_from_slice(&self.key.bytes)
            .map_err(|e| format!("初始化 WebDAV 解密器失败: {}", e))?;
        let mut hasher = Sha256::new();
        let mut remaining = plain_size;
        let mut frame_index = 0u64;
        let mut encrypted = vec![0u8; chunk_size_usize + TAG_LEN as usize];
        while remaining > 0 {
            let mut len_bytes = [0u8; 4];
            reader
                .read_exact(&mut len_bytes)
                .await
                .map_err(|e| format!("读取云端加密文件分片失败: {}", e))?;
            let plain_len = u32::from_le_bytes(len_bytes);
            if plain_len == 0
                || plain_len as u64 > remaining
                || plain_len as usize > chunk_size_usize
            {
                return Err("云端加密文件分片长度无效".to_string());
            }

            let mut nonce = [0u8; NONCE_LEN];
            reader
                .read_exact(&mut nonce)
                .await
                .map_err(|e| format!("读取云端加密文件分片失败: {}", e))?;
            let encrypted_len = plain_len as usize + TAG_LEN as usize;
            reader
                .read_exact(&mut encrypted[..encrypted_len])
                .await
                .map_err(|e| format!("读取云端加密文件分片失败: {}", e))?;

            let (ciphertext, tag) = encrypted[..encrypted_len].split_at_mut(plain_len as usize);
            cipher
                .decrypt_in_place_detached(
                    XNonce::from_slice(&nonce),
                    &file_frame_aad(path, frame_index, plain_len),
                    ciphertext,
                    Tag::from_slice(tag),
                )
                .map_err(|_| "WebDAV 云端文件解密失败，请检查云端加密密码".to_string())?;
            if ciphertext.len() != plain_len as usize {
                return Err("云端文件解密长度异常".to_string());
            }
            hasher.update(&ciphertext[..]);
            writer
                .write_all(&ciphertext[..])
                .await
                .map_err(|e| format!("写入本地下载文件失败: {}", e))?;

            remaining -= plain_len as u64;
            frame_index = frame_index
                .checked_add(1)
                .ok_or_else(|| "云端文件分片数量过多".to_string())?;
        }

        writer
            .flush()
            .await
            .map_err(|e| format!("写入本地下载文件失败: {}", e))?;
        Ok(hex::encode(hasher.finalize()))
    }

    pub fn encrypt_bytes(&self, path: &str, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key.bytes)
            .map_err(|e| format!("初始化 WebDAV 加密器失败: {}", e))?;
        let payload = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: path.as_bytes(),
                },
            )
            .map_err(|e| format!("加密 WebDAV 数据失败: {}", e))?;
        let envelope = DataEnvelope {
            format: DATA_FORMAT.to_string(),
            cipher: CipherEnvelope {
                name: CIPHER_NAME.to_string(),
                nonce: general_purpose::STANDARD.encode(nonce),
            },
            payload: general_purpose::STANDARD.encode(payload),
        };
        serde_json::to_vec(&envelope).map_err(|e| format!("编码 WebDAV 加密信封失败: {}", e))
    }

    pub fn decrypt_bytes(&self, path: &str, encrypted: &[u8]) -> Result<Vec<u8>, String> {
        let envelope: DataEnvelope = serde_json::from_slice(encrypted)
            .map_err(|_| "WebDAV 数据不是 QuickClipboard 加密格式".to_string())?;
        if envelope.format != DATA_FORMAT {
            return Err("WebDAV 数据加密格式不兼容".to_string());
        }
        if envelope.cipher.name != CIPHER_NAME {
            return Err("WebDAV 数据加密算法不受支持".to_string());
        }
        let nonce = general_purpose::STANDARD
            .decode(envelope.cipher.nonce)
            .map_err(|e| format!("解析 WebDAV 加密 nonce 失败: {}", e))?;
        if nonce.len() != NONCE_LEN {
            return Err("WebDAV 加密 nonce 长度无效".to_string());
        }
        let payload = general_purpose::STANDARD
            .decode(envelope.payload)
            .map_err(|e| format!("解析 WebDAV 加密数据失败: {}", e))?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key.bytes)
            .map_err(|e| format!("初始化 WebDAV 解密器失败: {}", e))?;
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &payload,
                    aad: path.as_bytes(),
                },
            )
            .map_err(|_| "WebDAV 数据解密失败，请检查云端加密密码".to_string())
    }
}

fn validate_config(config: &WebdavE2eeConfig) -> Result<(), String> {
    if config.format != CONFIG_FORMAT {
        return Err("WebDAV 云端加密配置格式不兼容".to_string());
    }
    if config.kdf.name != KDF_NAME {
        return Err("WebDAV 云端加密 KDF 不受支持".to_string());
    }
    if config.kdf.memory_kib == 0 || config.kdf.iterations == 0 || config.kdf.parallelism == 0 {
        return Err("WebDAV 云端加密 KDF 参数无效".to_string());
    }
    Ok(())
}

fn derive_master_key(config: &WebdavE2eeConfig, password: &str) -> Result<[u8; KEY_LEN], String> {
    let salt = general_purpose::STANDARD
        .decode(&config.kdf.salt)
        .map_err(|e| format!("解析 WebDAV 云端加密 salt 失败: {}", e))?;
    let params = Params::new(
        config.kdf.memory_kib,
        config.kdf.iterations,
        config.kdf.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|e| format!("WebDAV 云端加密 KDF 参数无效: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .map_err(|e| format!("派生 WebDAV 云端加密密钥失败: {}", e))?;
    Ok(key)
}

fn cache_key(scope: &str, config: &WebdavE2eeConfig, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!(
        "{}\n{}\n{}",
        scope,
        config.kdf.salt,
        hex::encode(hasher.finalize())
    )
}

fn validate_file_chunk_size(chunk_size: usize) -> Result<(), String> {
    if chunk_size == 0 || chunk_size > MAX_FILE_CHUNK_SIZE {
        return Err("云端文件分片大小无效".to_string());
    }
    Ok(())
}

fn file_frame_aad(path: &str, frame_index: u64, plain_len: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(FILE_FRAME_AAD_PREFIX.len() + path.len() + 20);
    out.extend_from_slice(FILE_FRAME_AAD_PREFIX);
    out.extend_from_slice(path.as_bytes());
    out.extend_from_slice(&frame_index.to_le_bytes());
    out.extend_from_slice(&plain_len.to_le_bytes());
    out
}

async fn read_file_chunk<R>(reader: &mut R, buffer: &mut [u8]) -> Result<usize, String>
where
    R: AsyncRead + Unpin,
{
    let mut read_total = 0usize;
    while read_total < buffer.len() {
        let read = reader
            .read(&mut buffer[read_total..])
            .await
            .map_err(|e| format!("读取待上传文件失败: {}", e))?;
        if read == 0 {
            break;
        }
        read_total = read_total.saturating_add(read);
    }
    Ok(read_total)
}

#[cfg(test)]
mod tests {
    use super::{clear_cached_keys, context_for_config, create_config};
    use tokio::io::AsyncReadExt;

    #[test]
    fn encrypts_and_decrypts_webdav_payload() {
        clear_cached_keys();
        let config = create_config();
        let context = context_for_config("test", &config, "secret").unwrap();
        let encrypted = context
            .encrypt_bytes("history/index.json", b"hello")
            .unwrap();
        assert_ne!(encrypted, b"hello");
        let decrypted = context
            .decrypt_bytes("history/index.json", &encrypted)
            .unwrap();
        assert_eq!(decrypted, b"hello");
    }

    #[test]
    fn rejects_payload_moved_to_another_path() {
        clear_cached_keys();
        let config = create_config();
        let context = context_for_config("test", &config, "secret").unwrap();
        let encrypted = context
            .encrypt_bytes("history/index.json", b"hello")
            .unwrap();
        let result = context.decrypt_bytes("favorites/index.json", &encrypted);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn streams_encrypted_file_frames() {
        clear_cached_keys();
        let config = create_config();
        let context = context_for_config("test", &config, "secret").unwrap();
        let path = "cloud_files/objects/test.qcf";
        let plaintext = (0..10000)
            .map(|value| (value % 251) as u8)
            .collect::<Vec<_>>();
        let plain_len = plaintext.len() as u64;
        let plain_for_task = plaintext.clone();

        let (encrypted_writer, encrypted_reader) = tokio::io::duplex(4096);
        let write_context = context.clone();
        let write_task = tokio::spawn(async move {
            let reader = std::io::Cursor::new(plain_for_task);
            write_context
                .write_encrypted_file(path, reader, encrypted_writer, plain_len, 1024, None)
                .await
        });

        let (decrypted_writer, mut decrypted_reader) = tokio::io::duplex(4096);
        let read_context = context.clone();
        let read_task = tokio::spawn(async move {
            read_context
                .read_encrypted_file(path, encrypted_reader, decrypted_writer)
                .await
        });

        let mut out = Vec::new();
        decrypted_reader.read_to_end(&mut out).await.unwrap();
        write_task.await.unwrap().unwrap();
        read_task.await.unwrap().unwrap();
        assert_eq!(out, plaintext);
    }
}
