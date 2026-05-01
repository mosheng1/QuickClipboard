use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

static APP_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));
static REGISTERED_SHORTCUTS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
static HOTKEYS_ENABLED: AtomicBool = AtomicBool::new(true);
static FOREGROUND_GLOBALLY_DISABLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HotkeyActivation {
    Active,
    Inactive,
}

#[derive(Debug)]
struct HotkeySyncState {
    current: HotkeyActivation,
    desired: HotkeyActivation,
    syncing: bool,
}

static HOTKEY_SYNC_STATE: Lazy<Mutex<HotkeySyncState>> = Lazy::new(|| {
    Mutex::new(HotkeySyncState {
        current: HotkeyActivation::Active,
        desired: HotkeyActivation::Active,
        syncing: false,
    })
});

static ACTIVE_PASTE_KEYS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

// 检查快捷键是否首次按下
fn try_activate_key(key_id: &str) -> bool {
    let mut active = ACTIVE_PASTE_KEYS.lock();
    if active.contains(key_id) {
        false
    } else {
        active.insert(key_id.to_string());
        true
    }
}

// 检查快捷键是否处于活跃状态（重复按下）
fn is_key_active(key_id: &str) -> bool {
    ACTIVE_PASTE_KEYS.lock().contains(key_id)
}

// 释放快捷键
fn deactivate_key(key_id: &str) {
    ACTIVE_PASTE_KEYS.lock().remove(key_id);
}

// 快捷键注册状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutStatus {
    pub id: String,
    pub shortcut: String,
    pub success: bool,
    pub error: Option<String>,
}

static SHORTCUT_STATUS: Lazy<Mutex<HashMap<String, ShortcutStatus>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn init_hotkey_manager(app: AppHandle, _window: WebviewWindow) {
    *APP_HANDLE.lock() = Some(app);
}

fn is_foreground_globally_disabled() -> bool {
    FOREGROUND_GLOBALLY_DISABLED.load(Ordering::Relaxed)
}

fn apply_activation(desired: HotkeyActivation) {
    match desired {
        HotkeyActivation::Active => {
            let _ = reload_from_settings();
        }
        HotkeyActivation::Inactive => {
            unregister_all();
        }
    }
}

pub fn sync_hotkeys_for_foreground() {
    let settings = crate::get_settings();
    let globally_disabled = crate::services::system::is_front_app_globally_disabled_from_settings();
    FOREGROUND_GLOBALLY_DISABLED.store(globally_disabled, Ordering::Relaxed);

    let desired = if !settings.hotkeys_enabled
        || !HOTKEYS_ENABLED.load(Ordering::Relaxed)
        || globally_disabled
    {
        HotkeyActivation::Inactive
    } else {
        HotkeyActivation::Active
    };

    {
        let mut state = HOTKEY_SYNC_STATE.lock();
        state.desired = desired;

        if state.syncing {
            return;
        }

        if state.current == state.desired {
            return;
        }

        state.syncing = true;
    }

    std::thread::spawn(|| loop {
        let desired_now = {
            let state = HOTKEY_SYNC_STATE.lock();
            state.desired
        };

        apply_activation(desired_now);

        let mut state = HOTKEY_SYNC_STATE.lock();
        state.current = desired_now;

        if state.current == state.desired {
            state.syncing = false;
            break;
        }
    });
}

fn get_app() -> Result<AppHandle, String> {
    APP_HANDLE
        .lock()
        .clone()
        .ok_or_else(|| "热键管理器未初始化".to_string())
}

fn parse_shortcut(shortcut_str: &str) -> Result<Shortcut, String> {
    let normalized = shortcut_str
        .replace("Win+", "Super+")
        .replace("Ctrl+", "Control+");
    
    normalized.parse::<Shortcut>()
        .map_err(|e| format!("解析快捷键失败: {}", e))
}

pub fn register_shortcut<F>(id: &str, shortcut_str: &str, handler: F) -> Result<(), String>
where
    F: Fn(&AppHandle) + Send + Sync + 'static,
{
    let app = get_app()?;
    
    unregister_shortcut(id);
    
    let shortcut = match parse_shortcut(shortcut_str) {
        Ok(s) => s,
        Err(_e) => {
            update_shortcut_status(id, shortcut_str, false, Some("REGISTRATION_FAILED".to_string()));
            return Err("REGISTRATION_FAILED".to_string());
        }
    };
    
    match app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                handler(app);
            }
        }) {
        Ok(_) => {
            REGISTERED_SHORTCUTS.lock().push((id.to_string(), shortcut_str.to_string()));
            update_shortcut_status(id, shortcut_str, true, None);
            println!("已注册快捷键 [{}]: {}", id, shortcut_str);
            Ok(())
        }
        Err(e) => {
            let error_msg = if e.to_string().contains("already registered") {
                "CONFLICT".to_string()
            } else {
                "REGISTRATION_FAILED".to_string()
            };
            update_shortcut_status(id, shortcut_str, false, Some(error_msg.clone()));
            Err(format!("注册快捷键失败: {}", e))
        }
    }
}

pub fn unregister_shortcut(id: &str) {
    let app = match get_app() {
        Ok(app) => app,
        Err(_) => return,
    };
    
    let mut shortcuts = REGISTERED_SHORTCUTS.lock();
    if let Some(pos) = shortcuts.iter().position(|(registered_id, _)| registered_id == id) {
        let (_, shortcut_str) = shortcuts.remove(pos);
        if let Ok(shortcut) = parse_shortcut(&shortcut_str) {
            let _ = app.global_shortcut().unregister(shortcut);
            println!("已注销快捷键 [{}]: {}", id, shortcut_str);
        }
    }
    
    clear_shortcut_status(id);
}

pub fn register_toggle_hotkey(shortcut_str: &str) -> Result<(), String> {
    register_shortcut("toggle", shortcut_str, |app| {
        if is_foreground_globally_disabled() {
            return;
        }
        let _ = crate::toggle_main_window_visibility(app);
    })
}

pub fn register_open_settings_hotkey(shortcut_str: &str) -> Result<(), String> {
    register_shortcut("open_settings", shortcut_str, |app| {
        if is_foreground_globally_disabled() {
            return;
        }

        let app_clone = app.clone();
        std::thread::spawn(move || {
            if let Err(e) = crate::windows::settings_window::open_settings_window(&app_clone) {
                eprintln!("打开设置窗口失败: {}", e);
            }
        });
    })
}

pub fn register_quickpaste_hotkey(shortcut_str: &str) -> Result<(), String> {
    let app = get_app()?;
    
    unregister_shortcut("quickpaste");
    
    let shortcut = parse_shortcut(shortcut_str)?;
    
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if crate::services::low_memory::is_low_memory_mode() {
                    return;
                }

                if is_foreground_globally_disabled() {
                    return;
                }
                
                let settings = crate::get_settings();
                let is_keyboard_mode = settings.quickpaste_paste_on_modifier_release;
                let is_visible = crate::windows::quickpaste::is_visible();
                
                if is_keyboard_mode && is_visible {
                    return;
                }
                
                if let Err(e) = crate::windows::quickpaste::show_quickpaste_window(&app) {
                    eprintln!("显示便捷粘贴窗口失败: {}", e);
                }
            } else if event.state == ShortcutState::Released {
                if crate::services::low_memory::is_low_memory_mode() {
                    return;
                }

                if is_foreground_globally_disabled() {
                    return;
                }
                
                let settings = crate::get_settings();
                if settings.quickpaste_paste_on_modifier_release {
                    return;
                }
                
                if let Some(window) = app.get_webview_window("quickpaste") {
                    let _ = window.emit("quickpaste-hide", ());
                }
                
                let app_clone = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if let Err(e) = crate::windows::quickpaste::hide_quickpaste_window(&app_clone) {
                        eprintln!("隐藏便捷粘贴窗口失败: {}", e);
                    }
                });
            }
        })
        .map_err(|e| format!("注册便捷粘贴快捷键失败: {}", e))?;
    
    REGISTERED_SHORTCUTS.lock().push(("quickpaste".to_string(), shortcut_str.to_string()));
    
    println!("已注册便捷粘贴快捷键: {}", shortcut_str);
    Ok(())
}

#[cfg(feature = "screenshot-suite")]
pub fn register_screenshot_hotkey(shortcut_str: &str) -> Result<(), String> {
    register_shortcut("screenshot", shortcut_str, |app| {
        if crate::services::low_memory::is_low_memory_mode() {
            return;
        }

        if is_foreground_globally_disabled() {
            return;
        }
        if let Err(e) = screenshot_suite::start_screenshot(app) {
            eprintln!("启动截图窗口失败: {}", e);
        }
    })
}

#[cfg(not(feature = "screenshot-suite"))]
pub fn register_screenshot_hotkey(_shortcut_str: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "screenshot-suite")]
pub fn register_screenshot_quick_save_hotkey(shortcut_str: &str) -> Result<(), String> {
    register_shortcut("screenshot_quick_save", shortcut_str, |app| {
        if crate::services::low_memory::is_low_memory_mode() {
            return;
        }
        if is_foreground_globally_disabled() {
            return;
        }
        if let Err(e) = screenshot_suite::start_screenshot_quick_save(app) {
            eprintln!("启动快速保存截图失败: {}", e);
        }
    })
}

#[cfg(not(feature = "screenshot-suite"))]
pub fn register_screenshot_quick_save_hotkey(_shortcut_str: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "screenshot-suite")]
pub fn register_screenshot_quick_pin_hotkey(shortcut_str: &str) -> Result<(), String> {
    register_shortcut("screenshot_quick_pin", shortcut_str, |app| {
        if crate::services::low_memory::is_low_memory_mode() {
            return;
        }
        if is_foreground_globally_disabled() {
            return;
        }
        if let Err(e) = screenshot_suite::start_screenshot_quick_pin(app) {
            eprintln!("启动快速贴图截图失败: {}", e);
        }
    })
}

#[cfg(not(feature = "screenshot-suite"))]
pub fn register_screenshot_quick_pin_hotkey(_shortcut_str: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "screenshot-suite")]
pub fn register_screenshot_quick_ocr_hotkey(shortcut_str: &str) -> Result<(), String> {
    register_shortcut("screenshot_quick_ocr", shortcut_str, |app| {
        if crate::services::low_memory::is_low_memory_mode() {
            return;
        }
        if is_foreground_globally_disabled() {
            return;
        }
        if let Err(e) = screenshot_suite::start_screenshot_quick_ocr(app) {
            eprintln!("启动快速OCR截图失败: {}", e);
        }
    })
}

#[cfg(not(feature = "screenshot-suite"))]
pub fn register_screenshot_quick_ocr_hotkey(_shortcut_str: &str) -> Result<(), String> {
    Ok(())
}

pub fn register_toggle_clipboard_monitor_hotkey(shortcut_str: &str) -> Result<(), String> {
    register_shortcut("toggle_clipboard_monitor", shortcut_str, |app| {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            if let Err(e) = crate::commands::settings::toggle_clipboard_monitor(&app_clone) {
                eprintln!("切换剪贴板监听状态失败: {}", e);
            }
        });
    })
}

pub fn register_toggle_paste_with_format_hotkey(shortcut_str: &str) -> Result<(), String> {
    register_shortcut("toggle_paste_with_format", shortcut_str, |app| {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            if let Err(e) = crate::commands::settings::toggle_paste_with_format(&app_clone) {
                eprintln!("切换格式粘贴状态失败: {}", e);
            }
        });
    })
}

pub fn register_paste_plain_text_hotkey(shortcut_str: &str) -> Result<(), String> {
    let app = get_app()?;

    unregister_shortcut("paste_plain_text");

    let shortcut = parse_shortcut(shortcut_str)?;
    let key_id = "paste_plain_text".to_string();

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            match event.state {
                ShortcutState::Pressed => {
                    if try_activate_key(&key_id) {
                        // 首次按下
                        let app = app.clone();
                        let key_id = key_id.clone();
                        std::thread::spawn(move || {
                            if let Err(e) = handle_paste_plain_text_press(&app) {
                                eprintln!("纯文本粘贴失败: {}", e);
                                deactivate_key(&key_id);
                            }
                        });
                    } else if is_key_active(&key_id) {
                        // 重复按下
                        std::thread::spawn(|| {
                            let _ = simulate_paste_only();
                        });
                    }
                }
                ShortcutState::Released => {
                    deactivate_key(&key_id);
                }
            }
        })
        .map_err(|e| format!("注册纯文本粘贴快捷键失败: {}", e))?;

    REGISTERED_SHORTCUTS
        .lock()
        .push(("paste_plain_text".to_string(), shortcut_str.to_string()));
    update_shortcut_status("paste_plain_text", shortcut_str, true, None);
    println!("已注册纯文本粘贴快捷键: {}", shortcut_str);
    Ok(())
}

// 首次按下
fn handle_paste_plain_text_press(app: &AppHandle) -> Result<(), String> {
    use crate::services::database::{query_clipboard_items, get_clipboard_item_by_id, QueryParams};
    use crate::services::paste::paste_handler::paste_clipboard_item_with_format;
    use crate::services::paste::PasteAction;

    let state = crate::get_window_state();
    let is_window_visible = state.state == crate::WindowState::Visible && !state.is_hidden;

    if is_window_visible {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.emit("paste-plain-text-selected", ());
        }
    } else {
        let items = query_clipboard_items(QueryParams {
            offset: 0,
            limit: 1,
            search: None,
            content_type: None,
        })?
        .items;

        if let Some(item) = items.first() {
            let full_item = get_clipboard_item_by_id(item.id)?
                .ok_or_else(|| format!("剪贴板项 {} 不存在", item.id))?;
            paste_clipboard_item_with_format(&full_item, Some(PasteAction::PlainText))?;
        }
    }

    Ok(())
}

pub fn register_number_shortcuts(modifier: &str) -> Result<(), String> {
    let app = get_app()?;
    
    unregister_number_shortcuts();
    
    {
        let mut status_map = SHORTCUT_STATUS.lock();
        status_map.remove("number_shortcuts");
    }
    
    let is_f_key = modifier.ends_with("F");
    let prefix = if is_f_key {
        modifier.strip_suffix("F").unwrap_or("").trim_end_matches('+')
    } else {
        modifier
    };
    
    let mut failed_shortcuts: Vec<String> = Vec::new();
    
    for num in 1..=9 {
        let id = format!("number_{}", num);
        let shortcut_str = if is_f_key {
            if prefix.is_empty() {
                format!("F{}", num)
            } else {
                format!("{}+F{}", prefix, num)
            }
        } else {
            format!("{}+{}", modifier, num)
        };
        
        if let Ok(shortcut) = parse_shortcut(&shortcut_str) {
            let key_id = format!("number_{}", num);
            let index = (num - 1) as usize;

            match app
                .global_shortcut()
                .on_shortcut(shortcut, move |_app, _shortcut, event| {
                    match event.state {
                        ShortcutState::Pressed => {
                            if try_activate_key(&key_id) {
                                // 首次按下
                                let key_id = key_id.clone();
                                if let Err(e) = handle_number_shortcut_press(index) {
                                    eprintln!("执行数字快捷键 {} 失败: {}", index + 1, e);
                                    deactivate_key(&key_id);
                                }
                            } else if is_key_active(&key_id) {
                                // 重复按下
                                let _ = simulate_paste_only();
                            }
                        }
                        ShortcutState::Released => {
                            deactivate_key(&key_id);
                        }
                    }
                })
            {
                Ok(_) => {
                    REGISTERED_SHORTCUTS.lock().push((id, shortcut_str.clone()));
                    println!("已注册数字快捷键: {}", shortcut_str);
                }
                Err(e) => {
                    eprintln!(
                        "注册数字快捷键 {} 失败: {}，继续注册其他快捷键",
                        shortcut_str, e
                    );
                    failed_shortcuts.push(shortcut_str);
                }
            }
        }
    }
    
    if !failed_shortcuts.is_empty() {
        let mut status_map = SHORTCUT_STATUS.lock();
        status_map.insert("number_shortcuts".to_string(), ShortcutStatus {
            id: "number_shortcuts".to_string(),
            shortcut: failed_shortcuts.join(", "),
            success: false,
            error: Some("REGISTRATION_FAILED".to_string()),
        });
    }
    
    Ok(())
}

pub fn unregister_number_shortcuts() {
    let mut shortcuts = REGISTERED_SHORTCUTS.lock();
    let number_shortcuts: Vec<_> = shortcuts
        .iter()
        .filter(|(id, _)| id.starts_with("number_"))
        .cloned()
        .collect();
    
    for (id, shortcut_str) in number_shortcuts {
        if let Ok(shortcut) = parse_shortcut(&shortcut_str) {
            if let Ok(app) = get_app() {
                let _ = app.global_shortcut().unregister(shortcut);
                println!("已注销数字快捷键: {}", shortcut_str);
            }
        }
        shortcuts.retain(|(sid, _)| sid != &id);
    }
}

// 首次按下
fn handle_number_shortcut_press(index: usize) -> Result<(), String> {
    use crate::services::database::{query_clipboard_items, get_clipboard_item_by_id, QueryParams};
    use crate::services::paste::paste_handler::paste_clipboard_item_with_update;

    let items = query_clipboard_items(QueryParams {
        offset: 0,
        limit: 9,
        search: None,
        content_type: None,
    })?
    .items;

    let item = items.get(index).ok_or_else(|| {
        format!(
            "剪贴板项索引 {} 超出范围（共 {} 项）",
            index + 1,
            items.len()
        )
    })?;

    let full_item = get_clipboard_item_by_id(item.id)?
        .ok_or_else(|| format!("剪贴板项 {} 不存在", item.id))?;

    paste_clipboard_item_with_update(&full_item)
}

// 重复按下
fn simulate_paste_only() -> Result<(), String> {
    use crate::services::paste::keyboard::simulate_paste;

    std::thread::sleep(std::time::Duration::from_millis(50));
    simulate_paste()?;
    std::thread::sleep(std::time::Duration::from_millis(50));

    Ok(())
}

pub fn unregister_all() {
    let shortcuts = REGISTERED_SHORTCUTS.lock().clone();
    for (id, _) in shortcuts {
        unregister_shortcut(&id);
    }
}

pub fn enable_hotkeys() -> Result<(), String> {
    if HOTKEYS_ENABLED.load(Ordering::Relaxed) {
        return Ok(());
    }
    
    reload_from_settings()?;
    HOTKEYS_ENABLED.store(true, Ordering::Relaxed);
    println!("已启用全局热键");
    Ok(())
}

pub fn disable_hotkeys() {
    if !HOTKEYS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    
    unregister_all();
    HOTKEYS_ENABLED.store(false, Ordering::Relaxed);
    println!("已禁用全局热键");
}

pub fn is_hotkeys_enabled() -> bool {
    HOTKEYS_ENABLED.load(Ordering::Relaxed)
}

// 更新快捷键状态
fn update_shortcut_status(id: &str, shortcut: &str, success: bool, error: Option<String>) {
    let mut status_map = SHORTCUT_STATUS.lock();
    status_map.insert(
        id.to_string(),
        ShortcutStatus {
            id: id.to_string(),
            shortcut: shortcut.to_string(),
            success,
            error,
        },
    );
}

// 获取所有快捷键状态
pub fn get_shortcut_statuses() -> Vec<ShortcutStatus> {
    let status_map = SHORTCUT_STATUS.lock();
    status_map.values().cloned().collect()
}

// 获取单个快捷键状态
pub fn get_shortcut_status(id: &str) -> Option<ShortcutStatus> {
    let status_map = SHORTCUT_STATUS.lock();
    status_map.get(id).cloned()
}

// 清除快捷键状态
fn clear_shortcut_status(id: &str) {
    let mut status_map = SHORTCUT_STATUS.lock();
    status_map.remove(id);
}

pub fn reload_from_settings() -> Result<(), String> {
    let settings = crate::get_settings();

    unregister_all();
    {
        let mut status_map = SHORTCUT_STATUS.lock();
        status_map.clear();
    }

    if !settings.hotkeys_enabled {
        return Ok(());
    }

    if is_foreground_globally_disabled() {
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    if super::wayland_shortcuts::is_wayland_session() {
        reload_wayland_shortcuts(&settings);
        return Ok(());
    }

    reload_x11_shortcuts(&settings)
}

#[cfg(target_os = "linux")]
fn reload_wayland_shortcuts(settings: &crate::services::settings::AppSettings) {
    use super::wayland_shortcuts::{register_wayland_shortcut, unregister_all_wayland_shortcuts};

    if let Err(e) = unregister_all_wayland_shortcuts() {
        eprintln!("清除 Wayland 快捷键失败: {}", e);
    }

    if !settings.toggle_shortcut.is_empty() {
        if let Err(e) = register_wayland_shortcut("toggle", &settings.toggle_shortcut, "--toggle") {
            eprintln!("注册 Wayland 切换快捷键失败: {}", e);
        } else {
            update_shortcut_status("toggle", &settings.toggle_shortcut, true, None);
        }
    }

    if settings.quickpaste_enabled && !settings.quickpaste_shortcut.is_empty() {
        if let Err(e) = register_wayland_shortcut("quickpaste", &settings.quickpaste_shortcut, "--quickpaste") {
            eprintln!("注册 Wayland 便捷粘贴快捷键失败: {}", e);
        } else {
            update_shortcut_status("quickpaste", &settings.quickpaste_shortcut, true, None);
        }
    }

    if !settings.open_settings_shortcut.is_empty() {
        if let Err(e) = register_wayland_shortcut("settings", &settings.open_settings_shortcut, "--settings") {
            eprintln!("注册 Wayland 设置快捷键失败: {}", e);
        } else {
            update_shortcut_status("open_settings", &settings.open_settings_shortcut, true, None);
        }
    }

    println!("Wayland 快捷键注册完成（通过 GNOME 自定义快捷键）");
}

fn reload_x11_shortcuts(settings: &crate::services::settings::AppSettings) -> Result<(), String> {
    if !settings.toggle_shortcut.is_empty() {
        if let Err(e) = register_toggle_hotkey(&settings.toggle_shortcut) {
            eprintln!("注册主窗口切换快捷键失败: {}", e);
        }
    }

    if !settings.open_settings_shortcut.is_empty() {
        if let Err(e) = register_open_settings_hotkey(&settings.open_settings_shortcut) {
            eprintln!("注册打开设置快捷键失败: {}", e);
        }
    }

    if settings.quickpaste_enabled && !settings.quickpaste_shortcut.is_empty() {
        if let Err(e) = register_quickpaste_hotkey(&settings.quickpaste_shortcut) {
            eprintln!("注册预览窗口快捷键失败: {}", e);
        }
    }

    if settings.screenshot_enabled && !settings.screenshot_shortcut.is_empty() {
        if let Err(e) = register_screenshot_hotkey(&settings.screenshot_shortcut) {
            eprintln!("注册截图快捷键失败: {}", e);
        }
    }

    if settings.screenshot_enabled && !settings.screenshot_quick_save_shortcut.is_empty() {
        if let Err(e) = register_screenshot_quick_save_hotkey(&settings.screenshot_quick_save_shortcut) {
            eprintln!("注册快速保存截图快捷键失败: {}", e);
        }
    }

    if settings.screenshot_enabled && !settings.screenshot_quick_pin_shortcut.is_empty() {
        if let Err(e) = register_screenshot_quick_pin_hotkey(&settings.screenshot_quick_pin_shortcut) {
            eprintln!("注册快速贴图截图快捷键失败: {}", e);
        }
    }

    if settings.screenshot_enabled && !settings.screenshot_quick_ocr_shortcut.is_empty() {
        if let Err(e) = register_screenshot_quick_ocr_hotkey(&settings.screenshot_quick_ocr_shortcut) {
            eprintln!("注册快速OCR截图快捷键失败: {}", e);
        }
    }

    if !settings.toggle_clipboard_monitor_shortcut.is_empty() {
        if let Err(e) = register_toggle_clipboard_monitor_hotkey(&settings.toggle_clipboard_monitor_shortcut) {
            eprintln!("注册切换剪贴板监听快捷键失败: {}", e);
        }
    }

    if !settings.toggle_paste_with_format_shortcut.is_empty() {
        if let Err(e) = register_toggle_paste_with_format_hotkey(&settings.toggle_paste_with_format_shortcut) {
            eprintln!("注册切换格式粘贴快捷键失败: {}", e);
        }
    }

    if !settings.paste_plain_text_shortcut.is_empty() {
        if let Err(e) = register_paste_plain_text_hotkey(&settings.paste_plain_text_shortcut) {
            eprintln!("注册纯文本粘贴快捷键失败: {}", e);
        }
    }

    if settings.number_shortcuts && !settings.number_shortcuts_modifier.is_empty() {
        if let Err(e) = register_number_shortcuts(&settings.number_shortcuts_modifier) {
            eprintln!("注册数字快捷键失败: {}", e);
        }
    }

    Ok(())
}

