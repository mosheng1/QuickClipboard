// 持久化存储服务
// 封装 tauri-plugin-store，供 Rust 代码使用

use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

// 默认存储文件名
const DEFAULT_STORE_FILE: &str = "app-store.json";

// 全局 AppHandle 引用
static APP_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));

// 初始化存储服务
pub fn init(app: &AppHandle) {
    let mut handle = APP_HANDLE.lock().unwrap();
    *handle = Some(app.clone());
}

// 获取存储路径
fn get_store_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(DEFAULT_STORE_FILE)
}

// 获取值
pub fn get<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
    let handle = APP_HANDLE.lock().unwrap();
    let app = handle.as_ref()?;

    let store_path = get_store_path(app);
    let store = app.store(store_path).ok()?;

    store.get(key).and_then(|v| serde_json::from_value(v).ok())
}

// 设置值
pub fn set<T: serde::Serialize>(key: &str, value: &T) -> Result<(), String> {
    let handle = APP_HANDLE.lock().unwrap();
    let app = handle.as_ref().ok_or("AppHandle 未初始化")?;

    let store_path = get_store_path(app);
    let store = app.store(store_path).map_err(|e| e.to_string())?;

    let json_value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    store.set(key, json_value);
    store.save().map_err(|e| e.to_string())?;

    Ok(())
}

// 删除值
pub fn delete(key: &str) -> Result<(), String> {
    let handle = APP_HANDLE.lock().unwrap();
    let app = handle.as_ref().ok_or("AppHandle 未初始化")?;

    let store_path = get_store_path(app);
    let store = app.store(store_path).map_err(|e| e.to_string())?;

    store.delete(key);
    store.save().map_err(|e| e.to_string())?;

    Ok(())
}

// 检查键是否存在
pub fn has(key: &str) -> bool {
    let handle = APP_HANDLE.lock().unwrap();
    let Some(app) = handle.as_ref() else {
        return false;
    };

    let store_path = get_store_path(app);
    let Ok(store) = app.store(store_path) else {
        return false;
    };

    store.has(key)
}

// 获取所有键
pub fn keys() -> Vec<String> {
    let handle = APP_HANDLE.lock().unwrap();
    let Some(app) = handle.as_ref() else {
        return vec![];
    };

    let store_path = get_store_path(app);
    let Ok(store) = app.store(store_path) else {
        return vec![];
    };

    store.keys().into_iter().map(|s| s.to_string()).collect()
}
