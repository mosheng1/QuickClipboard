use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tauri::{
    AppHandle, DragDropEvent, Emitter, Manager, PhysicalPosition, PhysicalSize, Url, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

const DROP_PROXY_LABEL: &str = "drop-proxy";
const DROP_PROXY_PATHS_EVENT: &str = "drop-proxy-paths";
const DROP_PROXY_LEAVE_EVENT: &str = "drop-proxy-leave";
const DROP_PROXY_RESOURCE_DIR: &str = "drop_proxy_resources";
const TRANSFER_SHELF_LABEL_PREFIX: &str = "transfer-shelf-";

static DROP_PROXY_TARGET_LABEL: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropProxyBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DropProxyPathsPayload {
    target_label: String,
    paths: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DropProxyLeavePayload {
    target_label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropProxyResourcePayload {
    pub path: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropProxyCursorPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropProxyRouteResult {
    pub routed: bool,
    pub target_label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropProxyCleanupPayload {
    pub deleted: usize,
}

fn set_target_label(target_label: Option<String>) -> Option<String> {
    let mut current = DROP_PROXY_TARGET_LABEL.lock();
    let previous = current.clone();
    *current = target_label;
    previous
}

fn current_target_label() -> Option<String> {
    DROP_PROXY_TARGET_LABEL.lock().clone()
}

fn create_drop_proxy(app: &AppHandle) -> Result<WebviewWindow, String> {
    let window = WebviewWindowBuilder::new(
        app,
        DROP_PROXY_LABEL,
        WebviewUrl::External(Url::parse("about:blank").map_err(|e| e.to_string())?),
    )
    .title("拖放接收层")
    .inner_size(320.0, 180.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .resizable(false)
    .focusable(false)
    .maximizable(false)
    .minimizable(false)
    .drag_and_drop(true)
    .build()
    .map_err(|e| format!("创建拖放代理窗口失败: {}", e))?;

    bind_drop_events(&window, app.clone());

    Ok(window)
}

fn bind_drop_events(window: &WebviewWindow, app: AppHandle) {
    window.on_window_event(move |event| {
        if let WindowEvent::DragDrop(drag_event) = event {
            match drag_event {
                DragDropEvent::Drop { paths, .. } => {
                    let payload_paths = paths_to_strings(paths);
                    if payload_paths.is_empty() {
                        emit_drop_leave(&app);
                        let _ = hide_drop_proxy(&app);
                        return;
                    }

                    emit_drop_paths(&app, payload_paths);
                    let _ = hide_drop_proxy(&app);
                }
                DragDropEvent::Leave => {
                    emit_drop_leave(&app);
                    let _ = hide_drop_proxy(&app);
                }
                _ => {}
            }
        }
    });
}

fn emit_drop_paths(app: &AppHandle, paths: Vec<String>) {
    if let Some(target_label) = current_target_label() {
        emit_drop_paths_to(app, &target_label, paths);
    }
}

fn emit_drop_leave(app: &AppHandle) {
    if let Some(target_label) = current_target_label() {
        emit_drop_leave_to(app, &target_label);
    }
}

fn emit_drop_paths_to(app: &AppHandle, target_label: &str, paths: Vec<String>) {
    let _ = app.emit_to(
        target_label,
        DROP_PROXY_PATHS_EVENT,
        DropProxyPathsPayload {
            target_label: target_label.to_string(),
            paths,
        },
    );
}

fn emit_drop_leave_to(app: &AppHandle, target_label: &str) {
    let _ = app.emit_to(
        target_label,
        DROP_PROXY_LEAVE_EVENT,
        DropProxyLeavePayload {
            target_label: target_label.to_string(),
        },
    );
}

fn get_or_create_drop_proxy(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(DROP_PROXY_LABEL)
        .map(Ok)
        .unwrap_or_else(|| create_drop_proxy(app))
}

async fn run_on_main_thread_result<F>(app: &AppHandle, task: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String> + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(task());
    })
    .map_err(|error| format!("调度拖放代理任务失败: {}", error))?;

    tokio::task::spawn_blocking(move || {
        receiver
            .recv()
            .map_err(|_| "拖放代理任务已取消".to_string())?
    })
    .await
    .map_err(|error| format!("等待拖放代理任务失败: {}", error))?
}

fn ensure_drop_proxy_sync(app: &AppHandle) -> Result<(), String> {
    let _ = get_or_create_drop_proxy(app)?;
    Ok(())
}

pub async fn ensure_drop_proxy(app: AppHandle) -> Result<(), String> {
    let task_app = app.clone();
    run_on_main_thread_result(&app, move || ensure_drop_proxy_sync(&task_app)).await
}

fn show_drop_proxy_sync(
    app: &AppHandle,
    target_label: &str,
    bounds: DropProxyBounds,
) -> Result<(), String> {
    let window = app
        .get_webview_window(target_label)
        .ok_or_else(|| "目标窗口不存在，无法显示拖放代理".to_string())?;
    let proxy = get_or_create_drop_proxy(app)?;

    let scale_factor = window
        .scale_factor()
        .map_err(|e| format!("读取窗口缩放因子失败: {}", e))?;
    let inner_position = window
        .inner_position()
        .map_err(|e| format!("读取窗口位置失败: {}", e))?;

    let x = inner_position.x + (bounds.x * scale_factor).round() as i32;
    let y = inner_position.y + (bounds.y * scale_factor).round() as i32;
    let width = (bounds.width * scale_factor).round().max(1.0) as u32;
    let height = (bounds.height * scale_factor).round().max(1.0) as u32;

    let next_target_label = window.label().to_string();
    let previous_target_label = set_target_label(Some(next_target_label.clone()));
    if let Some(previous_target_label) = previous_target_label {
        if previous_target_label != next_target_label {
            emit_drop_leave_to(app, &previous_target_label);
        }
    }

    if let Err(error) = proxy.set_position(PhysicalPosition::new(x, y)) {
        let _ = set_target_label(None);
        return Err(format!("设置拖放代理位置失败: {}", error));
    }

    if let Err(error) = proxy.set_size(PhysicalSize::new(width, height)) {
        let _ = set_target_label(None);
        return Err(format!("设置拖放代理尺寸失败: {}", error));
    }

    let _ = proxy.set_always_on_top(true);
    if let Err(error) = proxy.show() {
        let _ = set_target_label(None);
        return Err(format!("显示拖放代理窗口失败: {}", error));
    }

    Ok(())
}

pub async fn show_drop_proxy(window: WebviewWindow, bounds: DropProxyBounds) -> Result<(), String> {
    let app = window.app_handle();
    let target_label = window.label().to_string();
    let task_app = app.clone();
    run_on_main_thread_result(&app, move || {
        show_drop_proxy_sync(&task_app, &target_label, bounds)
    })
    .await
}

pub fn hide_drop_proxy(app: &AppHandle) -> Result<(), String> {
    if let Some(target_label) = set_target_label(None) {
        emit_drop_leave_to(app, &target_label);
    }
    if let Some(window) = app.get_webview_window(DROP_PROXY_LABEL) {
        let _ = window.hide();
    }
    Ok(())
}

pub fn dispose_drop_proxy(app: &AppHandle) -> Result<(), String> {
    let _ = set_target_label(None);
    if let Some(window) = app.get_webview_window(DROP_PROXY_LABEL) {
        let _ = window.close();
    }
    Ok(())
}

pub fn route_paths_at_cursor(
    app: &AppHandle,
    source_label: &str,
    paths: Vec<String>,
    cursor_pos: DropProxyCursorPosition,
) -> Result<DropProxyRouteResult, String> {
    if paths.is_empty() {
        return Ok(DropProxyRouteResult {
            routed: false,
            target_label: None,
        });
    }

    let x = cursor_pos.x.round() as i32;
    let y = cursor_pos.y.round() as i32;
    let target_label = app
        .webview_windows()
        .into_iter()
        .filter(|(label, _)| {
            label.starts_with(TRANSFER_SHELF_LABEL_PREFIX) && label.as_str() != source_label
        })
        .find_map(|(label, window)| {
            let position = window.outer_position().ok()?;
            let size = window.outer_size().ok()?;
            let left = position.x;
            let top = position.y;
            let right = left.saturating_add(size.width as i32);
            let bottom = top.saturating_add(size.height as i32);
            (x >= left && x <= right && y >= top && y <= bottom).then_some(label)
        });

    if let Some(target_label) = target_label {
        emit_drop_paths_to(app, &target_label, paths);
        emit_drop_leave_to(app, &target_label);
        let _ = hide_drop_proxy(app);
        return Ok(DropProxyRouteResult {
            routed: true,
            target_label: Some(target_label),
        });
    }

    Ok(DropProxyRouteResult {
        routed: false,
        target_label: None,
    })
}

pub async fn save_drop_resource(
    filename: String,
    data: Vec<u8>,
) -> Result<DropProxyResourcePayload, String> {
    tokio::task::spawn_blocking(move || save_drop_resource_blocking(&filename, &data))
        .await
        .map_err(|error| format!("保存拖放资源任务失败: {}", error))?
}

pub async fn save_drop_url(
    filename: String,
    url: String,
) -> Result<DropProxyResourcePayload, String> {
    tokio::task::spawn_blocking(move || save_drop_url_blocking(&filename, &url))
        .await
        .map_err(|error| format!("保存拖放 URL 任务失败: {}", error))?
}

pub async fn cleanup_orphan_resources(min_age_ms: u64) -> Result<DropProxyCleanupPayload, String> {
    tokio::task::spawn_blocking(move || {
        cleanup_orphan_resources_blocking(Duration::from_millis(min_age_ms))
    })
    .await
    .map_err(|error| format!("清理拖放临时资源任务失败: {}", error))?
}

pub fn schedule_cleanup_orphan_resources(min_age_ms: u64, delay_ms: u64) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        let _ = cleanup_orphan_resources(min_age_ms).await;
    });
}

fn resource_dir() -> Result<PathBuf, String> {
    Ok(crate::services::get_data_directory()?.join(DROP_PROXY_RESOURCE_DIR))
}

fn save_drop_resource_blocking(
    filename: &str,
    data: &[u8],
) -> Result<DropProxyResourcePayload, String> {
    if data.is_empty() {
        return Err("拖放资源内容为空".to_string());
    }

    let target_dir = resource_dir()?;
    std::fs::create_dir_all(&target_dir)
        .map_err(|error| format!("创建拖放资源目录失败: {}", error))?;

    let safe_name = sanitize_filename(filename);
    let path = next_available_path(&target_dir, &safe_name);
    std::fs::write(&path, data).map_err(|error| format!("写入拖放资源失败: {}", error))?;

    Ok(DropProxyResourcePayload {
        path: path.to_string_lossy().to_string(),
    })
}

fn save_drop_url_blocking(filename: &str, url: &str) -> Result<DropProxyResourcePayload, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("拖放 URL 为空".to_string());
    }

    let parsed =
        reqwest::Url::parse(url).map_err(|error| format!("解析拖放 URL 失败: {}", error))?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("仅支持 http 或 https 拖放 URL".to_string()),
    }
    let url_path = parsed.path().to_string();

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("创建拖放 URL 下载客户端失败: {}", error))?;

    let response = client
        .get(parsed)
        .send()
        .map_err(|error| format!("下载拖放 URL 失败: {}", error))?;

    if !response.status().is_success() {
        return Err(format!("下载拖放 URL 失败: HTTP {}", response.status()));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .unwrap_or_default();
    let bytes = response
        .bytes()
        .map_err(|error| format!("读取拖放 URL 内容失败: {}", error))?;
    let resolved_filename = resolve_url_filename(filename, &url_path, &content_type);
    save_drop_resource_blocking(&resolved_filename, bytes.as_ref())
}

fn cleanup_orphan_resources_blocking(min_age: Duration) -> Result<DropProxyCleanupPayload, String> {
    let target_dir = resource_dir()?;
    if !target_dir.exists() {
        return Ok(DropProxyCleanupPayload { deleted: 0 });
    }

    let target_root = target_dir
        .canonicalize()
        .map_err(|error| format!("读取拖放资源目录失败: {}", error))?;
    let referenced_paths = crate::windows::transfer_shelf::persisted_file_paths();
    let referenced = referenced_paths
        .into_iter()
        .filter_map(|path| PathBuf::from(path).canonicalize().ok())
        .filter(|path| path.starts_with(&target_root))
        .collect::<HashSet<_>>();

    let now = SystemTime::now();
    let mut deleted = 0;
    let entries = std::fs::read_dir(&target_root)
        .map_err(|error| format!("扫描拖放资源目录失败: {}", error))?;
    for entry in entries.flatten() {
        let metadata = match entry.metadata() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !metadata.is_file() {
            continue;
        }

        let path = match entry.path().canonicalize() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !path.starts_with(&target_root) || referenced.contains(&path) {
            continue;
        }

        let old_enough = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .map(|age| age >= min_age)
            .unwrap_or(true);
        if !old_enough {
            continue;
        }

        if std::fs::remove_file(&path).is_ok() {
            deleted += 1;
        }
    }

    Ok(DropProxyCleanupPayload { deleted })
}

fn resolve_url_filename(filename: &str, url_path: &str, content_type: &str) -> String {
    let name = sanitize_filename(filename);
    let path = Path::new(&name);
    if path.extension().and_then(|value| value.to_str()).is_some() {
        return name;
    }

    let url_ext = Path::new(url_path)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty());
    let content_ext = extension_from_content_type(content_type);
    match url_ext.or(content_ext) {
        Some(ext) => format!("{}.{}", name, ext),
        None => name,
    }
}

fn extension_from_content_type(content_type: &str) -> Option<&'static str> {
    let normalized = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    match normalized.as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/bmp" => Some("bmp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

fn sanitize_filename(filename: &str) -> String {
    let trimmed = filename.trim();
    let name = Path::new(trimmed)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("drop-resource");
    let sanitized = name
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\u{1f}' => '_',
            _ => ch,
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches(['.', ' ']).trim();
    if sanitized.is_empty() {
        "drop-resource".to_string()
    } else {
        sanitized.chars().take(160).collect()
    }
}

fn next_available_path(dir: &Path, filename: &str) -> PathBuf {
    let mut path = dir.join(filename);
    if !path.exists() {
        return path;
    }

    let stem = Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("drop-resource");
    let ext = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty());

    for index in 1.. {
        let candidate = match ext {
            Some(ext) => format!("{}_{}.{}", stem, index, ext),
            None => format!("{}_{}", stem, index),
        };
        path = dir.join(candidate);
        if !path.exists() {
            return path;
        }
    }

    path
}

fn paths_to_strings(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}
