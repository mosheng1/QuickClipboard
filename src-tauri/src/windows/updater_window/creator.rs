use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri::{Emitter, Listener};
use tokio::time::interval;

static FORCE_UPDATE_MODE: AtomicBool = AtomicBool::new(false);
static UPDATE_BANNER_STATE: LazyLock<Mutex<Option<UpdateBannerState>>> =
    LazyLock::new(|| Mutex::new(None));
static UPDATE_WINDOW_PAYLOAD: LazyLock<Mutex<Option<serde_json::Value>>> =
    LazyLock::new(|| Mutex::new(None));
const AUTO_UPDATE_CHECK_INTERVAL_SECS: u64 = 60 * 60;
const LAST_AUTO_CHECK_AT_KEY: &str = "updater.last_auto_check_at";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBannerState {
    pub current_version: String,
    pub latest_version: String,
}

pub fn is_force_update_mode() -> bool {
    FORCE_UPDATE_MODE.load(Ordering::Relaxed)
}

pub fn get_update_banner_state() -> Option<UpdateBannerState> {
    UPDATE_BANNER_STATE
        .lock()
        .ok()
        .and_then(|state| state.clone())
}

fn set_update_banner_state(app: &AppHandle, state: Option<UpdateBannerState>) {
    if let Ok(mut guard) = UPDATE_BANNER_STATE.lock() {
        *guard = state.clone();
    }
    let _ = app.emit("update-banner-state-changed", state);
}

fn set_update_window_payload(payload: Option<serde_json::Value>) {
    if let Ok(mut guard) = UPDATE_WINDOW_PAYLOAD.lock() {
        *guard = payload;
    }
}

fn get_update_window_payload() -> Option<serde_json::Value> {
    UPDATE_WINDOW_PAYLOAD
        .lock()
        .ok()
        .and_then(|payload| payload.clone())
}

fn emit_update_payload(window: &WebviewWindow, payload: serde_json::Value) {
    let _ = window.emit("update-config", payload.clone());

    let win_for_emit = window.clone();
    window.once("updater-ready", move |_| {
        let _ = win_for_emit.emit("update-config", payload);
    });
}

fn parse_env_bool(key: &str) -> Option<bool> {
    let raw = std::env::var(key).ok()?;
    let v = raw.trim().to_lowercase();
    match v.as_str() {
        "1" | "true" | "yes" | "y" | "on" => Some(true),
        "0" | "false" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

fn is_prerelease(version: &str) -> bool {
    let v = version.to_lowercase();
    v.contains("alpha") || v.contains("beta") || v.contains("rc") || v.contains("dev")
}

fn normalize_update_check_interval(value: &str) -> &'static str {
    match value {
        "every3days" => "every3days",
        "weekly" => "weekly",
        _ => "daily",
    }
}

fn update_check_interval_seconds(value: &str) -> u64 {
    match normalize_update_check_interval(value) {
        "every3days" => 3 * 24 * 60 * 60,
        "weekly" => 7 * 24 * 60 * 60,
        _ => 24 * 60 * 60,
    }
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn resolve_use_beta_channel(
    settings: &crate::services::AppSettings,
    is_current_prerelease: bool,
) -> bool {
    if let Some(include_beta_updates) = settings.include_beta_updates {
        return include_beta_updates;
    }

    match std::env::var("QC_UPDATE_CHANNEL") {
        Ok(v) if v.trim().eq_ignore_ascii_case("beta") => true,
        Ok(v) if v.trim().eq_ignore_ascii_case("stable") => false,
        _ => is_current_prerelease,
    }
}

async fn check_updates_if_due(app: &AppHandle) -> Result<bool, String> {
    let settings = crate::services::get_settings();
    let now = current_unix_timestamp();
    let interval_secs = update_check_interval_seconds(&settings.update_check_interval);
    let last_check_at = crate::services::store::get::<u64>(LAST_AUTO_CHECK_AT_KEY).unwrap_or(0);

    if last_check_at > 0 && now.saturating_sub(last_check_at) < interval_secs {
        return Ok(false);
    }

    crate::services::store::set(LAST_AUTO_CHECK_AT_KEY, &now)?;
    check_updates(app, !settings.disable_update_popup).await
}

// 检测当前运行的程序是否为安装版
fn is_installed_version() -> bool {
    use std::path::Path;
    use winreg::enums::*;
    use winreg::RegKey;

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let current_dir = match current_exe.parent() {
        Some(p) => p.to_string_lossy().to_lowercase(),
        None => return false,
    };

    let reg_paths = [
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\QuickClipboard",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\QuickClipboard",
        ),
    ];

    for (hkey, path) in reg_paths {
        if let Ok(key) = RegKey::predef(hkey).open_subkey(path) {
            if let Ok(loc) = key.get_value::<String, _>("InstallLocation") {
                let install_path = loc.trim_end_matches('\\').to_lowercase();
                if !install_path.is_empty() && current_dir == install_path {
                    return true;
                }
            }
            if let Ok(uninstall) = key.get_value::<String, _>("UninstallString") {
                let uninstall_clean = uninstall.trim_matches('"');
                if let Some(parent) = Path::new(uninstall_clean).parent() {
                    let install_path = parent.to_string_lossy().to_lowercase();
                    if current_dir == install_path {
                        return true;
                    }
                }
            }
            if let Ok(icon) = key.get_value::<String, _>("DisplayIcon") {
                let icon_clean = icon.split(',').next().unwrap_or("").trim_matches('"');
                if let Some(parent) = Path::new(icon_clean).parent() {
                    let install_path = parent.to_string_lossy().to_lowercase();
                    if current_dir == install_path {
                        return true;
                    }
                }
            }
        }
    }

    false
}

pub fn start_update_checker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _ = check_updates_if_due(&app).await;

        let mut ticker = interval(Duration::from_secs(AUTO_UPDATE_CHECK_INTERVAL_SECS));
        ticker.tick().await;

        loop {
            ticker.tick().await;
            let _ = check_updates_if_due(&app).await;
        }
    });
}

pub fn open_updater_window(app: &AppHandle, force_update: bool) -> Result<WebviewWindow, String> {
    let window = WebviewWindowBuilder::new(
        app,
        "updater",
        WebviewUrl::App("windows/updater/index.html".into()),
    )
    .title("更新")
    .inner_size(360.0, 130.0)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .visible(true)
    .focused(false)
    .focusable(false)
    .drag_and_drop(false)
    .build()
    .map_err(|e| format!("创建更新窗口失败: {}", e))?;

    if let Ok(size) = window.outer_size() {
        let margin: i32 = 12;

        if let Ok(monitor) = crate::screen::ScreenUtils::get_monitor_at_cursor(app) {
            let work_area = monitor.work_area();
            let x = work_area.position.x + work_area.size.width as i32 - size.width as i32 - margin;
            let y =
                work_area.position.y + work_area.size.height as i32 - size.height as i32 - margin;
            let _ = window.set_position(tauri::PhysicalPosition::new(
                x.max(work_area.position.x),
                y.max(work_area.position.y),
            ));
        }
    }

    if force_update {
        let app_for_event = app.clone();
        window.on_window_event(move |event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
            }
            tauri::WindowEvent::Destroyed => {
                app_for_event.exit(0);
            }
            _ => {}
        });
    }

    Ok(window)
}

pub async fn open_cached_update_window(app: &AppHandle) -> Result<bool, String> {
    let payload = match get_update_window_payload() {
        Some(payload) => payload,
        None => return Ok(false),
    };

    let force_update = payload
        .get("forceUpdate")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let window = if let Some(window) = app.get_webview_window("updater") {
        let _ = window.show();
        window
    } else {
        open_updater_window(app, force_update)?
    };

    emit_update_payload(&window, payload);
    Ok(true)
}

async fn check_updates(app: &AppHandle, should_open_window: bool) -> Result<bool, String> {
    use std::time::Duration;
    use tauri_plugin_updater::UpdaterExt;

    let settings = crate::services::get_settings();
    let current_version = app.package_info().version.to_string();
    let is_current_prerelease = is_prerelease(&current_version);

    let entry_url = "https://api.quickclipboard.cn/update/latest.json";

    let mut stable_json_url: Option<String> = None;
    let mut beta_json_url: Option<String> = None;

    if let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        if let Ok(resp) = client.get(entry_url).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                stable_json_url = json
                    .get("stableJson")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                beta_json_url = json
                    .get("betaJson")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    let stable_url = stable_json_url
        .unwrap_or_else(|| "https://api.quickclipboard.cn/update/stable_latest.json".to_string());
    let beta_url = beta_json_url
        .unwrap_or_else(|| "https://api.quickclipboard.cn/update/beta_latest.json".to_string());

    let use_beta_channel = resolve_use_beta_channel(&settings, is_current_prerelease);

    let chosen_manifest_url = if use_beta_channel {
        beta_url
    } else {
        stable_url
    };
    let chosen_manifest_url_str = chosen_manifest_url.clone();

    let mut force_update = false;
    let mut notes: Option<serde_json::Value> = None;

    if let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        if let Ok(resp) = client.get(&chosen_manifest_url_str).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                force_update = json
                    .get("forceUpdate")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                notes = json.get("notes").cloned();
            }
        }
    }

    let chosen_manifest_endpoint =
        tauri::Url::parse(&chosen_manifest_url_str).map_err(|e| format!("{}", e))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![chosen_manifest_endpoint])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => {
            let new_version = update.version.clone();

            if !use_beta_channel && is_prerelease(&new_version) {
                set_update_banner_state(app, None);
                set_update_window_payload(None);
                return Ok(false);
            }

            set_update_banner_state(
                app,
                Some(UpdateBannerState {
                    current_version: current_version.clone(),
                    latest_version: new_version.clone(),
                }),
            );

            if force_update {
                FORCE_UPDATE_MODE.store(true, Ordering::Relaxed);
                if let Some(main_window) = app.get_webview_window("main") {
                    crate::hide_main_window(&main_window);
                }
                if let Some(qp_window) = app.get_webview_window("quickpaste") {
                    let _ = qp_window.hide();
                }
                let _ = crate::hotkey::disable_hotkeys();
            }

            // 检测是否为便携版/免安装版（不自动更新）
            let mut is_portable = crate::services::is_portable_build()
                || std::env::current_exe()
                    .ok()
                    .and_then(|e| {
                        e.parent().map(|p| {
                            p.join("portable.txt").exists() || p.join("portable.flag").exists()
                        })
                    })
                    .unwrap_or(false)
                || !is_installed_version();

            if let Some(v) = parse_env_bool("QC_FORCE_PORTABLE") {
                is_portable = v;
            }

            if !force_update && !should_open_window {
                let payload = serde_json::json!({
                    "forceUpdate": false,
                    "version": new_version,
                    "notes": notes,
                    "isPortable": is_portable,
                });
                set_update_window_payload(Some(payload));
                return Ok(true);
            }

            let window = if let Some(w) = app.get_webview_window("updater") {
                let _ = w.show();
                w
            } else {
                open_updater_window(app, force_update)?
            };

            let payload = serde_json::json!({
                "forceUpdate": if is_portable { false } else { force_update },
                "version": new_version,
                "notes": notes,
                "isPortable": is_portable,
            });

            set_update_window_payload(Some(payload.clone()));
            emit_update_payload(&window, payload);

            Ok(true)
        }
        None => {
            set_update_banner_state(app, None);
            set_update_window_payload(None);
            Ok(false)
        }
    }
}

pub async fn check_updates_and_open_window(app: &AppHandle) -> Result<bool, String> {
    check_updates(app, true).await
}
